use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Tool {
    pub name: String,
    pub description: String,
    pub parameters: serde_json::Value,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ToolCall {
    pub id: String,
    pub name: String,
    pub arguments: serde_json::Value,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ToolResult {
    pub call_id: String,
    pub output: String,
    pub error: Option<String>,
}

pub async fn execute_tool(call: &ToolCall) -> ToolResult {
    ToolResult {
        call_id: call.id.clone(),
        output: format!("tool {} executed with {}", call.name, call.arguments),
        error: None,
    }
}

pub fn parse_tool_calls(response: &str) -> Vec<ToolCall> {
    let Ok(value) = serde_json::from_str::<serde_json::Value>(response) else {
        return Vec::new();
    };

    let calls = value
        .get("tool_calls")
        .and_then(|value| value.as_array())
        .cloned()
        .unwrap_or_default();

    calls
        .into_iter()
        .filter_map(|value| serde_json::from_value::<ToolCall>(value).ok())
        .collect()
}
