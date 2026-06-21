//! Bridge Client — Async JSON-RPC client to the TS bridge (stdio).
//!
//! Methods: send_prompt, stream_tokens, cancel, list_skills, compact_context.
//! Handles reconnection and timeouts.
//!
//! Wraps the existing `bridge::Bridge` in an async-friendly interface.

use std::sync::Arc;
use std::time::{Duration, Instant};

use anyhow::Context;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use tokio::sync::Mutex;
use tokio::time::timeout;
use tracing::{debug, error, info, instrument, warn};

use crate::agent::providers::ProviderConfig;
use crate::bridge::Bridge;
use crate::shutdown::CancelToken;

/// Default timeout for bridge calls.
pub const DEFAULT_TIMEOUT: Duration = Duration::from_secs(30);

/// Prompt parameters for send_prompt.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PromptParams {
    /// The model to use.
    pub model: String,
    /// The messages in the conversation.
    pub messages: Vec<PromptMessage>,
    /// The system prompt.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub system: Option<String>,
    /// Maximum tokens to generate.
    #[serde(default)]
    pub max_tokens: Option<u32>,
    /// Temperature for generation.
    #[serde(default)]
    pub temperature: Option<f32>,
    /// Tools available to the model.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tools: Option<Vec<Value>>,
}

/// A single message in a prompt.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PromptMessage {
    pub role: String,
    pub content: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tool_calls: Option<Vec<Value>>,
}

/// Response from send_prompt.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PromptResponse {
    /// The text content of the response.
    pub content: String,
    /// Tool calls requested by the model.
    #[serde(default)]
    pub tool_calls: Vec<ToolCallResponse>,
    /// Token usage information.
    #[serde(default)]
    pub usage: Option<TokenUsage>,
    /// The finish reason.
    #[serde(default)]
    pub finish_reason: Option<String>,
}

/// A tool call in the response.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolCallResponse {
    pub id: String,
    pub name: String,
    pub arguments: Value,
}

/// Token usage statistics.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TokenUsage {
    pub prompt_tokens: u32,
    pub completion_tokens: u32,
    pub total_tokens: u32,
}

/// Compact context parameters.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CompactContextParams {
    /// The messages to compact.
    pub messages: Vec<PromptMessage>,
    /// Maximum tokens for the summary.
    #[serde(default)]
    pub max_summary_tokens: Option<u32>,
}

/// Compact context response.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CompactContextResponse {
    /// The summary text.
    pub summary: String,
    /// The number of original tokens.
    pub original_tokens: u32,
    /// The number of summary tokens.
    pub summary_tokens: u32,
}

/// A streaming token chunk.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TokenChunk {
    /// The token text.
    pub text: String,
    /// Whether this is the final chunk.
    #[serde(default)]
    pub done: bool,
}

/// The async bridge client wrapping the raw Bridge.
#[derive(Debug)]
pub struct BridgeClient {
    /// The underlying bridge connection.
    bridge: Bridge,
    /// Whether the client is connected.
    connected: bool,
    /// Number of reconnection attempts.
    reconnect_attempts: usize,
    /// Maximum reconnection attempts.
    max_reconnect_attempts: usize,
    /// Current provider configuration.
    provider: Option<ProviderConfig>,
    /// Cancellation token for graceful shutdown.
    cancel_token: Option<CancelToken>,
}

impl BridgeClient {
    /// Connect to the bridge using the given command.
    #[instrument(skip(command))]
    pub async fn connect(command: &str) -> anyhow::Result<Self> {
        info!(%command, "Connecting to bridge");
        let bridge = Bridge::with_timeout(command, DEFAULT_TIMEOUT)
            .await
            .context("Failed to start bridge process")?;

        info!("Bridge connected successfully");

        Ok(Self {
            bridge,
            connected: true,
            reconnect_attempts: 0,
            max_reconnect_attempts: 3,
            provider: None,
            cancel_token: None,
        })
    }

    /// Connect to the bridge with a specific provider.
    #[instrument(skip(command, provider), fields(provider_name = %provider.name))]
    pub async fn connect_with_provider(
        command: &str,
        provider: ProviderConfig,
    ) -> anyhow::Result<Self> {
        info!("Connecting to bridge with provider");
        let bridge = Bridge::with_timeout(command, DEFAULT_TIMEOUT)
            .await
            .context("Failed to start bridge process")?;

        Ok(Self {
            bridge,
            connected: true,
            reconnect_attempts: 0,
            max_reconnect_attempts: 3,
            provider: Some(provider),
            cancel_token: None,
        })
    }

    /// Set the provider configuration.
    pub fn set_provider(&mut self, provider: ProviderConfig) {
        self.provider = Some(provider);
    }

