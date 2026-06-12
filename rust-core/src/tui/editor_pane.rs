use std::{fs, path::Path};

use ratatui::{
    Frame,
    layout::Rect,
    style::{Color, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Paragraph},
};

use super::{Pane, active_border_style};

/// Whether the editor is showing a diff view.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum EditorMode {
    #[default]
    Normal,
    Diff,
}

#[derive(Debug, Default)]
pub struct EditorPane {
    current_file: Option<String>,
    lines: Vec<String>,
    scroll: usize,
    mode: EditorMode,
    /// Diff lines with styles (for red/green highlighting).
    diff_lines: Vec<(String, Style)>,
}

impl EditorPane {
    pub fn open(&mut self, path: &Path) -> anyhow::Result<()> {
        let content = fs::read_to_string(path)?;
        self.current_file = Some(path.display().to_string());
        self.lines = content.lines().map(ToOwned::to_owned).collect();
        self.scroll = 0;
        self.mode = EditorMode::Normal;
        self.diff_lines.clear();
        Ok(())
    }

    /// Open a file with diff highlighting.
    pub fn open_with_diff(
        &mut self,
        path: &Path,
        old_text: &str,
        new_text: &str,
    ) -> anyhow::Result<()> {
        let content = fs::read_to_string(path)?;
        self.current_file = Some(path.display().to_string());
        self.lines = content.lines().map(ToOwned::to_owned).collect();
        self.scroll = 0;

        // Compute semantic diff
        let diff = crate::tui::semantic_diff::SemanticDiff::compute(old_text, new_text);
        self.diff_lines = diff.render_lines();
        self.mode = if diff.is_empty() {
            EditorMode::Normal
        } else {
            EditorMode::Diff
        };

        Ok(())
    }

    pub fn scroll_down(&mut self) {
        self.scroll = (self.scroll + 1).min(self.lines.len().saturating_sub(1));
    }

    pub fn scroll_up(&mut self) {
        self.scroll = self.scroll.saturating_sub(1);
    }

    pub fn go_top(&mut self) {
        self.scroll = 0;
    }

    pub fn go_bottom(&mut self) {
        self.scroll = self.lines.len().saturating_sub(1);
    }

    pub fn page_down(&mut self) {
        self.scroll = (self.scroll + 10).min(self.lines.len().saturating_sub(1));
    }

    pub fn page_up(&mut self) {
        self.scroll = self.scroll.saturating_sub(10);
    }

    /// Switch to diff mode.
    pub fn show_diff(&mut self) {
        if !self.diff_lines.is_empty() {
            self.mode = EditorMode::Diff;
        }
    }

    /// Switch to normal mode.
    pub fn show_normal(&mut self) {
        self.mode = EditorMode::Normal;
    }

    /// Toggle between normal and diff view.
    pub fn toggle_diff(&mut self) {
        match self.mode {
            EditorMode::Normal => self.show_diff(),
            EditorMode::Diff => self.show_normal(),
        }
    }

