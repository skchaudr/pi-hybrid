use ratatui::{
    Frame,
    layout::Rect,
    style::{Color, Style},
    text::Line,
    widgets::{Block, Borders, Paragraph},
};

use super::{Pane, active_border_style};
use crate::tui::semantic_diff::SemanticDiff;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PlanDecision {
    Pending,
    Approved,
    Rejected,
    Editing,
}

/// A diff view that can be shown in the plan pane.
#[derive(Debug, Clone, Default)]
pub struct DiffView {
    /// The semantic diff to display.
    pub diff: Option<SemanticDiff>,
    /// Whether to show diff view instead of plan view.
    pub show_diff: bool,
}

#[derive(Debug)]
pub struct PlanPane {
    decision: PlanDecision,
    text: String,
    /// Current diff view for file edit preview.
    diff_view: DiffView,
}

impl Default for PlanPane {
    fn default() -> Self {
        Self {
            decision: PlanDecision::Pending,
            text: "Plans, approvals, and execution checkpoints.".to_string(),
            diff_view: DiffView::default(),
        }
    }
}

impl PlanPane {
    pub fn set_lines(&mut self, lines: Vec<String>) {
        self.text = if lines.is_empty() {
            "No active plan.".to_string()
        } else {
            lines.join("\n")
        };
        // Hide diff when plan text changes
        self.diff_view.show_diff = false;
    }

    /// Set a semantic diff to display.
    pub fn set_diff(&mut self, old_text: &str, new_text: &str) {
        let diff = SemanticDiff::compute(old_text, new_text);
        if !diff.is_empty() {
            self.diff_view.diff = Some(diff);
            self.diff_view.show_diff = true;
        }
    }

    /// Toggle between plan view and diff view.
    pub fn toggle_diff(&mut self) {
        if self.diff_view.diff.is_some() {
            self.diff_view.show_diff = !self.diff_view.show_diff;
        }
    }

    /// Check if a diff is available for display.
    pub fn has_diff(&self) -> bool {
        self.diff_view.diff.is_some()
    }

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
        let footer = if is_active {
            if self.diff_view.show_diff {
                "a=approve r=reject e=edit  d=toggle diff"
            } else {
                "a=approve r=reject e=edit"
            }
        } else {
            "a approve  r reject  e edit"
        };

        let mut body: Vec<Line<'_>> = Vec::new();

        // Show semantic diff if available and enabled
        if self.diff_view.show_diff {
            if let Some(ref diff) = self.diff_view.diff {
                for (text, style) in diff.render_lines() {
                    body.push(Line::styled(text, style));
                }
            }
        } else {
            // Show plan text
            body.extend(self.text.lines().map(Line::raw));
            body.push(Line::raw(""));
            body.push(Line::raw(format!("Status: {:?}", self.decision)));
        }

        body.push(Line::styled(
            "— Pi Hybrid v0.1.0 —",
            Style::default().fg(Color::DarkGray),
        ));
        body.push(Line::styled(
            footer,
            Style::default().fg(if is_active {
                Color::Cyan
            } else {
                Color::DarkGray
            }),
        ));

        frame.render_widget(Paragraph::new(body).block(block), area);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ratatui::{Terminal, backend::TestBackend, layout::Rect};

