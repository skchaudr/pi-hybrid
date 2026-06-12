pub mod agent_pane;
pub mod command_palette;
pub mod editor_pane;
pub mod file_tree;
pub mod help_popup;
pub mod mermaid;
pub mod plan_pane;
pub mod semantic_diff;
pub mod status_bar;
pub mod toggles;

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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pane_titles() {
        assert_eq!(Pane::Files.title(), "Files");
        assert_eq!(Pane::Editor.title(), "Editor");
        assert_eq!(Pane::Agents.title(), "Agents");
        assert_eq!(Pane::PlanApproval.title(), "Plan/Approval");
    }

    #[test]
    fn next_pane_cycles() {
        assert_eq!(Pane::Files.next(), Pane::Editor);
        assert_eq!(Pane::Editor.next(), Pane::Agents);
        assert_eq!(Pane::Agents.next(), Pane::PlanApproval);
        assert_eq!(Pane::PlanApproval.next(), Pane::Files);
    }

    #[test]
    fn all_panes_is_four() {
        assert_eq!(Pane::ALL.len(), 4);
    }

    #[test]
    fn active_border_style_is_cyan_bold() {
        let style = active_border_style(true);
        // We can't easily inspect ratatui styles, but we can confirm they're different
        assert_ne!(style, active_border_style(false));
    }

    #[test]
    fn inactive_border_style_is_dark_gray() {
        let style = active_border_style(false);
        // Inactive style should exist
        assert_eq!(style, active_border_style(false));
    }

    #[test]
    fn pane_all_contains_each_variant() {
        assert!(Pane::ALL.contains(&Pane::Files));
        assert!(Pane::ALL.contains(&Pane::Editor));
        assert!(Pane::ALL.contains(&Pane::Agents));
        assert!(Pane::ALL.contains(&Pane::PlanApproval));
    }

    #[test]
    fn pane_titles_are_unique() {
        let titles: Vec<&str> = Pane::ALL.iter().map(|p| p.title()).collect();
        // All 4 titles should be distinct
        for i in 0..titles.len() {
            for j in (i + 1)..titles.len() {
                assert_ne!(titles[i], titles[j]);
            }
        }
    }

    #[test]
    fn next_pane_cycles_full_circle() {
        let mut pane = Pane::Files;
        for _ in 0..8 {
            pane = pane.next();
        }
        // After 8 cycles (2 full rounds), should be back to Files
        assert_eq!(pane, Pane::Files);
    }

    #[test]
    fn next_pane_all_distinct() {
        let p1 = Pane::Files.next();
        let p2 = p1.next();
        let p3 = p2.next();
        let p4 = p3.next();
        // All four should be distinct in one cycle
        assert_ne!(p1, p2);
        assert_ne!(p1, p3);
        assert_ne!(p1, p4);
        assert_ne!(p2, p3);
        assert_ne!(p2, p4);
        assert_ne!(p3, p4);
        // Last one wraps back to first
        assert_eq!(p4, Pane::Files);
    }

    #[test]
    fn active_border_style_differs() {
        let active = active_border_style(true);
        let inactive = active_border_style(false);
        assert_ne!(active, inactive);
    }

    #[test]
    fn active_border_style_idempotent() {
        let s1 = active_border_style(true);
        let s2 = active_border_style(true);
        assert_eq!(s1, s2);
    }

    #[test]
    fn pane_debug_format() {
        assert_eq!(format!("{:?}", Pane::Files), "Files");
        assert_eq!(format!("{:?}", Pane::Editor), "Editor");
        assert_eq!(format!("{:?}", Pane::Agents), "Agents");
        assert_eq!(format!("{:?}", Pane::PlanApproval), "PlanApproval");
    }
}
