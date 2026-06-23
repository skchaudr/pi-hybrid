use ratatui::{
    Frame,
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::Line,
    widgets::{Block, Borders, Clear, Paragraph},
};

#[derive(Debug, Default)]
pub struct HelpPopup {
    open: bool,
}

impl HelpPopup {
    pub fn open(&mut self) {
        self.open = true;
    }

    pub fn close(&mut self) {
        self.open = false;
    }

    pub fn is_open(&self) -> bool {
        self.open
    }

    pub fn render(&self, frame: &mut Frame<'_>, area: Rect) {
        if !self.open {
            return;
        }

        let popup = centered_rect(58, 62, area);
        frame.render_widget(Clear, popup);
        let block = Block::default()
            .title(" Help ")
            .borders(Borders::ALL)
            .border_style(
                Style::default()
                    .fg(Color::Cyan)
                    .add_modifier(Modifier::BOLD),
            );
        let lines = vec![
            Line::styled(
                "Navigation",
                Style::default()
                    .fg(Color::Cyan)
                    .add_modifier(Modifier::BOLD),
            ),
            Line::raw("  j/k, arrows: move selection or scroll"),
            Line::raw("  gg / G: top / bottom"),
            Line::raw("  Ctrl+d / Ctrl+u: page down / page up"),
            Line::raw(""),
            Line::styled(
                "Panes",
                Style::default()
                    .fg(Color::Cyan)
                    .add_modifier(Modifier::BOLD),
            ),
            Line::raw("  Tab: cycle panes"),
            Line::raw("  Tab: reach Files, Editor, Agents, Plan"),
            Line::raw("  F2-F4: focus Editor, Agents, Plan"),
            Line::raw(""),
            Line::styled(
                "Actions",
                Style::default()
                    .fg(Color::Cyan)
                    .add_modifier(Modifier::BOLD),
            ),
            Line::raw("  Ctrl+P / Cmd+P: command palette"),
            Line::raw("  Enter: open selected file"),
            Line::raw("  a/r/e: approve, reject, edit plan"),
            Line::raw("  q: quit"),
            Line::raw(""),
            Line::styled(
                "Toggles",
                Style::default()
                    .fg(Color::Cyan)
                    .add_modifier(Modifier::BOLD),
            ),
            Line::raw("  F5: file tree"),
            Line::raw("  F6: agent pane"),
            Line::raw("  F7: dark/light mode"),
            Line::raw("  Esc: close popup"),
        ];
        frame.render_widget(Paragraph::new(lines).block(block), popup);
    }
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

#[cfg(test)]
mod tests {
    use super::*;
    use ratatui::{Terminal, backend::TestBackend};

    fn render_popup(open: bool) -> String {
        let mut popup = HelpPopup::default();
        if open {
            popup.open();
        }
        let backend = TestBackend::new(80, 40);
        let mut terminal = Terminal::new(backend).expect("terminal");
        let area = Rect::new(0, 0, 80, 40);
        terminal
            .draw(|frame| {
                popup.render(frame, area);
            })
            .expect("draw");
        let buffer = terminal.backend().buffer();
        let text: String = (0..buffer.area().height)
            .flat_map(|y| (0..buffer.area().width).map(move |x| buffer[(x, y)].symbol()))
            .collect();
        text
    }

    #[test]
    fn closed_popup_renders_nothing() {
        let output = render_popup(false);
        // Should be mostly empty (cleared screen)
        assert!(!output.contains("Help"));
    }

    #[test]
    fn open_popup_shows_content() {
        let output = render_popup(true);
        assert!(output.contains("Help"));
        assert!(output.contains("Navigation"));
        assert!(output.contains("j/k"));
    }

    #[test]
    fn open_popup_has_all_sections() {
        let output = render_popup(true);
        assert!(output.contains("Navigation"));
        assert!(output.contains("Panes"));
        assert!(output.contains("Actions"));
        assert!(output.contains("Toggles"));
    }

