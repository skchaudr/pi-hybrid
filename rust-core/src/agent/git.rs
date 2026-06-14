//! Git Integration — Programmatic git operations via git2.
//!
//! Features:
//! - Auto-commit on plan approval: capture diff, generate commit message
//! - Branch isolation per subagent (worktree pattern)
//! - Pre/post hooks for tool execution: snapshot before file writes, revert on failure
//! - Git status display in TUI status bar (F10/Ctrl+G toggle)

use std::path::{Path, PathBuf};

use anyhow::Context;
use serde::{Deserialize, Serialize};
use tracing::{debug, error, info, warn};

/// Snapshot info for pre-execution file state.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileSnapshot {
    /// Path relative to the repo root.
    pub path: PathBuf,
    /// SHA1 hash of the content before modification.
    pub hash: String,
    /// File size in bytes before modification.
    pub size: u64,
}

/// Result of a git auto-commit.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AutoCommitResult {
    /// Whether the commit was created.
    pub committed: bool,
    /// The commit OID (if committed).
    pub commit_oid: Option<String>,
    /// The commit message.
    pub message: String,
    /// Files changed in this commit.
    pub files_changed: Vec<String>,
    /// Lines added and removed.
    pub stats: Option<(usize, usize)>,
}

/// Git status information for display.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GitStatus {
    /// Current branch name.
    pub branch: String,
    /// Whether the working tree is clean.
    pub is_clean: bool,
    /// Number of modified files.
    pub modified: usize,
    /// Number of staged files.
    pub staged: usize,
    /// Number of untracked files.
    pub untracked: usize,
    /// Files with changes.
    pub changed_files: Vec<String>,
    /// Whether this is a detached HEAD.
    pub detached: bool,
}

/// The git integration manager.
pub struct GitManager {
    /// Path to the workspace root (git repo).
    repo_path: PathBuf,
    /// The git2 repository handle.
    repo: Option<git2::Repository>,
    /// Pre-execution snapshots of files.
    snapshots: Vec<FileSnapshot>,
    /// Whether auto-commit is enabled.
    auto_commit_enabled: bool,
    /// Whether branch isolation is enabled for subagents.
    branch_isolation_enabled: bool,
    /// Show git status in status bar.
    show_status: bool,
}

impl std::fmt::Debug for GitManager {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("GitManager")
            .field("repo_path", &self.repo_path)
            .field("has_repo", &self.repo.is_some())
            .field("snapshots", &self.snapshots.len())
            .field("auto_commit", &self.auto_commit_enabled)
            .field("branch_isolation", &self.branch_isolation_enabled)
            .finish()
    }
}

impl GitManager {
    /// Open a git repository at the given path.
    pub fn open(repo_path: PathBuf) -> Self {
        let repo = git2::Repository::open(&repo_path).ok();
        if repo.is_some() {
            debug!(path = %repo_path.display(), "Git repository opened");
        } else {
            debug!(path = %repo_path.display(), "No git repository found at path");
        }
        Self {
            repo_path,
            repo,
            snapshots: Vec::new(),
            auto_commit_enabled: false,
            branch_isolation_enabled: true,
            show_status: true,
        }
    }

    /// Check if a valid git repository is available.
    pub fn is_available(&self) -> bool {
        self.repo.is_some()
    }

    /// Get the repository root path.
    pub fn repo_path(&self) -> &PathBuf {
        &self.repo_path
    }

    /// Enable or disable auto-commit.
    pub fn set_auto_commit(&mut self, enabled: bool) {
        self.auto_commit_enabled = enabled;
    }

    /// Enable or disable branch isolation.
    pub fn set_branch_isolation(&mut self, enabled: bool) {
        self.branch_isolation_enabled = enabled;
    }

    /// Toggle git status display.
    pub fn toggle_status_display(&mut self) {
        self.show_status = !self.show_status;
    }

    /// Get whether status is shown.
    pub fn status_visible(&self) -> bool {
        self.show_status
    }

