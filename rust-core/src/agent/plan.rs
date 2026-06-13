//! Plan → Review → Approve → Execute workflow.
//!
//! The agent generates an editable plan (`Vec<Step>` with description, tool, status).
//! Plans are displayed in the plan_pane via JSON messages to the TUI.
//! User approves/rejects/edits via hotkeys (a/r/e).
//! Only approved steps execute.
//! Shows inline unified diffs for file edits before approval.

use serde::{Deserialize, Serialize};
use tracing::{debug, trace};

/// The status of a single plan step.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum StepStatus {
    /// Step has not been reviewed yet.
    Pending,
    /// Step has been approved by the user.
    Approved,
    /// Step has been rejected by the user.
    Rejected,
    /// Step is currently being executed.
    Executing,
    /// Step completed successfully.
    Completed,
    /// Step failed with an error.
    Failed,
}

impl StepStatus {
    pub fn as_str(&self) -> &'static str {
        match self {
            StepStatus::Pending => "pending",
            StepStatus::Approved => "approved",
            StepStatus::Rejected => "rejected",
            StepStatus::Executing => "executing",
            StepStatus::Completed => "completed",
            StepStatus::Failed => "failed",
        }
    }

    pub fn icon(&self) -> &'static str {
        match self {
            StepStatus::Pending => "○",
            StepStatus::Approved => "✓",
            StepStatus::Rejected => "✗",
            StepStatus::Executing => "⟳",
            StepStatus::Completed => "●",
            StepStatus::Failed => "✕",
        }
    }
}

/// A single step in a plan.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Step {
    /// Human-readable description of what this step does.
    pub description: String,
    /// The tool to execute (e.g., "read_file", "write_file", "run_shell").
    pub tool: String,
    /// Parameters for the tool call.
    pub params: serde_json::Value,
    /// Current status of this step.
    pub status: StepStatus,
    /// Optional unified diff preview for file edits.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub diff_preview: Option<String>,
    /// Result output after execution.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub result: Option<String>,
}

impl Step {
    /// Create a new pending step.
    pub fn new(
        description: impl Into<String>,
        tool: impl Into<String>,
        params: serde_json::Value,
    ) -> Self {
        Self {
            description: description.into(),
            tool: tool.into(),
            params,
            status: StepStatus::Pending,
            diff_preview: None,
            result: None,
        }
    }

    /// Check if this step modifies files (needs diff preview).
    pub fn is_file_edit(&self) -> bool {
        matches!(
            self.tool.as_str(),
            "write_file" | "patch" | "edit_file" | "replace"
        )
    }

    /// Generate a unified diff summary for display.
    pub fn diff_summary(&self) -> Option<String> {
        self.diff_preview.clone().or_else(|| {
            if self.is_file_edit() {
                Some(format!(
                    "File edit: {} with params {}",
                    self.tool,
                    serde_json::to_string_pretty(&self.params).unwrap_or_default()
                ))
            } else {
                None
            }
        })
    }

    /// Format the step for TUI display.
    pub fn display_line(&self) -> String {
        format!(
            "{} {} — {} [{}]",
            self.status.icon(),
            self.description,
            self.tool,
            self.status.as_str()
        )
    }
}

/// Overall plan status.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum PlanStatus {
    /// Plan is being drafted by the agent.
    Draft,
    /// Plan is ready for user review.
    PendingApproval,
    /// User approved the plan, execution can begin.
    Approved,
    /// User rejected the plan.
    Rejected,
    /// Plan is currently being executed.
    Executing,
    /// All approved steps have completed.
    Completed,
    /// One or more steps failed.
    Failed,
}

impl PlanStatus {
    pub fn as_str(&self) -> &'static str {
        match self {
            PlanStatus::Draft => "draft",
            PlanStatus::PendingApproval => "pending_approval",
            PlanStatus::Approved => "approved",
            PlanStatus::Rejected => "rejected",
            PlanStatus::Executing => "executing",
            PlanStatus::Completed => "completed",
            PlanStatus::Failed => "failed",
        }
    }
}

