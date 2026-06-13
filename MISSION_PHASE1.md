# PHASE 1 MISSION: Bridge + Enhanced TUI

You are in `/Users/sab-mini/my-pi-hybrid/`. Phase 0 is COMPLETE:
- Workspace builds clean (`cargo build` succeeds)
- `rust-core/src/main.rs` has a basic 4-pane Ratatui TUI (Files/Editor/Agents/PlanApproval)
- Tab cycles panes, 'q' quits
- Reference: `rust-core-temp/` (pi_agent_rust — 20K files, rich TUI patterns)

## YOUR JOB: Build the bridge and enhance the TUI

### 1. Study the reference
Read these files for patterns:
- `rust-core-temp/src/keybindings.rs` — vim-style keybinding patterns
- `rust-core-temp/src/theme.rs` — theming approach
- `rust-core-temp/src/session.rs` — session management
- `rust-core-temp/src/interactive/keybindings.rs` — interactive mode keybinds

### 2. Enhance the TUI (rust-core/src/tui/)
Replace the placeholder `src/tui/mod.rs` with real modules:

**`src/tui/file_tree.rs`** — Left pane
- Use `ignore` or `walkdir` crate to list files in the workspace
- Show file tree with indentation
- j/k to navigate, Enter to select

**`src/tui/editor_pane.rs`** — Center pane  
- Display file contents (read from disk)
- j/k scroll, basic line numbers
- Placeholder for inline diff viewing (Phase 4)

**`src/tui/agent_pane.rs`** — Right pane
- Shows subagent status (placeholder: "No agents running")
- Will be wired to Tokio tasks in Phase 2

**`src/tui/plan_pane.rs`** — Bottom pane
- Shows plan/approval text
- Hotkeys: 'a' to approve, 'r' to reject, 'e' to edit plan

**`src/tui/status_bar.rs`** — Top bar
- Model name, memory usage, mode
- Move existing status bar code here

### 3. Add keybindings (rust-core/src/keybindings.rs)
- Global: 'q' quit, 'Tab' cycle panes, ':' command mode placeholder
- Vim-style: j/k for navigation, gg/G for top/bottom, Ctrl+d/u for page
- Mouse: click to select panes (enable mouse support in crossterm)
- Approve/Reject: 'a' approve, 'r' reject (in plan pane)
- F1-F4: jump to Files/Editor/Agents/Plan panes

### 4. Build the JSON-RPC bridge (rust-core/src/bridge/)
Create a working stdio JSON-RPC bridge to talk to a TS Pi process:

**`src/bridge/json_rpc.rs`**
- Spawn a child process (configurable command/path)
- Send JSON-RPC requests over stdin
- Read JSON-RPC responses from stdout
- Handle errors and timeouts gracefully
- Use tokio::process for async

**`src/bridge/mod.rs`** — public API
- `Bridge::new(command: &str) -> Result<Self>`
- `Bridge::call(method: &str, params: Value) -> Result<Value>`
- `Bridge::list_skills() -> Result<Vec<String>>`

### 5. Add required Cargo.toml deps
Add to `rust-core/Cargo.toml`:
```toml
walkdir = "2"
ignore = "0.4"
```

### 6. Wire it together in main.rs
- Update main.rs to use the new tui modules
- Show file tree in left pane (listing actual files)
- Wire keybindings
- Initialize bridge (configurable, but default to a test mode)
- The TUI must build and render

### 7. Create a bridge test
In `src/bridge/`, add `#[cfg(test)]` tests:
- Test JSON-RPC message serialization/deserialization  
- Test bridge startup/shutdown (mock echo process)
- Test timeout handling

## CONSTRAINTS
- Edition 2024, aarch64-apple-darwin
- Keep the bridge protocol simple — JSON-RPC 2.0 over stdio
- The bridge is for LATER integration with actual TS Pi code — for now, test with a mock
- Do NOT remove or break the existing pane system from Phase 0
- All new modules must have `pub mod` exports
- `cargo build` from workspace root must succeed
- `cargo test` must pass all tests (existing + new)

## REFERENCE
- Study `rust-core-temp/src/keybindings.rs` for vim-style keybinding patterns
- Study `rust-core-temp/src/theme.rs` for color/styling approach
- Ratatui docs: https://docs.rs/ratatui/latest/ratatui/
- JSON-RPC 2.0 spec: https://www.jsonrpc.org/specification

## VERIFICATION
When done, report:
1. `cargo build` status
2. `cargo test` count and results
3. What the TUI looks like now (describe each pane's content)
4. Whether the bridge module exports the public API described above