    /// Set the cancellation token for graceful shutdown.
    pub fn set_cancel_token(&mut self, token: CancelToken) {
        self.cancel_token = Some(token);
    }

    /// Check if cancellation has been requested.
    fn check_cancelled(&self) -> bool {
        self.cancel_token
            .as_ref()
            .map(|t| t.is_cancelled())
            .unwrap_or(false)
    }

    /// Get the current provider configuration.
    pub fn provider(&self) -> Option<&ProviderConfig> {
        self.provider.as_ref()
    }

    /// Run a prompt with a specific provider name.
    pub async fn run_with_provider(
        &mut self,
        prompt: &str,
        provider_name: &str,
    ) -> anyhow::Result<PromptResponse> {
        // Build params using provider config if available
        let model = self
            .provider
            .as_ref()
            .map(|p| p.default_model.clone())
            .unwrap_or_else(|| "default".to_string());

        let params = PromptParams {
            model,
            messages: vec![PromptMessage {
                role: "user".to_string(),
                content: prompt.to_string(),
                tool_calls: None,
            }],
            system: Some(default_system_prompt()),
            max_tokens: None,
            temperature: None,
            tools: None,
        };

        self.send_prompt(&params).await
    }

    /// Check if the client is connected.
    pub fn is_connected(&self) -> bool {
        self.connected
    }

    /// Send a prompt to the LLM and get a response.
    #[instrument(skip(self, params), fields(model = %params.model))]
    pub async fn send_prompt(&mut self, params: &PromptParams) -> anyhow::Result<PromptResponse> {
        if self.check_cancelled() {
            warn!("send_prompt skipped — cancellation requested");
            anyhow::bail!("Operation cancelled");
        }

        if !self.connected {
            error!("Bridge is not connected");
            anyhow::bail!("Bridge is not connected");
        }

        let params_value = serde_json::to_value(params)?;

        debug!("Sending prompt via bridge");

        let started = Instant::now();
        let result = timeout(
            DEFAULT_TIMEOUT,
            self.bridge.call("send_prompt", params_value),
        )
        .await
        .context("send_prompt timed out")??;
        let latency_ms = started.elapsed().as_millis() as u64;

        let response: PromptResponse =
            serde_json::from_value(result).context("Failed to parse prompt response")?;

        if let Some(ref usage) = response.usage {
            debug!(
                latency_ms,
                prompt_tokens = usage.prompt_tokens,
                completion_tokens = usage.completion_tokens,
                total_tokens = usage.total_tokens,
                "Prompt completed"
            );
        } else {
            debug!(latency_ms, "Prompt completed");
        }

        Ok(response)
    }

    /// Stream tokens from the LLM.
    /// Returns a receiver for token chunks.
    #[instrument(skip(self, params), fields(model = %params.model))]
    pub async fn stream_tokens(
        &mut self,
        params: &PromptParams,
    ) -> anyhow::Result<tokio::sync::mpsc::UnboundedReceiver<TokenChunk>> {
        if self.check_cancelled() {
            warn!("stream_tokens skipped — cancellation requested");
            anyhow::bail!("Operation cancelled");
        }

        if !self.connected {
            error!("Bridge is not connected for streaming");
            anyhow::bail!("Bridge is not connected");
        }

        debug!("Starting token stream");

        let params_value = serde_json::to_value(params)?;

        // For now, we simulate streaming by making a regular call
        // In a full implementation, this would use a streaming JSON-RPC mechanism
        let result = timeout(
            DEFAULT_TIMEOUT,
            self.bridge.call("stream_tokens", params_value),
        )
        .await
        .context("stream_tokens timed out")??;

        let (tx, rx) = tokio::sync::mpsc::unbounded_channel();

        // Parse the response as a single chunk
        if let Some(text) = result.get("text").and_then(|v| v.as_str()) {
            let _ = tx.send(TokenChunk {
                text: text.to_string(),
                done: false,
            });
        }
        let _ = tx.send(TokenChunk {
            text: String::new(),
            done: true,
        });

        Ok(rx)
    }

    /// Cancel an ongoing operation.
    #[instrument(skip(self))]
    pub async fn cancel(&mut self) -> anyhow::Result<()> {
        if !self.connected {
            return Ok(());
        }

        debug!("Cancelling bridge operation");

        let _ = timeout(
            Duration::from_secs(2),
            self.bridge.call("cancel", Value::Null),
        )
        .await;

        Ok(())
    }

    /// List available skills from the bridge.
    #[instrument(skip(self))]
    pub async fn list_skills(&mut self) -> anyhow::Result<Vec<String>> {
        if !self.connected {
            error!("Bridge not connected for list_skills");
            anyhow::bail!("Bridge is not connected");
        }

        debug!("Listing skills from bridge");

        let result = timeout(
            DEFAULT_TIMEOUT,
            self.bridge.call("list_skills", Value::Null),
        )
        .await
        .context("list_skills timed out")??;

        let skills: Vec<String> =
            serde_json::from_value(result).context("Failed to parse skills list")?;

        debug!(skill_count = skills.len(), "Skills retrieved");

        Ok(skills)
    }