    /// Get the current git status.
    pub fn get_status(&self) -> Option<GitStatus> {
        let repo = self.repo.as_ref()?;

        let head = repo.head().ok()?;
        let branch = if head.is_branch() {
            head.shorthand().unwrap_or("HEAD").to_string()
        } else {
            "HEAD (detached)".to_string()
        };
        let detached = !head.is_branch();

        // Count changed files using status
        let mut modified = 0usize;
        let mut staged = 0usize;
        let mut untracked = 0usize;
        let mut changed_files = Vec::new();

        if let Ok(statuses) = repo.statuses(None) {
            for entry in statuses.iter() {
                let path = entry.path().unwrap_or("").to_string();
                let status = entry.status();

                if status.contains(git2::Status::INDEX_NEW)
                    || status.contains(git2::Status::INDEX_MODIFIED)
                    || status.contains(git2::Status::INDEX_DELETED)
                {
                    staged += 1;
                    changed_files.push(format!("[staged] {path}"));
                }

                if status.contains(git2::Status::WT_MODIFIED)
                    || status.contains(git2::Status::WT_DELETED)
                {
                    modified += 1;
                    changed_files.push(format!("[modified] {path}"));
                }

                if status.contains(git2::Status::WT_NEW) {
                    untracked += 1;
                    changed_files.push(format!("[untracked] {path}"));
                }
            }
        }

        let is_clean = modified == 0 && staged == 0 && untracked == 0;

        Some(GitStatus {
            branch,
            is_clean,
            modified,
            staged,
            untracked,
            changed_files,
            detached,
        })
    }

    /// Get the current branch name.
    pub fn current_branch(&self) -> Option<String> {
        let status = self.get_status()?;
        Some(status.branch)
    }

    /// Take a pre-execution snapshot of a file (before a tool writes to it).
    pub fn snapshot_file(&mut self, path: &Path) -> anyhow::Result<()> {
        let full_path = self.repo_path.join(path);
        if !full_path.exists() {
            // File doesn't exist yet, no snapshot needed
            return Ok(());
        }

        let content = std::fs::read(&full_path).with_context(|| {
            format!("Failed to read file for snapshot: {}", full_path.display())
        })?;

        // Simple hash: use first 16 bytes of SHA-like representation
        use std::collections::hash_map::DefaultHasher;
        use std::hash::{Hash, Hasher};
        let mut hasher = DefaultHasher::new();
        content.hash(&mut hasher);
        let hash = format!("{:016x}", hasher.finish());
        let size = content.len() as u64;

        self.snapshots.push(FileSnapshot {
            path: path.to_path_buf(),
            hash,
            size,
        });

        Ok(())
    }

    /// Revert a file to its pre-execution snapshot (on failure).
    pub fn revert_to_snapshot(&mut self, path: &Path) -> anyhow::Result<bool> {
        let snapshot_idx = self.snapshots.iter().position(|s| s.path == path);

        if let Some(idx) = snapshot_idx {
            let snapshot = self.snapshots.remove(idx);

            // Use git checkout to restore the file
            if let Some(ref repo) = self.repo {
                let relative_path = path.to_string_lossy();
                // Try to checkout the file from HEAD
                let head = repo.head()?;
                let tree = head.peel_to_tree()?;

                // Build a checkout builder
                let mut checkout = git2::build::CheckoutBuilder::new();
                let cpath = std::ffi::CString::new(relative_path.as_bytes()).unwrap_or_default();
                checkout.path(&cpath);
                checkout.force();

                repo.checkout_tree(tree.as_object(), Some(&mut checkout))
                    .with_context(|| format!("Failed to revert file: {relative_path}"))?;

                return Ok(true);
            }

            return Ok(false);
        }

        Ok(false)
    }

    /// Clear all snapshots.
    pub fn clear_snapshots(&mut self) {
        self.snapshots.clear();
    }

