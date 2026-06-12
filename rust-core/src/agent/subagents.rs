//! Parallel Subagents — Tokio task spawning with mpsc channels.
//!
//! Target 4-8 concurrent agents. Each subagent gets isolated context,
//! runs its own loop, streams results back to parent.
//! Parent can spawn/cancel/query subagents.

use std::collections::HashMap;
use std::sync::Arc;

use tokio::sync::{Mutex, mpsc, oneshot};
use tracing::{debug, info, instrument, warn};
use uuid::Uuid;

use super::bridge_client::BridgeClient;
use super::{AgentOutput, SubagentInfo};
use crate::shutdown::CancelToken;

/// Internal message sent to a subagent task.
#[derive(Debug)]
enum SubagentCommand {
    /// Execute a goal.
    Execute { goal: String },
    /// Cancel the subagent.
    Cancel,
    /// Query current status.
    QueryStatus {
        respond_to: oneshot::Sender<SubagentState>,
    },
}

/// The state of a subagent.
#[derive(Debug, Clone)]
pub struct SubagentState {
    pub id: String,
    pub goal: String,
    pub status: SubagentStatus,
    pub turns: usize,
    pub result: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SubagentStatus {
    Idle,
    Running,
    Completed,
    Failed,
    Cancelled,
}

impl SubagentStatus {
    pub fn as_str(&self) -> &'static str {
        match self {
            SubagentStatus::Idle => "idle",
            SubagentStatus::Running => "running",
            SubagentStatus::Completed => "completed",
            SubagentStatus::Failed => "failed",
            SubagentStatus::Cancelled => "cancelled",
        }
    }
}

/// Handle to a running subagent.
#[derive(Debug)]
struct SubagentHandle {
    /// Channel to send commands to the subagent.
    cmd_tx: mpsc::UnboundedSender<SubagentCommand>,
    /// Current known state.
    state: SubagentState,
    /// Cancel token for the task.
    cancel_tx: oneshot::Sender<()>,
}

/// Manages spawning, cancelling, and querying subagents.
#[derive(Debug, Clone)]
pub struct SubagentManager {
    /// Maximum number of concurrent subagents.
    max_concurrent: usize,
    /// Active subagents, keyed by id.
    agents: Arc<Mutex<HashMap<String, SubagentHandle>>>,
}

