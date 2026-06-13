#![allow(
    dead_code,
    unused_imports,
    unused_mut,
    unused_variables,
    unused_assignments
)]

mod agent;
mod bridge;
mod config;
mod headless;
mod keybindings;
mod session;
mod shutdown;
mod tools;
mod tui;

use std::{
    io::{self, IsTerminal, Stdout},
    path::PathBuf,
    time::{Duration, Instant},
};

use tracing::{debug, error, info, trace, warn};
use tracing_subscriber::{EnvFilter, fmt};

use crossterm::{
    event::{self, DisableMouseCapture, EnableMouseCapture, Event, KeyEventKind},
    execute,
    terminal::{EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode},
};
use keybindings::{Action, KeyBindings};
use ratatui::{
    Frame, Terminal,
    backend::CrosstermBackend,
    layout::{Constraint, Direction, Layout, Rect},
};
use tokio::{runtime::Runtime, sync::mpsc};
use tui::{
    Pane,
    agent_pane::AgentPane,
    command_palette::{Command, CommandPalette},
    editor_pane::EditorPane,
    file_tree::FileTree,
    help_popup::HelpPopup,
    plan_pane::PlanPane,
    status_bar,
    toggles::Toggles,
};

use crate::agent::git::GitManager;
use crate::agent::plugins::{PluginInfo as PluginMetaInfo, PluginRegistry};
use crate::agent::providers::ProviderRegistry;
use crate::agent::{AgentInput, AgentOutput, SubagentInfo};
use crate::shutdown::CancelToken;
use crate::tui::agent_pane::{AgentRow, PluginRow};
use crate::tui::mermaid::{MermaidWidget, extract_mermaid_blocks};
use crate::tui::semantic_diff::SemanticDiff;

#[derive(Debug)]
struct App {
    active_pane: Pane,
    should_quit: bool,
    keybindings: KeyBindings,
    bridge_command: String,
    file_tree: FileTree,
    editor: EditorPane,
    agents: AgentPane,
    plan: PlanPane,
    toggles: Toggles,
    command_palette: CommandPalette,
    help_popup: HelpPopup,
    plugin_registry: PluginRegistry,
    provider_registry: ProviderRegistry,
    git_manager: GitManager,
    mermaid_widget: Option<MermaidWidget>,
    show_mermaid_render: bool,
    runtime: Option<Runtime>,
    agent_tx: Option<mpsc::UnboundedSender<AgentInput>>,
    agent_rx: Option<mpsc::UnboundedReceiver<AgentOutput>>,
    agent_handle: Option<tokio::task::JoinHandle<()>>,
    agent_counts: (usize, usize),
    status_notification: Option<(String, Instant)>,
    bridge_connected: bool,
    error_message: Option<(String, Instant)>,
    git_branch: Option<String>,
    cancel_token: CancelToken,
}

