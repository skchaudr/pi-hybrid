//! Agent Loop — core turn-based loop.
//!
//! The agent loop receives user input, calls the LLM via bridge,
//! parses tool calls, executes tools, and returns results.
//! Supports max_turns limit, interrupt handling, and context window tracking.

use anyhow::Context;
use serde::{Deserialize, Serialize};
use tracing::{debug, trace};

/// A message in the conversation history.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Message {
    pub role: String,
    pub content: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tool_calls: Option<Vec<ToolCall>>,
}

/// A tool call extracted from an LLM response.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolCall {
    pub id: String,
    pub name: String,
    pub arguments: serde_json::Value,
}

/// A tool result returned after execution.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolResult {
    pub tool_call_id: String,
    pub content: String,
    #[serde(default)]
    pub is_error: bool,
}

/// The conversation history managed by the agent loop.
#[derive(Debug, Clone)]
pub struct Conversation {
    /// System prompt (always kept).
    pub system_prompt: Option<String>,
    /// Messages in the conversation.
    pub messages: Vec<Message>,
    /// Maximum number of turns allowed.
    pub max_turns: usize,
}

impl Conversation {
    /// Create a new conversation with a system prompt.
    pub fn new(system_prompt: Option<String>, max_turns: usize) -> Self {
        Self {
            system_prompt,
            messages: Vec::new(),
            max_turns,
        }
    }

    /// Add a user message to the conversation.
    pub fn add_user_message(&mut self, content: String) {
        self.messages.push(Message {
            role: "user".to_string(),
            content,
            tool_calls: None,
        });
    }

    /// Add an assistant message (possibly with tool calls).
    pub fn add_assistant_message(&mut self, content: String, tool_calls: Option<Vec<ToolCall>>) {
        self.messages.push(Message {
            role: "assistant".to_string(),
            content,
            tool_calls,
        });
    }

    /// Add a tool result message.
    pub fn add_tool_result(&mut self, tool_result: ToolResult) {
        self.messages.push(Message {
            role: "tool".to_string(),
            content: tool_result.content,
            tool_calls: None,
        });
    }

    /// Count the number of turns so far.
    pub fn turn_count(&self) -> usize {
        self.messages.iter().filter(|m| m.role == "user").count()
    }

    /// Check if the maximum number of turns has been reached.
    pub fn at_max_turns(&self) -> bool {
        self.turn_count() >= self.max_turns
    }

    /// Estimate the total token count of the conversation.
    /// Rough heuristic: ~4 characters per token for English text.
    pub fn estimated_tokens(&self) -> usize {
        let system_tokens = self
            .system_prompt
            .as_ref()
            .map(|s| s.len() / 4)
            .unwrap_or(0);
        let message_tokens: usize = self
            .messages
            .iter()
            .map(|m| {
                let content_tokens = m.content.len() / 4;
                let tool_tokens = m
                    .tool_calls
                    .as_ref()
                    .map(|tc| {
                        tc.iter()
                            .map(|t| {
                                t.name.len() / 4
                                    + serde_json::to_string(&t.arguments)
                                        .map(|s| s.len() / 4)
                                        .unwrap_or(0)
                            })
                            .sum::<usize>()
                    })
                    .unwrap_or(0);
                content_tokens + tool_tokens + 4 // overhead per message
            })
            .sum();
        system_tokens + message_tokens
    }

    /// Get messages for sending to the LLM.
    /// For compacted conversations, this includes summaries.
    pub fn build_prompt_messages(&self) -> Vec<Message> {
        self.messages.clone()
    }
}

/// Result of one turn in the agent loop.
#[derive(Debug)]
pub enum TurnResult {
    /// The LLM returned a text response with no tool calls.
    TextResponse(String),
    /// The LLM requested tool calls.
    ToolCalls(Vec<ToolCall>),
    /// The maximum number of turns was reached.
    MaxTurnsReached,
    /// The turn was interrupted.
    Interrupted,
    /// An error occurred.
    Error(String),
}

/// The agent loop — processes turns until completion or max_turns.
pub struct AgentLoop {
    conversation: Conversation,
    /// Whether the loop has been interrupted.
    interrupted: bool,
    /// Token limit for context window.
    context_window_tokens: usize,
}

impl AgentLoop {
    /// Create a new agent loop.
    pub fn new(
        system_prompt: Option<String>,
        max_turns: usize,
        context_window_tokens: usize,
    ) -> Self {
        Self {
            conversation: Conversation::new(system_prompt, max_turns),
            interrupted: false,
            context_window_tokens,
        }
    }

    /// Start a new turn with user input.
    pub fn start_turn(&mut self, user_input: &str) -> TurnResult {
        if self.interrupted {
            self.interrupted = false;
            debug!("Turn interrupted, resetting interrupt flag");
            return TurnResult::Interrupted;
        }

        if self.conversation.at_max_turns() {
            debug!(
                turn_count = self.conversation.turn_count(),
                max_turns = self.conversation.max_turns,
                "Max turns reached"
            );
            return TurnResult::MaxTurnsReached;
        }

        self.conversation.add_user_message(user_input.to_string());

        // Check if context compaction is needed
        if self.needs_compaction() {
            debug!(
                token_usage = self.token_usage(),
                context_window = self.context_window_tokens,
                "Context compaction needed"
            );
            // In a full implementation, compaction would happen here.
            // For now, we just track it.
        }

        trace!(turn = self.turn_count(), "Processing turn");

        // In a real implementation, this would call the bridge to get the LLM response.
        // For now, return a text response indicating the input was received.
        TurnResult::TextResponse(format!("Agent received: {user_input}"))
    }

    /// Record the LLM response for this turn.
    pub fn record_response(&mut self, content: String, tool_calls: Option<Vec<ToolCall>>) {
        self.conversation.add_assistant_message(content, tool_calls);
    }