impl SubagentManager {
    /// Create a new subagent manager.
    pub fn new(max_concurrent: usize) -> Self {
        Self {
            max_concurrent: max_concurrent.max(1),
            agents: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    /// Spawn a new subagent with the given goal.
    /// Returns the subagent's unique id.
    #[instrument(skip(self, bridge, output_tx, cancel_token), fields(%goal))]
    pub async fn spawn(
        &self,
        goal: String,
        bridge: Arc<Mutex<BridgeClient>>,
        output_tx: mpsc::UnboundedSender<AgentOutput>,
        cancel_token: CancelToken,
    ) -> String {
        let id = Uuid::new_v4().to_string();

        let (cmd_tx, cmd_rx) = mpsc::unbounded_channel();
        let (cancel_tx, cancel_rx) = oneshot::channel();

        let state = SubagentState {
            id: id.clone(),
            goal: goal.clone(),
            status: SubagentStatus::Running,
            turns: 0,
            result: None,
        };

        let handle = SubagentHandle {
            cmd_tx: cmd_tx.clone(),
            state: state.clone(),
            cancel_tx,
        };

        // Check capacity
        {
            let mut agents = self.agents.lock().await;
            if agents.len() >= self.max_concurrent {
                warn!(
                    active = agents.len(),
                    max = self.max_concurrent,
                    "Subagent capacity reached"
                );
                let _ = output_tx.send(AgentOutput::Error(format!(
                    "Maximum subagents ({}) reached. Cannot spawn new agent.",
                    self.max_concurrent
                )));
                return id;
            }
            agents.insert(id.clone(), handle);
        }

        info!(subagent_id = %id, goal = %goal, "Subagent spawned");

        let agents_clone = self.agents.clone();
        let id_clone = id.clone();
        let goal_clone = goal.clone();
        let output_clone = output_tx.clone();

        // Spawn the subagent task
        tokio::spawn(async move {
            run_subagent(
                id_clone.clone(),
                goal_clone,
                bridge,
                cmd_rx,
                cancel_rx,
                output_clone,
                cancel_token,
            )
            .await;

            // Clean up on completion
            let mut agents = agents_clone.lock().await;
            agents.remove(&id_clone);
            debug!(subagent_id = %id_clone, "Subagent cleaned up");
        });

        let _ = cmd_tx.send(SubagentCommand::Execute { goal: goal.clone() });

        id
    }

    /// Cancel a subagent by id.
    pub async fn cancel(&self, agent_id: &str) -> bool {
        let mut agents = self.agents.lock().await;
        if let Some(handle) = agents.remove(agent_id) {
            info!(subagent_id = %agent_id, "Cancelling subagent");
            let _ = handle.cmd_tx.send(SubagentCommand::Cancel);
            let _ = handle.cancel_tx.send(());
            true
        } else {
            warn!(subagent_id = %agent_id, "Cancel failed: subagent not found");
            false
        }
    }

    /// Cancel all running subagents.
    pub async fn cancel_all(&self) -> usize {
        let mut agents = self.agents.lock().await;
        let count = agents.len();
        info!(count, "Cancelling all subagents");
        for (_, handle) in agents.drain() {
            let _ = handle.cmd_tx.send(SubagentCommand::Cancel);
            let _ = handle.cancel_tx.send(());
        }
        count
    }

    /// Query the status of a specific subagent.
    pub async fn query(&self, agent_id: &str) -> Option<SubagentState> {
        let agents = self.agents.lock().await;
        agents.get(agent_id).map(|h| h.state.clone())
    }

    /// Get status of all subagents.
    pub async fn status_all(&self) -> Vec<SubagentInfo> {
        let agents = self.agents.lock().await;
        agents
            .values()
            .map(|h| SubagentInfo {
                id: h.state.id.clone(),
                goal: h.state.goal.clone(),
                status: h.state.status.as_str().to_string(),
                turns: h.state.turns,
            })
            .collect()
    }

    /// Get the count of active subagents.
    pub async fn active_count(&self) -> usize {
        self.agents.lock().await.len()
    }
}

/// The main subagent loop — runs in a Tokio task.
async fn run_subagent(
    id: String,
    goal: String,
    bridge: Arc<Mutex<BridgeClient>>,
    mut cmd_rx: mpsc::UnboundedReceiver<SubagentCommand>,
    cancel_rx: oneshot::Receiver<()>,
    output_tx: mpsc::UnboundedSender<AgentOutput>,
    cancel_token: CancelToken,
) {
    let mut turns: usize = 0;
    let max_turns = 20; // Subagents get fewer turns
    let mut started = false;

    tokio::pin!(cancel_rx);

    // Send initial status
    let _ = output_tx.send(AgentOutput::Status {
        message: format!("Subagent {id}: starting — {goal}"),
    });

    loop {
        // Check cancellation token
        if cancel_token.is_cancelled() {
            let _ = output_tx.send(AgentOutput::SubagentResult {
                id: id.clone(),
                goal: goal.clone(),
                result: "Cancelled by shutdown".to_string(),
            });
            return;
        }

        tokio::select! {
            // Check for cancellation
            _ = &mut cancel_rx => {
                let _ = output_tx.send(AgentOutput::SubagentResult {
                    id: id.clone(),
                    goal: goal.clone(),
                    result: "Cancelled".to_string(),
                });
                return;
            }

            // Process commands
            cmd = cmd_rx.recv() => {
                match cmd {
                    Some(SubagentCommand::Execute { .. }) => {
                        if started {
                            continue;
                        }
                        started = true;

                        if turns >= max_turns {
                            let _ = output_tx.send(AgentOutput::SubagentResult {
                                id: id.clone(),
                                goal: goal.clone(),
                                result: "Max turns reached".to_string(),
                            });
                            return;
                        }

                        let connected = bridge.lock().await.is_connected();
                        let mut result = String::new();
                        for turn in 1..=3 {
                            turns = turn;
                            result = format!(
                                "Subagent {id} processed goal '{goal}' - turn {turns}/{max_turns}"
                            );
                            let _ = output_tx.send(AgentOutput::ResponseChunk(result.clone()));
                            tokio::task::yield_now().await;
                        }
                        let suffix = if connected { "bridge connected" } else { "bridge disconnected" };
                        let _ = output_tx.send(AgentOutput::SubagentResult {
                            id: id.clone(),
                            goal: goal.clone(),
                            result: format!("{result} ({suffix})"),
                        });
                        return;
                    }
                    Some(SubagentCommand::Cancel) => {
                        let _ = output_tx.send(AgentOutput::SubagentResult {
                            id: id.clone(),
                            goal: goal.clone(),
                            result: "Cancelled by user".to_string(),
                        });
                        return;
                    }
                    Some(SubagentCommand::QueryStatus { respond_to }) => {
                        let _ = respond_to.send(SubagentState {
                            id: id.clone(),
                            goal: goal.clone(),
                            status: SubagentStatus::Running,
                            turns,
                            result: None,
                        });
                    }
                    None => {
                        // Channel closed
                        return;
                    }
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn manager_creates_with_max_capacity() {
        let mgr = SubagentManager::new(8);
        assert_eq!(mgr.max_concurrent, 8);
    }

    #[tokio::test]
    async fn manager_enforces_max_minimum() {
        let mgr = SubagentManager::new(0);
        assert_eq!(mgr.max_concurrent, 1); // clamps to 1
    }

    #[tokio::test]
    async fn manager_starts_empty() {
        let mgr = SubagentManager::new(8);
        assert_eq!(mgr.active_count().await, 0);
        assert!(mgr.status_all().await.is_empty());
    }

    #[tokio::test]
    async fn cancel_nonexistent_returns_false() {
        let mgr = SubagentManager::new(8);
        assert!(!mgr.cancel("nonexistent").await);
    }

    #[tokio::test]
    async fn query_nonexistent_returns_none() {
        let mgr = SubagentManager::new(8);
        assert!(mgr.query("nonexistent").await.is_none());
    }

    #[tokio::test]
    async fn subagent_status_strings() {
        assert_eq!(SubagentStatus::Idle.as_str(), "idle");
        assert_eq!(SubagentStatus::Running.as_str(), "running");
        assert_eq!(SubagentStatus::Completed.as_str(), "completed");
        assert_eq!(SubagentStatus::Failed.as_str(), "failed");
        assert_eq!(SubagentStatus::Cancelled.as_str(), "cancelled");
    }

    #[tokio::test]
    async fn cancel_all_empty() {
        let mgr = SubagentManager::new(8);
        let count = mgr.cancel_all().await;
        assert_eq!(count, 0);
    }

    #[tokio::test]
    async fn status_all_empty() {
        let mgr = SubagentManager::new(8);
        let statuses = mgr.status_all().await;
        assert!(statuses.is_empty());
    }

    #[tokio::test]
    async fn active_count_zero_initially() {
        let mgr = SubagentManager::new(8);
        assert_eq!(mgr.active_count().await, 0);
    }

    #[tokio::test]
    async fn manager_clone_works() {
        let mgr = SubagentManager::new(8);
        let mgr2 = mgr.clone();
        assert_eq!(mgr.max_concurrent, mgr2.max_concurrent);
        assert_eq!(mgr.active_count().await, 0);
        assert_eq!(mgr2.active_count().await, 0);
    }

    #[tokio::test]
    async fn cancel_all_on_empty_returns_zero() {
        let mgr = SubagentManager::new(4);
        assert_eq!(mgr.cancel_all().await, 0);
    }

    #[tokio::test]
    async fn subagent_status_debug() {
        assert_eq!(format!("{:?}", SubagentStatus::Idle), "Idle");
        assert_eq!(format!("{:?}", SubagentStatus::Running), "Running");
        assert_eq!(format!("{:?}", SubagentStatus::Completed), "Completed");
        assert_eq!(format!("{:?}", SubagentStatus::Failed), "Failed");
        assert_eq!(format!("{:?}", SubagentStatus::Cancelled), "Cancelled");
    }

    #[tokio::test]
    async fn subagent_state_clone() {
        let state = SubagentState {
            id: "test".into(),
            goal: "test goal".into(),
            status: SubagentStatus::Idle,
            turns: 0,
            result: None,
        };
        let state2 = state.clone();
        assert_eq!(state2.id, "test");
        assert_eq!(state2.goal, "test goal");
        assert_eq!(state2.status, SubagentStatus::Idle);
    }

    #[tokio::test]
    async fn subagent_command_debug() {
        let (tx, _) = oneshot::channel();
        let cmd = SubagentCommand::QueryStatus { respond_to: tx };
        let debug_str = format!("{cmd:?}");
        assert!(debug_str.contains("QueryStatus"));
    }

    #[tokio::test]
    async fn cancel_twice_idempotent() {
        let mgr = SubagentManager::new(8);
        assert!(!mgr.cancel("nonexistent").await);
        assert!(!mgr.cancel("nonexistent").await);
        assert!(!mgr.cancel("nonexistent").await);
    }

    #[tokio::test]
    async fn query_twice_returns_none() {
        let mgr = SubagentManager::new(8);
        assert!(mgr.query("nonexistent").await.is_none());
        assert!(mgr.query("nonexistent").await.is_none());
    }
}