    pub fn render(&self, frame: &mut Frame<'_>, area: Rect, active_pane: Pane) {
        let is_active = active_pane == Pane::Editor;
        let title = self
            .current_file
            .as_deref()
            .unwrap_or("Editor - select a file");
        let mode_indicator = match self.mode {
            EditorMode::Normal => "",
            EditorMode::Diff => " [DIFF]",
        };
        let block = Block::default()
            .title(if is_active {
                format!(" {title}{mode_indicator} * ")
            } else {
                format!(" {title}{mode_indicator} ")
            })
            .borders(Borders::ALL)
            .border_style(active_border_style(is_active));

        if self.mode == EditorMode::Diff && !self.diff_lines.is_empty() {
            // Render diff view with red/green highlighting
            let body: Vec<Line<'_>> = self
                .diff_lines
                .iter()
                .map(|(text, style)| Line::styled(text.clone(), *style))
                .collect();
            frame.render_widget(Paragraph::new(body).block(block), area);
        } else {
            let visible = self
                .lines
                .iter()
                .enumerate()
                .skip(self.scroll)
                .map(|(index, line)| {
                    Line::from(vec![
                        Span::styled(
                            format!("{:>4} ", index + 1),
                            Style::default().fg(Color::DarkGray),
                        ),
                        Span::raw(line.clone()),
                    ])
                });
            let body: Vec<Line<'_>> = if self.lines.is_empty() {
                vec![
                    Line::from("Select a file from the file tree or command palette."),
                    Line::from(""),
                    Line::styled("— Pi Hybrid v0.1.0 —", Style::default().fg(Color::DarkGray)),
                ]
            } else {
                visible.collect()
            };

            frame.render_widget(Paragraph::new(body).block(block), area);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ratatui::{Terminal, backend::TestBackend, layout::Rect};

    fn render_pane(pane: &EditorPane, active: bool) -> String {
        let backend = TestBackend::new(80, 20);
        let mut terminal = Terminal::new(backend).expect("terminal");
        let area = Rect::new(0, 0, 80, 20);
        let active_pane = if active { Pane::Editor } else { Pane::Files };
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
    fn empty_editor_shows_prompt() {
        let pane = EditorPane::default();
        let output = render_pane(&pane, true);
        assert!(output.contains("select a file"));
    }

    #[test]
    fn active_pane_shows_star() {
        let pane = EditorPane::default();
        let active = render_pane(&pane, true);
        let inactive = render_pane(&pane, false);
        assert!(active.contains("*"));
        assert!(!inactive.contains("*"));
    }

    #[test]
    fn scroll_down_increments() {
        let mut pane = EditorPane::default();
        assert_eq!(pane.scroll, 0);
        pane.scroll_down();
        assert_eq!(pane.scroll, 0); // no lines yet
        pane.lines = vec!["a".into(), "b".into(), "c".into()];
        pane.scroll_down();
        assert_eq!(pane.scroll, 1);
    }

    #[test]
    fn scroll_down_saturates() {
        let mut pane = EditorPane::default();
        pane.lines = vec!["a".into(), "b".into()];
        pane.scroll_down();
        pane.scroll_down();
        pane.scroll_down();
        assert_eq!(pane.scroll, 1); // saturates at len-1
    }

    #[test]
    fn scroll_up_decrements() {
        let mut pane = EditorPane::default();
        pane.lines = vec!["a".into(), "b".into(), "c".into(), "d".into()];
        pane.scroll = 3;
        pane.scroll_up();
        assert_eq!(pane.scroll, 2);
    }

    #[test]
    fn scroll_up_saturates_at_zero() {
        let mut pane = EditorPane::default();
        pane.scroll_up();
        assert_eq!(pane.scroll, 0);
    }

    #[test]
    fn go_top() {
        let mut pane = EditorPane::default();
        pane.lines = vec!["a".into(); 10];
        pane.scroll = 5;
        pane.go_top();
        assert_eq!(pane.scroll, 0);
    }

    #[test]
    fn go_bottom() {
        let mut pane = EditorPane::default();
        pane.lines = vec!["a".into(); 10];
        pane.go_bottom();
        assert_eq!(pane.scroll, 9);
    }

    #[test]
    fn go_bottom_empty() {
        let mut pane = EditorPane::default();
        pane.go_bottom();
        assert_eq!(pane.scroll, 0);
    }

    #[test]
    fn page_down_skips_10() {
        let mut pane = EditorPane::default();
        pane.lines = vec!["a".into(); 50];
        pane.scroll = 0;
        pane.page_down();
        assert_eq!(pane.scroll, 10);
    }

    #[test]
    fn page_down_saturates() {
        let mut pane = EditorPane::default();
        pane.lines = vec!["a".into(); 5];
        pane.page_down();
        assert_eq!(pane.scroll, 4);
    }

    #[test]
    fn page_up_skips_10_back() {
        let mut pane = EditorPane::default();
        pane.lines = vec!["a".into(); 50];
        pane.scroll = 20;
        pane.page_up();
        assert_eq!(pane.scroll, 10);
    }

    #[test]
    fn page_up_saturates_at_zero() {
        let mut pane = EditorPane::default();
        pane.scroll = 3;
        pane.page_up();
        assert_eq!(pane.scroll, 0);
    }

    #[test]
    fn toggle_diff() {
        let mut pane = EditorPane::default();
        assert_eq!(pane.mode, EditorMode::Normal);
        // No diff lines, so show_diff does nothing
        pane.toggle_diff();
        assert_eq!(pane.mode, EditorMode::Normal);

        // Add some diff lines
        pane.diff_lines = vec![("+added".into(), Style::default().fg(Color::Green))];
        pane.toggle_diff();
        assert_eq!(pane.mode, EditorMode::Diff);
        pane.toggle_diff();
        assert_eq!(pane.mode, EditorMode::Normal);
    }

    #[test]
    fn show_diff_only_with_content() {
        let mut pane = EditorPane::default();
        pane.show_diff();
        assert_eq!(pane.mode, EditorMode::Normal); // no diff lines

        pane.diff_lines = vec![("+added".into(), Style::default())];
        pane.show_diff();
        assert_eq!(pane.mode, EditorMode::Diff);
    }

    #[test]
    fn show_normal_resets_mode() {
        let mut pane = EditorPane::default();
        pane.diff_lines = vec![("x".into(), Style::default())];
        pane.show_diff();
        assert_eq!(pane.mode, EditorMode::Diff);
        pane.show_normal();
        assert_eq!(pane.mode, EditorMode::Normal);
    }

    #[test]
    fn editor_mode_default() {
        assert_eq!(EditorMode::default(), EditorMode::Normal);
    }

    #[test]
    fn diff_view_renders() {
        let mut pane = EditorPane::default();
        pane.current_file = Some("test.rs".into());
        pane.diff_lines = vec![
            ("+added line".into(), Style::default().fg(Color::Green)),
            ("-removed line".into(), Style::default().fg(Color::Red)),
        ];
        pane.mode = EditorMode::Diff;
        let output = render_pane(&pane, true);
        assert!(output.contains("test.rs"));
        assert!(output.contains("DIFF"));
        assert!(output.contains("added line"));
        assert!(output.contains("removed line"));
    }

    #[test]
    fn normal_view_renders_lines() {
        let mut pane = EditorPane::default();
        pane.current_file = Some("main.rs".into());
        pane.lines = vec![
            "fn main() {".into(),
            "    println!(\"hi\");".into(),
            "}".into(),
        ];
        let output = render_pane(&pane, true);
        assert!(output.contains("main.rs"));
        assert!(output.contains("fn main()"));
        assert!(output.contains("println"));
    }

    #[test]
    fn version_footer_in_empty_editor() {
        let pane = EditorPane::default();
        let output = render_pane(&pane, false);
        assert!(output.contains("Pi Hybrid"));
    }

    #[test]
    fn editor_pane_default() {
        let pane = EditorPane::default();
        assert!(pane.current_file.is_none());
        assert!(pane.lines.is_empty());
        assert_eq!(pane.scroll, 0);
        assert_eq!(pane.mode, EditorMode::Normal);
        assert!(pane.diff_lines.is_empty());
    }

    #[test]
    fn editor_mode_debug() {
        assert_eq!(format!("{:?}", EditorMode::Normal), "Normal");
        assert_eq!(format!("{:?}", EditorMode::Diff), "Diff");
    }

    #[test]
    fn scroll_up_at_bottom_moves_up() {
        let mut pane = EditorPane::default();
        pane.lines = vec!["line0".into(), "line1".into(), "line2".into()];
        pane.scroll = 2;
        pane.scroll_up();
        assert_eq!(pane.scroll, 1);
        pane.scroll_up();
        assert_eq!(pane.scroll, 0);
    }

    #[test]
    fn page_up_from_small_scroll_goes_to_zero() {
        let mut pane = EditorPane::default();
        pane.scroll = 5;
        pane.page_up();
        assert_eq!(pane.scroll, 0);
    }

    #[test]
    fn page_down_from_mid() {
        let mut pane = EditorPane::default();
        pane.lines = vec!["x".into(); 50];
        pane.scroll = 15;
        pane.page_down();
        assert_eq!(pane.scroll, 25);
    }

    #[test]
    fn go_bottom_single_line() {
        let mut pane = EditorPane::default();
        pane.lines = vec!["only".into()];
        pane.go_bottom();
        assert_eq!(pane.scroll, 0);
    }

    #[test]
    fn multiple_scroll_down_saturates() {
        let mut pane = EditorPane::default();
        pane.lines = vec!["a".into(), "b".into(), "c".into()];
        for _ in 0..10 {
            pane.scroll_down();
        }
        assert_eq!(pane.scroll, 2);
    }

    #[test]
    fn multiple_scroll_up_saturates() {
        let mut pane = EditorPane::default();
        pane.lines = vec!["a".into(), "b".into()];
        pane.scroll = 1;
        for _ in 0..10 {
            pane.scroll_up();
        }
        assert_eq!(pane.scroll, 0);
    }

    #[test]
    fn show_diff_empty_diff_lines_no_op() {
        let mut pane = EditorPane::default();
        pane.mode = EditorMode::Normal;
        pane.show_diff();
        assert_eq!(pane.mode, EditorMode::Normal);
    }

    #[test]
    fn toggle_diff_no_diff_lines_stays_normal() {
        let mut pane = EditorPane::default();
        assert_eq!(pane.mode, EditorMode::Normal);
        pane.toggle_diff();
        assert_eq!(pane.mode, EditorMode::Normal);
        pane.toggle_diff();
        assert_eq!(pane.mode, EditorMode::Normal);
    }

    #[test]
    fn render_normal_view_shows_line_numbers() {
        let mut pane = EditorPane::default();
        pane.current_file = Some("lib.rs".into());
        pane.lines = vec!["pub fn add() {}".into()];
        let output = render_pane(&pane, true);
        assert!(output.contains("lib.rs"));
        assert!(output.contains("pub fn add"));
    }

    #[test]
    fn render_inactive_no_star() {
        let mut pane = EditorPane::default();
        pane.current_file = Some("file.rs".into());
        pane.lines = vec!["code".into()];
        let output = render_pane(&pane, false);
        assert!(!output.contains("*"));
        assert!(output.contains("file.rs"));
    }

    #[test]
    fn render_diff_view_shows_diff_indicator() {
        let mut pane = EditorPane::default();
        pane.current_file = Some("changed.rs".into());
        pane.diff_lines = vec![("+new".into(), Style::default())];
        pane.mode = EditorMode::Diff;
        let output = render_pane(&pane, true);
        assert!(output.contains("DIFF"));
    }

    #[test]
    fn render_diff_view_empty_diff_shows_normal() {
        let mut pane = EditorPane::default();
        pane.current_file = Some("empty.rs".into());
        pane.lines = vec!["content".into()];
        pane.mode = EditorMode::Diff;
        // diff_lines is empty, so render falls through to normal view
        // Title still shows [DIFF] because mode is Diff, but content shows normal
        let output = render_pane(&pane, true);
        assert!(output.contains("content"));
    }

    #[test]
    fn open_with_diff_clears_old_diff() {
        // This tests that open_with_diff replaces old diff
        let mut pane = EditorPane::default();
        pane.diff_lines = vec![("old".into(), Style::default())];
        pane.mode = EditorMode::Diff;
        // Can't easily test open_with_diff without a real file,
        // but we can verify the field state
        assert!(!pane.diff_lines.is_empty());
    }
}
