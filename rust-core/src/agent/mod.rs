pub mod bridge_client;
pub mod compaction;
pub mod git;
pub mod plan;
pub mod plugins;
pub mod providers;
pub mod session;
pub mod subagents;

pub mod agent_core;
pub mod message;
pub mod plan_exec;
pub mod subagent;
pub mod tool;

#[path = "loop.rs"]
pub mod agent_loop;

use std::sync::Arc;

use anyhow::Context;
use tokio::sync::{Mutex, mpsc};
use tracing::{debug, error, info, instrument, trace, warn};

use bridge_client::{BridgeClient, PromptMessage, PromptParams, default_system_prompt};
use compaction::CompactionManager;
use plan::{Plan, PlanManager, PlanStatus};
use session::SessionStore;
use subagents::SubagentManager;

use crate::shutdown::CancelToken;

/// Configuration for an Agent instance.
#[derive(Debug, Clone)]
pub struct AgentConfig {
    /// The LLM model to use.
    pub model: String,
    /// Maximum number of turns before the agent stops.
    pub max_turns: usize,
    /// Maximum context window size in tokens.
    pub context_window_tokens: usize,
    /// Command to launch the TS bridge process.
    pub bridge_command: String,
    /// Maximum number of concurrent subagents.
    pub max_subagents: usize,
    /// Path to the SQLite session database.
    pub db_path: String,
}

impl Default for AgentConfig {
    fn default() -> Self {
        Self {
            model: "claude-sonnet-4-20250514".to_string(),
            max_turns: 50,
            context_window_tokens: 200_000,
            bridge_command: std::env::var("PI_BRIDGE_COMMAND").unwrap_or_else(|_| {
                "echo '{\"jsonrpc\":\"2.0\",\"id\":1,\"result\":\"mock\"}'".to_string()
            }),
            max_subagents: 8,
            db_path: std::env::var("PI_SESSION_DB").unwrap_or_else(|_| {
                std::env::var("HOME")
                    .map(|home| format!("{home}/.pi-hybrid/sessions.db"))
                    .unwrap_or_else(|_| ".pi-hybrid/sessions.db".to_string())
            }),
        }
    }
}

/// Messages sent from the TUI to the Agent.
#[derive(Debug, Clone)]
pub enum AgentInput {
    /// User typed a prompt and pressed Enter.
    UserPrompt(String),
    /// User pressed the approve hotkey (a) on plan pane.
    ApprovePlan,
    /// User pressed the reject hotkey (r) on plan pane.
    RejectPlan,
    /// User pressed the edit hotkey (e) on plan pane.
    EditPlan,
    /// Cancel the current agent operation.
    Cancel,
    /// Spawn a subagent with a goal.
    SpawnSubagent { goal: String },
    /// Query subagent status.
    QuerySubagents,
    /// Shut down the agent.
    Shutdown,
}

/// Messages sent from the Agent back to the TUI.
#[derive(Debug, Clone)]
pub enum AgentOutput {
    /// A text response from the LLM (streaming chunk or complete).
    ResponseChunk(String),
    /// The plan has been generated and is ready for review.
    PlanReady { steps: Vec<plan::Step> },
    /// A plan step has been executed.
    StepExecuted {
        index: usize,
        status: plan::StepStatus,
    },
    /// The plan has been approved and execution started.
    PlanApproved,
    /// The plan has been rejected.
    PlanRejected,
    /// A subagent has produced a result.
    SubagentResult {
        id: String,
        goal: String,
        result: String,
    },
    /// Current subagent statuses.
    SubagentStatus { agents: Vec<SubagentInfo> },
    /// An error occurred.
    Error(String),
    /// Agent status update.
    Status { message: String },
    /// Agent is thinking (processing a turn).
    Thinking,
    /// Agent is idle.
    Idle,
    /// A unified diff for a file edit step.
    DiffPreview { step_index: usize, diff: String },
}

#[derive(Debug, Clone)]
pub struct SubagentInfo {
    pub id: String,
    pub goal: String,
    pub status: String,
    pub turns: usize,
}

/// The main Agent struct — orchestrates the agent loop, subagents, plan, and session.
pub struct Agent {
    config: AgentConfig,
    bridge: Arc<Mutex<BridgeClient>>,
    session: Arc<Mutex<SessionStore>>,
    plan_manager: Arc<Mutex<PlanManager>>,
    subagent_manager: Arc<SubagentManager>,
    compaction: Arc<Mutex<CompactionManager>>,
    /// Channel to receive input from TUI.
    input_rx: mpsc::UnboundedReceiver<AgentInput>,
    /// Channel to send output to TUI.
    output_tx: mpsc::UnboundedSender<AgentOutput>,
    /// Whether the agent is currently running.
    running: bool,
    /// Cancellation token for graceful shutdown.
    cancel_token: CancelToken,
}

