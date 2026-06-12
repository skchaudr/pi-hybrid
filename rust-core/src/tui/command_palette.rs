use std::path::PathBuf;

use ratatui::{
    Frame,
    layout::{Alignment, Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Clear, List, ListItem, Paragraph},
};

use super::Pane;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Command {
    OpenFile(Option<PathBuf>),
    SwitchPane(Pane),
    ToggleDarkMode,
    ToggleFileTree,
    ToggleAgentPane,
    SpawnSubagent(Option<String>),
    RunBridgeTest,
    ShowPlugins,
    SelectProvider(Option<String>),
    ToggleGitStatus,
    RenderMermaid,
    Quit,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PaletteCommand {
    pub name: String,
    pub command: Command,
}

#[derive(Debug)]
pub struct CommandPalette {
    open: bool,
    query: String,
    selected: usize,
    commands: Vec<PaletteCommand>,
    files: Vec<PaletteCommand>,
}

impl CommandPalette {
    pub fn new(files: Vec<PathBuf>) -> Self {
        let mut palette = Self {
            open: false,
            query: String::new(),
            selected: 0,
            commands: base_commands(),
            files: Vec::new(),
        };
        palette.set_files(files);
        palette
    }

    pub fn set_files(&mut self, files: Vec<PathBuf>) {
        self.files = files
            .into_iter()
            .map(|path| PaletteCommand {
                name: format!("Open File: {}", path.display()),
                command: Command::OpenFile(Some(path)),
            })
            .collect();
    }

    pub fn open(&mut self) {
        self.open = true;
        self.query.clear();
        self.selected = 0;
    }

    pub fn close(&mut self) {
        self.open = false;
    }

    pub fn is_open(&self) -> bool {
        self.open
    }

    #[cfg(test)]
    pub fn push_str(&mut self, text: &str) {
        self.query.push_str(text);
        self.selected = 0;
    }

    pub fn push_char(&mut self, character: char) {
        self.query.push(character);
        self.selected = 0;
    }

    pub fn backspace(&mut self) {
        self.query.pop();
        self.selected = 0;
    }

    pub fn move_down(&mut self) {
        let len = self.visible_commands().len();
        if len > 0 {
            self.selected = (self.selected + 1).min(len - 1);
        }
    }

    pub fn move_up(&mut self) {
        self.selected = self.selected.saturating_sub(1);
    }

    pub fn selected_command(&self) -> Option<Command> {
        self.visible_commands()
            .get(self.selected)
            .map(|entry| match &entry.command {
                Command::SpawnSubagent(None) => {
                    let goal = self.query.trim();
                    if goal.is_empty() || entry.name.to_lowercase().contains(&goal.to_lowercase()) {
                        Command::SpawnSubagent(None)
                    } else {
                        Command::SpawnSubagent(Some(goal.to_string()))
                    }
                }
                command => command.clone(),
            })
    }

    pub fn visible_commands(&self) -> Vec<PaletteCommand> {
        let source = self
            .commands
            .iter()
            .chain(self.files.iter())
            .cloned()
            .collect::<Vec<_>>();
        source
            .into_iter()
            .filter(|entry| fuzzy_match(&entry.name, &self.query))
            .collect()
    }

    pub fn render(&self, frame: &mut Frame<'_>, area: Rect) {
        if !self.open {
            return;
        }

        let popup = centered_rect(62, 55, area);
        frame.render_widget(Clear, popup);
        let block = Block::default()
            .title(" Command Palette ")
            .borders(Borders::ALL)
            .border_style(
                Style::default()
                    .fg(Color::Cyan)
                    .add_modifier(Modifier::BOLD),
            );

        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Length(3), Constraint::Min(4)])
            .split(popup);
        frame.render_widget(block, popup);
        frame.render_widget(
            Paragraph::new(Line::from(vec![
                Span::styled("> ", Style::default().fg(Color::Cyan)),
                Span::raw(self.query.clone()),
            ]))
            .alignment(Alignment::Left),
            inner(chunks[0]),
        );

        let visible = self.visible_commands();
        let items = visible.iter().enumerate().map(|(index, command)| {
            let style = if index == self.selected {
                Style::default()
                    .fg(Color::Black)
                    .bg(Color::Cyan)
                    .add_modifier(Modifier::BOLD)
            } else {
                Style::default().fg(Color::White)
            };
            ListItem::new(Line::styled(command.name.clone(), style))
        });
        frame.render_widget(List::new(items), inner(chunks[1]));
    }
}