    /// Compact context by summarizing messages.
    #[instrument(skip(self, params))]
    pub async fn compact_context(
        &mut self,
        params: &CompactContextParams,
    ) -> anyhow::Result<CompactContextResponse> {
        if self.check_cancelled() {
            warn!("compact_context skipped — cancellation requested");
            anyhow::bail!("Operation cancelled");
        }

        if !self.connected {
            error!("Bridge not connected for compact_context");
            anyhow::bail!("Bridge is not connected");
        }

        debug!("Compacting context via bridge");

        let params_value = serde_json::to_value(params)?;

        let result = timeout(
            DEFAULT_TIMEOUT,
            self.bridge.call("compact_context", params_value),
        )
        .await
        .context("compact_context timed out")??;

        let response: CompactContextResponse =
            serde_json::from_value(result).context("Failed to parse compact context response")?;

        info!(
            original_tokens = response.original_tokens,
            summary_tokens = response.summary_tokens,
            "Context compacted"
        );

        Ok(response)
    }

    /// Attempt to reconnect to the bridge.
    #[instrument(skip(self, command))]
    pub async fn reconnect(&mut self, command: &str) -> anyhow::Result<()> {
        if self.reconnect_attempts >= self.max_reconnect_attempts {
            error!(
                attempts = self.reconnect_attempts,
                max = self.max_reconnect_attempts,
                "Max reconnection attempts reached"
            );
            anyhow::bail!(
                "Max reconnection attempts ({}) reached",
                self.max_reconnect_attempts
            );
        }

        self.reconnect_attempts += 1;
        warn!(attempt = self.reconnect_attempts, "Reconnecting to bridge");

        // Dropping the old bridge will kill the child process
        let bridge = Bridge::with_timeout(command, DEFAULT_TIMEOUT)
            .await
            .context("Failed to reconnect bridge")?;

        self.bridge = bridge;
        self.connected = true;
        info!("Bridge reconnection successful");

        Ok(())
    }

    /// Get the number of reconnection attempts made.
    pub fn reconnect_attempts(&self) -> usize {
        self.reconnect_attempts
    }

    /// Reset the reconnection counter.
    pub fn reset_reconnect_attempts(&mut self) {
        self.reconnect_attempts = 0;
    }
}

