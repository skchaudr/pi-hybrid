# Analysis of Headless Mode

The `headless` mode in `rust-core/src/headless.rs` currently implements a simulated JSON-RPC server.
When a `run` request is received, `handle_run` creates a `Session` record in memory, sets a mock status message, and returns a simulated response.
It does not invoke the actual agent loop via `agent::spawn_agent`, which handles the TUI interaction in `main.rs`.

## Decision

The answer is **(b)**: `handle_run` should be wired to the real `agent::spawn_agent` so that headless mode proves the same agent/session behavior as the TUI.
The headless mode is intended for CLI and CI integration. Running purely simulated sessions without actually executing agent behavior completely defeats the purpose of CI testing and CLI automation, as it does not test the real interaction with `agent::spawn_agent` or touch the `bridge_client`/`providers` in the way `main.rs` does for the TUI mode.

Therefore, we need to adapt `headless.rs` to start a Tokio runtime (or accept one) and wire `handle_run` to instantiate an agent using `agent::spawn_agent()`, handle the input/output channels, and translate the output messages back to JSON-RPC notifications.

Below is the proposal diff to implement this wiring.


## Proposed Diff

```diff
--- rust-core/src/headless.rs
+++ rust-core/src/headless.rs
@@ -10,6 +10,13 @@
 use std::sync::{Arc, Mutex};

 use serde::{Deserialize, Serialize};
 use serde_json::Value;
+use tokio::runtime::Runtime;
+use tokio::sync::mpsc;
+use crate::agent::{spawn_agent, AgentConfig};
+use crate::agent::message::{AgentInput, AgentOutput};
+use crate::shutdown::CancelToken;
 use tracing::{debug, error, info, warn};

@@ -107,6 +114,13 @@
             running: false,
             next_id: AtomicU64::new(1),
         }
     }

+    // Note: The actual Server state would need to be updated to maintain references to
+    // agent channels (e.g. input_tx, output_rx) per session, likely stored in the Session struct.
+    // We also need a tokio runtime to spawn the agent tasks, or make the whole server async.
+
     /// Run the JSON-RPC server, reading from stdin and writing to stdout.
     /// Blocks until EOF or shutdown.
-    pub fn run(&mut self) -> anyhow::Result<()> {
+    pub fn run(&mut self) -> anyhow::Result<()> {
@@ -243,15 +257,41 @@
         self.sessions.insert(session_id.clone(), session);
         self.active_session_id = Some(session_id.clone());

-        // Simulate agent response (in production, this would call the agent loop)
-        let result = serde_json::json!({
-            "session_id": session_id,
-            "status": "started",
-            "prompt": prompt,
-            "provider": provider,
-            "model": model,
-            "message": format!("Agent processing: '{}'", prompt)
-        });
-
-        let response = RpcResponse {
+        // ==========================================
+        // PROPOSED WIRING TO ACTUAL AGENT
+        // ==========================================
+        //
+        // 1. Create an AgentConfig (with defaults or from the request)
+        // let config = AgentConfig {
+        //     model: model.unwrap_or("default").to_string(),
+        //     max_turns,
+        //     ..Default::default()
+        // };
+        //
+        // 2. Spawn the agent inside a tokio runtime
+        // let cancel_token = CancelToken::new();
+        // let (input_tx, mut output_rx, handle) = tokio::runtime::Handle::current().block_on(
+        //     spawn_agent(config, cancel_token)
+        // ).expect("Failed to spawn agent");
+        //
+        // 3. Send the prompt to the agent
+        // let _ = input_tx.send(AgentInput::Message(prompt.to_string()));
+        //
+        // 4. Update the Session struct to hold the `input_tx` so it can be resumed/canceled
+        // self.sessions.get_mut(&session_id).unwrap().input_tx = Some(input_tx);
+        //
+        // 5. Spawn a background tokio task to read from `output_rx` and send JSON-RPC
+        //    notifications to stdout (or a designated channel).
+        //
+        // let result = serde_json::json!({
+        //     "session_id": session_id,
+        //     "status": "started",
+        //     "prompt": prompt,
+        //     "provider": provider,
+        //     "model": model,
+        //     "message": format!("Real Agent starting for: '{}'", prompt)
+        // });
+        // ==========================================
+
+        // For now (simulated):
+        let result = serde_json::json!({
```