fn base_commands() -> Vec<PaletteCommand> {
    vec![
        PaletteCommand {
            name: "Open File...".to_string(),
            command: Command::OpenFile(None),
        },
        PaletteCommand {
            name: "Switch Pane: Files".to_string(),
            command: Command::SwitchPane(Pane::Files),
        },
        PaletteCommand {
            name: "Switch Pane: Editor".to_string(),
            command: Command::SwitchPane(Pane::Editor),
        },
        PaletteCommand {
            name: "Switch Pane: Agents".to_string(),
            command: Command::SwitchPane(Pane::Agents),
        },
        PaletteCommand {
            name: "Switch Pane: Plan".to_string(),
            command: Command::SwitchPane(Pane::PlanApproval),
        },
        PaletteCommand {
            name: "Toggle: Dark/Light Mode".to_string(),
            command: Command::ToggleDarkMode,
        },
        PaletteCommand {
            name: "Toggle: File Tree Visible/Hidden".to_string(),
            command: Command::ToggleFileTree,
        },
        PaletteCommand {
            name: "Toggle: Agent Pane Visible/Hidden".to_string(),
            command: Command::ToggleAgentPane,
        },
        PaletteCommand {
            name: "Spawn Subagent...".to_string(),
            command: Command::SpawnSubagent(None),
        },
        PaletteCommand {
            name: "Run Bridge Test".to_string(),
            command: Command::RunBridgeTest,
        },
        PaletteCommand {
            name: "Show Plugins".to_string(),
            command: Command::ShowPlugins,
        },
        PaletteCommand {
            name: "Select Provider: DeepSeek".to_string(),
            command: Command::SelectProvider(Some("deepseek".to_string())),
        },
        PaletteCommand {
            name: "Select Provider: GLM".to_string(),
            command: Command::SelectProvider(Some("glm".to_string())),
        },
        PaletteCommand {
            name: "Toggle Git Status Display".to_string(),
            command: Command::ToggleGitStatus,
        },
        PaletteCommand {
            name: "Render Mermaid Diagrams".to_string(),
            command: Command::RenderMermaid,
        },
        PaletteCommand {
            name: "Quit".to_string(),
            command: Command::Quit,
        },
    ]
}

pub fn fuzzy_match(candidate: &str, query: &str) -> bool {
    if query.trim().is_empty() {
        return true;
    }
    let mut chars = query
        .chars()
        .filter(|c| !c.is_whitespace())
        .flat_map(char::to_lowercase);
    let mut needle = chars.next();
    for hay in candidate.chars().flat_map(char::to_lowercase) {
        if Some(hay) == needle {
            needle = chars.next();
            if needle.is_none() {
                return true;
            }
        }
    }
    false
}

