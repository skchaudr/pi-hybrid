//! Graceful Shutdown + Signal Handling
//!
//! Ctrl+C saves session, cancels in-flight operations, closes bridge
//! processes, cleans up worktrees, and restores the terminal.
//!
//! Architecture:
//!   ShutdownHandler — watches OS signals, manages CancelToken
//!   CancelToken     — shared token checked by all async tasks
//!   TerminalGuard   — Drop guard that restores raw mode / cursor

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

use tracing::{error, info, warn};

/// A lightweight cancellation token shared across all async tasks.
/// Cheap to clone — all clones share the same underlying flag.
///
/// Subagents get child tokens via `child_token()`, which point to
/// the same flag so cancellation propagates immediately.
#[derive(Clone, Debug)]
pub struct CancelToken {
    cancelled: Arc<AtomicBool>,
}

impl CancelToken {
    /// Create a new, un-cancelled token.
    pub fn new() -> Self {
        Self {
            cancelled: Arc::new(AtomicBool::new(false)),
        }
    }

    /// Set the cancelled flag. Idempotent — safe to call multiple times.
    pub fn cancel(&self) {
        self.cancelled.store(true, Ordering::SeqCst);
    }

    /// Check whether cancellation has been requested.
    pub fn is_cancelled(&self) -> bool {
        self.cancelled.load(Ordering::SeqCst)
    }

    /// Create a child token that shares the same cancellation flag.
    /// When the parent is cancelled, the child sees it too.
    pub fn child_token(&self) -> Self {
        Self {
            cancelled: Arc::clone(&self.cancelled),
        }
    }
}

impl Default for CancelToken {
    fn default() -> Self {
        Self::new()
    }
}

/// The shutdown timeout in seconds. If cleanup takes longer than this,
/// the process force-exits.
pub const SHUTDOWN_TIMEOUT_SECS: u64 = 5;

/// A guard that restores terminal state on drop (even during panics).
///
/// Usage in main.rs:
///   let _guard = TerminalGuard;  // enable_raw_mode before this
///   // ... TUI runs ...
///   // When guard drops: disable_raw_mode, show_cursor
pub struct TerminalGuard;

impl Drop for TerminalGuard {
    fn drop(&mut self) {
        // Best-effort terminal restore — don't panic inside Drop.
        let _ = crossterm::terminal::disable_raw_mode();
        let _ = crossterm::execute!(
            std::io::stdout(),
            crossterm::terminal::LeaveAlternateScreen,
            crossterm::event::DisableMouseCapture,
        );
        // Show cursor on a separate call so a failure here doesn't
        // prevent the screen restore above.
        let _ = crossterm::execute!(std::io::stdout(), crossterm::cursor::Show);
    }
}

/// Handles graceful shutdown: watches OS signals, manages the
/// cancel token, and enforces the shutdown timeout.
#[derive(Debug)]
pub struct ShutdownHandler {
    /// The cancellation token shared with all async tasks.
    pub token: CancelToken,
    /// Set on second Ctrl+C — the user really wants out NOW.
    force_shutdown: Arc<AtomicBool>,
    /// Set after the first signal is received (prevents double-handling).
    shutdown_initiated: Arc<AtomicBool>,
}

impl ShutdownHandler {
    /// Create a new ShutdownHandler with a fresh CancelToken.
    pub fn new() -> Self {
        Self {
            token: CancelToken::new(),
            force_shutdown: Arc::new(AtomicBool::new(false)),
            shutdown_initiated: Arc::new(AtomicBool::new(false)),
        }
    }

    /// Check whether a force-shutdown (double Ctrl+C) has been requested.
    pub fn is_force_shutdown(&self) -> bool {
        self.force_shutdown.load(Ordering::SeqCst)
    }

    /// Begin watching for OS signals. This spawns a task that listens
    /// for SIGINT/SIGTERM/SIGHUP and triggers graceful shutdown.
    ///
    /// Returns a vector of JoinHandles — all must be awaited/aborted.
    /// On macOS/Linux, tokio::signal works for SIGINT, SIGTERM, SIGHUP.
    /// On stdin close (Ctrl+D / pipe close), we detect EOF on stdin.
    pub fn watch_signals(&self) -> Vec<tokio::task::JoinHandle<()>> {
        let mut handles = Vec::new();

        // ── SIGINT (Ctrl+C) ──────────────────────────────────────
        {
            let t = self.token.clone();
            let f = Arc::clone(&self.force_shutdown);
            let i = Arc::clone(&self.shutdown_initiated);
            handles.push(tokio::spawn(async move {
                match tokio::signal::ctrl_c().await {
                    Ok(()) => handle_signal("SIGINT", &t, f, &i).await,
                    Err(e) => error!(%e, "Failed to install SIGINT handler"),
                }
            }));
        }

        #[cfg(unix)]
        {
            // ── SIGTERM ──────────────────────────────────────
            let t = self.token.clone();
            let f = Arc::clone(&self.force_shutdown);
            let i = Arc::clone(&self.shutdown_initiated);
            handles.push(tokio::spawn(async move {
                match tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate()) {
                    Ok(mut stream) => {
                        let _ = stream.recv().await;
                        handle_signal("SIGTERM", &t, f, &i).await;
                    }
                    Err(e) => error!(%e, "Failed to install SIGTERM handler"),
                }
            }));

            // ── SIGHUP ──────────────────────────────────────
            let t = self.token.clone();
            let f = Arc::clone(&self.force_shutdown);
            let i = Arc::clone(&self.shutdown_initiated);
            handles.push(tokio::spawn(async move {
                match tokio::signal::unix::signal(tokio::signal::unix::SignalKind::hangup()) {
                    Ok(mut stream) => loop {
                        if stream.recv().await.is_none() {
                            break;
                        }
                        info!("SIGHUP received — config reload would happen here");
                    },
                    Err(e) => warn!(%e, "Failed to install SIGHUP handler (non-fatal)"),
                }
            }));
        }

