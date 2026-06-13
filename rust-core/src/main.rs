mod agent;
mod bridge;
mod keybindings;
mod session;
mod tools;
mod tui;

use std::{
    io::{self, Stdout},
    path::PathBuf,
    time::Duration,
};

use crossterm::{
    event::{self, DisableMouseCapture, EnableMouseCapture, Event, KeyEventKind},
    execute,
    terminal::{EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode},
};
use keybindings::{Action, KeyBindings};
use ratatui::{
    Frame, Terminal,
    backend::CrosstermBackend,
    layout::{Alignment, Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    widgets::{Block, Borders, Clear, Paragraph},
};
use tui::{
    Pane, agent_pane::AgentPane, editor_pane::EditorPane, file_tree::FileTree, plan_pane::PlanPane,
    status_bar,
};

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
    overlay: Option<Overlay>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Overlay {
    CommandPalette,
    Help,
}

impl App {
    fn new(workspace_root: PathBuf) -> Self {
        Self {
            active_pane: Pane::Editor,
            should_quit: false,
            keybindings: KeyBindings::default(),
            bridge_command: std::env::var("PI_BRIDGE_COMMAND")
                .unwrap_or_else(|_| "test-mode".to_string()),
            file_tree: FileTree::load(workspace_root),
            editor: EditorPane::default(),
            agents: AgentPane,
            plan: PlanPane::default(),
            overlay: None,
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
        if contains(layout.files, point) {
            self.active_pane = Pane::Files;
        } else if contains(layout.editor, point) {
            self.active_pane = Pane::Editor;
        } else if contains(layout.agents, point) {
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
                if self.active_pane == Pane::Files {
                    if let Some(path) = self.file_tree.selected_path() {
                        let _ = self.editor.open(&path);
                        self.active_pane = Pane::Editor;
                    }
                }
            }
            Action::ApprovePlan => self.plan.approve(),
            Action::RejectPlan => self.plan.reject(),
            Action::EditPlan => self.plan.edit(),
            Action::FocusPane(pane) => self.active_pane = pane,
            Action::OpenCommandPalette => self.overlay = Some(Overlay::CommandPalette),
            Action::ShowHelp => self.overlay = Some(Overlay::Help),
            Action::DismissOverlay => self.overlay = None,
            Action::MouseFocus { column, row } => self.focus_at(column, row, layout),
        }
    }
}

fn main() -> anyhow::Result<()> {
    let mut terminal = setup_terminal()?;
    let result = run(&mut terminal);
    restore_terminal(&mut terminal)?;
    result
}

fn setup_terminal() -> anyhow::Result<Terminal<CrosstermBackend<Stdout>>> {
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen, EnableMouseCapture)?;
    let backend = CrosstermBackend::new(stdout);
    Ok(Terminal::new(backend)?)
}

fn restore_terminal(terminal: &mut Terminal<CrosstermBackend<Stdout>>) -> anyhow::Result<()> {
    disable_raw_mode()?;
    execute!(
        terminal.backend_mut(),
        LeaveAlternateScreen,
        DisableMouseCapture
    )?;
    terminal.show_cursor()?;
    Ok(())
}

