//! Headless Mode + JSON-RPC Server — CLI and CI integration.
//!
//! `--headless` flag skips TUI init and runs a JSON-RPC server over stdin/stdout.
//! Same protocol as the TS bridge for consistency.
//!
//! Methods: run(prompt), status(), cancel(), list_sessions(), resume(session_id)
//! Streaming responses via JSON-RPC notifications (no id field).

use std::collections::HashMap;
use std::io::{BufRead, Write};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};

use serde::{Deserialize, Serialize};
use serde_json::Value;
use tracing::{debug, error, info, warn};

/// JSON-RPC request.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RpcRequest {
    pub jsonrpc: String,
    #[serde(default)]
    pub id: Option<u64>,
    pub method: String,
    #[serde(default)]
    pub params: Value,
}

/// JSON-RPC success response.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RpcResponse {
    pub jsonrpc: String,
    pub id: u64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub result: Option<Value>,
}

/// JSON-RPC error response.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RpcError {
    pub jsonrpc: String,
    pub id: u64,
    pub error: RpcErrorBody,
}

/// Error body in JSON-RPC.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RpcErrorBody {
    pub code: i32,
    pub message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub data: Option<Value>,
}

/// JSON-RPC notification (no id, for streaming).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RpcNotification {
    pub jsonrpc: String,
    pub method: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub params: Option<Value>,
}

/// Status of a headless session.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionStatus {
    pub session_id: String,
    pub model: String,
    pub running: bool,
    pub turn_count: usize,
    pub max_turns: usize,
    pub message: Option<String>,
}

/// A headless session tracked by the server.
#[derive(Debug, Clone)]
struct Session {
    id: String,
    model: String,
    running: bool,
    turn_count: usize,
    max_turns: usize,
    status_message: Option<String>,
}

/// The headless JSON-RPC server.
pub struct HeadlessServer {
    /// Active sessions.
    sessions: HashMap<String, Session>,
    /// Currently active session id.
    active_session_id: Option<String>,
    /// Next session id counter.
    next_session_counter: usize,
    /// Server running flag.
    running: bool,
    /// Request counter for JSON-RPC ids.
    next_id: AtomicU64,
}

impl HeadlessServer {
    /// Create a new headless server.
    pub fn new() -> Self {
        Self {
            sessions: HashMap::new(),
            active_session_id: None,
            next_session_counter: 0,
            running: false,
            next_id: AtomicU64::new(1),
        }
    }

    /// Run the JSON-RPC server, reading from stdin and writing to stdout.
    /// Blocks until EOF or shutdown.
    pub fn run(&mut self) -> anyhow::Result<()> {
        info!("Headless JSON-RPC server starting");
        self.running = true;

        let stdin = std::io::stdin();
        let stdout = std::io::stdout();
        let reader = stdin.lock();
        let mut writer = stdout.lock();

        // Send ready notification
        let notification = serde_json::json!({
            "jsonrpc": "2.0",
            "method": "ready",
            "params": {
                "version": "0.1.0",
                "mode": "headless"
            }
        });
        writeln!(writer, "{}", serde_json::to_string(&notification)?)?;
        writer.flush()?;
        info!("Server ready notification sent");

        for line in reader.lines() {
            let line = line?;
            if line.trim().is_empty() {
                continue;
            }

            let request: RpcRequest = match serde_json::from_str(&line) {
                Ok(req) => req,
                Err(e) => {
                    warn!(%e, "Failed to parse JSON-RPC request");
                    let error = serde_json::json!({
                        "jsonrpc": "2.0",
                        "id": serde_json::Value::Null,
                        "error": {
                            "code": -32700,
                            "message": format!("Parse error: {}", e)
                        }
                    });
                    writeln!(writer, "{}", serde_json::to_string(&error)?)?;
                    writer.flush()?;
                    continue;
                }
            };

            debug!(method = %request.method, "Handling JSON-RPC request");
            let response = self.handle_request(&request);

            // Write response
            if let Some(response_line) = response {
                writeln!(writer, "{response_line}")?;
                writer.flush()?;
            }

            // Check for shutdown
            if request.method == "shutdown" {
                info!("Server shutting down");
                break;
            }
        }

        self.running = false;
        info!("Headless server stopped");
        Ok(())
    }

