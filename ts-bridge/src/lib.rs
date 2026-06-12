//! ts-bridge — JSON-RPC over stdio bridge connecting Rust core to Pi TypeScript skills.
//!
//! This crate provides:
//! - JSON-RPC protocol types for communicating with a TypeScript child process
//! - Methods: call_skill, list_skills, send_prompt, register_tool
//! - Full skills registry exposure

pub mod rpc;

use std::io::{BufRead, BufReader, Write};
use std::process::{Child, ChildStdin, ChildStdout, Command, Stdio};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Duration;

use anyhow::Context;
use serde::{Deserialize, Serialize};
use serde_json::Value;

const DEFAULT_TIMEOUT: Duration = Duration::from_secs(30);

/// A JSON-RPC 2.0 request.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JsonRpcRequest {
    pub jsonrpc: String,
    pub id: u64,
    pub method: String,
    pub params: Value,
}

/// A JSON-RPC 2.0 response.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JsonRpcResponse {
    pub jsonrpc: String,
    pub id: u64,
    #[serde(default)]
    pub result: Option<Value>,
    #[serde(default)]
    pub error: Option<JsonRpcError>,
}

/// A JSON-RPC 2.0 error object.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JsonRpcError {
    pub code: i64,
    pub message: String,
    #[serde(default)]
    pub data: Option<Value>,
}

/// Information about a registered skill.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SkillInfo {
    pub name: String,
    pub description: String,
    #[serde(default)]
    pub parameters: Value,
}

/// Arguments for calling a skill.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CallSkillArgs {
    pub name: String,
    pub args: Value,
}

/// Prompt parameters for send_prompt.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SendPromptArgs {
    pub text: String,
    #[serde(default)]
    pub provider: Option<String>,
    #[serde(default)]
    pub model: Option<String>,
    #[serde(default)]
    pub system: Option<String>,
}

/// A streaming token from send_prompt.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TokenChunk {
    pub text: String,
    #[serde(default)]
    pub done: bool,
}

/// Tool registration parameters.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RegisterToolArgs {
    pub name: String,
    pub schema: Value,
    pub handler: String,
}

/// Result of calling a skill.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CallSkillResult {
    pub success: bool,
    pub data: Value,
    #[serde(default)]
    pub error: Option<String>,
}

/// The main bridge struct that manages the TypeScript child process.
#[derive(Debug)]
pub struct TsBridge {
    child: Option<Child>,
    stdin: Option<ChildStdin>,
    stdout: Option<BufReader<ChildStdout>>,
    next_id: AtomicU64,
    timeout: Duration,
}

impl TsBridge {
    /// Create a new bridge by spawning a TypeScript process.
    pub fn spawn(command: &str, args: &[&str]) -> anyhow::Result<Self> {
        let mut child = Command::new(command)
            .args(args)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::inherit())
            .spawn()
            .with_context(|| format!("Failed to spawn TS bridge process: {command}"))?;

        let stdin = child.stdin.take().context("Failed to get bridge stdin")?;
        let stdout = child.stdout.take().context("Failed to get bridge stdout")?;

