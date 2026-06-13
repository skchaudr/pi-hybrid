use ratatui::{
    Frame,
    layout::Rect,
    widgets::{Block, Borders, Paragraph},
};

use super::{Pane, active_border_style};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PlanDecision {
    Pending,
    Approved,
    Rejected,
    Editing,
}

#[derive(Debug)]
pub struct PlanPane {
    decision: PlanDecision,
    text: String,
}

impl Default for PlanPane {
    fn default() -> Self {
        Self {
            decision: PlanDecision::Pending,
            text: "Plans, approvals, and execution checkpoints.".to_string(),
        }
    }
}

impl PlanPane {
    pub fn approve(&mut self) {
        self.decision = PlanDecision::Approved;
    }

    pub fn reject(&mut self) {
        self.decision = PlanDecision::Rejected;
    }

    pub fn edit(&mut self) {
        self.decision = PlanDecision::Editing;
    }

    pub fn render(&self, frame: &mut Frame<'_>, area: Rect, active_pane: Pane) {
        let is_active = active_pane == Pane::PlanApproval;
        let block = Block::default()
            .title(if is_active {
                " Plan/Approval * "
            } else {
                " Plan/Approval "
            })
            .borders(Borders::ALL)
            .border_style(active_border_style(is_active));
        let body = format!(
            "{}\n\nStatus: {:?}  |  a approve  r reject  e edit",
            self.text, self.decision
        );
        frame.render_widget(Paragraph::new(body).block(block), area);
    }
}