fn run(terminal: &mut Terminal<CrosstermBackend<Stdout>>) -> anyhow::Result<()> {
    let mut app = App::new(std::env::current_dir()?);

    while !app.should_quit {
        let size = terminal.size()?;
        let layout = layout_for(Rect::new(0, 0, size.width, size.height));
        terminal.draw(|frame| draw(frame, &app, layout))?;

        if event::poll(Duration::from_millis(200))? {
            match event::read()? {
                Event::Key(key) if key.kind == KeyEventKind::Press => {
                    if let Some(action) = app.keybindings.handle_key(key, app.active_pane) {
                        app.handle_action(action, &layout);
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

    Ok(())
}

#[derive(Debug, Clone, Copy)]
struct ScreenLayout {
    files: Rect,
    editor: Rect,
    agents: Rect,
    plan: Rect,
}

fn layout_for(area: Rect) -> ScreenLayout {
    let vertical = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(1),
            Constraint::Min(8),
            Constraint::Length(7),
        ])
        .split(area);

    let body = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage(22),
            Constraint::Percentage(56),
            Constraint::Percentage(22),
        ])
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

    status_bar::render(frame, vertical[0], app.active_pane, &app.bridge_command);
    app.file_tree.render(frame, layout.files, app.active_pane);
    app.editor.render(frame, layout.editor, app.active_pane);
    app.agents.render(frame, layout.agents, app.active_pane);
    app.plan.render(frame, layout.plan, app.active_pane);
    render_overlay(frame, app.overlay);
}

fn contains(rect: Rect, (column, row): (u16, u16)) -> bool {
    column >= rect.x
        && column < rect.x.saturating_add(rect.width)
        && row >= rect.y
        && row < rect.y.saturating_add(rect.height)
}

fn render_overlay(frame: &mut Frame<'_>, overlay: Option<Overlay>) {
    let Some(overlay) = overlay else {
        return;
    };

    let area = centered_rect(frame.area(), 62, 50);
    frame.render_widget(Clear, area);

    let (title, body) = match overlay {
        Overlay::CommandPalette => (
            " Command Palette ",
            "Open File...\nSwitch Pane: Files\nSwitch Pane: Editor\nSwitch Pane: Agents\nSwitch Pane: Plan\nToggle: Dark/Light Mode\nToggle: File Tree Visible/Hidden\nToggle: Agent Pane Visible/Hidden\nRun Bridge Test\nQuit\n\nEsc dismisses",
        ),
        Overlay::Help => (
            " Help ",
            "Navigation: tab cycle panes, j/k move, gg top, G bottom\nPanes: F2 editor, F3 agents, F4 plan\nActions: ctrl+p command palette, q quit, esc dismiss\nPlan: a approve, r reject, e edit\nMouse: click a pane to focus it",
        ),
    };

    let block = Block::default()
        .title(title)
        .borders(Borders::ALL)
        .border_style(
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        );
    frame.render_widget(
        Paragraph::new(body)
            .block(block)
            .alignment(Alignment::Left)
            .style(Style::default().fg(Color::White).bg(Color::Black)),
        area,
    );
}

fn centered_rect(area: Rect, percent_x: u16, percent_y: u16) -> Rect {
    let vertical = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Percentage((100 - percent_y) / 2),
            Constraint::Percentage(percent_y),
            Constraint::Percentage((100 - percent_y) / 2),
        ])
        .split(area);
    let horizontal = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage((100 - percent_x) / 2),
            Constraint::Percentage(percent_x),
            Constraint::Percentage((100 - percent_x) / 2),
        ])
        .split(vertical[1]);
    horizontal[1]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tab_cycles_through_all_panes_and_wraps() {
        let mut app = App::new(PathBuf::from("."));

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
        let mut app = App::new(PathBuf::from("."));
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
    fn overlay_actions_update_visible_state() {
        let mut app = App::new(PathBuf::from("."));
        let layout = ScreenLayout {
            files: Rect::new(0, 1, 10, 10),
            editor: Rect::new(10, 1, 20, 10),
            agents: Rect::new(30, 1, 10, 10),
            plan: Rect::new(0, 11, 40, 5),
        };

        app.handle_action(Action::OpenCommandPalette, &layout);
        assert_eq!(app.overlay, Some(Overlay::CommandPalette));

        app.handle_action(Action::ShowHelp, &layout);
        assert_eq!(app.overlay, Some(Overlay::Help));

        app.handle_action(Action::DismissOverlay, &layout);
        assert_eq!(app.overlay, None);

        app.handle_action(Action::Quit, &layout);
        assert!(app.should_quit);
    }
}
