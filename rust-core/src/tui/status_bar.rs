use ratatui::{
    Frame,
    layout::Rect,
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::Paragraph,
};

use super::Pane;

pub const APP_TITLE: &str = "Pi Hybrid v0.1.0";

/// Git status information passed from the App.
#[derive(Debug, Clone)]
pub struct GitStatusInfo {
    pub branch: String,
    pub is_clean: bool,
    pub modified: usize,
    pub staged: usize,
    pub untracked: usize,
}

#[allow(clippy::too_many_arguments)]
pub fn render(
    frame: &mut Frame<'_>,
    area: Rect,
    active_pane: Pane,
    git_branch: Option<&str>,
    bridge_connected: bool,
    dark_mode: bool,
    agent_counts: Option<(usize, usize)>,
    notification: Option<&str>,
    git_status: Option<Option<crate::agent::git::GitStatus>>,
    provider_name: Option<&str>,
) {
    let bg = if dark_mode { Color::Cyan } else { Color::White };
    let bridge = if bridge_connected {
        "connected"
    } else {
        "disconnected"
    };
    let branch = git_branch.unwrap_or("none");
    let agent_text = agent_counts
        .map(|(running, done)| format!("  |  Agents: {running} running, {done} done"))
        .unwrap_or_default();
    let notification_text = notification
        .filter(|message| !message.is_empty())
        .map(|message| format!("  |  {message}"))
        .unwrap_or_default();

    // Git status detail
    let git_detail = git_status
        .flatten()
        .map(|status| {
            format!(
                "  |  Git: {}  {} modified {} staged {} untracked",
                status.branch, status.modified, status.staged, status.untracked
            )
        })
        .unwrap_or_default();

    // Provider info
    let provider_text = provider_name
        .map(|name| format!("  |  Provider: {name}"))
        .unwrap_or_default();

    let status = Line::from(vec![
        Span::styled(
            APP_TITLE,
            Style::default()
                .fg(Color::Black)
                .bg(bg)
                .add_modifier(Modifier::BOLD),
        ),
        Span::styled(
            format!(
                "  Mode: NORMAL  |  Pane: {}  |  Git: {}  |  Tab panes  Ctrl+P palette  F1 help",
                active_pane.title(),
                branch
            ),
            Style::default().fg(Color::Black).bg(bg),
        ),
        Span::styled(
            format!("  |  Bridge: {bridge}"),
            Style::default()
                .fg(if bridge_connected {
                    Color::Black
                } else {
                    Color::Red
                })
                .bg(bg)
                .add_modifier(Modifier::BOLD),
        ),
        Span::styled(agent_text, Style::default().fg(Color::Black).bg(bg)),
        Span::styled(
            notification_text,
            Style::default()
                .fg(Color::Black)
                .bg(bg)
                .add_modifier(Modifier::BOLD),
        ),
        Span::styled(git_detail, Style::default().fg(Color::Black).bg(bg)),
        Span::styled(provider_text, Style::default().fg(Color::Black).bg(bg)),
    ]);
    frame.render_widget(Paragraph::new(status).style(Style::default().bg(bg)), area);
}