impl App {
    fn new(
        workspace_root: PathBuf,
        pi_config: config::PiConfig,
        cancel_token: CancelToken,
    ) -> Self {
        debug!(workspace = %workspace_root.display(), "Initializing App");
        // Derive agent config from the validated PiConfig.
        let agent_config = pi_config.to_agent_config();
        let bridge_command = agent_config.bridge_command.clone();
        let runtime = Runtime::new().ok();
        let (agent_tx, agent_rx, agent_handle) = if let Some(runtime) = &runtime {
            runtime
                .block_on(agent::spawn_agent(agent_config, cancel_token.clone()))
                .map(|(tx, rx, handle)| (Some(tx), Some(rx), Some(handle)))
                .unwrap_or((None, None, None))
        } else {
            (None, None, None)
        };

        // Initialize plugin registry with sample plugins
        let mut plugin_registry = PluginRegistry::new();
        {
            use crate::agent::plugins::{NativePlugin, PyPlugin, TsPlugin};
            use std::sync::Arc;

            let native_hello = Arc::new(NativePlugin::new(
                "hello_world",
                "A native Rust plugin that greets",
                |args| {
                    let name = args.get("name").and_then(|v| v.as_str()).unwrap_or("World");
                    Ok(serde_json::json!({"greeting": format!("Hello, {name}!")}))
                },
            ));
            plugin_registry.register(native_hello);

            let ts_analyzer = Arc::new(TsPlugin::new(
                "ts_code_analyzer",
                "TypeScript-based code analysis plugin",
                |args| {
                    Ok(serde_json::json!({
                        "analysis": "TS bridge would analyze code here",
                        "input": args
                    }))
                },
            ));
            plugin_registry.register(ts_analyzer);

            let py_formatter = Arc::new(PyPlugin::new(
                "py_formatter",
                "Python-based code formatter plugin",
                |args| {
                    Ok(serde_json::json!({
                        "formatted": "Python extension would format code here",
                        "input": args
                    }))
                },
            ));
            plugin_registry.register(py_formatter);
        }

        // Initialize Git manager
        let git_manager = GitManager::open(workspace_root.clone());
        let git_available = git_manager.is_available();
        if git_available {
            info!("Git repository detected");
        }

        info!(plugin_count = plugin_registry.len(), "App initialized");

        Self {
            active_pane: Pane::Editor,
            should_quit: false,
            keybindings: KeyBindings::default(),
            bridge_command,
            file_tree: FileTree::load(workspace_root.clone()),
            editor: EditorPane::default(),
            agents: AgentPane::default(),
            plan: PlanPane::default(),
            toggles: Toggles::default(),
            command_palette: CommandPalette::new(FileTree::load(workspace_root).relative_paths()),
            help_popup: HelpPopup::default(),
            plugin_registry,
            provider_registry: ProviderRegistry::with_builtins(),
            git_manager,
            mermaid_widget: None,
            show_mermaid_render: false,
            runtime,
            agent_tx,
            agent_rx,
            agent_handle,
            agent_counts: (0, 0),
            status_notification: None,
            bridge_connected: true,
            error_message: None,
            git_branch: current_git_branch(),
            cancel_token,
        }
    }

    fn cycle_pane(&mut self) {
        self.active_pane = self.active_pane.next();
    }

    fn quit(&mut self) {
        self.should_quit = true;
    }

    fn focus_at(&mut self, column: u16, row: u16, layout: &ScreenLayout) {
        let point = (column, row);
        if self.toggles.show_file_tree && contains(layout.files, point) {
            self.active_pane = Pane::Files;
        } else if contains(layout.editor, point) {
            self.active_pane = Pane::Editor;
        } else if self.toggles.show_agent_pane && contains(layout.agents, point) {
            self.active_pane = Pane::Agents;
        } else if contains(layout.plan, point) {
            self.active_pane = Pane::PlanApproval;
        }
    }

