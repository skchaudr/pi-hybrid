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

    pub fn relative_paths(&self) -> Vec<PathBuf> {
        self.entries
            .iter()
            .map(|entry| entry.path.clone())
            .collect()
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

    #[test]
    fn empty_tree_navigation_all_noops() {
        let mut tree = FileTree {
            root: PathBuf::from("."),
            entries: vec![],
            selected: 0,
        };
        tree.move_down();
        assert_eq!(tree.selected, 0);
        tree.move_up();
        assert_eq!(tree.selected, 0);
        tree.page_down();
        assert_eq!(tree.selected, 0);
        tree.page_up();
        assert_eq!(tree.selected, 0);
        tree.go_bottom();
        assert_eq!(tree.selected, 0);
        assert!(tree.selected_path().is_none());
        assert!(tree.relative_paths().is_empty());
    }

    #[test]
    fn tree_navigation_full_range() {
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
                FileEntry {
                    path: "c".into(),
                    display: "c".into(),
                },
                FileEntry {
                    path: "d".into(),
                    display: "d".into(),
                },
            ],
            selected: 0,
        };
        // go_bottom
        tree.go_bottom();
        assert_eq!(tree.selected, 3);
        // go_top
        tree.go_top();
        assert_eq!(tree.selected, 0);
        // page_down with few entries
        tree.page_down();
        assert_eq!(tree.selected, 3);
        // page_up
        tree.page_up();
        assert_eq!(tree.selected, 0);
    }

    #[test]
    fn selected_path_joins_with_root() {
        let root = PathBuf::from("/project");
        let tree = FileTree {
            root: root.clone(),
            entries: vec![
                FileEntry {
                    path: PathBuf::from("src/main.rs"),
                    display: "src/main.rs".into(),
                },
                FileEntry {
                    path: PathBuf::from("Cargo.toml"),
                    display: "Cargo.toml".into(),
                },
            ],
            selected: 0,
        };
        assert_eq!(
            tree.selected_path(),
            Some(PathBuf::from("/project/src/main.rs"))
        );
    }

    #[test]
    fn relative_paths_collects_all() {
        let tree = FileTree {
            root: PathBuf::from("."),
            entries: vec![
                FileEntry {
                    path: PathBuf::from("a.rs"),
                    display: "a.rs".into(),
                },
                FileEntry {
                    path: PathBuf::from("b.rs"),
                    display: "b.rs".into(),
                },
            ],
            selected: 0,
        };
        let paths = tree.relative_paths();
        assert_eq!(paths.len(), 2);
        assert_eq!(paths[0], PathBuf::from("a.rs"));
        assert_eq!(paths[1], PathBuf::from("b.rs"));
    }

    #[test]
    fn page_navigation_with_many_entries() {
        let entries: Vec<FileEntry> = (0..25)
            .map(|i| FileEntry {
                path: PathBuf::from(format!("file_{i}.rs")),
                display: format!("file_{i}.rs"),
            })
            .collect();
        let mut tree = FileTree {
            root: PathBuf::from("."),
            entries,
            selected: 0,
        };
        tree.page_down();
        assert_eq!(tree.selected, 10);
        tree.page_down();
        assert_eq!(tree.selected, 20);
        tree.page_down();
        assert_eq!(tree.selected, 24); // clamped to max
        tree.page_up();
        assert_eq!(tree.selected, 14);
        tree.page_up();
        assert_eq!(tree.selected, 4);
        tree.page_up();
        assert_eq!(tree.selected, 0); // clamped to 0
    }

    #[test]
    fn file_entry_debug_and_eq() {
        let a = FileEntry {
            path: PathBuf::from("a"),
            display: "a".into(),
        };
        let b = FileEntry {
            path: PathBuf::from("a"),
            display: "a".into(),
        };
        assert_eq!(a, b);
        let c = FileEntry {
            path: PathBuf::from("b"),
            display: "b".into(),
        };
        assert_ne!(a, c);
    }

    #[test]
    fn list_workspace_files_in_current_dir() {
        // This test exercises the real walker but is capped at a low limit
        let entries = list_workspace_files(Path::new("."), 5);
        // We should get some files (the current dir has files)
        assert!(entries.len() <= 5);
        // Entries should have relative paths
        for entry in &entries {
            assert!(!entry.path.as_os_str().is_empty());
            assert!(!entry.display.is_empty());
        }
    }
}