/// A plan containing multiple steps.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Plan {
    /// Unique identifier for this plan.
    pub id: String,
    /// Human-readable title.
    pub title: String,
    /// The plan steps.
    pub steps: Vec<Step>,
    /// Overall plan status.
    pub status: PlanStatus,
}

impl Plan {
    /// Create a new plan from a list of steps.
    pub fn new(id: impl Into<String>, title: impl Into<String>, steps: Vec<Step>) -> Self {
        Self {
            id: id.into(),
            title: title.into(),
            steps,
            status: PlanStatus::Draft,
        }
    }

    /// Submit the plan for review.
    pub fn submit_for_approval(&mut self) {
        self.status = PlanStatus::PendingApproval;
    }

    /// Count steps by status.
    pub fn count_by_status(&self, status: StepStatus) -> usize {
        self.steps.iter().filter(|s| s.status == status).count()
    }

    /// Check if all steps are in a terminal state.
    pub fn is_done(&self) -> bool {
        self.steps.iter().all(|s| {
            matches!(
                s.status,
                StepStatus::Completed | StepStatus::Rejected | StepStatus::Failed
            )
        })
    }

    /// Get a summary for TUI display.
    pub fn display_summary(&self) -> String {
        let total = self.steps.len();
        let approved = self.count_by_status(StepStatus::Approved);
        let completed = self.count_by_status(StepStatus::Completed);
        let failed = self.count_by_status(StepStatus::Failed);
        let rejected = self.count_by_status(StepStatus::Rejected);

        format!(
            "{} [{}] — {}/{} steps | {}✓ {}● {}✕ {}✗",
            self.title,
            self.status.as_str(),
            completed,
            total,
            approved,
            completed,
            failed,
            rejected,
        )
    }
}

/// Manages the current plan lifecycle.
#[derive(Debug)]
pub struct PlanManager {
    /// The current active plan, if any.
    current_plan: Option<Plan>,
    /// History of completed/rejected plans.
    history: Vec<Plan>,
}

impl PlanManager {
    /// Create a new empty plan manager.
    pub fn new() -> Self {
        Self {
            current_plan: None,
            history: Vec::new(),
        }
    }

    /// Create a new plan, replacing any existing one.
    pub fn create_plan(
        &mut self,
        id: impl Into<String>,
        title: impl Into<String>,
        steps: Vec<Step>,
    ) -> &Plan {
        // Archive any existing plan
        if let Some(plan) = self.current_plan.take() {
            debug!(plan_id = %plan.id, "Archiving previous plan");
            self.history.push(plan);
        }
        let plan = Plan::new(id, title, steps);
        debug!(plan_id = %plan.id, step_count = plan.steps.len(), "New plan created");
        self.current_plan = Some(plan);
        self.current_plan.as_ref().expect("plan was just set above")
    }

    /// Submit the current plan for approval.
    pub fn submit_for_approval(&mut self) {
        if let Some(plan) = &mut self.current_plan {
            plan.submit_for_approval();
            debug!(plan_id = %plan.id, "Plan submitted for approval");
        }
    }

    /// Approve all pending steps in the current plan.
    pub fn approve(&mut self) {
        if let Some(plan) = &mut self.current_plan
            && plan.status == PlanStatus::PendingApproval
        {
            plan.status = PlanStatus::Approved;
            for step in &mut plan.steps {
                if step.status == StepStatus::Pending {
                    step.status = StepStatus::Approved;
                }
            }
            debug!(plan_id = %plan.id, "Plan approved");
        }
    }

    /// Reject the current plan.
    pub fn reject(&mut self) {
        if let Some(plan) = &mut self.current_plan
            && plan.status == PlanStatus::PendingApproval
        {
            plan.status = PlanStatus::Rejected;
            for step in &mut plan.steps {
                if step.status == StepStatus::Pending {
                    step.status = StepStatus::Rejected;
                }
            }
            debug!(plan_id = %plan.id, "Plan rejected, moving to history");
            // Move to history
            if let Some(plan) = self.current_plan.take() {
                self.history.push(plan);
            }
        }
    }

