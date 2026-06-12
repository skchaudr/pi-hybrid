use std::time::Duration;

use tokio::task::JoinHandle;

use super::agent_core::{Agent, AgentConfig, AgentOutput};
use super::message::{Message, Role};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AgentStatus {
    Pending,
    Running,
    Done,
    Cancelled,
    Missing,
}

pub struct SubagentPool {
    agents: Vec<SubagentSlot>,
    max_concurrent: usize,
}

struct SubagentSlot {
    handle: JoinHandle<AgentOutput>,
    status: AgentStatus,
}

impl SubagentPool {
    pub fn new(max: usize) -> Self {
        Self {
            agents: Vec::new(),
            max_concurrent: max.max(1),
        }
    }

    pub async fn spawn(&mut self, config: AgentConfig) -> usize {
        if self.running_count() >= self.max_concurrent {
            return usize::MAX;
        }

        let id = self.agents.len();
        let handle = tokio::spawn(async move {
            let mut agent = Agent::new(config);
            agent.add_message(Message::new(Role::User, format!("subagent-{id}")));
            agent.run().await.unwrap_or_else(|error| AgentOutput {
                content: format!("subagent failed: {error}"),
                completed: false,
                turns: 0,
            })
        });

        self.agents.push(SubagentSlot {
            handle,
            status: AgentStatus::Running,
        });
        id
    }

    pub async fn status(&self, id: usize) -> AgentStatus {
        let Some(slot) = self.agents.get(id) else {
            return AgentStatus::Missing;
        };
        if slot.handle.is_finished() {
            AgentStatus::Done
        } else {
            slot.status
        }
    }

    pub async fn cancel(&mut self, id: usize) {
        if let Some(slot) = self.agents.get_mut(id) {
            slot.handle.abort();
            slot.status = AgentStatus::Cancelled;
        }
    }

    pub async fn await_all(self) -> Vec<AgentOutput> {
        let mut outputs = Vec::new();
        for slot in self.agents {
            if let Ok(output) = slot.handle.await {
                outputs.push(output);
            }
        }
        outputs
    }

    fn running_count(&self) -> usize {
        self.agents
            .iter()
            .filter(|slot| !slot.handle.is_finished() && slot.status == AgentStatus::Running)
            .count()
    }
}

pub async fn sleep_for_test(ms: u64) {
    tokio::time::sleep(Duration::from_millis(ms)).await;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn pool_spawns_and_awaits_multiple_agents() {
        let mut pool = SubagentPool::new(4);
        let first = pool.spawn(AgentConfig::default()).await;
        let second = pool.spawn(AgentConfig::default()).await;

        assert_ne!(first, second);
        assert_eq!(pool.status(first).await, AgentStatus::Running);

        let outputs = pool.await_all().await;

        assert_eq!(outputs.len(), 2);
        assert!(outputs.iter().all(|output| output.completed));
    }
}
