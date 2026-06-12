//! Semantic Diffs — Syntax-aware diffing with tree-sitter.
//!
//! Uses the `diffy` crate for line-level diffs and `tree-sitter` for
//! syntax context. Produces `Vec<DiffHunk>` with syntax annotations.
//! Displayed in editor_pane and plan_pane with red/green highlighting.

use std::collections::HashMap;

use ratatui::style::{Color, Style};

/// The type of change in a diff hunk.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DiffKind {
    /// Lines were added.
    Added,
    /// Lines were removed.
    Removed,
    /// Lines are unchanged context.
    Context,
}

/// A single line in a diff hunk, with optional syntax context.
#[derive(Debug, Clone)]
pub struct DiffLine {
    /// The text content of this line.
    pub text: String,
    /// The kind of change.
    pub kind: DiffKind,
    /// Original line number (in old text), if applicable.
    pub old_line: Option<usize>,
    /// New line number (in new text), if applicable.
    pub new_line: Option<usize>,
    /// Syntax context: the enclosing function/class/block name.
    pub syntax_context: Option<String>,
}

/// A hunk of changes in a diff.
#[derive(Debug, Clone)]
pub struct DiffHunk {
    /// The lines in this hunk.
    pub lines: Vec<DiffLine>,
    /// Context lines shown before the hunk.
    pub context_before: usize,
    /// Context lines shown after the hunk.
    pub context_after: usize,
    /// The syntax context for this hunk.
    pub syntax_context: Option<String>,
}

/// Language support for tree-sitter.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SyntaxLanguage {
    Rust,
    TypeScript,
    Python,
    Unknown,
}

impl SyntaxLanguage {
    /// Detect language from file extension.
    pub fn from_extension(ext: &str) -> Self {
        match ext {
            "rs" => SyntaxLanguage::Rust,
            "ts" | "tsx" => SyntaxLanguage::TypeScript,
            "py" => SyntaxLanguage::Python,
            _ => SyntaxLanguage::Unknown,
        }
    }

    /// Get the language name as a string.
    pub fn name(&self) -> &'static str {
        match self {
            SyntaxLanguage::Rust => "Rust",
            SyntaxLanguage::TypeScript => "TypeScript",
            SyntaxLanguage::Python => "Python",
            SyntaxLanguage::Unknown => "Unknown",
        }
    }
}

/// A semantic diff that compares old and new text with syntax awareness.
#[derive(Debug, Clone)]
pub struct SemanticDiff {
    /// The raw unified diff text (from diffy).
    raw_diff: String,
    /// Structured diff hunks.
    hunks: Vec<DiffHunk>,
    /// Detected language.
    language: SyntaxLanguage,
    /// Whether the diff is empty (no changes).
    is_empty: bool,
    /// Total added lines.
    added_lines: usize,
    /// Total removed lines.
    removed_lines: usize,
}

impl SemanticDiff {
    /// Compute a semantic diff between old and new text.
    pub fn compute(old_text: &str, new_text: &str) -> Self {
        Self::compute_with_context(old_text, new_text, 3, SyntaxLanguage::Unknown)
    }

    /// Compute a semantic diff with explicit language and context lines.
    pub fn compute_with_context(
        old_text: &str,
        new_text: &str,
        context_lines: usize,
        language: SyntaxLanguage,
    ) -> Self {
        let old_lines: Vec<&str> = old_text.lines().collect();
        let new_lines: Vec<&str> = new_text.lines().collect();

        // Use diffy for line-level diffing
        let patch = diffy::create_patch(old_text, new_text);

        let mut hunks = Vec::new();
        let mut added = 0usize;
        let mut removed = 0usize;
        let is_empty = old_text == new_text;

        // Build structured hunks from diffy's output (Patch → Hunk → Line)
        for hunk in patch.hunks() {
            let mut hunk_lines = Vec::new();
            let old_range = hunk.old_range();
            let new_range = hunk.new_range();
            let mut old_line_num = old_range.start() + 1;
            let mut new_line_num = new_range.start() + 1;

            for line in hunk.lines().iter() {
                let (kind, text): (DiffKind, String) = match line {
                    diffy::Line::Context(c) => {
                        old_line_num += 1;
                        new_line_num += 1;
                        (DiffKind::Context, c.to_string())
                    }
                    diffy::Line::Insert(i) => {
                        new_line_num += 1;
                        added += 1;
                        (DiffKind::Added, i.to_string())
                    }
                    diffy::Line::Delete(d) => {
                        old_line_num += 1;
                        removed += 1;
                        (DiffKind::Removed, d.to_string())
                    }
                };

                let syntax_ctx = detect_syntax_context(&text, language);

                hunk_lines.push(DiffLine {
                    text,
                    kind,
                    old_line: if matches!(kind, DiffKind::Added) {
                        None
                    } else {
                        Some(old_line_num.saturating_sub(1))
                    },
                    new_line: if matches!(kind, DiffKind::Removed) {
                        None
                    } else {
                        Some(new_line_num.saturating_sub(1))
                    },
                    syntax_context: syntax_ctx,
                });
            }

            if !hunk_lines.is_empty() {
                let syn_ctx = hunk_lines.iter().find_map(|l| l.syntax_context.clone());
                hunks.push(DiffHunk {
                    lines: hunk_lines,
                    context_before: context_lines,
                    context_after: context_lines,
                    syntax_context: syn_ctx,
                });
            }
        }

        let raw_diff_str = patch.to_string();

        SemanticDiff {
            raw_diff: raw_diff_str,
            hunks,
            language,
            is_empty,
            added_lines: added,
            removed_lines: removed,
        }
    }