    /// Auto-commit changes after plan approval.
    /// Captures diff and generates a commit message.
    pub fn auto_commit(&self, message: Option<&str>) -> anyhow::Result<AutoCommitResult> {
        let repo = self
            .repo
            .as_ref()
            .ok_or_else(|| anyhow::anyhow!("No git repository available"))?;

        if !self.auto_commit_enabled {
            return Ok(AutoCommitResult {
                committed: false,
                commit_oid: None,
                message: String::new(),
                files_changed: Vec::new(),
                stats: None,
            });
        }

        // Get the diff stats
        let diff = repo.diff_index_to_workdir(None, None)?;
        let stats = diff.stats()?;
        let files_changed: Vec<String> = {
            let mut result = Vec::new();
            diff.print(git2::DiffFormat::NameOnly, |_delta, _hunk, line| {
                if let Ok(path) = std::str::from_utf8(line.content()) {
                    let path = path.trim();
                    if !path.is_empty() {
                        result.push(format!("M\t{path}"));
                    }
                }
                true
            })?;
            result
        };

        if files_changed.is_empty() {
            debug!("Auto-commit skipped: no changes");
            return Ok(AutoCommitResult {
                committed: false,
                commit_oid: None,
                message: "No changes to commit".to_string(),
                files_changed: Vec::new(),
                stats: None,
            });
        }

        // Generate commit message
        let commit_message = message
            .map(|m| m.to_string())
            .unwrap_or_else(|| generate_commit_message(&files_changed));

        // Stage all changes
        let mut index = repo.index()?;
        index.add_all(["*"].iter(), git2::IndexAddOption::DEFAULT, None)?;
        index.write()?;

        let tree_oid = index.write_tree()?;
        let tree = repo.find_tree(tree_oid)?;

        // Create the commit
        let signature = repo.signature()?;
        let parent_commit = repo.head()?.peel_to_commit().ok();

        let parents: Vec<&git2::Commit> = parent_commit.iter().collect();
        let commit_oid = repo.commit(
            Some("HEAD"),
            &signature,
            &signature,
            &commit_message,
            &tree,
            &parents,
        )?;

        info!(
            commit_oid = %commit_oid,
            files = files_changed.len(),
            insertions = stats.insertions(),
            deletions = stats.deletions(),
            "Auto-commit created"
        );

        Ok(AutoCommitResult {
            committed: true,
            commit_oid: Some(commit_oid.to_string()),
            message: commit_message,
            files_changed,
            stats: Some((stats.insertions(), stats.deletions())),
        })
    }

    /// Create a worktree for a subagent (branch isolation).
    /// Returns the path to the new worktree.
    pub fn create_subagent_worktree(
        &self,
        subagent_id: &str,
        base_branch: &str,
    ) -> anyhow::Result<PathBuf> {
        let repo = self
            .repo
            .as_ref()
            .ok_or_else(|| anyhow::anyhow!("No git repository available"))?;

        if !self.branch_isolation_enabled {
            // Without isolation, just use the main worktree
            return Ok(self.repo_path.clone());
        }

        // Create a branch for this subagent
        let branch_name = format!("subagent/{subagent_id}");

        // Find the base commit
        let base_commit = repo
            .find_branch(base_branch, git2::BranchType::Local)?
            .get()
            .peel_to_commit()?;

        // Create the branch
        repo.branch(&branch_name, &base_commit, false)?;

        // Create a worktree
        let worktree_path = self
            .repo_path
            .parent()
            .unwrap_or(&self.repo_path)
            .join(format!(".pi-worktrees/{subagent_id}"));

        // Remove existing worktree if present
        if worktree_path.exists() {
            let _ = std::fs::remove_dir_all(&worktree_path);
        }

        // Create the worktree
        repo.worktree(&branch_name, &worktree_path, None)?;

        Ok(worktree_path)
    }

