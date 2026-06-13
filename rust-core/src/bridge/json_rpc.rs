use std::{
    process::Stdio,
    sync::atomic::{AtomicU64, Ordering},
    time::Duration,
};

use serde::{Deserialize, Serialize};
use serde_json::Value;
#[cfg(test)]
use serde_json::json;
use tokio::{
    io::{AsyncBufReadExt, AsyncWriteExt, BufReader},
    process::{Child, ChildStdin, ChildStdout, Command},
    time::timeout,
};

const DEFAULT_TIMEOUT: Duration = Duration::from_secs(2);

#[allow(dead_code)]
#[derive(Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct JsonRpcRequest {
    pub jsonrpc: String,
    pub id: u64,
    pub method: String,
    pub params: Value,
}

#[allow(dead_code)]
#[derive(Debug, Serialize, Deserialize, PartialEq)]
pub struct JsonRpcResponse {
    pub jsonrpc: String,
    pub id: u64,
    #[serde(default)]
    pub result: Option<Value>,
    #[serde(default)]
    pub error: Option<Value>,
}

#[allow(dead_code)]
#[derive(Debug)]
pub struct Bridge {
    child: Child,
    stdin: ChildStdin,
    stdout: BufReader<ChildStdout>,
    next_id: AtomicU64,
    timeout: Duration,
}

impl Bridge {
    #[allow(dead_code)]
    pub async fn new(command: &str) -> anyhow::Result<Self> {
        Self::with_timeout(command, DEFAULT_TIMEOUT).await
    }

    #[allow(dead_code)]
    pub async fn with_timeout(command: &str, timeout_duration: Duration) -> anyhow::Result<Self> {
        if command.trim().is_empty() {
            anyhow::bail!("bridge command cannot be empty");
        }
        let mut child = Command::new("sh")
            .arg("-c")
            .arg(command)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn()?;
        let stdin = child
            .stdin
            .take()
            .ok_or_else(|| anyhow::anyhow!("bridge child stdin unavailable"))?;
        let stdout = child
            .stdout
            .take()
            .ok_or_else(|| anyhow::anyhow!("bridge child stdout unavailable"))?;

        Ok(Self {
            child,
            stdin,
            stdout: BufReader::new(stdout),
            next_id: AtomicU64::new(1),
            timeout: timeout_duration,
        })
    }

    #[allow(dead_code)]
    pub async fn call(&mut self, method: &str, params: Value) -> anyhow::Result<Value> {
        let id = self.next_id.fetch_add(1, Ordering::SeqCst);
        let request = JsonRpcRequest {
            jsonrpc: "2.0".to_string(),
            id,
            method: method.to_string(),
            params,
        };
        let mut payload = serde_json::to_vec(&request)?;
        payload.push(b'\n');
        self.stdin.write_all(&payload).await?;
        self.stdin.flush().await?;

        let mut line = String::new();
        let bytes = timeout(self.timeout, self.stdout.read_line(&mut line)).await??;
        if bytes == 0 {
            anyhow::bail!("bridge closed stdout before responding");
        }

        let response: JsonRpcResponse = serde_json::from_str(&line)?;
        if response.id != id {
            anyhow::bail!(
                "bridge response id {} did not match request id {}",
                response.id,
                id
            );
        }
        if let Some(error) = response.error {
            anyhow::bail!("bridge error: {error}");
        }
        response
            .result
            .ok_or_else(|| anyhow::anyhow!("bridge response missing result"))
    }

    #[allow(dead_code)]
    pub async fn list_skills(&mut self) -> anyhow::Result<Vec<String>> {
        let value = self.call("list_skills", Value::Null).await?;
        Ok(serde_json::from_value(value)?)
    }
}

impl Drop for Bridge {
    fn drop(&mut self) {
        let _ = self.child.start_kill();
    }
}

#[allow(dead_code)]
pub fn serialize_request(id: u64, method: &str, params: Value) -> anyhow::Result<String> {
    Ok(serde_json::to_string(&JsonRpcRequest {
        jsonrpc: "2.0".to_string(),
        id,
        method: method.to_string(),
        params,
    })?)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn serializes_json_rpc_request() {
        let serialized = serialize_request(7, "skills/list", json!({"limit": 2})).unwrap();
        let request: JsonRpcRequest = serde_json::from_str(&serialized).unwrap();

        assert_eq!(request.jsonrpc, "2.0");
        assert_eq!(request.id, 7);
        assert_eq!(request.method, "skills/list");
        assert_eq!(request.params, json!({"limit": 2}));
    }

    #[tokio::test]
    async fn bridge_starts_and_reads_mock_response() {
        let mock = "printf '{\"jsonrpc\":\"2.0\",\"id\":1,\"result\":[\"shell\",\"files\"]}\\n'";
        let mut bridge = Bridge::new(&format!("sh -c {mock:?}")).await.unwrap();

        let skills = bridge.list_skills().await.unwrap();

        assert_eq!(skills, vec!["shell", "files"]);
    }

    #[tokio::test]
    async fn bridge_times_out_when_child_is_silent() {
        let mut bridge = Bridge::with_timeout("sleep 2", Duration::from_millis(50))
            .await
            .unwrap();

        let err = bridge.call("noop", Value::Null).await.unwrap_err();

        assert!(err.to_string().contains("deadline has elapsed"));
    }
}