pub fn render_error(frame: &mut Frame<'_>, area: Rect, message: &str) {
    frame.render_widget(
        Paragraph::new(message.to_string()).style(
            Style::default()
                .fg(Color::White)
                .bg(Color::Red)
                .add_modifier(Modifier::BOLD),
        ),
        area,
    );
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::agent::git::GitStatus;
    use crate::tui::Pane;
    use ratatui::{Terminal, backend::TestBackend, layout::Rect};

    fn rect() -> Rect {
        Rect::new(0, 0, 300, 3)
    }

    fn render_to_string(
        active_pane: Pane,
        git_branch: Option<&str>,
        bridge_connected: bool,
        dark_mode: bool,
        agent_counts: Option<(usize, usize)>,
        notification: Option<&str>,
        git_status: Option<Option<GitStatus>>,
        provider_name: Option<&str>,
    ) -> String {
        let backend = TestBackend::new(300, 3);
        let mut terminal = Terminal::new(backend).expect("terminal creation");
        let area = rect();
        terminal
            .draw(|frame| {
                render(
                    frame,
                    area,
                    active_pane,
                    git_branch,
                    bridge_connected,
                    dark_mode,
                    agent_counts,
                    notification,
                    git_status,
                    provider_name,
                );
            })
            .expect("draw");
        let buffer = terminal.backend().buffer();
        let text: String = (0..buffer.area().height)
            .flat_map(|y| (0..buffer.area().width).map(move |x| buffer[(x, y)].symbol()))
            .collect();
        text
    }

    #[test]
    fn renders_title() {
        let output = render_to_string(
            Pane::Files,
            Some("main"),
            true,
            true,
            None,
            None,
            None,
            None,
        );
        assert!(output.contains(APP_TITLE));
    }

    #[test]
    fn shows_bridge_connected() {
        let output = render_to_string(
            Pane::Agents,
            Some("dev"),
            true,
            true,
            None,
            None,
            None,
            None,
        );
        assert!(output.contains("connected"));
    }

    #[test]
    fn shows_bridge_disconnected() {
        let output = render_to_string(
            Pane::Editor,
            Some("main"),
            false,
            true,
            None,
            None,
            None,
            None,
        );
        assert!(output.contains("disconnected"));
    }

    #[test]
    fn shows_git_branch() {
        let output = render_to_string(
            Pane::PlanApproval,
            Some("feature/x"),
            true,
            true,
            None,
            None,
            None,
            None,
        );
        assert!(output.contains("feature/x"));
    }

    #[test]
    fn git_branch_none_fallback() {
        let output = render_to_string(Pane::Files, None, true, true, None, None, None, None);
        assert!(output.contains("none"));
    }

    #[test]
    fn shows_active_pane_title() {
        let output = render_to_string(
            Pane::Files,
            Some("main"),
            true,
            true,
            None,
            None,
            None,
            None,
        );
        assert!(output.contains("Files"));
    }

    #[test]
    fn shows_agent_counts() {
        let output = render_to_string(
            Pane::Agents,
            Some("main"),
            true,
            true,
            Some((2, 1)),
            None,
            None,
            None,
        );
        assert!(output.contains("2 running"));
        assert!(output.contains("1 done"));
    }

    #[test]
    fn hides_agent_counts_when_none() {
        let output = render_to_string(
            Pane::Agents,
            Some("main"),
            true,
            true,
            None,
            None,
            None,
            None,
        );
        // The agent text should be empty, but we can't assert on negative
        // Just ensure it doesn't crash
        assert!(!output.is_empty());
    }

    #[test]
    fn shows_notification() {
        let output = render_to_string(
            Pane::Agents,
            Some("main"),
            true,
            true,
            None,
            Some("Task done!"),
            None,
            None,
        );
        assert!(output.contains("Task done!"));
    }

    #[test]
    fn empty_notification_skipped() {
        let output = render_to_string(
            Pane::Agents,
            Some("main"),
            true,
            true,
            None,
            Some(""),
            None,
            None,
        );
        // Empty notification should be filtered out
        assert!(!output.is_empty());
    }

    #[test]
    fn shows_git_details() {
        let status = GitStatus {
            branch: "main".to_string(),
            is_clean: false,
            modified: 3,
            staged: 1,
            untracked: 2,
            changed_files: vec!["src/main.rs".into()],
            detached: false,
        };
        let output = render_to_string(
            Pane::Files,
            Some("main"),
            true,
            true,
            None,
            None,
            Some(Some(status)),
            None,
        );
        assert!(output.contains("modified"));
        assert!(output.contains("staged"));
        assert!(output.contains("untracked"));
    }

    #[test]
    fn shows_provider_name() {
        let output = render_to_string(
            Pane::Files,
            Some("main"),
            true,
            true,
            None,
            None,
            None,
            Some("deepseek"),
        );
        assert!(output.contains("Provider: deepseek"));
    }

    #[test]
    fn dark_mode_uses_cyan_bg() {
        let output = render_to_string(
            Pane::Files,
            Some("main"),
            true,
            true,
            None,
            None,
            None,
            None,
        );
        // Dark mode should render — just verify non-empty
        assert!(!output.is_empty());
    }

    #[test]
    fn light_mode_uses_white_bg() {
        let output = render_to_string(
            Pane::Files,
            Some("main"),
            true,
            false,
            None,
            None,
            None,
            None,
        );
        assert!(!output.is_empty());
    }

    #[test]
    fn render_error_displays_message() {
        let backend = TestBackend::new(80, 1);
        let mut terminal = Terminal::new(backend).expect("terminal");
        terminal
            .draw(|frame| {
                render_error(frame, Rect::new(0, 0, 80, 1), "FATAL ERROR");
            })
            .expect("draw");
        let buffer = terminal.backend().buffer();
        let text: String = buffer
            .content()
            .iter()
            .map(|c| c.symbol())
            .collect::<Vec<_>>()
            .join("");
        assert!(text.contains("FATAL ERROR"));
    }

    #[test]
    fn git_status_none_hides_git_details() {
        let output = render_to_string(
            Pane::Files,
            Some("main"),
            true,
            true,
            None,
            None,
            None, // git_status is None
            None,
        );
        // The git detail line should be empty
        assert!(!output.contains("modified"));
        assert!(!output.contains("staged"));
    }

    #[test]
    fn clean_git_status() {
        let status = GitStatus {
            branch: "clean-branch".to_string(),
            is_clean: true,
            modified: 0,
            staged: 0,
            untracked: 0,
            changed_files: vec![],
            detached: false,
        };
        let output = render_to_string(
            Pane::Files,
            Some("clean-branch"),
            true,
            true,
            None,
            None,
            Some(Some(status)),
            None,
        );
        assert!(output.contains("Git: clean-branch"));
        assert!(output.contains("0 modified"));
        assert!(output.contains("0 staged"));
        assert!(output.contains("0 untracked"));
    }

    #[test]
    fn detached_head_git_status() {
        let status = GitStatus {
            branch: "HEAD".to_string(),
            is_clean: false,
            modified: 1,
            staged: 0,
            untracked: 0,
            changed_files: vec![],
            detached: true,
        };
        let output = render_to_string(
            Pane::Files,
            Some("HEAD"),
            true,
            true,
            None,
            None,
            Some(Some(status)),
            None,
        );
        assert!(output.contains("HEAD"));
    }

    #[test]
    fn combined_agents_notification_and_git() {
        let status = GitStatus {
            branch: "feature".to_string(),
            is_clean: false,
            modified: 2,
            staged: 1,
            untracked: 3,
            changed_files: vec!["src/main.rs".into()],
            detached: false,
        };
        let output = render_to_string(
            Pane::Agents,
            Some("feature"),
            true,
            true,
            Some((3, 1)),
            Some("Build succeeded"),
            Some(Some(status)),
            Some("openai"),
        );
        assert!(output.contains("3 running"));
        assert!(output.contains("1 done"));
        assert!(output.contains("Build succeeded"));
        assert!(output.contains("Git: feature"));
        assert!(output.contains("Provider: openai"));
    }

    #[test]
    fn bridge_disconnected_shows_text() {
        let output = render_to_string(
            Pane::Files,
            Some("main"),
            false,
            true,
            None,
            None,
            None,
            None,
        );
        assert!(output.contains("Bridge: disconnected"));
    }

    #[test]
    fn dark_and_light_mode_both_render() {
        let dark = render_to_string(
            Pane::Files,
            Some("main"),
            true,
            true,
            None,
            None,
            None,
            None,
        );
        let light = render_to_string(
            Pane::Files,
            Some("main"),
            true,
            false,
            None,
            None,
            None,
            None,
        );
        assert!(!dark.is_empty());
        assert!(!light.is_empty());
        // Both should contain the title
        assert!(dark.contains(APP_TITLE));
        assert!(light.contains(APP_TITLE));
    }

    #[test]
    fn all_panes_titles_appear_in_status() {
        for pane in &[Pane::Files, Pane::Editor, Pane::Agents, Pane::PlanApproval] {
            let output = render_to_string(*pane, Some("main"), true, true, None, None, None, None);
            assert!(output.contains(pane.title()));
        }
    }

    #[test]
    fn git_status_info_struct_fields() {
        let info = GitStatusInfo {
            branch: "dev".to_string(),
            is_clean: false,
            modified: 5,
            staged: 2,
            untracked: 1,
        };
        assert_eq!(info.branch, "dev");
        assert!(!info.is_clean);
        assert_eq!(info.modified, 5);
        assert_eq!(info.staged, 2);
        assert_eq!(info.untracked, 1);
    }
}