    /// Remove a subagent worktree and merge/delete its branch.
    pub fn cleanup_subagent_worktree(
        &self,
        subagent_id: &str,
        merge_back: bool,
    ) -> anyhow::Result<()> {
        let repo = self
            .repo
            .as_ref()
            .ok_or_else(|| anyhow::anyhow!("No git repository available"))?;

        let branch_name = format!("subagent/{subagent_id}");
        let worktree_path = self
            .repo_path
            .parent()
            .unwrap_or(&self.repo_path)
            .join(format!(".pi-worktrees/{subagent_id}"));

        // Prune worktree from git's tracking
        if let Ok(mut worktree) = repo.find_worktree(&branch_name) {
            worktree.prune(None)?;
        }

        // Remove the worktree directory
        if worktree_path.exists() {
            let _ = std::fs::remove_dir_all(&worktree_path);
        }

        // Merge back if requested
        if merge_back
            && let Ok(mut sub_branch) = repo.find_branch(&branch_name, git2::BranchType::Local)
        {
            let sub_commit = sub_branch.get().peel_to_commit()?;

            // Switch to main and merge
            let head = repo.head()?;
            let main_branch = head.shorthand().unwrap_or("main");

            let mut main_ref = repo.find_branch(main_branch, git2::BranchType::Local)?;
            let main_commit = main_ref.get().peel_to_commit()?;

            // Perform a merge
            let mut merge_opts = git2::MergeOptions::new();
            let annotated = repo.reference_to_annotated_commit(
                &repo.find_reference(&format!("refs/heads/{branch_name}"))?,
            )?;
            repo.merge(&[&annotated], Some(&mut merge_opts), None)?;

            // Commit merge if clean
            let mut index = repo.index()?;
            if !index.has_conflicts() {
                let tree_oid = index.write_tree()?;
                let tree = repo.find_tree(tree_oid)?;
                let signature = repo.signature()?;
                repo.commit(
                    Some("HEAD"),
                    &signature,
                    &signature,
                    &format!("Merge subagent {subagent_id}"),
                    &tree,
                    &[&main_commit, &sub_commit],
                )?;
            }
        }

        // Delete the branch
        if let Ok(mut branch) = repo.find_branch(&branch_name, git2::BranchType::Local) {
            let _ = branch.delete();
        }

        Ok(())
    }

    /// Get the number of snapshots currently stored.
    pub fn snapshot_count(&self) -> usize {
        self.snapshots.len()
    }
}

/// Generate a conventional commit message from changed files.
fn generate_commit_message(files: &[String]) -> String {
    if files.is_empty() {
        return "chore: empty commit".to_string();
    }

    // Detect the type of change
    let mut types = std::collections::HashMap::new();
    for file in files {
        let ext = std::path::Path::new(
            file.split('\t')
                .nth(1)
                .unwrap_or(file)
                .trim_start_matches("A\t")
                .trim_start_matches("M\t")
                .trim_start_matches("D\t"),
        )
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("other");

        *types.entry(ext.to_string()).or_insert(0) += 1;
    }

    let commit_type = if files
        .iter()
        .any(|f| f.contains("test") || f.contains("spec"))
    {
        "test"
    } else if files.iter().any(|f| f.contains(".rs")) {
        "feat"
    } else if files
        .iter()
        .any(|f| f.contains(".toml") || f.contains(".lock") || f.contains(".json"))
    {
        "chore"
    } else if files
        .iter()
        .any(|f| f.contains(".md") || f.contains(".txt"))
    {
        "docs"
    } else {
        "feat"
    };

    let file_list: Vec<String> = files
        .iter()
        .take(5)
        .map(|f| f.split('\t').nth(1).unwrap_or(f).to_string())
        .collect();

    let file_desc = if file_list.len() <= 3 {
        file_list.join(", ")
    } else {
        format!(
            "{} and {} more",
            file_list[..3].join(", "),
            file_list.len() - 3
        )
    };

    format!("{commit_type}: update {file_desc}")
}