    fn handle_action(&mut self, action: Action, layout: &ScreenLayout) {
        match action {
            Action::Quit => self.quit(),
            Action::CyclePane => self.cycle_pane(),
            Action::CommandMode => {}
            Action::OpenCommandPalette => self.command_palette.open(),
            Action::OpenHelp => self.help_popup.open(),
            Action::CloseOverlay => {
                self.command_palette.close();
                self.help_popup.close();
            }
            Action::ToggleFileTree => {
                self.toggles.toggle_file_tree();
                if self.active_pane == Pane::Files && !self.toggles.show_file_tree {
                    self.active_pane = Pane::Editor;
                }
            }
            Action::ToggleAgentPane => {
                self.toggles.toggle_agent_pane();
                if self.active_pane == Pane::Agents && !self.toggles.show_agent_pane {
                    self.active_pane = Pane::Editor;
                }
            }
            Action::ToggleDarkMode => self.toggles.toggle_dark_mode(),
            Action::PaletteConfirm => {
                if let Some(command) = self.command_palette.selected_command() {
                    self.command_palette.close();
                    self.execute_command(command);
                }
            }
            Action::PaletteBackspace => self.command_palette.backspace(),
            Action::PaletteInput(character) => self.command_palette.push_char(character),
            Action::MoveDown => match self.active_pane {
                Pane::Files => self.file_tree.move_down(),
                Pane::Editor => self.editor.scroll_down(),
                _ => {}
            },
            Action::MoveUp => match self.active_pane {
                Pane::Files => self.file_tree.move_up(),
                Pane::Editor => self.editor.scroll_up(),
                _ => {}
            },
            Action::GoTop => match self.active_pane {
                Pane::Files => self.file_tree.go_top(),
                Pane::Editor => self.editor.go_top(),
                _ => {}
            },
            Action::GoBottom => match self.active_pane {
                Pane::Files => self.file_tree.go_bottom(),
                Pane::Editor => self.editor.go_bottom(),
                _ => {}
            },
            Action::PageDown => match self.active_pane {
                Pane::Files => self.file_tree.page_down(),
                Pane::Editor => self.editor.page_down(),
                _ => {}
            },
            Action::PageUp => match self.active_pane {
                Pane::Files => self.file_tree.page_up(),
                Pane::Editor => self.editor.page_up(),
                _ => {}
            },
            Action::Select => {
                if self.active_pane == Pane::Files
                    && let Some(path) = self.file_tree.selected_path()
                {
                    let _ = self.editor.open(&path);
                    self.active_pane = Pane::Editor;
                }
            }
            Action::RejectPlan => {
                debug!("User rejected plan");
                self.plan.reject();
                self.send_agent_input(AgentInput::RejectPlan);
            }
            Action::EditPlan => {
                debug!("User editing plan");
                self.plan.edit();
                self.send_agent_input(AgentInput::EditPlan);
            }
            Action::SpawnSubagent => self.spawn_subagent("TUI requested subagent".to_string()),
            Action::ApprovePlan => {
                info!("User approved plan");
                self.plan.approve();
                self.send_agent_input(AgentInput::ApprovePlan);
                // Auto-commit on plan approval if git is available
                if self.git_manager.is_available()
                    && let Ok(result) = self.git_manager.auto_commit(None)
                    && result.committed
                {
                    info!(
                        commit_oid = result.commit_oid.as_deref().unwrap_or("unknown"),
                        "Auto-committed on plan approval"
                    );
                    self.show_notification(format!(
                        "Auto-committed: {} ({})",
                        result.commit_oid.as_deref().unwrap_or("unknown"),
                        result.message
                    ));
                }
            }
            Action::FocusPane(pane) => self.active_pane = pane,
            Action::MouseFocus { column, row } => self.focus_at(column, row, layout),
            Action::TogglePlugins => self.toggle_plugins(),
            Action::ToggleGitStatus => {
                self.git_manager.toggle_status_display();
                self.show_notification(format!(
                    "Git status: {}",
                    if self.git_manager.status_visible() {
                        "shown"
                    } else {
                        "hidden"
                    }
                ));
            }
            Action::RenderMermaid => {
                self.show_mermaid_render = !self.show_mermaid_render;
                if self.show_mermaid_render {
                    // Try to extract mermaid from agent notifications
                    let test_text = self.agents.last_notification_text().unwrap_or("");
                    let diagrams = extract_mermaid_blocks(test_text);
                    if !diagrams.is_empty() {
                        self.mermaid_widget = Some(MermaidWidget::new(diagrams));
                    }
                } else {
                    self.mermaid_widget = None;
                }
            }
        }
    }

    fn handle_palette_action(&mut self, action: Action) {
        match action {
            Action::MoveDown => self.command_palette.move_down(),
            Action::MoveUp => self.command_palette.move_up(),
            _ => self.handle_action(action, &ScreenLayout::default()),
        }
    }