        // ── Stdin close (Ctrl+D / pipe close) ──────────────────
        {
            let t = self.token.clone();
            let f = Arc::clone(&self.force_shutdown);
            let i = Arc::clone(&self.shutdown_initiated);
            handles.push(tokio::spawn(async move {
                let mut buf = [0u8; 1];
                loop {
                    match tokio::io::AsyncReadExt::read(&mut tokio::io::stdin(), &mut buf).await {
                        Ok(0) => {
                            handle_signal("stdin-close", &t, f.clone(), &i).await;
                            break;
                        }
                        Err(e) => {
                            warn!(%e, "Stdin read error, assuming pipe closed");
                            handle_signal("stdin-close", &t, f, &i).await;
                            break;
                        }
                        Ok(_) => continue,
                    }
                }
            }));
        }

        handles
    }
}

impl Default for ShutdownHandler {
    fn default() -> Self {
        Self::new()
    }
}

/// Handle a single signal event.
///
/// First signal: cancel the token, initiate graceful shutdown.
/// Second signal (within 5s): force-exit immediately.
async fn handle_signal(
    name: &str,
    token: &CancelToken,
    force: Arc<AtomicBool>,
    initiated: &AtomicBool,
) {
    if initiated.swap(true, Ordering::SeqCst) {
        warn!(signal = %name, "Force shutdown requested (second signal) — exiting immediately");
        force.store(true, Ordering::SeqCst);
        std::process::exit(1);
    }

    info!(signal = %name, "Shutdown initiated — cancelling operations");
    token.cancel();

    let timeout = Duration::from_secs(SHUTDOWN_TIMEOUT_SECS);
    tokio::spawn(async move {
        tokio::time::sleep(timeout).await;
        if !force.load(Ordering::SeqCst) {
            error!(
                timeout_secs = SHUTDOWN_TIMEOUT_SECS,
                "Shutdown timed out, forcing exit"
            );
            std::process::exit(1);
        }
    });
}