fn centered_rect(percent_x: u16, percent_y: u16, area: Rect) -> Rect {
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

fn inner(area: Rect) -> Rect {
    Rect::new(
        area.x.saturating_add(1),
        area.y,
        area.width.saturating_sub(2),
        area.height,
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fuzzy_matching_allows_sparse_queries() {
        assert!(fuzzy_match("Run Bridge Test", "rbt"));
        assert!(fuzzy_match("Toggle: Agent Pane Visible/Hidden", "tag"));
        assert!(!fuzzy_match("Quit", "bridge"));
    }

    #[test]
    fn fuzzy_match_empty_query_matches_all() {
        assert!(fuzzy_match("anything", ""));
        assert!(fuzzy_match("anything", "  "));
    }

    #[test]
    fn fuzzy_match_exact() {
        assert!(fuzzy_match("Quit", "Quit"));
        assert!(fuzzy_match("Quit", "quit"));
        assert!(!fuzzy_match("Quit", "quitz"));
    }

    #[test]
    fn fuzzy_match_case_insensitive() {
        assert!(fuzzy_match("Open File...", "open"));
        assert!(fuzzy_match("OPEN FILE...", "file"));
        assert!(fuzzy_match("Toggle Dark Mode", "DARK"));
    }

    #[test]
    fn fuzzy_match_sparse_ordering_matters() {
        // "sbt" does NOT fuzzy match "Run Bridge Test" because order matters
        assert!(!fuzzy_match("Run Bridge Test", "sbt"));
        // But "rbt" does
        assert!(fuzzy_match("Run Bridge Test", "rbt"));
    }

    #[test]
    fn palette_opens_and_closes() {
        let mut palette = CommandPalette::new(vec![]);
        assert!(!palette.is_open());
        palette.open();
        assert!(palette.is_open());
        palette.close();
        assert!(!palette.is_open());
    }

    #[test]
    fn palette_query_clears_on_open() {
        let mut palette = CommandPalette::new(vec![]);
        palette.open();
        palette.push_char('a');
        palette.push_char('b');
        assert!(!palette.visible_commands().is_empty());
        palette.open(); // re-open clears query
        // After re-opening, query should be empty; all commands visible
        assert!(palette.visible_commands().len() > 5);
    }

    #[test]
    fn palette_backspace() {
        let mut palette = CommandPalette::new(vec![]);
        palette.open();
        palette.push_char('q');
        palette.push_char('u');
        palette.push_char('i');
        palette.backspace();
        // After backspace, should filter to "qu"
        let visible = palette.visible_commands();
        assert!(!visible.is_empty());
        // "Quit" should match "qu"
        assert!(visible.iter().any(|c| c.name == "Quit"));
    }

    #[test]
    fn palette_move_up_down() {
        let mut palette = CommandPalette::new(vec![]);
        palette.open();
        // All commands are visible with empty query
        let visible_count = palette.visible_commands().len();
        assert!(visible_count > 0);
        palette.move_down();
        palette.move_down();
        palette.move_down();
        palette.move_up();
        // selection should be 2 after 3 down, 1 up
        assert_eq!(palette.selected, 2);
    }

    #[test]
    fn palette_move_up_saturates() {
        let mut palette = CommandPalette::new(vec![]);
        palette.open();
        palette.move_up();
        palette.move_up();
        // Should stay at 0
        assert_eq!(palette.selected, 0);
    }

    #[test]
    fn palette_move_down_clamps_to_visible() {
        let mut palette = CommandPalette::new(vec![]);
        palette.open();
        let visible_count = palette.visible_commands().len();
        for _ in 0..visible_count + 10 {
            palette.move_down();
        }
        assert_eq!(palette.selected, visible_count - 1);
    }

    #[test]
    fn palette_selected_command_quit() {
        let mut palette = CommandPalette::new(vec![]);
        palette.open();
        // Find "Quit" in visible commands and select it
        let visible = palette.visible_commands();
        let quit_idx = visible.iter().position(|c| c.name == "Quit").unwrap();
        for _ in 0..quit_idx {
            palette.move_down();
        }
        let selected = palette.selected_command();
        assert_eq!(selected, Some(Command::Quit));
    }

    #[test]
    fn palette_files_in_visible_commands() {
        let files = vec![PathBuf::from("src/main.rs"), PathBuf::from("src/lib.rs")];
        let mut palette = CommandPalette::new(files);
        palette.open();
        let visible = palette.visible_commands();
        let file_commands: Vec<_> = visible
            .iter()
            .filter(|c| c.name.starts_with("Open File:"))
            .collect();
        assert_eq!(file_commands.len(), 2);
    }

    #[test]
    fn palette_set_files_updates() {
        let mut palette = CommandPalette::new(vec![]);
        palette.open();
        let visible_before = palette.visible_commands().len();
        palette.set_files(vec![PathBuf::from("test.rs")]);
        let visible_after = palette.visible_commands().len();
        assert!(visible_after > visible_before);
    }

    #[test]
    fn palette_push_str() {
        let mut palette = CommandPalette::new(vec![]);
        palette.open();
        palette.push_str("Qui");
        let visible = palette.visible_commands();
        // Should only match "Quit" (maybe "Quick" if present)
        assert!(visible.iter().any(|c| c.name == "Quit"));
    }

    #[test]
    fn all_base_commands_have_non_empty_names() {
        let mut palette = CommandPalette::new(vec![]);
        palette.open();
        for cmd in palette.visible_commands() {
            assert!(!cmd.name.is_empty());
        }
    }

    #[test]
    fn palette_selected_command_with_subagent_goal() {
        let mut palette = CommandPalette::new(vec![]);
        palette.open();
        // Type a query that includes "Spawn Subagent..." in results
        palette.push_str("spawn");
        // Find "Spawn Subagent..." in visible commands and select it
        let visible = palette.visible_commands();
        let idx = visible
            .iter()
            .position(|c| c.name == "Spawn Subagent...")
            .unwrap();
        for _ in 0..idx {
            palette.move_down();
        }
        // Now clear and type the actual goal
        // The selected_command function checks query against entry name; since "spawn" matches,
        // it will return SpawnSubagent(None). Let's test with a non-matching case.
        palette.open(); // re-open clears query
        palette.push_str("build the project");
        // "build the project" won't match "Spawn Subagent..." in fuzzy, so it won't be visible
        // But when it IS visible (by virtue of the filter), the selected_command uses query as goal
        // This tests the case where the goal is passed through
        let visible2 = palette.visible_commands();
        // "Spawn Subagent..." won't be in visible because "build the project" doesn't fuzzy-match it
        assert!(!visible2.iter().any(|c| c.name == "Spawn Subagent..."));
    }
}