    fn execute_command(&mut self, command: Command) {
        match command {
            Command::OpenFile(path) => {
                if let Some(path) = path.or_else(|| self.file_tree.selected_path()) {
                    if let Err(err) = self.editor.open(&path) {
                        self.show_error(format!("Open failed: {err}"));
                    } else {
                        self.active_pane = Pane::Editor;
                    }
                }
            }
            Command::SwitchPane(pane) => self.active_pane = pane,
            Command::ToggleDarkMode => self.toggles.toggle_dark_mode(),
            Command::ToggleFileTree => {
                self.handle_action(Action::ToggleFileTree, &ScreenLayout::default())
            }
            Command::ToggleAgentPane => {
                self.handle_action(Action::ToggleAgentPane, &ScreenLayout::default())
            }
            Command::SpawnSubagent(goal) => {
                self.spawn_subagent(goal.unwrap_or_else(|| "Command palette subagent".to_string()))
            }
            Command::RunBridgeTest => self.run_bridge_test(),
            Command::ShowPlugins => self.toggle_plugins(),
            Command::SelectProvider(name) => {
                if let Some(name) = name {
                    if self.provider_registry.set_active(&name).is_ok() {
                        self.show_notification(format!("Provider switched to: {name}"));
                        // Update status bar with new provider
                    } else {
                        self.show_error(format!("Provider not found: {name}"));
                    }
                }
            }
            Command::ToggleGitStatus => {
                self.git_manager.toggle_status_display();
                self.show_notification(format!(
                    "Git status: {}",
                    if self.git_manager.status_visible() {
                        "shown"
                    } else {
                        "hidden"
                    }
                ));
            }
            Command::RenderMermaid => {
                self.show_mermaid_render = !self.show_mermaid_render;
                // Extract diagrams from agent pane output
                // In a full implementation, we'd get this from agent output
                if self.show_mermaid_render {
                    let test_text = self.agents.last_notification_text().unwrap_or("");
                    let diagrams = extract_mermaid_blocks(test_text);
                    if !diagrams.is_empty() {
                        self.mermaid_widget = Some(MermaidWidget::new(diagrams));
                        let count = self
                            .mermaid_widget
                            .as_ref()
                            .map(|w| w.diagram_count())
                            .unwrap_or(0);
                        self.show_notification(format!("{} Mermaid diagram(s) detected", count));
                    } else {
                        self.show_error("No Mermaid diagrams found in agent output".to_string());
                        self.show_mermaid_render = false;
                    }
                } else {
                    self.mermaid_widget = None;
                }
            }
            Command::Quit => self.quit(),
        }
    }

    fn run_bridge_test(&mut self) {
        if self.bridge_command.trim().is_empty() {
            self.bridge_connected = false;
            self.show_error("Bridge test failed: bridge command is empty".to_string());
        } else {
            self.bridge_connected = true;
            self.error_message = None;
        }
    }

    fn show_error(&mut self, message: String) {
        warn!(%message, "Displaying error to user");
        self.error_message = Some((message, Instant::now()));
    }

    fn show_notification(&mut self, message: String) {
        debug!(%message, "Showing notification");
        self.status_notification = Some((message.clone(), Instant::now()));
        self.agents.notify(message);
    }

    fn active_error(&self) -> Option<&str> {
        self.error_message
            .as_ref()
            .filter(|(_, created)| created.elapsed() < Duration::from_secs(3))
            .map(|(message, _)| message.as_str())
    }

    fn active_notification(&self) -> Option<&str> {
        self.status_notification
            .as_ref()
            .filter(|(_, created)| created.elapsed() < Duration::from_secs(4))
            .map(|(message, _)| message.as_str())
    }

    fn send_agent_input(&mut self, input: AgentInput) {
        trace!(?input, "Sending agent input");
        if let Some(tx) = &self.agent_tx {
            if tx.send(input).is_err() {
                warn!("Agent input channel closed");
                self.show_error("Agent loop is not available".to_string());
            }
        } else {
            warn!("Agent input channel not initialized");
            self.show_error("Agent loop is not available".to_string());
        }
    }

    fn spawn_subagent(&mut self, goal: String) {
        debug!(%goal, "Spawning subagent from TUI");
        self.send_agent_input(AgentInput::SpawnSubagent { goal });
        self.send_agent_input(AgentInput::QuerySubagents);
    }

    fn poll_agent_outputs(&mut self) {
        let mut outputs = Vec::new();
        if let Some(rx) = &mut self.agent_rx {
            while let Ok(output) = rx.try_recv() {
                outputs.push(output);
            }
        }

        for output in outputs {
            self.apply_agent_output(output);
        }
    }