    fn render_pane(pane: &PlanPane, active: bool) -> String {
        let backend = TestBackend::new(80, 20);
        let mut terminal = Terminal::new(backend).expect("terminal");
        let area = Rect::new(0, 0, 80, 20);
        let active_pane = if active {
            Pane::PlanApproval
        } else {
            Pane::Files
        };
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

    #[test]
    fn default_shows_placeholder() {
        let pane = PlanPane::default();
        let output = render_pane(&pane, true);
        assert!(output.contains("Plans"));
    }

    #[test]
    fn active_pane_shows_star() {
        let pane = PlanPane::default();
        let active = render_pane(&pane, true);
        let inactive = render_pane(&pane, false);
        assert!(active.contains("Plan/Approval *"));
        assert!(!inactive.contains("Plan/Approval *"));
    }

    #[test]
    fn shows_plan_lines() {
        let mut pane = PlanPane::default();
        pane.set_lines(vec![
            "Step 1: Setup".to_string(),
            "Step 2: Build".to_string(),
        ]);
        let output = render_pane(&pane, true);
        assert!(output.contains("Step 1"));
        assert!(output.contains("Step 2"));
    }

    #[test]
    fn empty_lines_shows_no_active_plan() {
        let mut pane = PlanPane::default();
        pane.set_lines(vec![]);
        let output = render_pane(&pane, true);
        assert!(output.contains("No active plan"));
    }

    #[test]
    fn set_lines_hides_diff() {
        let mut pane = PlanPane::default();
        pane.set_diff("old", "new");
        assert!(pane.diff_view.show_diff);
        pane.set_lines(vec!["new plan".into()]);
        assert!(!pane.diff_view.show_diff);
    }

    #[test]
    fn set_diff_shows_when_changes_exist() {
        let mut pane = PlanPane::default();
        pane.set_diff("old text\n", "new text\n");
        assert!(pane.has_diff());
        assert!(pane.diff_view.show_diff);
    }

    #[test]
    fn set_diff_no_change_no_show() {
        let mut pane = PlanPane::default();
        pane.set_diff("same\n", "same\n");
        assert!(!pane.has_diff());
        assert!(!pane.diff_view.show_diff);
    }

    #[test]
    fn toggle_diff() {
        let mut pane = PlanPane::default();
        pane.set_diff("old", "new");
        assert!(pane.diff_view.show_diff);
        pane.toggle_diff();
        assert!(!pane.diff_view.show_diff);
        pane.toggle_diff();
        assert!(pane.diff_view.show_diff);
    }

    #[test]
    fn toggle_diff_no_diff_no_op() {
        let mut pane = PlanPane::default();
        pane.toggle_diff(); // no diff set — no-op
        assert!(!pane.diff_view.show_diff);
    }

    #[test]
    fn approve_changes_decision() {
        let mut pane = PlanPane::default();
        pane.approve();
        let output = render_pane(&pane, true);
        assert!(output.contains("Approved"));
    }

    #[test]
    fn reject_changes_decision() {
        let mut pane = PlanPane::default();
        pane.reject();
        let output = render_pane(&pane, true);
        assert!(output.contains("Rejected"));
    }

    #[test]
    fn edit_changes_decision() {
        let mut pane = PlanPane::default();
        pane.edit();
        let output = render_pane(&pane, true);
        assert!(output.contains("Editing"));
    }

    #[test]
    fn shows_footer_hints() {
        let pane = PlanPane::default();
        let output = render_pane(&pane, true);
        assert!(output.contains("approve"));
        assert!(output.contains("reject"));
        assert!(output.contains("edit"));
    }

    #[test]
    fn inactive_shows_hints_too() {
        let pane = PlanPane::default();
        let output = render_pane(&pane, false);
        assert!(output.contains("approve"));
    }

    #[test]
    fn diff_view_renders() {
        let mut pane = PlanPane::default();
        pane.set_diff("fn old() {}\n", "fn new() {}\n");
        assert!(pane.has_diff());
        let output = render_pane(&pane, true);
        // Should show semantic diff
        assert!(output.contains("Semantic Diff"));
    }

    #[test]
    fn version_footer_present() {
        let pane = PlanPane::default();
        let output = render_pane(&pane, true);
        assert!(output.contains("Pi Hybrid"));
    }

    #[test]
    fn plan_decision_enum_values() {
        assert_eq!(PlanDecision::Pending as i32, 0);
        assert_eq!(PlanDecision::Approved as i32, 1);
        assert_eq!(PlanDecision::Rejected as i32, 2);
        assert_eq!(PlanDecision::Editing as i32, 3);
    }

    #[test]
    fn diff_view_default() {
        let dv = DiffView::default();
        assert!(dv.diff.is_none());
        assert!(!dv.show_diff);
    }

    #[test]
    fn diff_view_clone() {
        let mut dv = DiffView::default();
        dv.show_diff = true;
        let dv2 = dv.clone();
        assert!(dv2.show_diff);
    }

    #[test]
    fn default_decision_is_pending() {
        let pane = PlanPane::default();
        let output = render_pane(&pane, true);
        assert!(output.contains("Pending"));
    }

    #[test]
    fn has_diff_false_by_default() {
        let pane = PlanPane::default();
        assert!(!pane.has_diff());
    }

    #[test]
    fn has_diff_true_after_set_diff() {
        let mut pane = PlanPane::default();
        pane.set_diff("old\n", "new\n");
        assert!(pane.has_diff());
    }

    #[test]
    fn diff_view_shows_footer_with_diff_toggle_hint() {
        let mut pane = PlanPane::default();
        pane.set_diff("old\n", "new\n");
        let output = render_pane(&pane, true);
        assert!(output.contains("d=toggle diff"));
    }

    #[test]
    fn inactive_footer_has_spaces_not_equals() {
        let pane = PlanPane::default();
        let output = render_pane(&pane, false);
        assert!(output.contains("a approve"));
        assert!(!output.contains("a=approve"));
    }

    #[test]
    fn inactive_does_not_show_diff_toggle() {
        let mut pane = PlanPane::default();
        pane.set_diff("old\n", "new\n");
        let output = render_pane(&pane, false);
        assert!(!output.contains("d=toggle"));
    }

    #[test]
    fn set_diff_identical_content_is_no_op() {
        let mut pane = PlanPane::default();
        pane.set_diff("same\n", "same\n");
        assert!(!pane.has_diff());
        assert!(!pane.diff_view.show_diff);
    }

    #[test]
    fn toggle_diff_after_set_diff_toggles() {
        let mut pane = PlanPane::default();
        pane.set_diff("old\n", "new\n");
        assert!(pane.diff_view.show_diff);
        let output1 = render_pane(&pane, true);
        // Should show semantic diff content
        pane.toggle_diff();
        assert!(!pane.diff_view.show_diff);
        let output2 = render_pane(&pane, true);
        assert_ne!(output1, output2);
    }

    #[test]
    fn multiple_decisions_override() {
        let mut pane = PlanPane::default();
        pane.approve();
        pane.reject();
        let output = render_pane(&pane, true);
        assert!(output.contains("Rejected"));
        assert!(!output.contains("Approved"));
    }

    #[test]
    fn set_lines_hides_diff_and_sets_plan() {
        let mut pane = PlanPane::default();
        pane.set_diff("old\n", "new\n");
        assert!(pane.diff_view.show_diff);
        pane.set_lines(vec!["New plan item".into()]);
        assert!(!pane.diff_view.show_diff);
        let output = render_pane(&pane, true);
        assert!(output.contains("New plan item"));
    }

    #[test]
    fn plan_decision_partial_eq() {
        assert_eq!(PlanDecision::Pending, PlanDecision::Pending);
        assert_ne!(PlanDecision::Approved, PlanDecision::Rejected);
    }

    #[test]
    fn plan_decision_clone() {
        let d = PlanDecision::Editing;
        let d2 = d.clone();
        assert_eq!(d, d2);
    }

    #[test]
    fn set_lines_empty_shows_no_active_plan() {
        let mut pane = PlanPane::default();
        pane.set_lines(vec![]);
        let output = render_pane(&pane, true);
        assert!(output.contains("No active plan"));
        assert!(output.contains("Pending"));
    }
}
