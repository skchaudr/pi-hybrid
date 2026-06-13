use std::{fs, path::Path};

use ratatui::{
    Frame,
    layout::Rect,
    style::{Color, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Paragraph},
};

use super::{Pane, active_border_style};

#[derive(Debug, Default)]
pub struct EditorPane {
    current_file: Option<String>,
    lines: Vec<String>,
    scroll: usize,
}

impl EditorPane {
    pub fn open(&mut self, path: &Path) -> anyhow::Result<()> {
        let content = fs::read_to_string(path)?;
        self.current_file = Some(path.display().to_string());
        self.lines = content.lines().map(ToOwned::to_owned).collect();
        self.scroll = 0;
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

    pub fn render(&self, frame: &mut Frame<'_>, area: Rect, active_pane: Pane) {
        let is_active = active_pane == Pane::Editor;
        let title = self
            .current_file
            .as_deref()
            .unwrap_or("Editor - select a file");
        let block = Block::default()
            .title(if is_active {
                format!(" {title} * ")
            } else {
                format!(" {title} ")
            })
            .borders(Borders::ALL)
            .border_style(active_border_style(is_active));
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
            vec![Line::from("Select a file from the left pane.")]
        } else {
            visible.collect()
        };

        frame.render_widget(Paragraph::new(body).block(block), area);
    }
}
