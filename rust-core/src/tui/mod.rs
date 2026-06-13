pub mod agent_pane;
pub mod editor_pane;
pub mod file_tree;
pub mod plan_pane;
pub mod status_bar;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Pane {
    Files,
    Editor,
    Agents,
    PlanApproval,
}

impl Pane {
    pub const ALL: [Pane; 4] = [Pane::Files, Pane::Editor, Pane::Agents, Pane::PlanApproval];

    pub fn title(self) -> &'static str {
        match self {
            Pane::Files => "Files",
            Pane::Editor => "Editor",
            Pane::Agents => "Agents",
            Pane::PlanApproval => "Plan/Approval",
        }
    }

    pub fn next(self) -> Self {
        let index = Self::ALL
            .iter()
            .position(|pane| *pane == self)
            .expect("active pane is part of Pane::ALL");
        Self::ALL[(index + 1) % Self::ALL.len()]
    }
}

pub fn active_border_style(is_active: bool) -> ratatui::style::Style {
    use ratatui::style::{Color, Modifier, Style};

    if is_active {
        Style::default()
            .fg(Color::Cyan)
            .add_modifier(Modifier::BOLD)
    } else {
        Style::default().fg(Color::DarkGray)
    }
}
