use std::path::{Path, PathBuf};

use ignore::WalkBuilder;
use ratatui::{
    Frame,
    layout::Rect,
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, List, ListItem, ListState},
};

use super::{Pane, active_border_style};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FileEntry {
    pub path: PathBuf,
    pub display: String,
}

#[derive(Debug)]
pub struct FileTree {
    root: PathBuf,
    entries: Vec<FileEntry>,
    selected: usize,
}

impl FileTree {
    pub fn load(root: impl Into<PathBuf>) -> Self {
        let root = root.into();
        let entries = list_workspace_files(&root, 200);
        Self {
            root,
            entries,
            selected: 0,
        }
    }

    pub fn selected_path(&self) -> Option<PathBuf> {
        self.entries
            .get(self.selected)
            .map(|entry| self.root.join(&entry.path))
    }

    pub fn move_down(&mut self) {
        if !self.entries.is_empty() {
            self.selected = (self.selected + 1).min(self.entries.len() - 1);
        }
    }

    pub fn move_up(&mut self) {
        self.selected = self.selected.saturating_sub(1);
    }

    pub fn go_top(&mut self) {
        self.selected = 0;
    }

    pub fn go_bottom(&mut self) {
        if !self.entries.is_empty() {
            self.selected = self.entries.len() - 1;
        }
    }

    pub fn page_down(&mut self) {
        if !self.entries.is_empty() {
            self.selected = (self.selected + 10).min(self.entries.len() - 1);
        }
    }

    pub fn page_up(&mut self) {
        self.selected = self.selected.saturating_sub(10);
    }

    pub fn render(&self, frame: &mut Frame<'_>, area: Rect, active_pane: Pane) {
        let is_active = active_pane == Pane::Files;
        let block = Block::default()
            .title(if is_active { " Files * " } else { " Files " })
            .borders(Borders::ALL)
            .border_style(active_border_style(is_active));
        let items = self.entries.iter().enumerate().map(|(index, entry)| {
            let marker = if index == self.selected { ">" } else { " " };
            ListItem::new(Line::from(vec![
                Span::styled(marker, Style::default().fg(Color::Cyan)),
                Span::raw(" "),
                Span::raw(entry.display.clone()),
            ]))
        });
        let mut state = ListState::default().with_selected(Some(self.selected));
        frame.render_stateful_widget(
            List::new(items)
                .block(block)
                .highlight_style(Style::default().add_modifier(Modifier::BOLD)),
            area,
            &mut state,
        );
    }
}

pub fn list_workspace_files(root: &Path, limit: usize) -> Vec<FileEntry> {
    WalkBuilder::new(root)
        .hidden(false)
        .git_ignore(true)
        .git_exclude(true)
        .parents(true)
        .build()
        .filter_map(Result::ok)
        .filter(|entry| {
            entry
                .file_type()
                .is_some_and(|file_type| file_type.is_file())
        })
        .filter_map(|entry| {
            let rel = entry.path().strip_prefix(root).ok()?.to_path_buf();
            let depth = rel.components().count().saturating_sub(1);
            let display = format!(
                "{}{}",
                "  ".repeat(depth),
                rel.file_name()?.to_string_lossy()
            );
            Some(FileEntry { path: rel, display })
        })
        .take(limit)
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tree_navigation_clamps_to_bounds() {
        let mut tree = FileTree {
            root: PathBuf::from("."),
            entries: vec![
                FileEntry {
                    path: "a".into(),
                    display: "a".into(),
                },
                FileEntry {
                    path: "b".into(),
                    display: "b".into(),
                },
            ],
            selected: 0,
        };

        tree.move_up();
        assert_eq!(tree.selected, 0);
        tree.move_down();
        tree.move_down();
        assert_eq!(tree.selected, 1);
        tree.go_top();
        assert_eq!(tree.selected, 0);
    }
}