    fn apply_agent_output(&mut self, output: AgentOutput) {
        match output {
            AgentOutput::PlanReady { steps } => {
                self.plan.set_lines(
                    steps
                        .into_iter()
                        .map(|step| step.display_line())
                        .collect::<Vec<_>>(),
                );
            }
            AgentOutput::SubagentStatus { agents } => self.update_agent_rows(agents),
            AgentOutput::SubagentResult { id, goal, result } => {
                self.show_notification(format!("Subagent complete: {goal}"));
                self.update_agent_rows(vec![SubagentInfo {
                    id,
                    goal,
                    status: "completed".to_string(),
                    turns: 3,
                }]);
                self.show_error(result);
            }
            AgentOutput::Status { message } => self.show_notification(message),
            AgentOutput::Error(message) => self.show_error(message),
            AgentOutput::PlanApproved => self.show_notification("Plan approved".to_string()),
            AgentOutput::PlanRejected => self.show_notification("Plan rejected".to_string()),
            AgentOutput::StepExecuted { index, status } => {
                self.show_notification(format!("Step {}: {:?}", index + 1, status));
            }
            AgentOutput::DiffPreview { step_index, diff } => {
                self.plan
                    .set_lines(vec![format!("Diff for step {}", step_index + 1), diff]);
            }
            AgentOutput::ResponseChunk(message) => {
                // Check if the message contains Mermaid diagrams
                if tui::mermaid::has_mermaid_diagrams(&message) {
                    self.show_notification(
                        "Mermaid diagram detected — use palette to render".to_string(),
                    );
                }
                self.show_notification(message);
            }
            AgentOutput::Thinking => self.show_notification("Agent thinking".to_string()),
            AgentOutput::Idle => {}
        }
    }

    fn update_agent_rows(&mut self, agents: Vec<SubagentInfo>) {
        let rows = agents
            .into_iter()
            .map(|agent| AgentRow {
                id: agent.id,
                goal: agent.goal,
                status: agent.status,
                turns: agent.turns,
            })
            .collect::<Vec<_>>();
        self.agents.set_agents(rows);
        self.agent_counts = self.agents.running_done_counts();
    }

    fn toggle_plugins(&mut self) {
        self.agents.toggle_plugins();
        if self.agents.show_plugins() {
            // Refresh plugin rows from registry
            let plugins = self.plugin_registry.list();
            let rows: Vec<PluginRow> = plugins
                .into_iter()
                .map(|p| PluginRow {
                    name: p.name,
                    description: p.description,
                    backend: p.backend.to_string(),
                    enabled: p.enabled,
                })
                .collect();
            self.agents.set_plugins(rows);
            self.show_notification(format!(
                "Loaded {} plugins (Native/TS/Python)",
                self.plugin_registry.len()
            ));
        } else {
            self.show_notification("Plugins hidden".to_string());
        }
    }
}