    /// Approve a specific step by index.
    pub fn approve_step(&mut self, index: usize) -> Option<&Step> {
        if let Some(plan) = &mut self.current_plan
            && let Some(step) = plan.steps.get_mut(index)
        {
            if step.status == StepStatus::Pending {
                step.status = StepStatus::Approved;
            }
            return Some(&*step);
        }
        None
    }

    /// Reject a specific step by index.
    pub fn reject_step(&mut self, index: usize) -> Option<&Step> {
        if let Some(plan) = &mut self.current_plan
            && let Some(step) = plan.steps.get_mut(index)
        {
            if step.status == StepStatus::Pending {
                step.status = StepStatus::Rejected;
            }
            return Some(&*step);
        }
        None
    }

    /// Mark a step as executing.
    pub fn start_step(&mut self, index: usize) {
        if let Some(plan) = &mut self.current_plan {
            plan.status = PlanStatus::Executing;
            if let Some(step) = plan.steps.get_mut(index)
                && step.status == StepStatus::Approved
            {
                step.status = StepStatus::Executing;
            }
        }
    }

    /// Mark a step as completed with its result.
    pub fn complete_step(&mut self, index: usize, result: Option<String>) {
        if let Some(plan) = &mut self.current_plan {
            if let Some(step) = plan.steps.get_mut(index) {
                step.status = StepStatus::Completed;
                step.result = result;
            }
            if plan.is_done() {
                plan.status = if plan.count_by_status(StepStatus::Failed) > 0 {
                    PlanStatus::Failed
                } else {
                    PlanStatus::Completed
                };
            }
        }
    }

    /// Mark a step as failed with an error.
    pub fn fail_step(&mut self, index: usize, error: String) {
        if let Some(plan) = &mut self.current_plan
            && let Some(step) = plan.steps.get_mut(index)
        {
            step.status = StepStatus::Failed;
            step.result = Some(error);
        }
    }

    /// Get a reference to the current plan.
    pub fn current_plan(&self) -> Option<&Plan> {
        self.current_plan.as_ref()
    }

    /// Get a mutable reference to the current plan.
    pub fn current_plan_mut(&mut self) -> Option<&mut Plan> {
        self.current_plan.as_mut()
    }

    /// Get approved steps ready for execution.
    pub fn approved_steps(&self) -> Vec<(usize, &Step)> {
        self.current_plan
            .as_ref()
            .map(|plan| {
                plan.steps
                    .iter()
                    .enumerate()
                    .filter(|(_, s)| s.status == StepStatus::Approved)
                    .collect()
            })
            .unwrap_or_default()
    }

    /// Generate a unified diff for a file edit step.
    pub fn generate_diff(&self, index: usize) -> Option<String> {
        let plan = self.current_plan.as_ref()?;
        let step = plan.steps.get(index)?;
        step.diff_summary()
    }

    /// Format the plan for TUI display as lines of text.
    pub fn format_for_display(&self) -> Vec<String> {
        let mut lines = Vec::new();

        if let Some(plan) = &self.current_plan {
            lines.push(plan.display_summary());
            lines.push(String::new());

            for (i, step) in plan.steps.iter().enumerate() {
                let line = format!("  {}. {}", i + 1, step.display_line());
                lines.push(line);

                // Show diff preview if available
                if let Some(diff) = &step.diff_preview {
                    for diff_line in diff.lines() {
                        lines.push(format!("     | {diff_line}"));
                    }
                }
            }

            if plan.is_done() {
                lines.push(String::new());
                lines.push(match plan.status {
                    PlanStatus::Completed => "✓ Plan completed successfully.".to_string(),
                    PlanStatus::Failed => "✕ Plan failed.".to_string(),
                    _ => String::new(),
                });
            }
        } else {
            lines.push("No active plan.".to_string());
            lines.push("Plans, approvals, and execution checkpoints.".to_string());
        }

        lines
    }
}

impl Default for PlanManager {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_steps() -> Vec<Step> {
        vec![
            Step::new(
                "Read the config file",
                "read_file",
                serde_json::json!({"path": "/tmp/config.toml"}),
            ),
            Step::new(
                "Update the database URL",
                "write_file",
                serde_json::json!({"path": "/tmp/config.toml", "content": "[db]\nurl = 'localhost'"}),
            ),
            Step::new(
                "Run the tests",
                "run_shell",
                serde_json::json!({"command": "cargo test"}),
            ),
        ]
    }