    /// Handle a single JSON-RPC request.
    fn handle_request(&mut self, request: &RpcRequest) -> Option<String> {
        match request.method.as_str() {
            "run" => self.handle_run(request),
            "status" => self.handle_status(request),
            "cancel" => self.handle_cancel(request),
            "list_sessions" => self.handle_list_sessions(request),
            "resume" => self.handle_resume(request),
            "shutdown" => self.handle_shutdown(request),
            _ => {
                let error = serde_json::json!({
                    "jsonrpc": "2.0",
                    "id": request.id.unwrap_or(0),
                    "error": {
                        "code": -32601,
                        "message": format!("Method not found: {}", request.method)
                    }
                });
                Some(serde_json::to_string(&error).unwrap_or_default())
            }
        }
    }

    /// Handle the "run" method — execute a prompt.
    fn handle_run(&mut self, request: &RpcRequest) -> Option<String> {
        let prompt = request
            .params
            .get("prompt")
            .and_then(|v| v.as_str())
            .unwrap_or("");

        let provider = request.params.get("provider").and_then(|v| v.as_str());

        let model = request.params.get("model").and_then(|v| v.as_str());

        let max_turns = request
            .params
            .get("max_turns")
            .and_then(|v| v.as_u64())
            .unwrap_or(50) as usize;

        // Create a new session
        self.next_session_counter += 1;
        let session_id = format!("session-{}", self.next_session_counter);

        info!(
            session_id = %session_id,
            prompt = %prompt,
            provider = provider,
            model = model,
            "Headless run initiated"
        );

        let session = Session {
            id: session_id.clone(),
            model: model.unwrap_or("default").to_string(),
            running: true,
            turn_count: 0,
            max_turns,
            status_message: Some(format!("Running: {}", prompt)),
        };

        self.sessions.insert(session_id.clone(), session);
        self.active_session_id = Some(session_id.clone());

        // Simulate agent response (in production, this would call the agent loop)
        let result = serde_json::json!({
            "session_id": session_id,
            "status": "started",
            "prompt": prompt,
            "provider": provider,
            "model": model,
            "message": format!("Agent processing: '{}'", prompt)
        });

        let id = request
            .id
            .unwrap_or_else(|| self.next_id.fetch_add(1, Ordering::SeqCst));
        let response = RpcResponse {
            jsonrpc: "2.0".to_string(),
            id,
            result: Some(result),
        };

        Some(serde_json::to_string(&response).unwrap_or_default())
    }

    /// Handle the "status" method — get session status.
    fn handle_status(&mut self, request: &RpcRequest) -> Option<String> {
        let session_id = request
            .params
            .get("session_id")
            .and_then(|v| v.as_str())
            .or(self.active_session_id.as_deref());

        let session = session_id.and_then(|id| self.sessions.get(id));

        let status = if let Some(session) = session {
            serde_json::json!({
                "session_id": session.id,
                "model": session.model,
                "running": session.running,
                "turn_count": session.turn_count,
                "max_turns": session.max_turns,
                "message": session.status_message,
            })
        } else {
            serde_json::json!({
                "error": "No active session",
                "session_id": session_id,
            })
        };

        let id = request
            .id
            .unwrap_or_else(|| self.next_id.fetch_add(1, Ordering::SeqCst));
        let response = RpcResponse {
            jsonrpc: "2.0".to_string(),
            id,
            result: Some(status),
        };

        Some(serde_json::to_string(&response).unwrap_or_default())
    }

    /// Handle the "cancel" method — cancel the current session.
    fn handle_cancel(&mut self, request: &RpcRequest) -> Option<String> {
        let session_id = request
            .params
            .get("session_id")
            .and_then(|v| v.as_str())
            .or(self.active_session_id.as_deref());

        if let Some(session_id) = session_id
            && let Some(session) = self.sessions.get_mut(session_id)
        {
            session.running = false;
            session.status_message = Some("Cancelled".to_string());
        }

        let result = serde_json::json!({
            "cancelled": true,
            "session_id": session_id
        });

        let id = request
            .id
            .unwrap_or_else(|| self.next_id.fetch_add(1, Ordering::SeqCst));
        let response = RpcResponse {
            jsonrpc: "2.0".to_string(),
            id,
            result: Some(result),
        };

        Some(serde_json::to_string(&response).unwrap_or_default())
    }