        Ok(Self {
            child: Some(child),
            stdin: Some(stdin),
            stdout: Some(BufReader::new(stdout)),
            next_id: AtomicU64::new(1),
            timeout: DEFAULT_TIMEOUT,
        })
    }

    /// Send a JSON-RPC request and read the response.
    pub fn call_method(&mut self, method: &str, params: Value) -> anyhow::Result<Value> {
        let id = self.next_id.fetch_add(1, Ordering::SeqCst);

        let request = JsonRpcRequest {
            jsonrpc: "2.0".to_string(),
            id,
            method: method.to_string(),
            params,
        };

        let payload = serde_json::to_string(&request)?;
        {
            let stdin = self.stdin.as_mut().context("Bridge stdin not available")?;
            writeln!(stdin, "{payload}")?;
            stdin.flush()?;
        }

        let mut line = String::new();
        {
            let stdout = self
                .stdout
                .as_mut()
                .context("Bridge stdout not available")?;
            let bytes_read = stdout.read_line(&mut line)?;
            if bytes_read == 0 {
                anyhow::bail!("Bridge process closed stdout");
            }
        }

        let response: JsonRpcResponse = serde_json::from_str(&line)?;

        if response.id != id {
            anyhow::bail!("Response id {} doesn't match request id {id}", response.id);
        }

        if let Some(error) = response.error {
            anyhow::bail!("Bridge RPC error (code {}): {}", error.code, error.message);
        }

        response
            .result
            .ok_or_else(|| anyhow::anyhow!("Bridge response missing result field"))
    }

    /// Call a TypeScript skill by name.
    pub fn call_skill(&mut self, name: &str, args: Value) -> anyhow::Result<CallSkillResult> {
        let params = serde_json::to_value(CallSkillArgs {
            name: name.to_string(),
            args,
        })?;
        let result = self.call_method("call_skill", params)?;
        let skill_result: CallSkillResult = serde_json::from_value(result)?;
        Ok(skill_result)
    }

    /// List all available TypeScript skills.
    pub fn list_skills(&mut self) -> anyhow::Result<Vec<SkillInfo>> {
        let result = self.call_method("list_skills", Value::Null)?;
        let skills: Vec<SkillInfo> = serde_json::from_value(result)?;
        Ok(skills)
    }

    /// Send a prompt to the TypeScript AI provider.
    pub fn send_prompt(
        &mut self,
        text: &str,
        provider: Option<&str>,
    ) -> anyhow::Result<Vec<TokenChunk>> {
        let params = serde_json::to_value(SendPromptArgs {
            text: text.to_string(),
            provider: provider.map(|s| s.to_string()),
            model: None,
            system: None,
        })?;
        let result = self.call_method("send_prompt", params)?;

        // The response could be either a single result or an array of tokens
        if let Some(array) = result.as_array() {
            let tokens: Vec<TokenChunk> = serde_json::from_value(Value::Array(array.clone()))?;
            Ok(tokens)
        } else {
            // Single token response
            let tokens: Vec<TokenChunk> = serde_json::from_value(result)?;
            Ok(tokens)
        }
    }

    /// Register a tool with the TypeScript system.
    pub fn register_tool(
        &mut self,
        name: &str,
        schema: Value,
        handler: &str,
    ) -> anyhow::Result<()> {
        let params = serde_json::to_value(RegisterToolArgs {
            name: name.to_string(),
            schema,
            handler: handler.to_string(),
        })?;
        let _ = self.call_method("register_tool", params)?;
        Ok(())
    }

    /// Check if the bridge process is still alive.
    pub fn is_alive(&mut self) -> bool {
        if let Some(ref mut child) = self.child {
            matches!(child.try_wait(), Ok(None))
        } else {
            false
        }
    }

    /// Kill the bridge process.
    pub fn shutdown(&mut self) {
        if let Some(mut child) = self.child.take() {
            let _ = child.kill();
            let _ = child.wait();
        }
    }

    /// Set the timeout for RPC calls.
    pub fn set_timeout(&mut self, timeout: Duration) {
        self.timeout = timeout;
    }
}

impl Drop for TsBridge {
    fn drop(&mut self) {
        self.shutdown();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_json_rpc_request_serialization() {
        let req = JsonRpcRequest {
            jsonrpc: "2.0".to_string(),
            id: 1,
            method: "call_skill".to_string(),
            params: serde_json::json!({"name": "test", "args": {}}),
        };
        let json = serde_json::to_string(&req).unwrap();
        assert!(json.contains("call_skill"));
        assert!(json.contains("2.0"));
    }

    #[test]
    fn test_json_rpc_response_deserialization() {
        let json = r#"{"jsonrpc":"2.0","id":1,"result":"ok"}"#;
        let resp: JsonRpcResponse = serde_json::from_str(json).unwrap();
        assert_eq!(resp.id, 1);
        assert_eq!(resp.result.unwrap(), serde_json::json!("ok"));
    }

    #[test]
    fn test_json_rpc_error_deserialization() {
        let json =
            r#"{"jsonrpc":"2.0","id":1,"error":{"code":-32601,"message":"Method not found"}}"#;
        let resp: JsonRpcResponse = serde_json::from_str(json).unwrap();
        assert!(resp.error.is_some());
        assert_eq!(resp.error.as_ref().unwrap().code, -32601);
    }

    #[test]
    fn test_skill_info_serialization() {
        let skill = SkillInfo {
            name: "shell".to_string(),
            description: "Run shell commands".to_string(),
            parameters: serde_json::json!({"type": "object"}),
        };
        let json = serde_json::to_string(&skill).unwrap();
        assert!(json.contains("shell"));
    }

    #[test]
    fn test_call_skill_args() {
        let args = CallSkillArgs {
            name: "test".to_string(),
            args: serde_json::json!({"key": "value"}),
        };
        let json = serde_json::to_value(&args).unwrap();
        assert_eq!(json["name"], "test");
    }

    #[test]
    fn test_send_prompt_args() {
        let args = SendPromptArgs {
            text: "Hello".to_string(),
            provider: Some("openai".to_string()),
            model: None,
            system: None,
        };
        let json = serde_json::to_string(&args).unwrap();
        assert!(json.contains("Hello"));
        assert!(json.contains("openai"));
    }

    #[test]
    fn test_register_tool_args() {
        let args = RegisterToolArgs {
            name: "my_tool".to_string(),
            schema: serde_json::json!({"type": "object", "properties": {}}),
            handler: "my_handler".to_string(),
        };
        let json = serde_json::to_string(&args).unwrap();
        assert!(json.contains("my_tool"));
        assert!(json.contains("my_handler"));
    }
}