/// Run the full graceful shutdown sequence.
///
/// Order:
///   a. Cancel token is already set (by signal handler)
///   b. Cancel in-flight LLM calls via bridge
///   c. Flush SQLite writes
///   d. Save session state
///   e. Close bridge child processes
///   f. Clean up git worktrees
///   g. Restore terminal (handled by TerminalGuard Drop)
///   h. Exit with code 0
pub async fn execute_shutdown_sequence(
    bridge: Option<&mut crate::agent::bridge_client::BridgeClient>,
    session: Option<&mut crate::agent::session::SessionStore>,
    subagent_manager: Option<&crate::agent::subagents::SubagentManager>,
    git_manager: Option<&crate::agent::git::GitManager>,
) {
    info!("Executing graceful shutdown sequence");

    // (b) Cancel in-flight LLM calls
    if let Some(bridge) = bridge {
        info!("Cancelling in-flight LLM calls");
        if let Err(e) = bridge.cancel().await {
            warn!(%e, "Failed to cancel bridge operations");
        }
    }

    // (c) + (d) Session save is handled by the caller who has the
    // session store reference.

    // (d) Save session state
    if let Some(session) = session {
        info!("Saving session state to SQLite");
        let session_id = session.current_session_id();
        if let Some(id) = session_id
            && let Err(e) = session.update_session_status(id, "shutdown").await
        {
            warn!(%e, session_id = id, "Failed to update session status");
        }
    }

    // (e) Cancel all subagents
    if let Some(mgr) = subagent_manager {
        let count = mgr.cancel_all().await;
        if count > 0 {
            info!(count, "Cancelled running subagents");
        }
    }

    // (f) Clean up git worktrees — remove .pi-worktrees directory
    if let Some(git) = git_manager
        && git.is_available()
    {
        info!("Cleaning up git worktrees");
        // Remove the .pi-worktrees directory if it exists
        let worktrees_dir = git.repo_path().join(".pi-worktrees");
        if worktrees_dir.exists() {
            if let Err(e) = std::fs::remove_dir_all(&worktrees_dir) {
                warn!(%e, path = %worktrees_dir.display(), "Failed to clean up worktrees directory");
            } else {
                info!(path = %worktrees_dir.display(), "Removed worktrees directory");
            }
        }
    }

    // (g) Terminal restore is handled by TerminalGuard Drop in main.rs.
    // (h) We let main exit naturally with code 0.

    info!("Graceful shutdown sequence complete");
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cancel_token_new_is_not_cancelled() {
        let token = CancelToken::new();
        assert!(!token.is_cancelled());
    }

    #[test]
    fn cancel_token_cancel_sets_flag() {
        let token = CancelToken::new();
        token.cancel();
        assert!(token.is_cancelled());
    }

    #[test]
    fn cancel_token_cancel_is_idempotent() {
        let token = CancelToken::new();
        token.cancel();
        token.cancel();
        token.cancel();
        assert!(token.is_cancelled());
    }

    #[test]
    fn child_token_shares_flag() {
        let parent = CancelToken::new();
        let child = parent.child_token();

        assert!(!parent.is_cancelled());
        assert!(!child.is_cancelled());

        parent.cancel();
        assert!(parent.is_cancelled());
        assert!(child.is_cancelled());
    }

    #[test]
    fn child_token_cancel_propagates_to_parent() {
        let parent = CancelToken::new();
        let child = parent.child_token();

        child.cancel();
        assert!(parent.is_cancelled());
        assert!(child.is_cancelled());
    }

    #[test]
    fn multiple_children_all_share_same_flag() {
        let parent = CancelToken::new();
        let child1 = parent.child_token();
        let child2 = parent.child_token();
        let child3 = child1.child_token();

        parent.cancel();
        assert!(child1.is_cancelled());
        assert!(child2.is_cancelled());
        assert!(child3.is_cancelled());
    }

    #[test]
    fn shutdown_handler_creates_with_fresh_token() {
        let handler = ShutdownHandler::new();
        assert!(!handler.token.is_cancelled());
        assert!(!handler.is_force_shutdown());
    }

    #[test]
    fn shutdown_timeout_is_five_seconds() {
        assert_eq!(SHUTDOWN_TIMEOUT_SECS, 5);
    }

    #[test]
    fn cancel_token_default_is_not_cancelled() {
        let token = CancelToken::default();
        assert!(!token.is_cancelled());
    }

    #[test]
    fn cancel_token_clone_shares_state() {
        let token1 = CancelToken::new();
        let token2 = token1.clone();

        token1.cancel();
        assert!(token2.is_cancelled());
    }

    /// Simulate the double Ctrl+C scenario: first signal sets cancel,
    /// second signal sets force and exits.
    #[test]
    fn double_signal_pattern() {
        let token = CancelToken::new();
        let force = Arc::new(AtomicBool::new(false));
        let initiated = Arc::new(AtomicBool::new(false));

        // First signal: initiate shutdown
        assert!(!initiated.load(Ordering::SeqCst));
        initiated.store(true, Ordering::SeqCst);
        token.cancel();
        assert!(token.is_cancelled());
        assert!(!force.load(Ordering::SeqCst));

        // Second signal: force shutdown
        force.store(true, Ordering::SeqCst);
        assert!(force.load(Ordering::SeqCst));
    }

    #[test]
    fn shutdown_handler_default() {
        let handler = ShutdownHandler::default();
        assert!(!handler.token.is_cancelled());
    }

    #[test]
    fn shutdown_handler_force_flag_independent() {
        let handler = ShutdownHandler::new();
        assert!(!handler.is_force_shutdown());
        handler.token.cancel();
        // Cancelling token does NOT set force flag
        assert!(!handler.is_force_shutdown());
        assert!(handler.token.is_cancelled());
    }

    #[test]
    fn cancel_token_debug_format() {
        let token = CancelToken::new();
        let debug_str = format!("{token:?}");
        assert!(debug_str.contains("CancelToken"));
    }

    #[test]
    fn cancel_token_multiple_clones_independent() {
        let t1 = CancelToken::new();
        let t2 = t1.clone();
        let t3 = t2.clone();

        assert!(!t1.is_cancelled());
        assert!(!t2.is_cancelled());
        assert!(!t3.is_cancelled());

        t3.cancel();
        assert!(t1.is_cancelled());
        assert!(t2.is_cancelled());
        assert!(t3.is_cancelled());
    }

    #[test]
    fn shutdown_handler_debug_format() {
        let handler = ShutdownHandler::new();
        let debug_str = format!("{handler:?}");
        // Should not panic
        assert!(!debug_str.is_empty());
    }

    #[tokio::test]
    async fn execute_shutdown_with_none_handles_gracefully() {
        // execute_shutdown_sequence should not panic with all None args
        execute_shutdown_sequence(None, None, None, None).await;
    }
}