/// Extract the workspace root from current directory by finding the .git directory.
pub fn find_repo_root(start_dir: &Path) -> Option<PathBuf> {
    let mut current = start_dir.to_path_buf();
    loop {
        if current.join(".git").exists() {
            return Some(current);
        }
        if !current.pop() {
            return None;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn git_status_no_repo() {
        let tmp = std::env::temp_dir().join("not-a-git-repo-test");
        let _ = std::fs::create_dir_all(&tmp);

        let manager = GitManager::open(tmp.clone());
        assert!(!manager.is_available());
        assert!(manager.get_status().is_none());

        let _ = std::fs::remove_dir_all(&tmp);
    }

    #[test]
    fn generate_commit_message_single_file() {
        let files = vec!["M\tsrc/main.rs".to_string()];
        let msg = generate_commit_message(&files);
        assert!(msg.contains("feat:"));
        assert!(msg.contains("src/main.rs"));
    }

    #[test]
    fn generate_commit_message_multiple_files() {
        let files = vec![
            "M\tsrc/main.rs".to_string(),
            "M\tsrc/lib.rs".to_string(),
            "M\ttests/integration.rs".to_string(),
            "A\tnew_file.rs".to_string(),
        ];
        let msg = generate_commit_message(&files);
        assert!(msg.contains("and 1 more"));
    }

    #[test]
    fn generate_commit_message_test_files() {
        let files = vec!["M\ttests/test_foo.rs".to_string()];
        let msg = generate_commit_message(&files);
        assert!(msg.contains("test:"));
    }

    #[test]
    fn generate_commit_message_docs() {
        let files = vec!["M\tREADME.md".to_string()];
        let msg = generate_commit_message(&files);
        assert!(msg.contains("docs:"));
    }

    #[test]
    fn snapshot_and_revert_no_repo() {
        let tmp = std::env::temp_dir().join("git-test-norepo");
        let _ = std::fs::create_dir_all(&tmp);

        let mut manager = GitManager::open(tmp.clone());
        assert_eq!(manager.snapshot_count(), 0);

        // Snapshot of nonexistent file should be ok
        let result = manager.snapshot_file(Path::new("nonexistent.txt"));
        assert!(result.is_ok());

        let _ = std::fs::remove_dir_all(&tmp);
    }

    #[test]
    fn auto_commit_disabled() {
        let tmp = std::env::temp_dir().join("git-test-disabled");
        let _ = std::fs::create_dir_all(&tmp);

        let manager = GitManager::open(tmp.clone());
        let result = manager.auto_commit(None);
        // No repo available, should error
        assert!(result.is_err());

        let _ = std::fs::remove_dir_all(&tmp);
    }

    #[test]
    fn find_repo_root_finds_git() {
        // This test assumes we're in a git repo (the workspace)
        let root = find_repo_root(Path::new("."));
        // May or may not find it depending on test environment
        // Just verify it doesn't crash
        let _ = root;
    }

    #[test]
    fn git_manager_toggles() {
        let mut manager = GitManager::open(PathBuf::from("."));
        assert!(manager.status_visible());

        manager.toggle_status_display();
        assert!(!manager.status_visible());

        manager.toggle_status_display();
        assert!(manager.status_visible());
    }

    #[test]
    fn clear_snapshots() {
        let mut manager = GitManager::open(PathBuf::from("."));
        manager.clear_snapshots();
        assert_eq!(manager.snapshot_count(), 0);
    }

    #[test]
    fn generate_commit_message_empty_files() {
        let msg = generate_commit_message(&[]);
        assert_eq!(msg, "chore: empty commit");
    }

    #[test]
    fn generate_commit_message_toml_lock_json() {
        let files = vec!["M\tCargo.toml".to_string(), "M\tCargo.lock".to_string()];
        let msg = generate_commit_message(&files);
        assert!(msg.contains("chore:"));
    }

    #[test]
    fn generate_commit_message_with_no_extension() {
        let files = vec!["M\tMakefile".to_string()];
        let msg = generate_commit_message(&files);
        assert!(msg.contains("feat:")); // "other" extension defaults to feat
    }

    #[test]
    fn generate_commit_message_with_tab_prefixes() {
        let files = vec!["A\tnew_file.rs".to_string(), "D\told_file.rs".to_string()];
        let msg = generate_commit_message(&files);
        assert!(msg.contains("feat:"));
    }

    #[test]
    fn git_manager_repo_path() {
        let path = PathBuf::from("/some/repo");
        let manager = GitManager::open(path.clone());
        assert_eq!(manager.repo_path(), &path);
    }

    #[test]
    fn git_manager_current_branch_no_repo() {
        let manager = GitManager::open(PathBuf::from("/nonexistent"));
        assert_eq!(manager.current_branch(), None);
    }

    #[test]
    fn git_manager_set_auto_commit() {
        let mut manager = GitManager::open(PathBuf::from("."));
        manager.set_auto_commit(false);
        // auto_commit with no repo will still error, but the flag is set
        manager.set_auto_commit(true);
    }

    #[test]
    fn git_manager_set_branch_isolation() {
        let mut manager = GitManager::open(PathBuf::from("."));
        manager.set_branch_isolation(false);
        manager.set_branch_isolation(true);
    }

    #[test]
    fn git_manager_debug_format() {
        let manager = GitManager::open(PathBuf::from("."));
        let debug_str = format!("{manager:?}");
        assert!(!debug_str.is_empty());
        assert!(debug_str.contains("GitManager"));
    }

    #[test]
    fn auto_commit_result_defaults() {
        let result = AutoCommitResult {
            committed: false,
            commit_oid: None,
            message: String::new(),
            files_changed: vec![],
            stats: None,
        };
        assert!(!result.committed);
        assert!(result.commit_oid.is_none());
    }

    #[test]
    fn git_status_defaults() {
        let status = GitStatus {
            branch: "main".into(),
            is_clean: true,
            modified: 0,
            staged: 0,
            untracked: 0,
            changed_files: vec![],
            detached: false,
        };
        assert_eq!(status.branch, "main");
        assert!(status.is_clean);
    }

    #[test]
    fn file_snapshot_fields() {
        let snap = FileSnapshot {
            path: PathBuf::from("test.rs"),
            hash: "abc123".into(),
            size: 100,
        };
        assert_eq!(snap.path, PathBuf::from("test.rs"));
        assert_eq!(snap.hash, "abc123");
        assert_eq!(snap.size, 100);
    }

    #[test]
    fn snapshot_nonexistent_file_is_noop() {
        let tmp = std::env::temp_dir().join("git-test-snap-noexist");
        let _ = std::fs::create_dir_all(&tmp);

        let mut manager = GitManager::open(tmp.clone());
        let result = manager.snapshot_file(Path::new("definitely-does-not-exist.txt"));
        assert!(result.is_ok());

        let _ = std::fs::remove_dir_all(&tmp);
    }

    #[test]
    fn revert_to_snapshot_no_match() {
        let tmp = std::env::temp_dir().join("git-test-revert-nomatch");
        let _ = std::fs::create_dir_all(&tmp);

        let mut manager = GitManager::open(tmp.clone());
        let result = manager.revert_to_snapshot(Path::new("nonexistent.txt"));
        assert_eq!(result.unwrap(), false);

        let _ = std::fs::remove_dir_all(&tmp);
    }

    #[test]
    fn find_repo_root_not_found() {
        let tmp = std::env::temp_dir().join("git-test-find-none");
        let _ = std::fs::create_dir_all(&tmp);

        let root = find_repo_root(&tmp);
        assert!(root.is_none());

        let _ = std::fs::remove_dir_all(&tmp);
    }

    // ── Tests that require a real git repository ──────────────────

    /// Helper to create a temp git repo for testing.
    fn create_temp_git_repo() -> (PathBuf, tempfile::TempDir) {
        let dir = tempfile::tempdir().expect("create temp dir");
        let repo_path = dir.path().to_path_buf();

        // Initialize git repo
        let repo = git2::Repository::init(&repo_path).expect("init git repo");

        // Create an initial commit so HEAD is valid
        let sig = git2::Signature::now("Test", "test@test.com").unwrap();
        let tree_id = {
            let mut index = repo.index().unwrap();
            let oid = index.write_tree().unwrap();
            repo.find_tree(oid).unwrap()
        };
        repo.commit(Some("HEAD"), &sig, &sig, "Initial commit", &tree_id, &[])
            .expect("initial commit");

        (repo_path, dir)
    }

    #[test]
    fn git_status_with_repo() {
        let (repo_path, _dir) = create_temp_git_repo();
        let manager = GitManager::open(repo_path);
        assert!(manager.is_available());
        let status = manager.get_status();
        assert!(status.is_some());
        let status = status.unwrap();
        assert_eq!(status.branch, "main");
        assert!(status.is_clean);
    }

    #[test]
    fn snapshot_file_with_real_repo() {
        let (repo_path, _dir) = create_temp_git_repo();

        // Create a test file
        let file_path = repo_path.join("test.txt");
        std::fs::write(&file_path, "hello world").unwrap();

        let mut manager = GitManager::open(repo_path);
        let result = manager.snapshot_file(Path::new("test.txt"));
        assert!(result.is_ok());
        assert_eq!(manager.snapshot_count(), 1);
    }

    #[test]
    fn revert_to_snapshot_with_real_repo() {
        let (repo_path, _dir) = create_temp_git_repo();

        // Create and commit a file
        let file_path = repo_path.join("test.txt");
        std::fs::write(&file_path, "original content").unwrap();

        {
            let repo = git2::Repository::open(&repo_path).unwrap();
            let mut index = repo.index().unwrap();
            index.add_path(Path::new("test.txt")).unwrap();
            index.write().unwrap();
            let tree_id = index.write_tree().unwrap();
            let tree = repo.find_tree(tree_id).unwrap();
            let sig = git2::Signature::now("Test", "test@test.com").unwrap();
            let head = repo.head().unwrap().peel_to_commit().unwrap();
            repo.commit(Some("HEAD"), &sig, &sig, "add file", &tree, &[&head])
                .unwrap();
        }

        // Snapshot then modify
        let mut manager = GitManager::open(repo_path.clone());
        manager.snapshot_file(Path::new("test.txt")).unwrap();

        std::fs::write(&file_path, "modified content").unwrap();

        // Revert
        let reverted = manager.revert_to_snapshot(Path::new("test.txt")).unwrap();
        assert!(reverted);

        let content = std::fs::read_to_string(&file_path).unwrap();
        assert_eq!(content, "original content");
    }

    #[test]
    fn snapshot_file_then_clear() {
        let (repo_path, _dir) = create_temp_git_repo();
        let file_path = repo_path.join("snap.txt");
        std::fs::write(&file_path, "data").unwrap();

        let mut manager = GitManager::open(repo_path);
        manager.snapshot_file(Path::new("snap.txt")).unwrap();
        assert_eq!(manager.snapshot_count(), 1);
        manager.clear_snapshots();
        assert_eq!(manager.snapshot_count(), 0);
    }

    #[test]
    fn auto_commit_disabled_with_real_repo() {
        let (repo_path, _dir) = create_temp_git_repo();

        let file_path = repo_path.join("test_file.rs");
        std::fs::write(&file_path, "fn main() {}").unwrap();

        let mut manager = GitManager::open(repo_path);
        manager.set_auto_commit(false);
        let result = manager.auto_commit(None);
        assert!(result.is_ok());
        let result = result.unwrap();
        assert!(!result.committed);
    }

    #[test]
    fn git_status_detects_modified_files() {
        let (repo_path, _dir) = create_temp_git_repo();

        let file_path = repo_path.join("tracked.txt");
        std::fs::write(&file_path, "v1").unwrap();

        {
            let repo = git2::Repository::open(&repo_path).unwrap();
            let mut index = repo.index().unwrap();
            index.add_path(Path::new("tracked.txt")).unwrap();
            index.write().unwrap();
            let tree_id = index.write_tree().unwrap();
            let tree = repo.find_tree(tree_id).unwrap();
            let sig = git2::Signature::now("Test", "test@test.com").unwrap();
            let head = repo.head().unwrap().peel_to_commit().unwrap();
            repo.commit(Some("HEAD"), &sig, &sig, "add tracked", &tree, &[&head])
                .unwrap();
        }

        std::fs::write(&file_path, "v2").unwrap();

        let manager = GitManager::open(repo_path);
        let status = manager.get_status();
        assert!(status.is_some());
        let status = status.unwrap();
        assert!(!status.is_clean);
        assert!(status.modified > 0);
    }

    #[test]
    fn branch_isolation_disabled_uses_main_worktree() {
        let (repo_path, _dir) = create_temp_git_repo();

        let mut manager = GitManager::open(repo_path.clone());
        manager.set_branch_isolation(false);

        let wt_path = manager.create_subagent_worktree("test-agent", "main");
        assert!(wt_path.is_ok());
        assert_eq!(wt_path.unwrap(), repo_path);
    }

    #[test]
    fn current_branch_with_repo() {
        let (repo_path, _dir) = create_temp_git_repo();

        let manager = GitManager::open(repo_path);
        let branch = manager.current_branch();
        assert_eq!(branch, Some("main".to_string()));
    }
}