/// Create a default system prompt for the agent.
pub fn default_system_prompt() -> String {
    r#"You are a helpful AI assistant running in the Pi Hybrid TUI.
You have access to tools for reading/writing files, running shell commands,
and managing the workspace.

When creating plans, break down tasks into clear, executable steps.
Each step should specify the tool and parameters needed.

Respond concisely and directly. Use tool calls when needed."#
        .to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn prompt_params_serialization() {
        let params = PromptParams {
            model: "test-model".to_string(),
            messages: vec![PromptMessage {
                role: "user".to_string(),
                content: "hello".to_string(),
                tool_calls: None,
            }],
            system: Some("You are helpful.".to_string()),
            max_tokens: Some(100),
            temperature: Some(0.7),
            tools: None,
        };

        let json = serde_json::to_string(&params).unwrap();
        assert!(json.contains("test-model"));
        assert!(json.contains("hello"));
        assert!(json.contains("You are helpful"));
    }

    #[test]
    fn prompt_response_deserialization() {
        let json = r#"{
            "content": "Hello! How can I help?",
            "tool_calls": [],
            "usage": {"prompt_tokens": 10, "completion_tokens": 5, "total_tokens": 15},
            "finish_reason": "stop"
        }"#;

        let response: PromptResponse = serde_json::from_str(json).unwrap();
        assert_eq!(response.content, "Hello! How can I help?");
        assert_eq!(response.usage.unwrap().total_tokens, 15);
    }

    #[test]
    fn compact_context_params() {
        let params = CompactContextParams {
            messages: vec![PromptMessage {
                role: "user".to_string(),
                content: "long message".to_string(),
                tool_calls: None,
            }],
            max_summary_tokens: Some(50),
        };

        let json = serde_json::to_string(&params).unwrap();
        assert!(json.contains("max_summary_tokens"));
    }

    #[test]
    fn token_chunk_serialization() {
        let chunk = TokenChunk {
            text: "hello".to_string(),
            done: false,
        };
        let json = serde_json::to_string(&chunk).unwrap();
        assert!(json.contains("hello"));
        assert!(json.contains("false"));
    }

    #[test]
    fn default_system_prompt_is_not_empty() {
        let prompt = default_system_prompt();
        assert!(!prompt.is_empty());
        assert!(prompt.contains("Pi Hybrid TUI"));
    }

    #[test]
    fn reconnect_attempts_tracking() {
        // We can't easily test actual reconnection without a real bridge,
        // but we can test the attempt tracking logic on a mock level.
        // This is tested in the integration tests.
    }

    #[test]
    fn prompt_message_serialization() {
        let msg = PromptMessage {
            role: "assistant".to_string(),
            content: "some content".to_string(),
            tool_calls: Some(vec![serde_json::json!({"id": "tc1", "name": "read_file"})]),
        };
        let json = serde_json::to_string(&msg).unwrap();
        assert!(json.contains("assistant"));
        assert!(json.contains("tool_calls"));
    }

    #[test]
    fn token_usage_struct() {
        let usage = TokenUsage {
            prompt_tokens: 100,
            completion_tokens: 50,
            total_tokens: 150,
        };
        assert_eq!(usage.total_tokens, 150);
        let json = serde_json::to_string(&usage).unwrap();
        assert!(json.contains("total_tokens"));
    }

    #[test]
    fn tool_call_response_struct() {
        let tc = ToolCallResponse {
            id: "call_1".into(),
            name: "read_file".into(),
            arguments: serde_json::json!({"path": "/tmp/test.txt"}),
        };
        assert_eq!(tc.id, "call_1");
        let json = serde_json::to_string(&tc).unwrap();
        assert!(json.contains("call_1"));
    }

    #[test]
    fn compact_context_response_struct() {
        let resp = CompactContextResponse {
            summary: "summary text".into(),
            original_tokens: 1000,
            summary_tokens: 200,
        };
        assert_eq!(resp.original_tokens, 1000);
    }

    #[test]
    fn default_timeout_is_30_seconds() {
        assert_eq!(DEFAULT_TIMEOUT, Duration::from_secs(30));
    }

    /// Regression: Bridge::new() uses a 2s read timeout; connect paths must pass
    /// BridgeClient::DEFAULT_TIMEOUT (30s) so slow real responses aren't cut off early.
    #[tokio::test]
    async fn connect_uses_client_default_timeout() {
        let mock = concat!(
            "sh -c 'sleep 3; ",
            "printf \"{\\\"jsonrpc\\\":\\\"2.0\\\",\\\"id\\\":1,\\\"result\\\":[\\\"ok\\\"]}\\n\"'",
        );
        let mut client = BridgeClient::connect(mock).await.unwrap();
        let skills = client.list_skills().await;

        assert!(
            skills.is_ok(),
            "3s mock response should succeed with 30s bridge timeout, got {:?}",
            skills.err()
        );
        assert_eq!(skills.unwrap(), vec!["ok".to_string()]);
    }

    #[tokio::test]
    async fn reconnect_uses_client_default_timeout() {
        let mock = concat!(
            "sh -c 'sleep 3; ",
            "printf \"{\\\"jsonrpc\\\":\\\"2.0\\\",\\\"id\\\":1,\\\"result\\\":[\\\"ok\\\"]}\\n\"'",
        );
        let mut client = BridgeClient::connect("true").await.unwrap();
        client.reconnect(mock).await.unwrap();
        let skills = client.list_skills().await;

        assert!(
            skills.is_ok(),
            "3s mock response after reconnect should succeed with 30s bridge timeout, got {:?}",
            skills.err()
        );
        assert_eq!(skills.unwrap(), vec!["ok".to_string()]);
    }

    #[test]
    fn prompt_params_no_system() {
        let params = PromptParams {
            model: "m".into(),
            messages: vec![],
            system: None,
            max_tokens: None,
            temperature: None,
            tools: None,
        };
        let json = serde_json::to_string(&params).unwrap();
        // system should be skipped
        assert!(!json.contains("system"));
    }

    #[test]
    fn prompt_message_no_tool_calls() {
        let msg = PromptMessage {
            role: "user".into(),
            content: "hello".into(),
            tool_calls: None,
        };
        let json = serde_json::to_string(&msg).unwrap();
        assert!(!json.contains("tool_calls"));
    }

    #[test]
    fn prompt_response_minimal() {
        let json = r#"{"content": "ok"}"#;
        let response: PromptResponse = serde_json::from_str(json).unwrap();
        assert_eq!(response.content, "ok");
        assert!(response.tool_calls.is_empty());
        assert!(response.usage.is_none());
        assert!(response.finish_reason.is_none());
    }

    #[test]
    fn token_chunk_done() {
        let chunk = TokenChunk {
            text: "".into(),
            done: true,
        };
        let json = serde_json::to_string(&chunk).unwrap();
        assert!(json.contains("true"));
    }
}
