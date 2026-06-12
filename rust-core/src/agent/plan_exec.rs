use serde::{Deserialize, Serialize};

use super::tool::{ToolCall, ToolResult, execute_tool};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum PlanStatus {
    Draft,
    AwaitingApproval,
    Approved,
    Rejected,
    Executing,
    Done,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PlanStep {
    pub description: String,
    pub tool_calls: Vec<ToolCall>,
    pub status: PlanStatus,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ExecutionPlan {
    pub steps: Vec<PlanStep>,
    pub status: PlanStatus,
}

impl ExecutionPlan {
    pub fn draft(steps: Vec<PlanStep>) -> Self {
        Self {
            steps,
            status: PlanStatus::Draft,
        }
    }

    pub fn submit(mut self) -> Self {
        self.status = PlanStatus::AwaitingApproval;
        self
    }

    pub fn approve(&mut self) {
        if self.status == PlanStatus::AwaitingApproval {
            self.status = PlanStatus::Approved;
            for step in &mut self.steps {
                step.status = PlanStatus::Approved;
            }
        }
    }

    pub fn reject(&mut self) {
        if self.status == PlanStatus::AwaitingApproval {
            self.status = PlanStatus::Rejected;
            for step in &mut self.steps {
                step.status = PlanStatus::Rejected;
            }
        }
    }

    pub async fn execute(&mut self) -> Vec<ToolResult> {
        if self.status != PlanStatus::Approved {
            return Vec::new();
        }

        self.status = PlanStatus::Executing;
        let mut results = Vec::new();
        for step in &mut self.steps {
            step.status = PlanStatus::Executing;
            for call in &step.tool_calls {
                results.push(execute_tool(call).await);
            }
            step.status = PlanStatus::Done;
        }
        self.status = PlanStatus::Done;
        results
    }
}

impl PlanStep {
    pub fn new(description: impl Into<String>, tool_calls: Vec<ToolCall>) -> Self {
        Self {
            description: description.into(),
            tool_calls,
            status: PlanStatus::Draft,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn approval_gates_execution() {
        let call = ToolCall {
            id: "call-1".to_string(),
            name: "echo".to_string(),
            arguments: serde_json::json!({"text": "hi"}),
        };
        let mut plan = ExecutionPlan::draft(vec![PlanStep::new("say hi", vec![call])]).submit();

        assert!(plan.execute().await.is_empty());

        plan.approve();
        let results = plan.execute().await;

        assert_eq!(plan.status, PlanStatus::Done);
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].call_id, "call-1");
    }
}