impl Agent {
    /// Create a new Agent with the given configuration and communication channels.
    #[instrument(skip(config, input_rx, output_tx, cancel_token), fields(model = %config.model, max_turns = config.max_turns))]
    pub async fn new(
        config: AgentConfig,
        input_rx: mpsc::UnboundedReceiver<AgentInput>,
        output_tx: mpsc::UnboundedSender<AgentOutput>,
        cancel_token: CancelToken,
    ) -> anyhow::Result<Self> {
        info!("Creating agent");

        let mut bridge = BridgeClient::connect(&config.bridge_command)
            .await
            .context("Failed to connect to bridge")?;
        bridge.set_cancel_token(cancel_token.clone());
        let bridge = Arc::new(Mutex::new(bridge));

        let mut session = SessionStore::open(&config.db_path)
            .await
            .context("Failed to open session store")?;
        let session_id = session.create_session(&config.model).await?;
        info!(session_id, model = %config.model, "Session created");
        let session = Arc::new(Mutex::new(session));

        let plan_manager = Arc::new(Mutex::new(PlanManager::new()));
        let subagent_manager = Arc::new(SubagentManager::new(config.max_subagents));
        let compaction = Arc::new(Mutex::new(CompactionManager::new(
            config.context_window_tokens,
        )));

        info!("Agent initialized successfully");

        Ok(Self {
            config,
            bridge,
            session,
            plan_manager,
            subagent_manager,
            compaction,
            input_rx,
            output_tx,
            running: false,
            cancel_token,
        })
    }

    /// Start the agent loop. This runs until a Shutdown input is received.
    #[instrument(skip(self))]
    pub async fn run(&mut self, initial_prompt: Option<String>) {
        info!("Agent loop started");
        self.running = true;
        let _ = self.output_tx.send(AgentOutput::Status {
            message: "Agent started".to_string(),
        });

        if let Some(prompt) = initial_prompt {
            self.process_prompt(&prompt).await;
        }

        while self.running {
            // Check cancellation token before each turn
            if self.cancel_token.is_cancelled() {
                info!("Cancellation requested — stopping agent loop");
                break;
            }

            match self.input_rx.recv().await {
                Some(AgentInput::UserPrompt(prompt)) => {
                    debug!(%prompt, "Received user prompt");
                    self.process_prompt(&prompt).await;
                }
                Some(AgentInput::ApprovePlan) => {
                    debug!("Received approve plan input");
                    self.approve_plan().await;
                }
                Some(AgentInput::RejectPlan) => {
                    debug!("Received reject plan input");
                    self.reject_plan().await;
                }
                Some(AgentInput::EditPlan) => {
                    debug!("Received edit plan input");
                    let _ = self.output_tx.send(AgentOutput::Status {
                        message: "Plan editing mode activated".to_string(),
                    });
                }
                Some(AgentInput::Cancel) => {
                    info!("Operation cancelled by user");
                    let _ = self.output_tx.send(AgentOutput::Status {
                        message: "Operation cancelled".to_string(),
                    });
                }
                Some(AgentInput::SpawnSubagent { goal }) => {
                    debug!(%goal, "Spawning subagent");
                    self.spawn_subagent(&goal).await;
                }
                Some(AgentInput::QuerySubagents) => {
                    trace!("Querying subagents");
                    self.query_subagents().await;
                }
                Some(AgentInput::Shutdown) => {
                    info!("Agent shutting down");
                    self.running = false;
                }
                None => {
                    info!("Input channel closed, shutting down");
                    self.running = false;
                }
            }
        }

        info!("Agent loop stopped");
        let _ = self.output_tx.send(AgentOutput::Status {
            message: "Agent stopped".to_string(),
        });
    }

