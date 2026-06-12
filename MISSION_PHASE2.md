# PHASE 2 MISSION: Agent Loop + Parallel Subagents

Phases 0-1B are complete. The TUI has 9 modules, command palette, toggles, vim keybindings, JSON-RPC bridge. Now make it think.

## YOUR JOB

### 1. Build the Agent Runtime (rust-core/src/agent/)

**`src/agent/agent_core.rs`** — The agent loop
```rust
pub struct AgentConfig {
    pub model: String,           // "gpt-5.5"
    pub max_turns: usize,        // default 20
    pub system_prompt: String,
    pub tools: Vec<Tool>,
}

pub struct Agent {
    config: AgentConfig,
    messages: Vec<Message>,      // conversation history
    turn_count: usize,
    bridge: Option<Bridge>,      // TS Pi bridge (optional)
}

impl Agent {
    pub fn new(config: AgentConfig) -> Self;
    pub fn add_message(&mut self, msg: Message);
    pub async fn run(&mut self) -> Result<AgentOutput>;
    pub fn plan(&self) -> String;    // generate execution plan
    pub fn summarize(&self) -> String; // context compaction
}
```

The agent loop:
1. Build prompt from messages
2. Send to model (via bridge or local)
3. Parse tool calls from response
4. Execute tools
5. Append results to messages
6. Repeat until max_turns or completion

**`src/agent/tool.rs`** — Tool system
```rust
pub struct Tool {
    pub name: String,
    pub description: String,
    pub parameters: serde_json::Value,  // JSON Schema
}

pub struct ToolCall {
    pub id: String,
    pub name: String,
    pub arguments: serde_json::Value,
}

pub struct ToolResult {
    pub call_id: String,
    pub output: String,
    pub error: Option<String>,
}
```

**`src/agent/message.rs`** — Message types
```rust
pub enum Role { System, User, Assistant, Tool }
pub struct Message {
    pub role: Role,
    pub content: String,
    pub tool_calls: Option<Vec<ToolCall>>,
    pub tool_call_id: Option<String>,
}
```

### 2. Parallel Subagents (rust-core/src/agent/subagent.rs)

Use Tokio for concurrent agent execution:
```rust
pub struct SubagentPool {
    agents: Vec<tokio::task::JoinHandle<AgentOutput>>,
    max_concurrent: usize,  // default 4-8
}

impl SubagentPool {
    pub fn new(max: usize) -> Self;
    pub async fn spawn(&mut self, config: AgentConfig) -> usize; // returns agent id
    pub async fn status(&self, id: usize) -> AgentStatus;
    pub async fn cancel(&mut self, id: usize);
    pub async fn await_all(self) -> Vec<AgentOutput>;
}
```

Subagents communicate via tokio::mpsc channels. Each subagent gets its own config, messages, and tool set. The pool manages lifecycle.

### 3. Plan → Review → Approve → Execute Flow

In `src/agent/plan_exec.rs`:
```rust
pub enum PlanStatus { Draft, AwaitingApproval, Approved, Rejected, Executing, Done }

pub struct ExecutionPlan {
    pub steps: Vec<PlanStep>,
    pub status: PlanStatus,
}

pub struct PlanStep {
    pub description: String,
    pub tool_calls: Vec<ToolCall>,
    pub status: PlanStatus,
}
```

- Agent generates plan → PlanStatus::AwaitingApproval
- Plan displayed in TUI plan pane
- User presses 'a' → approve, 'r' → reject, 'e' → edit
- Approved plans execute step by step
- Each step shows result in TUI

### 4. Session Persistence (rust-core/src/session/)

**`src/session/store.rs`** — SQLite-backed session store
```rust
pub struct SessionStore {
    db: sqlx::SqlitePool,
}

impl SessionStore {
    pub async fn open(path: &str) -> Result<Self>;
    pub async fn save_session(&self, id: &str, messages: &[Message]) -> Result<()>;
    pub async fn load_session(&self, id: &str) -> Result<Vec<Message>>;
    pub async fn list_sessions(&self) -> Result<Vec<SessionInfo>>;
}
```

Add to Cargo.toml: `sqlx = { version = "0.8", features = ["runtime-tokio", "sqlite"] }`

### 5. Wire into the TUI

Update main.rs:
- Agent pane: show running subagents with status
- Plan pane: show current execution plan with approve/reject
- F8: spawn new subagent (prompt input in command palette)
- Right side of status bar: "Agents: 3 running, 2 done"
- When subagent completes, flash a notification in status bar

### 6. Add Cargo.toml deps

```toml
sqlx = { version = "0.8", features = ["runtime-tokio", "sqlite"] }
uuid = { version = "1", features = ["v4"] }
```

## CONSTRAINTS
- Extend, don't rewrite. All Phase 0/1/1B code must still work.
- `cargo build` and `cargo test` must succeed
- The agent loop should be async (Tokio)
- Subagents run concurrently, not sequentially
- Session SQLite DB lives at `~/.pi-hybrid/sessions.db`
- Edition 2024, aarch64-apple-darwin

## REFERENCE
- Study `rust-core-temp/src/agent.rs` for agent loop patterns
- Study `rust-core-temp/src/compaction.rs` for context compression
- Tokio docs: https://docs.rs/tokio
- sqlx docs: https://docs.rs/sqlx

## VERIFICATION
1. `cargo build` clean
2. `cargo test` all passing (existing + new agent tests)
3. Agent loop compiles with async Tokio runtime
4. Subagent pool can spawn multiple concurrent agents
5. Plan/approve flow wired to TUI keybindings (a/r/e)
6. Session store can save and load agent conversations