fn main() -> anyhow::Result<()> {
    // ── CLI argument parsing ──────────────────────────────────────────
    let args: Vec<String> = std::env::args().collect();
    let headless = args.iter().any(|arg| arg == "--headless");

    // Parse --config <PATH>
    let config_path = parse_config_path(&args);

    // Parse --init-config flag: write a default, commented TOML and exit.
    if args.iter().any(|arg| arg == "--init-config") {
        let path = config_path.unwrap_or_else(config::default_config_path);
        let toml_content = config::generate_default_toml();
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        std::fs::write(&path, &toml_content)?;
        println!("Default configuration written to: {}", path.display());
        return Ok(());
    }

    // Load configuration (with env overrides, validation).
    let pi_config = config::PiConfig::load(config_path.as_deref())?;

    // Parse --validate-config flag: print report and exit.
    if args.iter().any(|arg| arg == "--validate-config") {
        println!("Configuration is valid.");
        println!("  Provider:     {}", pi_config.provider);
        println!(
            "  Providers:    {}",
            pi_config
                .providers
                .keys()
                .cloned()
                .collect::<Vec<_>>()
                .join(", ")
        );
        println!("  DB Path:      {}", pi_config.session.db_path);
        println!(
            "  Bridge:       {}",
            if pi_config.bridge.ts_bridge_path.is_empty() {
                "(none)"
            } else {
                &pi_config.bridge.ts_bridge_path
            }
        );
        println!("  Log Level:    {}", pi_config.logging.level);
        println!("  Max Turns:    {}", pi_config.agent.max_turns);
        println!("  Default Model: {}", pi_config.agent.default_model);

        // Print warnings.
        let warnings = pi_config.warnings();
        if !warnings.is_empty() {
            println!("\nWarnings:");
            for w in &warnings {
                println!("  - {w}");
            }
        }
        return Ok(());
    }

    // Determine log level: CLI flag takes highest priority, then config, then default.
    let log_level = parse_log_level(&args).unwrap_or(&pi_config.logging.level);

    // Determine if stdout is a TTY for format selection
    let is_tty = std::io::stdout().is_terminal();

    // Initialize tracing subscriber
    init_tracing(log_level, is_tty);

    tracing::info!(log_level, headless, is_tty, "Pi Hybrid starting");

    // Print any non-fatal warnings on startup.
    for w in pi_config.warnings() {
        tracing::warn!("Config warning: {w}");
    }

    if headless {
        tracing::info!("Running in headless mode");
        return headless::run_headless();
    }

    // ── Create shutdown handler ──────────────────────────────────────
    let shutdown_handler = shutdown::ShutdownHandler::new();
    let cancel_token = shutdown_handler.token.clone();

    // Create tokio runtime for async signal handling.
    // The enter guard lets tokio::spawn work inside a sync context.
    let rt = Runtime::new().expect("Failed to create tokio runtime");
    let _guard = rt.enter();

    // Spawn signal watcher tasks (SIGINT, SIGTERM, SIGHUP, stdin close)
    let _signal_handles = shutdown_handler.watch_signals();

    let mut terminal = setup_terminal()?;
    let result = run(&mut terminal, pi_config, cancel_token, &shutdown_handler);
    restore_terminal(&mut terminal)?;

    tracing::info!("Pi Hybrid exiting");
    result
}

/// Parse `--config <PATH>` from CLI arguments.
fn parse_config_path(args: &[String]) -> Option<std::path::PathBuf> {
    let mut iter = args.iter();
    while let Some(arg) = iter.next() {
        if arg == "--config" {
            return iter.next().map(std::path::PathBuf::from);
        }
    }
    None
}

fn setup_terminal() -> anyhow::Result<Terminal<CrosstermBackend<Stdout>>> {
    debug!("Setting up terminal — raw mode, alternate screen");
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen, EnableMouseCapture)?;
    let backend = CrosstermBackend::new(stdout);
    Ok(Terminal::new(backend)?)
}

fn restore_terminal(terminal: &mut Terminal<CrosstermBackend<Stdout>>) -> anyhow::Result<()> {
    debug!("Restoring terminal");
    disable_raw_mode()?;
    execute!(
        terminal.backend_mut(),
        LeaveAlternateScreen,
        DisableMouseCapture
    )?;
    terminal.show_cursor()?;
    Ok(())
}