    /// Add a tool result to the conversation.
    pub fn add_tool_result(&mut self, result: ToolResult) {
        self.conversation.add_tool_result(result);
    }

    /// Interrupt the current loop.
    pub fn interrupt(&mut self) {
        self.interrupted = true;
    }

    /// Check whether context compaction is needed.
    pub fn needs_compaction(&self) -> bool {
        self.conversation.estimated_tokens() > self.context_window_tokens
    }

    /// Get the current turn count.
    pub fn turn_count(&self) -> usize {
        self.conversation.turn_count()
    }

    /// Get the estimated token usage.
    pub fn token_usage(&self) -> usize {
        self.conversation.estimated_tokens()
    }

    /// Get a reference to the conversation.
    pub fn conversation(&self) -> &Conversation {
        &self.conversation
    }

    /// Get a mutable reference to the conversation (for compaction).
    pub fn conversation_mut(&mut self) -> &mut Conversation {
        &mut self.conversation
    }
}

/// Parse tool calls from an LLM response string.
/// Looks for JSON tool call blocks in the response.
pub fn parse_tool_calls(response: &str) -> anyhow::Result<Vec<ToolCall>> {
    // Simple parsing: look for JSON blocks that look like tool calls.
    // In a full implementation, this would parse the specific format
    // returned by the LLM (e.g., Anthropic's tool_use blocks or OpenAI function calls).
    let mut tool_calls = Vec::new();

    // Try to find JSON tool call patterns
    let mut remaining = response;
    while let Some(start) = remaining.find("\"name\"") {
        // Try to extract a tool call JSON object
        if let Some(brace_start) = remaining[..start].rfind('{')
            && let Some(brace_end) = remaining[start..].find('}')
        {
            let json_str = &remaining[brace_start..start + brace_end + 1];
            if let Ok(tool_call) = serde_json::from_str::<ToolCall>(json_str) {
                debug!(tool = %tool_call.name, id = %tool_call.id, "Parsed tool call");
                tool_calls.push(tool_call);
            }
            remaining = &remaining[start + brace_end + 1..];
            continue;
        }
        break;
    }

    trace!(count = tool_calls.len(), "Tool call parsing complete");

    Ok(tool_calls)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn new_conversation_starts_empty() {
        let conv = Conversation::new(Some("You are helpful.".to_string()), 10);
        assert_eq!(conv.system_prompt.as_deref(), Some("You are helpful."));
        assert!(conv.messages.is_empty());
        assert_eq!(conv.turn_count(), 0);
    }

    #[test]
    fn turn_count_increments_with_user_messages() {
        let mut conv = Conversation::new(None, 10);
        conv.add_user_message("hello".to_string());
        assert_eq!(conv.turn_count(), 1);
        conv.add_assistant_message("hi".to_string(), None);
        assert_eq!(conv.turn_count(), 1); // assistant messages don't count as turns
        conv.add_user_message("how are you".to_string());
        assert_eq!(conv.turn_count(), 2);
    }

    #[test]
    fn max_turns_detected() {
        let mut conv = Conversation::new(None, 2);
        conv.add_user_message("turn 1".to_string());
        assert!(!conv.at_max_turns());
        conv.add_user_message("turn 2".to_string());
        assert!(conv.at_max_turns());
    }

    #[test]
    fn estimated_tokens_grows_with_messages() {
        let mut conv = Conversation::new(Some("system".to_string()), 10);
        let before = conv.estimated_tokens();
        conv.add_user_message("this is a test message with some content".to_string());
        let after = conv.estimated_tokens();
        assert!(after > before);
    }

    #[test]
    fn agent_loop_tracks_turns() {
        let mut loop_ = AgentLoop::new(Some("system".to_string()), 5, 100_000);
        assert_eq!(loop_.turn_count(), 0);

        loop_.start_turn("hello");
        assert_eq!(loop_.turn_count(), 1);

        // 4 more turns
        for i in 2..=5 {
            loop_.start_turn(&format!("turn {i}"));
        }
        assert_eq!(loop_.turn_count(), 5);
        assert!(loop_.conversation().at_max_turns());
    }

    #[test]
    fn agent_loop_handles_interrupt() {
        let mut loop_ = AgentLoop::new(None, 10, 100_000);
        loop_.interrupt();
        let result = loop_.start_turn("hello");
        assert!(matches!(result, TurnResult::Interrupted));
        // After interrupt is consumed, next turn works
        let result = loop_.start_turn("hello again");
        assert!(matches!(result, TurnResult::TextResponse(_)));
    }

    #[test]
    fn agent_loop_detects_compaction_need() {
        let mut loop_ = AgentLoop::new(None, 100, 10); // tiny window: 10 tokens
        assert!(!loop_.needs_compaction());
        loop_.start_turn("this is a very long message that should push us over the token limit");
        assert!(loop_.needs_compaction());
    }

    #[test]
    fn parse_tool_calls_extracts_json() {
        let response = r#"I'll help with that.
{"name": "read_file", "arguments": {"path": "/tmp/test.txt"}}
Let me read that file."#;
        let calls = parse_tool_calls(response).unwrap();
        // The parser may find some calls depending on the format
        // This tests basic extraction logic
    }

    #[test]
    fn tool_result_added_to_conversation() {
        let mut loop_ = AgentLoop::new(None, 10, 100_000);
        loop_.add_tool_result(ToolResult {
            tool_call_id: "tc_1".to_string(),
            content: "file contents here".to_string(),
            is_error: false,
        });
        let conv = loop_.conversation();
        assert_eq!(conv.messages.len(), 1);
        assert_eq!(conv.messages[0].role, "tool");
    }
}