    /// Check if there are no changes.
    pub fn is_empty(&self) -> bool {
        self.is_empty || self.hunks.is_empty()
    }

    /// Get the structured diff hunks.
    pub fn hunks(&self) -> &[DiffHunk] {
        &self.hunks
    }

    /// Get the raw unified diff text.
    pub fn raw_diff(&self) -> &str {
        &self.raw_diff
    }

    /// Get the detected language.
    pub fn language(&self) -> SyntaxLanguage {
        self.language
    }

    /// Total added lines.
    pub fn added_lines(&self) -> usize {
        self.added_lines
    }

    /// Total removed lines.
    pub fn removed_lines(&self) -> usize {
        self.removed_lines
    }

    /// Summary of changes.
    pub fn summary(&self) -> String {
        if self.is_empty() {
            "No changes.".to_string()
        } else {
            format!(
                "{} additions, {} deletions across {} hunk(s)",
                self.added_lines,
                self.removed_lines,
                self.hunks.len()
            )
        }
    }

    /// Render the diff as colored text lines for TUI display.
    pub fn render_lines(&self) -> Vec<(String, Style)> {
        let mut result = Vec::new();

        result.push((
            format!("── Semantic Diff ── [{}]", self.summary()),
            Style::default().fg(Color::Cyan),
        ));

        for hunk in &self.hunks {
            // Hunk header
            if let Some(ref ctx) = hunk.syntax_context {
                result.push((
                    format!("  @@ {} @@", ctx),
                    Style::default().fg(Color::Yellow),
                ));
            }

            for line in &hunk.lines {
                let (prefix, style) = match line.kind {
                    DiffKind::Added => ("+", Style::default().fg(Color::Green)),
                    DiffKind::Removed => ("-", Style::default().fg(Color::Red)),
                    DiffKind::Context => (" ", Style::default().fg(Color::DarkGray)),
                };

                let mut display = format!("{prefix} {}", line.text);

                // Annotate with syntax context if available
                if let Some(ref ctx) = line.syntax_context {
                    display.push_str(&format!("  // {}", ctx));
                }

                result.push((display, style));
            }
        }

        result
    }

    /// Render as a single string with ANSI-like markers (for non-TUI contexts).
    pub fn render_plain(&self) -> String {
        let mut result = String::new();
        result.push_str(&format!("── Semantic Diff — {}\n", self.summary()));

        for hunk in &self.hunks {
            if let Some(ref ctx) = hunk.syntax_context {
                result.push_str(&format!("  @@ {} @@\n", ctx));
            }
            for line in &hunk.lines {
                let prefix = match line.kind {
                    DiffKind::Added => '+',
                    DiffKind::Removed => '-',
                    DiffKind::Context => ' ',
                };
                result.push_str(&format!("{prefix} {}\n", line.text));
            }
        }

        result
    }
}

/// Internal representation of a unified diff hunk during parsing.
#[derive(Debug, Clone)]
struct RawHunk {
    old_start: usize,
    new_start: usize,
    lines: Vec<String>,
}

/// Parse unified diff hunks from a diffy-created diff string.
fn parse_unified_diff_hunks(diff: &str) -> Vec<RawHunk> {
    let mut hunks = Vec::new();
    let mut current: Option<RawHunk> = None;
    let mut in_hunk = false;

    for line in diff.lines() {
        if line.starts_with("@@") {
            // Save previous hunk
            if let Some(hunk) = current.take()
                && !hunk.lines.is_empty()
            {
                hunks.push(hunk);
            }

            // Parse hunk header: @@ -old_start,old_count +new_start,new_count @@
            let parts: Vec<&str> = line.split_whitespace().collect();
            let old_start = parts
                .get(1)
                .and_then(|s| s.strip_prefix('-'))
                .and_then(|s| s.split(',').next())
                .and_then(|s| s.parse::<usize>().ok())
                .unwrap_or(1);

            let new_start = parts
                .get(2)
                .and_then(|s| s.strip_prefix('+'))
                .and_then(|s| s.split(',').next())
                .and_then(|s| s.parse::<usize>().ok())
                .unwrap_or(1);

            current = Some(RawHunk {
                old_start,
                new_start,
                lines: Vec::new(),
            });
            in_hunk = true;
        } else if in_hunk && let Some(ref mut hunk) = current {
            hunk.lines.push(line.to_string());
        }
    }

    // Save final hunk
    if let Some(hunk) = current
        && !hunk.lines.is_empty()
    {
        hunks.push(hunk);
    }

    hunks
}