    #[test]
    fn open_then_close() {
        let mut popup = HelpPopup::default();
        assert!(!popup.open);
        popup.open();
        assert!(popup.open);
        popup.close();
        assert!(!popup.open);
    }

    #[test]
    fn centered_rect_computes_correct_area() {
        let area = Rect::new(0, 0, 100, 100);
        let rect = centered_rect(50, 50, area);
        // Should be centered: 25% margin on each side
        assert_eq!(rect.x, 25);
        assert_eq!(rect.y, 25);
        assert_eq!(rect.width, 50);
        assert_eq!(rect.height, 50);
    }

    #[test]
    fn centered_rect_with_different_percents() {
        let area = Rect::new(0, 0, 80, 40);
        let rect = centered_rect(60, 30, area);
        assert_eq!(rect.x, 16); // (100-60)/2 = 20% of 80 = 16
        assert_eq!(rect.width, 48); // 60% of 80 = 48
        assert_eq!(rect.y, 14); // (100-30)/2 = 35% of 40 = 14
        assert_eq!(rect.height, 12); // 30% of 40 = 12
    }

    #[test]
    fn open_popup_has_quit_key() {
        let output = render_popup(true);
        assert!(output.contains("quit"));
    }

    #[test]
    fn default_popup_is_closed() {
        let popup = HelpPopup::default();
        assert!(!popup.open);
    }

    #[test]
    fn open_popup_has_escape_key() {
        let output = render_popup(true);
        assert!(output.contains("Esc"));
    }

    #[test]
    fn open_popup_has_specific_toggle_keys() {
        let output = render_popup(true);
        assert!(output.contains("F5"));
        assert!(output.contains("F6"));
        assert!(output.contains("F7"));
    }

    #[test]
    fn open_popup_has_command_palette_hint() {
        let output = render_popup(true);
        assert!(output.contains("Ctrl+P"));
    }

    #[test]
    fn open_popup_has_pane_shortcuts() {
        let output = render_popup(true);
        assert!(output.contains("F2-F4"));
        assert!(output.contains("Tab"));
    }

    #[test]
    fn open_popup_has_approve_reject_edit() {
        let output = render_popup(true);
        assert!(output.contains("a/r/e"));
    }

    #[test]
    fn centered_rect_full_area() {
        let area = Rect::new(0, 0, 100, 100);
        let rect = centered_rect(100, 100, area);
        assert_eq!(rect.x, 0);
        assert_eq!(rect.y, 0);
        assert_eq!(rect.width, 100);
        assert_eq!(rect.height, 100);
    }

    #[test]
    fn centered_rect_minimal() {
        let area = Rect::new(10, 10, 80, 40);
        let rect = centered_rect(10, 10, area);
        assert!(rect.width > 0);
        assert!(rect.height > 0);
    }

    #[test]
    fn help_popup_debug_format() {
        let popup = HelpPopup::default();
        let debug = format!("{:?}", popup);
        assert!(debug.contains("HelpPopup"));
    }
    #[test]
    fn content_does_not_overlap_borders() {
        let mut popup = HelpPopup::default();
        popup.open = true;
        let backend = TestBackend::new(80, 40);
        let mut terminal = Terminal::new(backend).expect("terminal");
        let area = Rect::new(0, 0, 80, 40);
        terminal
            .draw(|frame| {
                popup.render(frame, area);
            })
            .expect("draw");
        let buffer = terminal.backend().buffer();

        let rect = centered_rect(58, 62, area);

        for y in rect.y..rect.y + rect.height {
            let left = buffer[(rect.x, y)].symbol();
            let right = buffer[(rect.x + rect.width - 1, y)].symbol();
            if y == rect.y {
                assert_eq!(left, "┌");
                assert_eq!(right, "┐");
            } else if y == rect.y + rect.height - 1 {
                assert_eq!(left, "└");
                assert_eq!(right, "┘");
            } else {
                assert_eq!(left, "│");
                assert_eq!(right, "│");
            }
        }
    }
}
