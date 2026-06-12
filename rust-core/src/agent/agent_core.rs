use anyhow::Context;

use super::bridge_client::{PromptMessage, PromptParams};
use super::message::{Message, Role};
use super::tool::{Tool, execute_tool, parse_tool_calls};
use super::{bridge_client::BridgeClient, tool::ToolCall};

#[derive(Debug, Clone)]
pub struct AgentConfig {
    pub model: String,
    pub max_turns: usize,
    pub system_prompt: String,
    pub tools: Vec<Tool>,
}

impl Default for AgentConfig {
    fn default() -> Self {
        Self {
            model: "gpt-5.5".to_string(),
            max_turns: 20,
            system_prompt: "You are Pi Hybrid.".to_string(),
            tools: Vec::new(),
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct AgentOutput {
    pub content: String,
    pub completed: bool,
    pub turns: usize,
}

pub struct Agent {
    config: AgentConfig,
    messages: Vec<Message>,
    turn_count: usize,
    bridge: Option<BridgeClient>,
}

impl Agent {
    pub fn new(config: AgentConfig) -> Self {
        Self {
            config,
            messages: Vec::new(),
            turn_count: 0,
            bridge: None,
        }
    }

    pub fn with_bridge(config: AgentConfig, bridge: BridgeClient) -> Self {
        Self {
            config,
            messages: Vec::new(),
            turn_count: 0,
            bridge: Some(bridge),
        }
    }

    pub fn add_message(&mut self, msg: Message) {
        self.messages.push(msg);
    }

    pub fn messages(&self) -> &[Message] {
        &self.messages
    }

    pub async fn run(&mut self) -> anyhow::Result<AgentOutput> {
        let mut last_content = String::new();

        while self.turn_count < self.config.max_turns {
            self.turn_count += 1;
            let response = self.model_response().await?;
            let tool_calls = parse_tool_calls(&response);
            let content = if tool_calls.is_empty() {
                response
            } else {
                format!("{} tool call(s) requested", tool_calls.len())
            };

            self.messages.push(Message {
                role: Role::Assistant,
                content: content.clone(),
                tool_calls: (!tool_calls.is_empty()).then_some(tool_calls.clone()),
                tool_call_id: None,
            });

            last_content = content;

            if tool_calls.is_empty() {
                break;
            }

            for call in tool_calls {
                let result = execute_tool(&call).await;
                self.messages
                    .push(Message::tool_result(result.call_id, result.output));
            }
        }

        Ok(AgentOutput {
            content: last_content,
            completed: self.turn_count < self.config.max_turns,
            turns: self.turn_count,
        })
    }

    pub fn plan(&self) -> String {
        self.messages
            .iter()
            .filter(|message| message.role == Role::User)
            .map(|message| format!("- Respond to: {}", message.content))
            .collect::<Vec<_>>()
            .join("\n")
    }

    pub fn summarize(&self) -> String {
        let count = self.messages.len();
        let last = self
            .messages
            .last()
            .map(|message| message.content.as_str())
            .unwrap_or("empty");
        format!("{count} messages. Last: {last}")
    }

    async fn model_response(&mut self) -> anyhow::Result<String> {
        if let Some(bridge) = &mut self.bridge {
            let params = PromptParams {
                model: self.config.model.clone(),
                messages: self
                    .messages
                    .iter()
                    .filter(|message| message.role != Role::System)
                    .map(|message| PromptMessage {
                        role: message.role.as_str().to_string(),
                        content: message.content.clone(),
                        tool_calls: message.tool_calls.as_ref().map(|calls: &Vec<ToolCall>| {
                            calls
                                .iter()
                                .filter_map(|call| serde_json::to_value(call).ok())
                                .collect()
                        }),
                    })
                    .collect(),
                system: Some(self.config.system_prompt.clone()),
                max_tokens: None,
                temperature: None,
                tools: Some(
                    self.config
                        .tools
                        .iter()
                        .filter_map(|tool| serde_json::to_value(tool).ok())
                        .collect(),
                ),
            };
            return bridge
                .send_prompt(&params)
                .await
                .map(|response| response.content)
                .context("bridge prompt failed");
        }

        let prompt = self
            .messages
            .last()
            .map(|message| message.content.clone())
            .unwrap_or_default();
        Ok(format!("local response: {prompt}"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn local_agent_records_response() {
        let mut agent = Agent::new(AgentConfig::default());
        agent.add_message(Message::new(Role::User, "hello"));

        let output = agent.run().await.unwrap();

        assert!(output.completed);
        assert_eq!(output.turns, 1);
        assert_eq!(agent.messages().len(), 2);
        assert!(agent.summarize().contains("local response: hello"));
    }

    #[test]
    fn agent_config_defaults() {
        let config = AgentConfig::default();
        assert_eq!(config.model, "gpt-5.5");
        assert_eq!(config.max_turns, 20);
        assert!(config.tools.is_empty());
    }

    #[test]
    fn agent_plan_generates_from_messages() {
        let mut agent = Agent::new(AgentConfig::default());
        agent.add_message(Message::new(Role::User, "task 1"));
        agent.add_message(Message::new(Role::Assistant, "response 1"));
        agent.add_message(Message::new(Role::User, "task 2"));

        let plan = agent.plan();
        assert!(plan.contains("task 1"));
        assert!(plan.contains("task 2"));
        assert!(!plan.contains("response 1")); // only user messages
    }

    #[test]
    fn agent_summarize_empty() {
        let agent = Agent::new(AgentConfig::default());
        let summary = agent.summarize();
        assert_eq!(summary, "0 messages. Last: empty");
    }

    #[test]
    fn agent_output_equality() {
        let a = AgentOutput {
            content: "hello".into(),
            completed: true,
            turns: 3,
        };
        let b = AgentOutput {
            content: "hello".into(),
            completed: true,
            turns: 3,
        };
        assert_eq!(a, b);
    }

    #[test]
    fn agent_config_custom() {
        let config = AgentConfig {
            model: "custom-model".into(),
            max_turns: 50,
            system_prompt: "Custom prompt".into(),
            tools: vec![Tool {
                name: "test_tool".into(),
                description: "A test tool".into(),
                parameters: serde_json::json!({}),
            }],
        };
        let agent = Agent::new(config);
        assert_eq!(agent.messages().len(), 0);
    }

    #[tokio::test]
    async fn agent_with_many_turns() {
        let mut agent = Agent::new(AgentConfig {
            max_turns: 3,
            ..AgentConfig::default()
        });
        agent.add_message(Message::new(Role::User, "msg1"));
        agent.add_message(Message::new(Role::User, "msg2"));
        agent.add_message(Message::new(Role::User, "msg3"));

        let output = agent.run().await.unwrap();
        // Loop breaks after first non-tool response, so only 1 turn
        assert_eq!(output.turns, 1);
        assert!(output.completed);
        assert_eq!(agent.messages().len(), 4); // 3 user + 1 assistant
    }
}