fn run(
    terminal: &mut Terminal<CrosstermBackend<Stdout>>,
    pi_config: config::PiConfig,
    cancel_token: shutdown::CancelToken,
    shutdown_handler: &shutdown::ShutdownHandler,
) -> anyhow::Result<()> {
    info!("Starting TUI event loop");
    let mut app = App::new(std::env::current_dir()?, pi_config, cancel_token);

    while !app.should_quit {
        // Check for cancellation (Ctrl+C, etc.)
        if app.cancel_token.is_cancelled() && !app.should_quit {
            tracing::info!("Shutdown requested — stopping event loop");
            app.show_notification("Shutting down...".to_string());
            app.send_agent_input(AgentInput::Shutdown);
            app.should_quit = true;
            // Brief pause to let shutdown sequence execute
            continue;
        }

        app.poll_agent_outputs();
        let size = terminal.size()?;
        let layout = layout_for(Rect::new(0, 0, size.width, size.height), &app.toggles);
        terminal.draw(|frame| draw(frame, &app, layout))?;

        if event::poll(Duration::from_millis(200))? {
            match event::read()? {
                Event::Key(key) if key.kind == KeyEventKind::Press => {
                    let action = if app.command_palette.is_open() {
                        app.keybindings.handle_palette_key(key)
                    } else {
                        app.keybindings.handle_key(key, app.active_pane)
                    };
                    if let Some(action) = action {
                        if app.command_palette.is_open() {
                            app.handle_palette_action(action);
                        } else {
                            app.handle_action(action, &layout);
                        }
                    }
                }
                Event::Mouse(mouse) => {
                    if let Some(action) = app.keybindings.handle_mouse(mouse) {
                        app.handle_action(action, &layout);
                    }
                }
                _ => {}
            }
        }
    }

    info!("TUI event loop ended");
    Ok(())
}

/// Parse `--log-level <LEVEL>` from CLI arguments.
fn parse_log_level(args: &[String]) -> Option<&str> {
    let mut iter = args.iter();
    while let Some(arg) = iter.next() {
        if arg == "--log-level" {
            return iter.next().map(|s| s.as_str());
        }
    }
    None
}

/// Initialize the tracing subscriber with pretty (TTY) or JSON (pipe) output.
fn init_tracing(level: &str, is_tty: bool) {
    let env_filter = EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new(level));

    if is_tty {
        fmt::fmt()
            .with_env_filter(env_filter)
            .with_target(true)
            .with_thread_ids(false)
            .with_thread_names(false)
            .with_file(false)
            .with_line_number(false)
            .with_span_events(fmt::format::FmtSpan::NEW | fmt::format::FmtSpan::CLOSE)
            .init();
    } else {
        fmt::fmt()
            .with_env_filter(env_filter)
            .json()
            .with_target(true)
            .with_span_events(fmt::format::FmtSpan::NEW | fmt::format::FmtSpan::CLOSE)
            .init();
    }
}

#[derive(Debug, Clone, Copy, Default)]
struct ScreenLayout {
    files: Rect,
    editor: Rect,
    agents: Rect,
    plan: Rect,
}

fn layout_for(area: Rect, toggles: &Toggles) -> ScreenLayout {
    let vertical = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(1),
            Constraint::Min(8),
            Constraint::Length(7),
        ])
        .split(area);

    let left = if toggles.show_file_tree {
        Constraint::Percentage(22)
    } else {
        Constraint::Length(0)
    };
    let right = if toggles.show_agent_pane {
        Constraint::Percentage(22)
    } else {
        Constraint::Length(0)
    };
    let body = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([left, Constraint::Min(20), right])
        .split(vertical[1]);

    ScreenLayout {
        files: body[0],
        editor: body[1],
        agents: body[2],
        plan: vertical[2],
    }
}

fn draw(frame: &mut Frame<'_>, app: &App, layout: ScreenLayout) {
    let vertical = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(1),
            Constraint::Min(8),
            Constraint::Length(7),
        ])
        .split(frame.area());

    status_bar::render(
        frame,
        vertical[0],
        app.active_pane,
        app.git_manager.current_branch().as_deref(),
        app.bridge_connected,
        app.toggles.dark_mode,
        Some(app.agent_counts),
        app.active_notification(),
        app.git_manager
            .status_visible()
            .then(|| app.git_manager.get_status()),
        app.provider_registry.active_provider_name(),
    );
    if app.toggles.show_file_tree {
        app.file_tree.render(frame, layout.files, app.active_pane);
    }
    app.editor.render(frame, layout.editor, app.active_pane);
    if app.toggles.show_agent_pane {
        app.agents.render(frame, layout.agents, app.active_pane);
    }
    app.plan.render(frame, layout.plan, app.active_pane);
    if let Some(message) = app.active_error() {
        status_bar::render_error(
            frame,
            Rect::new(
                0,
                frame.area().height.saturating_sub(1),
                frame.area().width,
                1,
            ),
            message,
        );
    }
    app.help_popup.render(frame, frame.area());
    app.command_palette.render(frame, frame.area());
    if let Some(ref mermaid_widget) = app.mermaid_widget
        && app.show_mermaid_render
    {
        // Render Mermaid diagram at 60% center of screen
        use ratatui::layout::{Constraint, Direction, Layout};
        let v = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Percentage(20),
                Constraint::Percentage(60),
                Constraint::Percentage(20),
            ])
            .split(frame.area());
        let h = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([
                Constraint::Percentage(20),
                Constraint::Percentage(60),
                Constraint::Percentage(20),
            ])
            .split(v[1]);
        mermaid_widget.render(frame, h[1]);
    }
}

