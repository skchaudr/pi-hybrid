use ratatui::{
    Frame,
    layout::Rect,
    widgets::{Block, Borders, Paragraph},
};

use super::{Pane, active_border_style};

#[derive(Debug, Default)]
pub struct AgentPane;

impl AgentPane {
    pub fn render(&self, frame: &mut Frame<'_>, area: Rect, active_pane: Pane) {
        let is_active = active_pane == Pane::Agents;
        let block = Block::default()
            .title(if is_active { " Agents * " } else { " Agents " })
            .borders(Borders::ALL)
            .border_style(active_border_style(is_active));
        frame.render_widget(Paragraph::new("No agents running").block(block), area);
    }
}
