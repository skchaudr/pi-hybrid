# PHASE 0 MISSION: Pi Rust-Core Hybrid Workspace Setup

You are in `/Users/sab-mini/my-pi-hybrid/`. A Cargo workspace already exists with three crates: `rust-core`, `ts-bridge`, `py-extensions`. Two reference repos are cloning in the background (`rust-core-temp/` from pi_agent_rust, `grain-reference/` from grain-agent-harness). A `docs/` directory should be created.

## YOUR JOB: Complete Phase 0 Setup

### 1. WAIT for reference clones to complete
Check that `rust-core-temp/` and `grain-reference/` directories have content. If not, clone them:
- `git clone https://github.com/Dicklesworthstone/pi_agent_rust.git rust-core-temp`
- `git clone https://github.com/earendil-works/grain-agent-harness.git grain-reference`

### 2. Create rust-core source structure
Inside `rust-core/src/`, create these directories and a basic main.rs:
```
rust-core/src/
├── tui/           (empty mod.rs for now)
├── agent/         (empty mod.rs for now)
├── bridge/        (empty mod.rs for now)
├── tools/         (empty mod.rs for now)
├── session/       (empty mod.rs for now)
└── main.rs        (Ratatui "Hello Pi Hybrid" screen)
```

### 3. Implement main.rs
Create a working Ratatui app that:
- Shows a multi-pane layout (even if mostly empty)
- Displays "Pi Hybrid v0.1.0" in the top status bar
- Has at least: a left pane (labeled "Files"), center pane ("Editor"), right pane ("Agents"), bottom pane ("Plan/Approval")
- Responds to 'q' to quit, 'tab' to cycle panes
- Uses crossterm backend with Ratatui
- Compiles and runs cleanly with `cargo run`

### 4. Verify everything builds
- `cargo build` from workspace root succeeds
- `cargo run -p rust-core` displays the TUI
- ts-bridge and py-extensions compile as empty libs

### 5. Create docs/README.md
Brief summary of workspace structure and build instructions.

## CONSTRAINTS
- Edition 2024 for all crates
- Target: aarch64-apple-darwin (Apple Silicon)
- Keep it simple — Phase 0 is FOUNDATION, not the full agent
- All code must compile and the TUI must render

## REFERENCE
- Study `rust-core-temp/` for patterns from pi_agent_rust
- Study `grain-reference/` for agent harness patterns
- Ratatui docs: https://docs.rs/ratatui/latest/ratatui/

When complete, report: what was built, what compiles, and a screenshot description of the TUI.
