use ratatui::{
    Frame,
    layout::Rect,
    style::{Color, Style},
    text::Line,
    widgets::{Block, Borders, Paragraph},
};

use super::{Pane, active_border_style};

#[derive(Debug, Clone, Default)]
pub struct AgentRow {
    pub id: String,
    pub goal: String,
    pub status: String,
    pub turns: usize,
}

/// Info about a loaded plugin for display.
#[derive(Debug, Clone)]
pub struct PluginRow {
    pub name: String,
    pub description: String,
    pub backend: String,
    pub enabled: bool,
}

#[derive(Debug, Default)]
pub struct AgentPane {
    agents: Vec<AgentRow>,
    plugins: Vec<PluginRow>,
    last_notification: Option<String>,
    /// Whether to also show the plugins subsection.
    show_plugins: bool,
}

impl AgentPane {
    pub fn set_agents(&mut self, agents: Vec<AgentRow>) {
        self.agents = agents;
    }

    pub fn set_plugins(&mut self, plugins: Vec<PluginRow>) {
        self.plugins = plugins;
    }

    pub fn toggle_plugins(&mut self) {
        self.show_plugins = !self.show_plugins;
    }

    pub fn show_plugins(&self) -> bool {
        self.show_plugins
    }

    pub fn last_notification_text(&self) -> Option<&str> {
        self.last_notification.as_deref()
    }

    pub fn notify(&mut self, message: impl Into<String>) {
        self.last_notification = Some(message.into());
    }

    pub fn running_done_counts(&self) -> (usize, usize) {
        let running = self
            .agents
            .iter()
            .filter(|agent| agent.status == "running")
            .count();
        let done = self
            .agents
            .iter()
            .filter(|agent| matches!(agent.status.as_str(), "completed" | "done"))
            .count();
        (running, done)
    }