fn contains(rect: Rect, (column, row): (u16, u16)) -> bool {
    column >= rect.x
        && column < rect.x.saturating_add(rect.width)
        && row >= rect.y
        && row < rect.y.saturating_add(rect.height)
}

fn current_git_branch() -> Option<String> {
    crate::agent::git::find_repo_root(&std::env::current_dir().ok()?)
        .and_then(|root| GitManager::open(root).current_branch())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tab_cycles_through_all_panes_and_wraps() {
        let mut app = App::new(
            PathBuf::from("."),
            config::PiConfig::default(),
            CancelToken::new(),
        );

        assert_eq!(app.active_pane, Pane::Editor);
        app.cycle_pane();
        assert_eq!(app.active_pane, Pane::Agents);
        app.cycle_pane();
        assert_eq!(app.active_pane, Pane::PlanApproval);
        app.cycle_pane();
        assert_eq!(app.active_pane, Pane::Files);
        app.cycle_pane();
        assert_eq!(app.active_pane, Pane::Editor);
    }

    #[test]
    fn required_pane_titles_are_present() {
        let titles: Vec<&str> = Pane::ALL.iter().map(|pane| pane.title()).collect();

        assert_eq!(titles, vec!["Files", "Editor", "Agents", "Plan/Approval"]);
        assert_eq!(status_bar::APP_TITLE, "Pi Hybrid v0.1.0");
    }

    #[test]
    fn mouse_coordinates_focus_matching_pane() {
        let mut app = App::new(
            PathBuf::from("."),
            config::PiConfig::default(),
            CancelToken::new(),
        );
        let layout = ScreenLayout {
            files: Rect::new(0, 1, 10, 10),
            editor: Rect::new(10, 1, 20, 10),
            agents: Rect::new(30, 1, 10, 10),
            plan: Rect::new(0, 11, 40, 5),
        };

        app.focus_at(35, 5, &layout);

        assert_eq!(app.active_pane, Pane::Agents);
    }

    #[test]
    fn layout_expands_editor_when_side_panes_are_hidden() {
        let area = Rect::new(0, 0, 100, 40);
        let default_layout = layout_for(area, &tui::toggles::Toggles::default());
        let collapsed_layout = layout_for(
            area,
            &tui::toggles::Toggles {
                show_file_tree: false,
                show_agent_pane: false,
                dark_mode: true,
            },
        );

        assert!(collapsed_layout.files.width < default_layout.files.width);
        assert!(collapsed_layout.agents.width < default_layout.agents.width);
        assert!(collapsed_layout.editor.width > default_layout.editor.width);
    }

    #[test]
    fn app_toggles_panes_and_palette_filters_commands() {
        let mut app = App::new(
            PathBuf::from("."),
            config::PiConfig::default(),
            CancelToken::new(),
        );

        assert!(app.toggles.show_file_tree);
        app.handle_action(
            Action::ToggleFileTree,
            &layout_for(Rect::new(0, 0, 100, 40), &app.toggles),
        );
        assert!(!app.toggles.show_file_tree);

        app.handle_action(
            Action::OpenCommandPalette,
            &layout_for(Rect::new(0, 0, 100, 40), &app.toggles),
        );
        assert!(app.command_palette.is_open());
        app.command_palette.push_str("bridge");
        assert_eq!(
            app.command_palette.visible_commands()[0].name,
            "Run Bridge Test"
        );
    }
}