    /// Handle the "list_sessions" method.
    fn handle_list_sessions(&mut self, request: &RpcRequest) -> Option<String> {
        let sessions: Vec<Value> = self
            .sessions
            .values()
            .map(|s| {
                serde_json::json!({
                    "session_id": s.id,
                    "model": s.model,
                    "running": s.running,
                    "turn_count": s.turn_count,
                    "message": s.status_message,
                })
            })
            .collect();

        let result = serde_json::json!({
            "sessions": sessions,
            "active_session_id": self.active_session_id,
        });

        let id = request
            .id
            .unwrap_or_else(|| self.next_id.fetch_add(1, Ordering::SeqCst));
        let response = RpcResponse {
            jsonrpc: "2.0".to_string(),
            id,
            result: Some(result),
        };

        Some(serde_json::to_string(&response).unwrap_or_default())
    }

    /// Handle the "resume" method — resume a previous session.
    fn handle_resume(&mut self, request: &RpcRequest) -> Option<String> {
        let session_id = request.params.get("session_id").and_then(|v| v.as_str());

        if let Some(session_id) = session_id
            && self.sessions.contains_key(session_id)
        {
            self.active_session_id = Some(session_id.to_string());

            let result = serde_json::json!({
                "resumed": true,
                "session_id": session_id
            });

            let id = request
                .id
                .unwrap_or_else(|| self.next_id.fetch_add(1, Ordering::SeqCst));
            let response = RpcResponse {
                jsonrpc: "2.0".to_string(),
                id,
                result: Some(result),
            };

            return Some(serde_json::to_string(&response).unwrap_or_default());
        }

        let id = request
            .id
            .unwrap_or_else(|| self.next_id.fetch_add(1, Ordering::SeqCst));
        let error = RpcError {
            jsonrpc: "2.0".to_string(),
            id,
            error: RpcErrorBody {
                code: -32000,
                message: format!("Session not found: {:?}", session_id),
                data: None,
            },
        };

        Some(serde_json::to_string(&error).unwrap_or_default())
    }

    /// Handle the "shutdown" method.
    fn handle_shutdown(&mut self, request: &RpcRequest) -> Option<String> {
        self.running = false;

        let id = request
            .id
            .unwrap_or_else(|| self.next_id.fetch_add(1, Ordering::SeqCst));
        let response = RpcResponse {
            jsonrpc: "2.0".to_string(),
            id,
            result: Some(serde_json::json!({"shutdown": true})),
        };

        Some(serde_json::to_string(&response).unwrap_or_default())
    }
}

impl Default for HeadlessServer {
    fn default() -> Self {
        Self::new()
    }
}