    pub fn render(&self, frame: &mut Frame<'_>, area: Rect, active_pane: Pane) {
        let is_active = active_pane == Pane::Agents;
        let block = Block::default()
            .title(if is_active { " Agents * " } else { " Agents " })
            .borders(Borders::ALL)
            .border_style(active_border_style(is_active));

        let mut lines = Vec::new();

        // ── Subagents Section ─────────────────────────────────
        if self.agents.is_empty() {
            lines.push(Line::raw("No agents running"));
        } else {
            for agent in &self.agents {
                lines.push(Line::raw(format!(
                    "{}  {}  turns:{}",
                    agent.status, agent.goal, agent.turns
                )));
                lines.push(Line::styled(
                    format!("id {}", agent.id),
                    Style::default().fg(Color::DarkGray),
                ));
            }
        }

        // ── Plugins Section ───────────────────────────────────
        if self.show_plugins {
            lines.push(Line::raw(""));
            lines.push(Line::styled(
                "── Plugins ──",
                Style::default().fg(Color::Cyan),
            ));
            if self.plugins.is_empty() {
                lines.push(Line::raw("  No plugins loaded"));
            } else {
                for plugin in &self.plugins {
                    let enabled_marker = if plugin.enabled { "●" } else { "○" };
                    lines.push(Line::raw(format!(
                        "  {} {} [{}]",
                        enabled_marker, plugin.name, plugin.backend
                    )));
                    if !plugin.description.is_empty() {
                        lines.push(Line::styled(
                            format!("    {}", plugin.description),
                            Style::default().fg(Color::DarkGray),
                        ));
                    }
                }
            }
        }

        if let Some(message) = &self.last_notification {
            lines.push(Line::raw(""));
            lines.push(Line::styled(
                message.clone(),
                Style::default().fg(Color::Cyan),
            ));
        }
        lines.push(Line::raw(""));
        lines.push(Line::styled(
            "F8 spawn subagent  F9 plugins",
            Style::default().fg(Color::DarkGray),
        ));
        lines.push(Line::styled(
            "— Pi Hybrid v0.1.0 —",
            Style::default().fg(Color::DarkGray),
        ));

        frame.render_widget(Paragraph::new(lines).block(block), area);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tui::Pane;
    use ratatui::{Terminal, backend::TestBackend, layout::Rect};

    fn render_pane(pane: &AgentPane, active: bool) -> String {
        let backend = TestBackend::new(80, 20);
        let mut terminal = Terminal::new(backend).expect("terminal");
        let area = Rect::new(0, 0, 80, 20);
        let active_pane = if active { Pane::Agents } else { Pane::Files };
        terminal
            .draw(|frame| {
                pane.render(frame, area, active_pane);
            })
            .expect("draw");
        let buffer = terminal.backend().buffer();
        let mut text = String::new();
        for y in 0..buffer.area().height {
            for x in 0..buffer.area().width {
                text.push_str(buffer[(x, y)].symbol());
            }
            text.push('\n');
        }
        text
    }

    fn make_agent(id: &str, goal: &str, status: &str, turns: usize) -> AgentRow {
        AgentRow {
            id: id.to_string(),
            goal: goal.to_string(),
            status: status.to_string(),
            turns,
        }
    }

    fn make_plugin(name: &str, desc: &str, backend: &str, enabled: bool) -> PluginRow {
        PluginRow {
            name: name.to_string(),
            description: desc.to_string(),
            backend: backend.to_string(),
            enabled,
        }
    }

    #[test]
    fn empty_pane_shows_no_agents() {
        let pane = AgentPane::default();
        let output = render_pane(&pane, true);
        assert!(output.contains("No agents running"));
    }

    #[test]
    fn shows_agent_rows() {
        let mut pane = AgentPane::default();
        pane.set_agents(vec![
            make_agent("a1", "fix bug", "running", 5),
            make_agent("a2", "add feature", "completed", 12),
        ]);
        let output = render_pane(&pane, true);
        assert!(output.contains("fix bug"));
        assert!(output.contains("add feature"));
        assert!(output.contains("a1"));
        assert!(output.contains("a2"));
        assert!(output.contains("turns:5"));
        assert!(output.contains("turns:12"));
    }

    #[test]
    fn active_pane_shows_active_border() {
        let pane = AgentPane::default();
        let output_active = render_pane(&pane, true);
        let output_inactive = render_pane(&pane, false);
        // Active has "*" marker, inactive doesn't
        assert!(output_active.contains("Agents *"));
        assert!(!output_inactive.contains("Agents *"));
        assert!(output_inactive.contains("Agents "));
    }

    #[test]
    fn plugins_hidden_by_default() {
        let mut pane = AgentPane::default();
        pane.set_plugins(vec![make_plugin("rust-analyzer", "Rust LSP", "rust", true)]);
        let output = render_pane(&pane, true);
        assert!(!output.contains("Plugins"));
    }

    #[test]
    fn plugins_shown_when_toggled() {
        let mut pane = AgentPane::default();
        pane.set_plugins(vec![
            make_plugin("rust-analyzer", "Rust LSP", "rust", true),
            make_plugin("eslint", "JS linter", "node", false),
        ]);
        pane.toggle_plugins();
        let output = render_pane(&pane, true);
        assert!(output.contains("Plugins"));
        assert!(output.contains("rust-analyzer"));
        assert!(output.contains("rust"));
        assert!(output.contains("eslint"));
        assert!(output.contains("●")); // enabled marker
        assert!(output.contains("○")); // disabled marker
    }

    #[test]
    fn plugins_empty_set_shows_no_plugins_message() {
        let mut pane = AgentPane::default();
        pane.toggle_plugins();
        let output = render_pane(&pane, true);
        assert!(output.contains("No plugins loaded"));
    }

    #[test]
    fn toggles_plugins_on_off() {
        let mut pane = AgentPane::default();
        assert!(!pane.show_plugins());
        pane.toggle_plugins();
        assert!(pane.show_plugins());
        pane.toggle_plugins();
        assert!(!pane.show_plugins());
    }

    #[test]
    fn notification_shows() {
        let mut pane = AgentPane::default();
        pane.notify("Hello world");
        assert_eq!(pane.last_notification_text(), Some("Hello world"));
        let output = render_pane(&pane, true);
        assert!(output.contains("Hello world"));
    }

    #[test]
    fn notification_starts_none() {
        let pane = AgentPane::default();
        assert_eq!(pane.last_notification_text(), None);
    }

    #[test]
    fn running_done_counts() {
        let mut pane = AgentPane::default();
        pane.set_agents(vec![
            make_agent("a1", "task1", "running", 1),
            make_agent("a2", "task2", "running", 2),
            make_agent("a3", "task3", "completed", 3),
            make_agent("a4", "task4", "done", 4),
            make_agent("a5", "task5", "pending", 0),
        ]);
        let (running, done) = pane.running_done_counts();
        assert_eq!(running, 2);
        assert_eq!(done, 2);
    }

    #[test]
    fn running_done_counts_empty() {
        let pane = AgentPane::default();
        let (running, done) = pane.running_done_counts();
        assert_eq!(running, 0);
        assert_eq!(done, 0);
    }

    #[test]
    fn set_agents_replaces() {
        let mut pane = AgentPane::default();
        pane.set_agents(vec![make_agent("a1", "old", "running", 0)]);
        pane.set_agents(vec![make_agent("b1", "new", "running", 1)]);
        let output = render_pane(&pane, true);
        assert!(!output.contains("old"));
        assert!(output.contains("new"));
    }

    #[test]
    fn footer_keys_always_present() {
        let pane = AgentPane::default();
        let output = render_pane(&pane, true);
        assert!(output.contains("F8"));
        assert!(output.contains("F9"));
    }

    #[test]
    fn version_footer_shows() {
        let pane = AgentPane::default();
        let output = render_pane(&pane, true);
        assert!(output.contains("Pi Hybrid"));
    }

    #[test]
    fn agent_row_clone() {
        let row = make_agent("id1", "goal1", "running", 3);
        let row2 = row.clone();
        assert_eq!(row.id, row2.id);
        assert_eq!(row.goal, row2.goal);
    }

    #[test]
    fn plugin_row_clone() {
        let row = make_plugin("p1", "desc", "rust", true);
        let row2 = row.clone();
        assert_eq!(row.name, row2.name);
        assert_eq!(row.enabled, row2.enabled);
    }

    #[test]
    fn plugin_with_description_renders_description() {
        let mut pane = AgentPane::default();
        pane.set_plugins(vec![make_plugin(
            "lsp",
            "Language Server Protocol",
            "rust",
            true,
        )]);
        pane.toggle_plugins();
        let output = render_pane(&pane, true);
        assert!(output.contains("Language Server Protocol"));
    }

    #[test]
    fn plugin_without_description_no_extra_line() {
        let mut pane = AgentPane::default();
        pane.set_plugins(vec![make_plugin("minimal", "", "node", true)]);
        pane.toggle_plugins();
        let output = render_pane(&pane, true);
        assert!(output.contains("minimal"));
        // No description means no extra styled line
    }

    #[test]
    fn set_plugins_replaces_old_plugins() {
        let mut pane = AgentPane::default();
        pane.set_plugins(vec![make_plugin("old", "", "rust", true)]);
        pane.set_plugins(vec![make_plugin("new", "", "node", false)]);
        pane.toggle_plugins();
        let output = render_pane(&pane, true);
        assert!(!output.contains("old"));
        assert!(output.contains("new"));
    }

    #[test]
    fn notification_renders_with_plugins_visible() {
        let mut pane = AgentPane::default();
        pane.set_plugins(vec![make_plugin("p1", "", "rust", true)]);
        pane.toggle_plugins();
        pane.notify("Build complete");
        let output = render_pane(&pane, true);
        assert!(output.contains("Build complete"));
        assert!(output.contains("p1"));
    }

    #[test]
    fn running_done_counts_only_running() {
        let mut pane = AgentPane::default();
        pane.set_agents(vec![
            make_agent("a1", "task1", "running", 1),
            make_agent("a2", "task2", "running", 2),
            make_agent("a3", "task3", "running", 3),
        ]);
        let (running, done) = pane.running_done_counts();
        assert_eq!(running, 3);
        assert_eq!(done, 0);
    }

    #[test]
    fn running_done_counts_only_done() {
        let mut pane = AgentPane::default();
        pane.set_agents(vec![
            make_agent("a1", "task1", "completed", 1),
            make_agent("a2", "task2", "done", 2),
        ]);
        let (running, done) = pane.running_done_counts();
        assert_eq!(running, 0);
        assert_eq!(done, 2);
    }

    #[test]
    fn show_plugins_defaults_false() {
        let pane = AgentPane::default();
        assert!(!pane.show_plugins());
    }

    #[test]
    fn rendered_agent_has_status_and_turns() {
        let mut pane = AgentPane::default();
        pane.set_agents(vec![make_agent("a1", "fix bug", "running", 42)]);
        let output = render_pane(&pane, true);
        assert!(output.contains("running"));
        assert!(output.contains("turns:42"));
    }

    #[test]
    fn rendered_agent_has_id_line() {
        let mut pane = AgentPane::default();
        pane.set_agents(vec![make_agent("uuid-123", "task", "running", 1)]);
        let output = render_pane(&pane, true);
        assert!(output.contains("uuid-123"));
    }

    #[test]
    fn inactive_pane_no_star_in_title() {
        let mut pane = AgentPane::default();
        pane.set_agents(vec![make_agent("a1", "task", "running", 1)]);
        let output = render_pane(&pane, false);
        assert!(!output.contains("Agents *"));
    }

    #[test]
    fn agent_status_pending_not_counted_as_running_or_done() {
        let mut pane = AgentPane::default();
        pane.set_agents(vec![make_agent("a1", "task", "pending", 0)]);
        let (running, done) = pane.running_done_counts();
        assert_eq!(running, 0);
        assert_eq!(done, 0);
    }

    #[test]
    fn last_notification_text_returns_none_when_empty() {
        let mut pane = AgentPane::default();
        assert_eq!(pane.last_notification_text(), None);
        pane.notify("test");
        assert_eq!(pane.last_notification_text(), Some("test"));
    }
}