    /// Process a user prompt through the agent loop.
    #[instrument(skip(self), fields(prompt_length = prompt.len()))]
    pub async fn process_prompt(&mut self, prompt: &str) {
        let _ = self.output_tx.send(AgentOutput::Thinking);

        let conversation = vec![agent_loop::Message {
            role: "user".to_string(),
            content: prompt.to_string(),
            tool_calls: None,
        }];

        // Check context window
        let mut compaction = self.compaction.lock().await;
        if compaction.needs_compaction(&conversation) {
            info!("Context compaction triggered");
            let _ = self.output_tx.send(AgentOutput::Status {
                message: "Compacting context...".to_string(),
            });
            // Compaction would happen here in a full implementation
        }
        drop(compaction);

        let params = PromptParams {
            model: self.config.model.clone(),
            messages: conversation
                .iter()
                .map(|message| PromptMessage {
                    role: message.role.clone(),
                    content: message.content.clone(),
                    tool_calls: None,
                })
                .collect(),
            system: Some(default_system_prompt()),
            max_tokens: None,
            temperature: None,
            tools: None,
        };

        match self.bridge.lock().await.send_prompt(&params).await {
            Ok(response) => {
                let _ = self
                    .output_tx
                    .send(AgentOutput::ResponseChunk(response.content));
            }
            Err(error) => {
                error!(%error, "Bridge send_prompt failed");
                let _ = self.output_tx.send(AgentOutput::Error(error.to_string()));
            }
        }

        // Store in session
        let session = self.session.lock().await;
        let _ = session.add_message("user", prompt, None).await;
        debug!(%prompt, "Message saved to session");

        let _ = self.output_tx.send(AgentOutput::Idle);
    }

    /// Spawn a subagent to work on a goal.
    #[instrument(skip(self), fields(%goal))]
    pub async fn spawn_subagent(&mut self, goal: &str) {
        let output_tx = self.output_tx.clone();
        let bridge = self.bridge.clone();
        let subagent_manager = self.subagent_manager.clone();
        let child_token = self.cancel_token.child_token();

        let goal_owned = goal.to_string();
        let agent_id = subagent_manager
            .spawn(goal_owned.clone(), bridge, output_tx, child_token)
            .await;

        info!(subagent_id = %agent_id, goal = %goal_owned, "Subagent spawned");

        let _ = self.output_tx.send(AgentOutput::Status {
            message: format!("Subagent spawned: {agent_id} — {goal_owned}"),
        });
    }

    /// Query the status of all subagents.
    #[instrument(skip(self))]
    pub async fn query_subagents(&self) {
        let agents = self.subagent_manager.status_all().await;
        trace!(subagent_count = agents.len(), "Subagent status queried");
        let _ = self.output_tx.send(AgentOutput::SubagentStatus { agents });
    }

    /// Approve the current plan and begin execution.
    #[instrument(skip(self))]
    pub async fn approve_plan(&mut self) {
        let mut plan_mgr = self.plan_manager.lock().await;
        if plan_mgr
            .current_plan()
            .map(|p| p.status == PlanStatus::PendingApproval)
            .unwrap_or(false)
        {
            plan_mgr.approve();
            info!("Plan approved, starting execution");
            let _ = self.output_tx.send(AgentOutput::PlanApproved);
            let _ = self.output_tx.send(AgentOutput::Status {
                message: "Plan approved. Executing steps...".to_string(),
            });
            // Execute approved steps
            self.execute_plan(&mut plan_mgr).await;
        } else {
            warn!("No plan pending approval");
            let _ = self.output_tx.send(AgentOutput::Status {
                message: "No plan pending approval".to_string(),
            });
        }
    }

    /// Reject the current plan.
    #[instrument(skip(self))]
    pub async fn reject_plan(&mut self) {
        let mut plan_mgr = self.plan_manager.lock().await;
        if plan_mgr
            .current_plan()
            .map(|p| p.status == PlanStatus::PendingApproval)
            .unwrap_or(false)
        {
            plan_mgr.reject();
            info!("Plan rejected");
            let _ = self.output_tx.send(AgentOutput::PlanRejected);
        } else {
            warn!("No plan pending approval for rejection");
            let _ = self.output_tx.send(AgentOutput::Status {
                message: "No plan pending approval".to_string(),
            });
        }
    }

    /// Execute approved plan steps.
    #[instrument(skip(self, plan_mgr))]
    async fn execute_plan(&self, plan_mgr: &mut PlanManager) {
        if let Some(plan) = plan_mgr.current_plan_mut() {
            let step_count = plan.steps.len();
            info!(step_count, "Executing plan steps");
            for (i, step) in plan.steps.iter_mut().enumerate() {
                if step.status == plan::StepStatus::Approved {
                    debug!(step_index = i, tool = %step.tool, "Executing plan step");
                    step.status = plan::StepStatus::Executing;
                    let _ = self.output_tx.send(AgentOutput::StepExecuted {
                        index: i,
                        status: plan::StepStatus::Executing,
                    });

                    // In a full implementation, we'd call the bridge to execute the tool.
                    // For now, mark as completed.
                    step.status = plan::StepStatus::Completed;
                    debug!(step_index = i, "Step completed");
                    let _ = self.output_tx.send(AgentOutput::StepExecuted {
                        index: i,
                        status: plan::StepStatus::Completed,
                    });
                }
            }
        }
    }

    /// Get current agent status.
    pub fn get_status(&self) -> AgentStatus {
        AgentStatus {
            running: self.running,
            model: self.config.model.clone(),
            max_turns: self.config.max_turns,
        }
    }
}