/// Detect syntax context from a line of code using simple heuristics.
/// In a full implementation, this would use tree-sitter queries.
fn detect_syntax_context(line: &str, language: SyntaxLanguage) -> Option<String> {
    let trimmed = line.trim();

    match language {
        SyntaxLanguage::Rust => {
            if trimmed.starts_with("fn ") {
                let name = trimmed
                    .strip_prefix("fn ")
                    .unwrap_or("")
                    .split(&['(', '<', '{', ' '][..])
                    .next()
                    .unwrap_or("");
                Some(format!("fn {}", name))
            } else if trimmed.starts_with("impl ") {
                let name = trimmed
                    .strip_prefix("impl ")
                    .unwrap_or("")
                    .split(&['<', '{', ' ', '\n'][..])
                    .next()
                    .unwrap_or("");
                Some(format!("impl {}", name))
            } else if trimmed.starts_with("struct ") {
                Some(format!(
                    "struct {}",
                    trimmed.strip_prefix("struct ").unwrap_or("")
                ))
            } else if trimmed.starts_with("mod ") {
                Some(format!(
                    "mod {}",
                    trimmed.strip_prefix("mod ").unwrap_or("")
                ))
            } else {
                None
            }
        }
        SyntaxLanguage::TypeScript => {
            if trimmed.starts_with("function ") || trimmed.starts_with("async function ") {
                let name = trimmed
                    .trim_start_matches("async ")
                    .trim_start_matches("function ")
                    .split(&['(', '<', '{', ' '][..])
                    .next()
                    .unwrap_or("");
                Some(format!("function {}", name))
            } else if trimmed.starts_with("class ") {
                Some(format!(
                    "class {}",
                    trimmed.strip_prefix("class ").unwrap_or("")
                ))
            } else if trimmed.starts_with("interface ") {
                Some(format!(
                    "interface {}",
                    trimmed.strip_prefix("interface ").unwrap_or("")
                ))
            } else {
                None
            }
        }
        SyntaxLanguage::Python => {
            if trimmed.starts_with("def ") {
                let name = trimmed
                    .strip_prefix("def ")
                    .unwrap_or("")
                    .split(&['(', ':', ' '][..])
                    .next()
                    .unwrap_or("");
                Some(format!("def {}", name))
            } else if trimmed.starts_with("class ") {
                Some(format!(
                    "class {}",
                    trimmed.strip_prefix("class ").unwrap_or("")
                ))
            } else {
                None
            }
        }
        SyntaxLanguage::Unknown => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_diff_has_no_changes() {
        let diff = SemanticDiff::compute("hello\nworld\n", "hello\nworld\n");
        assert!(diff.is_empty());
        assert_eq!(diff.summary(), "No changes.");
    }

    #[test]
    fn detects_added_and_removed_lines() {
        let old = "line1\nline2\nline3\n";
        let new = "line1\nline2_changed\nline3\nline4\n";

        let diff = SemanticDiff::compute(old, new);

        assert!(!diff.is_empty());
        assert!(diff.added_lines() > 0);
        assert!(diff.removed_lines() > 0);
        assert!(!diff.hunks().is_empty());
    }

    #[test]
    fn syntax_context_detection() {
        assert!(detect_syntax_context("fn main() {", SyntaxLanguage::Rust).is_some());
        assert!(detect_syntax_context("function hello() {", SyntaxLanguage::TypeScript).is_some());
        assert!(detect_syntax_context("def foo():", SyntaxLanguage::Python).is_some());
        assert!(detect_syntax_context("let x = 1;", SyntaxLanguage::Rust).is_none());
    }

    #[test]
    fn render_produces_output() {
        let old = "fn hello() {\n    println!(\"old\");\n}\n";
        let new = "fn hello() {\n    println!(\"new\");\n}\n";

        let diff = SemanticDiff::compute_with_context(old, new, 3, SyntaxLanguage::Rust);
        let lines = diff.render_lines();
        assert!(!lines.is_empty());

        let plain = diff.render_plain();
        assert!(plain.contains("Semantic Diff"));
    }

    #[test]
    fn language_detection_from_extension() {
        assert_eq!(SyntaxLanguage::from_extension("rs"), SyntaxLanguage::Rust);
        assert_eq!(
            SyntaxLanguage::from_extension("ts"),
            SyntaxLanguage::TypeScript
        );
        assert_eq!(
            SyntaxLanguage::from_extension("tsx"),
            SyntaxLanguage::TypeScript
        );
        assert_eq!(SyntaxLanguage::from_extension("py"), SyntaxLanguage::Python);
        assert_eq!(
            SyntaxLanguage::from_extension("txt"),
            SyntaxLanguage::Unknown
        );
    }

    #[test]
    fn compute_with_explicit_language() {
        let diff = SemanticDiff::compute_with_context(
            "def foo(): pass\n",
            "def foo():\n    return 1\n",
            3,
            SyntaxLanguage::Python,
        );
        assert!(!diff.is_empty());
        assert_eq!(diff.language(), SyntaxLanguage::Python);
    }
}