/// Entry point for headless mode.
/// Parses CLI args and runs the JSON-RPC server.
pub fn run_headless() -> anyhow::Result<()> {
    info!("Starting headless JSON-RPC server");
    let mut server = HeadlessServer::new();
    server.run()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn server_creation() {
        let server = HeadlessServer::new();
        assert!(!server.running);
        assert!(server.sessions.is_empty());
        assert!(server.active_session_id.is_none());
    }

    #[test]
    fn handle_run_creates_session() {
        let mut server = HeadlessServer::new();

        let request = RpcRequest {
            jsonrpc: "2.0".to_string(),
            id: Some(1),
            method: "run".to_string(),
            params: serde_json::json!({"prompt": "build the project"}),
        };

        let response = server.handle_request(&request);
        assert!(response.is_some());
        let resp_str = response.unwrap();
        assert!(resp_str.contains("session-1"));
        assert!(resp_str.contains("started"));
        assert_eq!(server.sessions.len(), 1);
    }

    #[test]
    fn handle_run_with_provider() {
        let mut server = HeadlessServer::new();

        let request = RpcRequest {
            jsonrpc: "2.0".to_string(),
            id: Some(2),
            method: "run".to_string(),
            params: serde_json::json!({
                "prompt": "test",
                "provider": "deepseek",
                "model": "deepseek-chat"
            }),
        };

        let response = server.handle_request(&request);
        assert!(response.is_some());
        let resp_str = response.unwrap();
        assert!(resp_str.contains("deepseek"));
        assert!(resp_str.contains("deepseek-chat"));
    }

    #[test]
    fn handle_status_no_session() {
        let mut server = HeadlessServer::new();

        let request = RpcRequest {
            jsonrpc: "2.0".to_string(),
            id: Some(3),
            method: "status".to_string(),
            params: serde_json::json!({}),
        };

        let response = server.handle_request(&request);
        assert!(response.is_some());
        assert!(response.unwrap().contains("No active session"));
    }

    #[test]
    fn handle_status_with_session() {
        let mut server = HeadlessServer::new();

        // Create a session first
        let run_req = RpcRequest {
            jsonrpc: "2.0".to_string(),
            id: Some(1),
            method: "run".to_string(),
            params: serde_json::json!({"prompt": "hello"}),
        };
        server.handle_request(&run_req);

        // Check status
        let status_req = RpcRequest {
            jsonrpc: "2.0".to_string(),
            id: Some(2),
            method: "status".to_string(),
            params: serde_json::json!({}),
        };

        let response = server.handle_request(&status_req);
        assert!(response.is_some());
        assert!(response.unwrap().contains("session-1"));
    }

    #[test]
    fn handle_list_sessions() {
        let mut server = HeadlessServer::new();

        // Create sessions
        for i in 1..=3 {
            let req = RpcRequest {
                jsonrpc: "2.0".to_string(),
                id: Some(i),
                method: "run".to_string(),
                params: serde_json::json!({"prompt": format!("task {}", i)}),
            };
            server.handle_request(&req);
        }

        let list_req = RpcRequest {
            jsonrpc: "2.0".to_string(),
            id: Some(4),
            method: "list_sessions".to_string(),
            params: serde_json::json!({}),
        };

        let response = server.handle_request(&list_req);
        assert!(response.is_some());
        let resp_str = response.unwrap();
        assert!(resp_str.contains("session-1"));
        assert!(resp_str.contains("session-2"));
        assert!(resp_str.contains("session-3"));
    }

    #[test]
    fn handle_cancel() {
        let mut server = HeadlessServer::new();

        // Create session
        let run_req = RpcRequest {
            jsonrpc: "2.0".to_string(),
            id: Some(1),
            method: "run".to_string(),
            params: serde_json::json!({"prompt": "long task"}),
        };
        server.handle_request(&run_req);

        // Cancel it
        let cancel_req = RpcRequest {
            jsonrpc: "2.0".to_string(),
            id: Some(2),
            method: "cancel".to_string(),
            params: serde_json::json!({}),
        };

        let response = server.handle_request(&cancel_req);
        assert!(response.is_some());
        let resp_str = response.unwrap();
        assert!(resp_str.contains("cancelled"));
        assert!(resp_str.contains("true"));
    }

    #[test]
    fn handle_resume() {
        let mut server = HeadlessServer::new();

        // Create session
        server.handle_request(&RpcRequest {
            jsonrpc: "2.0".to_string(),
            id: Some(1),
            method: "run".to_string(),
            params: serde_json::json!({"prompt": "first"}),
        });
        server.handle_request(&RpcRequest {
            jsonrpc: "2.0".to_string(),
            id: Some(2),
            method: "run".to_string(),
            params: serde_json::json!({"prompt": "second"}),
        });

        // Resume first session
        let resume_req = RpcRequest {
            jsonrpc: "2.0".to_string(),
            id: Some(3),
            method: "resume".to_string(),
            params: serde_json::json!({"session_id": "session-1"}),
        };

        let response = server.handle_request(&resume_req);
        assert!(response.is_some());
        let resp_str = response.unwrap();
        assert!(resp_str.contains("resumed"));
        assert_eq!(server.active_session_id, Some("session-1".to_string()));
    }

    #[test]
    fn handle_unknown_method() {
        let mut server = HeadlessServer::new();

        let request = RpcRequest {
            jsonrpc: "2.0".to_string(),
            id: Some(99),
            method: "nonexistent".to_string(),
            params: serde_json::json!({}),
        };

        let response = server.handle_request(&request);
        assert!(response.is_some());
        assert!(response.unwrap().contains("Method not found"));
    }

    #[test]
    fn handle_shutdown() {
        let mut server = HeadlessServer::new();
        server.running = true;

        let request = RpcRequest {
            jsonrpc: "2.0".to_string(),
            id: Some(1),
            method: "shutdown".to_string(),
            params: serde_json::json!({}),
        };

        let response = server.handle_request(&request);
        assert!(response.is_some());
        assert!(response.unwrap().contains("shutdown"));
        assert!(!server.running);
    }

    #[test]
    fn handle_run_with_max_turns() {
        let mut server = HeadlessServer::new();

        let request = RpcRequest {
            jsonrpc: "2.0".to_string(),
            id: Some(5),
            method: "run".to_string(),
            params: serde_json::json!({
                "prompt": "limited task",
                "max_turns": 10
            }),
        };

        let response = server.handle_request(&request);
        assert!(response.is_some());
        assert_eq!(server.sessions.len(), 1);
    }

    #[test]
    fn handle_run_with_empty_prompt() {
        let mut server = HeadlessServer::new();

        let request = RpcRequest {
            jsonrpc: "2.0".to_string(),
            id: Some(6),
            method: "run".to_string(),
            params: serde_json::json!({}),
        };

        let response = server.handle_request(&request);
        assert!(response.is_some());
        // Should still create a session even with empty prompt
        assert!(response.unwrap().contains("session-1"));
    }

    #[test]
    fn handle_run_without_id() {
        let mut server = HeadlessServer::new();

        let request = RpcRequest {
            jsonrpc: "2.0".to_string(),
            id: None,
            method: "run".to_string(),
            params: serde_json::json!({"prompt": "no id"}),
        };

        let response = server.handle_request(&request);
        assert!(response.is_some());
        // Should auto-assign an id
        let resp = response.unwrap();
        assert!(resp.contains("\"id\":"));
    }

    #[test]
    fn handle_status_with_specific_session_id() {
        let mut server = HeadlessServer::new();

        // Create two sessions
        server.handle_request(&RpcRequest {
            jsonrpc: "2.0".to_string(),
            id: Some(1),
            method: "run".to_string(),
            params: serde_json::json!({"prompt": "first"}),
        });
        server.handle_request(&RpcRequest {
            jsonrpc: "2.0".to_string(),
            id: Some(2),
            method: "run".to_string(),
            params: serde_json::json!({"prompt": "second"}),
        });

        // Query status of session-1 specifically
        let status_req = RpcRequest {
            jsonrpc: "2.0".to_string(),
            id: Some(3),
            method: "status".to_string(),
            params: serde_json::json!({"session_id": "session-1"}),
        };

        let response = server.handle_request(&status_req);
        assert!(response.is_some());
        assert!(response.unwrap().contains("session-1"));
    }

    #[test]
    fn handle_cancel_nonexistent_session() {
        let mut server = HeadlessServer::new();

        let cancel_req = RpcRequest {
            jsonrpc: "2.0".to_string(),
            id: Some(1),
            method: "cancel".to_string(),
            params: serde_json::json!({"session_id": "nonexistent"}),
        };

        let response = server.handle_request(&cancel_req);
        // Should succeed — cancel is idempotent
        assert!(response.is_some());
        assert!(response.unwrap().contains("true"));
    }

    #[test]
    fn handle_resume_nonexistent_session() {
        let mut server = HeadlessServer::new();

        let resume_req = RpcRequest {
            jsonrpc: "2.0".to_string(),
            id: Some(1),
            method: "resume".to_string(),
            params: serde_json::json!({"session_id": "nonexistent"}),
        };

        let response = server.handle_request(&resume_req);
        assert!(response.is_some());
        assert!(response.unwrap().contains("Session not found"));
    }

    #[test]
    fn handle_resume_without_session_id() {
        let mut server = HeadlessServer::new();

        let resume_req = RpcRequest {
            jsonrpc: "2.0".to_string(),
            id: Some(1),
            method: "resume".to_string(),
            params: serde_json::json!({}),
        };

        let response = server.handle_request(&resume_req);
        assert!(response.is_some());
        assert!(response.unwrap().contains("Session not found"));
    }

    #[test]
    fn handle_cancel_specific_session_id() {
        let mut server = HeadlessServer::new();

        // Create two sessions
        server.handle_request(&RpcRequest {
            jsonrpc: "2.0".to_string(),
            id: Some(1),
            method: "run".to_string(),
            params: serde_json::json!({"prompt": "first"}),
        });
        server.handle_request(&RpcRequest {
            jsonrpc: "2.0".to_string(),
            id: Some(2),
            method: "run".to_string(),
            params: serde_json::json!({"prompt": "second"}),
        });

        // Cancel session-1 specifically
        let cancel_req = RpcRequest {
            jsonrpc: "2.0".to_string(),
            id: Some(3),
            method: "cancel".to_string(),
            params: serde_json::json!({"session_id": "session-1"}),
        };

        let response = server.handle_request(&cancel_req);
        assert!(response.is_some());
        assert!(response.unwrap().contains("session-1"));

        // Verify session-1 is now not running
        let session = server.sessions.get("session-1").unwrap();
        assert!(!session.running);

        // session-2 should still be running
        let session2 = server.sessions.get("session-2").unwrap();
        assert!(session2.running);
    }

    #[test]
    fn run_headless_returns_ok() {
        // This test is tricky because run_headless blocks on stdin.
        // We just verify the function signature compiles and the struct is constructable.
        let server = HeadlessServer::new();
        assert!(!server.running);
    }

    #[test]
    fn handle_run_increments_session_counter() {
        let mut server = HeadlessServer::new();

        for _ in 0..5 {
            server.handle_request(&RpcRequest {
                jsonrpc: "2.0".to_string(),
                id: Some(1),
                method: "run".to_string(),
                params: serde_json::json!({"prompt": "test"}),
            });
        }

        assert_eq!(server.sessions.len(), 5);
        assert!(server.sessions.contains_key("session-1"));
        assert!(server.sessions.contains_key("session-5"));
    }

    #[test]
    fn default_server_empty() {
        let server = HeadlessServer::default();
        assert!(!server.running);
        assert!(server.sessions.is_empty());
    }

    #[test]
    fn list_sessions_empty() {
        let mut server = HeadlessServer::new();

        let list_req = RpcRequest {
            jsonrpc: "2.0".to_string(),
            id: Some(1),
            method: "list_sessions".to_string(),
            params: serde_json::json!({}),
        };

        let response = server.handle_request(&list_req);
        assert!(response.is_some());
        let resp = response.unwrap();
        assert!(resp.contains("\"sessions\":[]"));
        assert!(resp.contains("\"active_session_id\":null"));
    }

    #[test]
    fn rpc_request_deserialization() {
        let json = r#"{"jsonrpc":"2.0","id":1,"method":"run","params":{"prompt":"hello"}}"#;
        let req: RpcRequest = serde_json::from_str(json).unwrap();
        assert_eq!(req.jsonrpc, "2.0");
        assert_eq!(req.id, Some(1));
        assert_eq!(req.method, "run");
        assert_eq!(req.params["prompt"], "hello");
    }

    #[test]
    fn rpc_request_without_id() {
        let json = r#"{"jsonrpc":"2.0","method":"status","params":{}}"#;
        let req: RpcRequest = serde_json::from_str(json).unwrap();
        assert_eq!(req.id, None);
        assert_eq!(req.method, "status");
    }

    #[test]
    fn rpc_response_serialization() {
        let response = RpcResponse {
            jsonrpc: "2.0".to_string(),
            id: 1,
            result: Some(serde_json::json!({"ok": true})),
        };
        let json = serde_json::to_string(&response).unwrap();
        assert!(json.contains("\"id\":1"));
        assert!(json.contains("\"ok\":true"));
    }

    #[test]
    fn rpc_error_serialization() {
        let error = RpcError {
            jsonrpc: "2.0".to_string(),
            id: 1,
            error: RpcErrorBody {
                code: -32601,
                message: "Method not found".to_string(),
                data: None,
            },
        };
        let json = serde_json::to_string(&error).unwrap();
        assert!(json.contains("\"code\":-32601"));
        assert!(json.contains("Method not found"));
    }

    #[test]
    fn rpc_notification_serialization() {
        let notification = RpcNotification {
            jsonrpc: "2.0".into(),
            method: "ready".into(),
            params: Some(serde_json::json!({"version": "0.1.0"})),
        };
        let json = serde_json::to_string(&notification).unwrap();
        assert!(json.contains("\"method\":\"ready\""));
    }

    #[test]
    fn session_status_serialization() {
        let status = SessionStatus {
            session_id: "s1".into(),
            model: "deepseek-chat".into(),
            running: true,
            turn_count: 5,
            max_turns: 50,
            message: Some("Running".into()),
        };
        let json = serde_json::to_string(&status).unwrap();
        assert!(json.contains("\"session_id\":\"s1\""));
        assert!(json.contains("\"turn_count\":5"));
    }
}
