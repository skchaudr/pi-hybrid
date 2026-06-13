# PHASE 1B MISSION: Command Palette + UI/UX Polish

Phase 1 is complete. You have a working 4-pane TUI with file tree, editor, agent status, plan/approval, vim keybindings, mouse support, and a JSON-RPC bridge. Now make it feel like a serious application.

## YOUR JOB

### 1. Command Palette (Ctrl+P / Cmd+P)
- Press Ctrl+P to open a fuzzy-searchable command palette overlay
- Commands to implement:
  - "Open File..." — fuzzy file search in workspace
  - "Switch Pane: Files/Editor/Agents/Plan" 
  - "Toggle: Dark/Light Mode"
  - "Toggle: File Tree Visible/Hidden"
  - "Toggle: Agent Pane Visible/Hidden"
  - "Run Bridge Test"
  - "Quit"
- Fuzzy match against command names as you type
- j/k navigate, Enter execute, Esc dismiss
- Render as a centered popup overlay with border

### 2. Visual Hierarchy + Context Indicators
- Active pane: cyan bold border with `*` in title
- Inactive panes: dark gray border
- Status bar must clearly show:
  - Current mode (NORMAL/INSERT — prep for future editing)
  - Active pane name
  - Git branch if available
  - "Pi Hybrid v0.1.0" branding
- When file tree has focus, highlight selected file
- When plan pane has focus, show "a=approve r=reject e=edit" hint in a footer bar

### 3. Toggle System
Create `src/tui/toggles.rs`:
```rust
pub struct Toggles {
    pub show_file_tree: bool,    // default true
    pub show_agent_pane: bool,   // default true  
    pub dark_mode: bool,         // default true
}
```
- F5: toggle file tree visibility (left pane collapses, center expands)
- F6: toggle agent pane visibility
- F7: toggle dark/light mode (swap color palette)
- Toggles persist in memory for the session

### 4. Polish Details
- Add a subtle "— Pi Hybrid v0.1.0 —" watermark in empty panes
- Smooth pane resizing (Layout constraints adapt when panes toggle)
- Status bar right side: "Bridge: connected" or "Bridge: disconnected"
- Error messages: if bridge fails, show a red error bar at bottom for 3 seconds

### 5. Help Popup
- F1 or '?' shows a help overlay listing all keybindings
- Organized by category: Navigation, Panes, Actions, Toggles
- Esc dismisses

## CONSTRAINTS
- Extend existing code — do NOT rewrite main.rs from scratch
- All new modules in `rust-core/src/tui/`
- Must compile with `cargo build` and pass `cargo test`
- Keep all Phase 0 and Phase 1 functionality intact
- Edition 2024, aarch64-apple-darwin

## VERIFICATION
1. `cargo build` clean
2. `cargo test` all passing
3. Ctrl+P opens command palette, fuzzy search works, commands execute
4. F5/F6/F7 toggles work, panes resize correctly
5. F1 shows help popup with all keybindings listed