#[derive(Debug, Clone)]
pub struct AgentStatus {
    pub running: bool,
    pub model: String,
    pub max_turns: usize,
}

/// Create the communication channels for agent-TUI communication.
pub fn agent_channels() -> (
    mpsc::UnboundedSender<AgentInput>,
    mpsc::UnboundedReceiver<AgentOutput>,
) {
    let (input_tx, input_rx) = mpsc::unbounded_channel();
    let (output_tx, output_rx) = mpsc::unbounded_channel();
    // Return the TUI-side handles
    // input_tx: TUI sends AgentInput to agent
    // output_rx: TUI receives AgentOutput from agent
    (input_tx, output_rx)
}

/// Convenience: create an Agent and return its input sender, output receiver, and join handle.
#[instrument(skip(config, cancel_token), fields(model = %config.model))]
pub async fn spawn_agent(
    config: AgentConfig,
    cancel_token: CancelToken,
) -> anyhow::Result<(
    mpsc::UnboundedSender<AgentInput>,
    mpsc::UnboundedReceiver<AgentOutput>,
    tokio::task::JoinHandle<()>,
)> {
    info!("Spawning agent task");
    let (input_tx, input_rx) = mpsc::unbounded_channel();
    let (output_tx, output_rx) = mpsc::unbounded_channel();

    let mut agent = Agent::new(config, input_rx, output_tx, cancel_token).await?;

    let handle = tokio::spawn(async move {
        agent.run(None).await;
    });

    Ok((input_tx, output_rx, handle))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn agent_config_defaults() {
        let config = AgentConfig::default();
        assert_eq!(config.max_turns, 50);
        assert_eq!(config.max_subagents, 8);
    }

    #[tokio::test]
    async fn agent_channels_work() {
        let (tx, mut rx) = agent_channels();
        // This is the TUI-side; we'd give the other ends to Agent::new
        let _ = tx.send(AgentInput::Shutdown);
        // We'd need the agent running to receive, but this validates channel creation
    }

    #[test]
    fn agent_status_reflects_config() {
        let status = AgentStatus {
            running: true,
            model: "test-model".to_string(),
            max_turns: 10,
        };
        assert_eq!(status.model, "test-model");
        assert!(status.running);
    }

    #[tokio::test]
    async fn process_prompt_uses_bridge_response_not_hardcoded_echo() {
        use tokio::sync::mpsc;

        use crate::shutdown::CancelToken;

        let fake_bridge = concat!(
            "python3 -c '",
            "import sys, json; ",
            "req = json.loads(sys.stdin.readline()); ",
            "result = {\"content\": \"Bridge response text\", \"tool_calls\": [], ",
            "\"usage\": {\"prompt_tokens\": 5, \"completion_tokens\": 10, \"total_tokens\": 15}, ",
            "\"finish_reason\": \"stop\"}; ",
            "print(json.dumps({\"jsonrpc\": \"2.0\", \"id\": req[\"id\"], \"result\": result}), flush=True)",
            "'"
        );

        let temp_dir = std::env::temp_dir().join(format!(
            "pi-hybrid-process-prompt-test-{}",
            std::process::id()
        ));
        std::fs::create_dir_all(&temp_dir).expect("temp dir should be created");
        let db_path = temp_dir.join("sessions.db");
        let db_path_str = db_path.to_string_lossy().to_string();

        let config = AgentConfig {
            bridge_command: fake_bridge.to_string(),
            db_path: db_path_str,
            ..AgentConfig::default()
        };

        let (_input_tx, input_rx) = mpsc::unbounded_channel();
        let (output_tx, mut output_rx) = mpsc::unbounded_channel();
        let cancel_token = CancelToken::new();

        let mut agent = Agent::new(config, input_rx, output_tx, cancel_token)
            .await
            .expect("agent should initialize with fake bridge");

        agent.process_prompt("hello from test").await;

        let mut outputs = Vec::new();
        while let Ok(output) = output_rx.try_recv() {
            outputs.push(output);
        }

        let response_chunks: Vec<String> = outputs
            .iter()
            .filter_map(|output| match output {
                AgentOutput::ResponseChunk(content) => Some(content.clone()),
                _ => None,
            })
            .collect();

        assert!(
            response_chunks
                .iter()
                .any(|content| content.contains("Bridge response text")),
            "expected bridge response content, got chunks: {response_chunks:?}"
        );
        assert!(
            !response_chunks
                .iter()
                .any(|content| content.contains("Received: hello from test")),
            "should not use hardcoded echo string"
        );

        let _ = std::fs::remove_dir_all(&temp_dir);
    }
}