    #[test]
    fn plan_lifecycle() {
        let mut mgr = PlanManager::new();
        mgr.create_plan("plan-1", "Test Plan", sample_steps());
        mgr.submit_for_approval();

        let plan = mgr.current_plan().unwrap();
        assert_eq!(plan.status, PlanStatus::PendingApproval);
        assert_eq!(plan.steps.len(), 3);
        assert_eq!(plan.count_by_status(StepStatus::Pending), 3);
    }

    #[test]
    fn approve_and_execute() {
        let mut mgr = PlanManager::new();
        mgr.create_plan("plan-1", "Test Plan", sample_steps());
        mgr.submit_for_approval();
        mgr.approve();

        let plan = mgr.current_plan().unwrap();
        assert_eq!(plan.status, PlanStatus::Approved);
        assert_eq!(plan.count_by_status(StepStatus::Approved), 3);

        // Execute step 0
        mgr.start_step(0);
        mgr.complete_step(0, Some("config read successfully".to_string()));
        let plan = mgr.current_plan().unwrap();
        assert_eq!(plan.steps[0].status, StepStatus::Completed);
    }

    #[test]
    fn reject_plan() {
        let mut mgr = PlanManager::new();
        mgr.create_plan("plan-1", "Test Plan", sample_steps());
        mgr.submit_for_approval();
        mgr.reject();

        assert!(mgr.current_plan().is_none());
        assert_eq!(mgr.history.len(), 1);
        assert_eq!(mgr.history[0].status, PlanStatus::Rejected);
    }

    #[test]
    fn step_display_line() {
        let step = Step::new(
            "Read file",
            "read_file",
            serde_json::json!({"path": "/tmp/test"}),
        );
        let line = step.display_line();
        assert!(line.contains("Read file"));
        assert!(line.contains("read_file"));
        assert!(line.contains("pending"));
    }

    #[test]
    fn file_edit_detection() {
        let read_step = Step::new("Read", "read_file", serde_json::json!({}));
        let write_step = Step::new("Write", "write_file", serde_json::json!({}));
        let patch_step = Step::new("Patch", "patch", serde_json::json!({}));

        assert!(!read_step.is_file_edit());
        assert!(write_step.is_file_edit());
        assert!(patch_step.is_file_edit());
    }

    #[test]
    fn diff_generation_for_file_edits() {
        let step = Step::new(
            "Update config",
            "write_file",
            serde_json::json!({"path": "/tmp/cfg"}),
        );
        let diff = step.diff_summary();
        assert!(diff.is_some());
        assert!(diff.unwrap().contains("write_file"));
    }

    #[test]
    fn format_for_display_shows_steps() {
        let mut mgr = PlanManager::new();
        mgr.create_plan("plan-1", "Display Test", sample_steps());
        mgr.submit_for_approval();
        mgr.approve();

        let lines = mgr.format_for_display();
        assert!(lines.len() > 3);
        assert!(lines[0].contains("Display Test"));
    }

    #[test]
    fn approve_single_step() {
        let mut mgr = PlanManager::new();
        mgr.create_plan("plan-1", "Test", sample_steps());
        mgr.submit_for_approval();

        // Only approve step 1
        mgr.approve_step(1);
        let plan = mgr.current_plan().unwrap();
        assert_eq!(plan.steps[0].status, StepStatus::Pending);
        assert_eq!(plan.steps[1].status, StepStatus::Approved);
        assert_eq!(plan.steps[2].status, StepStatus::Pending);
    }

    #[test]
    fn step_status_icons() {
        assert_eq!(StepStatus::Pending.icon(), "○");
        assert_eq!(StepStatus::Approved.icon(), "✓");
        assert_eq!(StepStatus::Rejected.icon(), "✗");
        assert_eq!(StepStatus::Executing.icon(), "⟳");
        assert_eq!(StepStatus::Completed.icon(), "●");
        assert_eq!(StepStatus::Failed.icon(), "✕");
    }
}
