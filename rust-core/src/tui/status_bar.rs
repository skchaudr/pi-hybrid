use ratatui::{
    Frame,
    layout::Rect,
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::Paragraph,
};

use super::Pane;

pub const APP_TITLE: &str = "Pi Hybrid v0.1.0";

pub fn render(frame: &mut Frame<'_>, area: Rect, active_pane: Pane, bridge_command: &str) {
    let status = Line::from(vec![
        Span::styled(
            APP_TITLE,
            Style::default()
                .fg(Color::Black)
                .bg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        ),
        Span::styled(
            format!(
                "  Model: test-mode  |  Memory: local  |  Mode: {}  |  Bridge: {}  |  Ctrl+P commands  F1 help  q quit",
                active_pane.title(),
                bridge_command
            ),
            Style::default().fg(Color::Black).bg(Color::Cyan),
        ),
    ]);
    frame.render_widget(
        Paragraph::new(status).style(Style::default().bg(Color::Cyan)),
        area,
    );
}
