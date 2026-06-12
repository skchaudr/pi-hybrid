//! Context Compaction — summarizes older messages when the context window fills.
//!
//! Strategy:
//! - Track estimated token counts for all messages.
//! - Keep the system prompt + most recent N messages intact.
//! - Summarize older message segments via the bridge LLM.
//! - Replace summarized segments with compact summary messages.

use serde::{Deserialize, Serialize};
use tracing::{debug, trace};

use super::agent_loop::Message;

/// A compacted segment of conversation history.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CompactedSegment {
    /// The original message range that was compacted.
    pub original_range: (usize, usize),
    /// Summary of the compacted messages.
    pub summary: String,
    /// Estimated token count of the summary.
    pub summary_tokens: usize,
    /// Estimated token count of the original messages.
    pub original_tokens: usize,
}

/// Manages context window compaction.
#[derive(Debug)]
pub struct CompactionManager {
    /// Maximum tokens allowed in the context window.
    max_tokens: usize,
    /// Number of recent messages to keep uncompacted.
    keep_recent: usize,
    /// Stored compacted segments.
    segments: Vec<CompactedSegment>,
    /// Whether compaction is currently in progress.
    compacting: bool,
}

impl CompactionManager {
    /// Create a new compaction manager.
    pub fn new(max_tokens: usize) -> Self {
        Self {
            max_tokens,
            keep_recent: 20, // Keep last 20 messages intact
            segments: Vec::new(),
            compacting: false,
        }
    }

    /// Create a compaction manager with a custom keep_recent value.
    pub fn with_recent(max_tokens: usize, keep_recent: usize) -> Self {
        Self {
            max_tokens,
            keep_recent,
            segments: Vec::new(),
            compacting: false,
        }
    }

    /// Estimate tokens for a message using a simple heuristic.
    /// ~4 characters per token for English text.
    pub fn estimate_tokens(text: &str) -> usize {
        // Simple heuristic: ~4 chars per token
        // In a full implementation, this would use a proper tokenizer
        (text.len() / 4).max(1)
    }

    /// Estimate total tokens for a list of messages.
    pub fn estimate_total_tokens(messages: &[Message]) -> usize {
        messages
            .iter()
            .map(|m| {
                let content_tokens = Self::estimate_tokens(&m.content);
                let tool_tokens = m
                    .tool_calls
                    .as_ref()
                    .map(|tc| {
                        tc.iter()
                            .map(|t| {
                                Self::estimate_tokens(&t.name) * 2
                                    + Self::estimate_tokens(
                                        &serde_json::to_string(&t.arguments).unwrap_or_default(),
                                    )
                            })
                            .sum::<usize>()
                    })
                    .unwrap_or(0);
                content_tokens + tool_tokens + 4 // overhead
            })
            .sum()
    }

    /// Check if the conversation needs compaction.
    pub fn needs_compaction(&self, messages: &[Message]) -> bool {
        let estimated = Self::estimate_total_tokens(messages);
        estimated > self.max_tokens
    }

    /// Calculate how many messages need to be compacted.
    pub fn compaction_target(&self, messages: &[Message]) -> Option<usize> {
        if messages.len() <= self.keep_recent {
            return None;
        }

        let total = Self::estimate_total_tokens(messages);
        if total <= self.max_tokens {
            return None;
        }

        Some(messages.len().saturating_sub(self.keep_recent))
    }

    /// Identify the segment of messages to compact.
    /// Returns (start_index, end_index) of messages to summarize,
    /// or None if no compaction is needed.
    pub fn find_compact_segment(&self, messages: &[Message]) -> Option<(usize, usize)> {
        if messages.len() <= self.keep_recent {
            return None;
        }

        if !self.needs_compaction(messages) {
            return None;
        }

        let compact_up_to = self.compaction_target(messages)?;
        if compact_up_to == 0 {
            return None;
        }

        Some((0, compact_up_to))
    }

    /// Build a prompt for the bridge to summarize a segment of messages.
    pub fn build_summarization_prompt(messages: &[Message]) -> String {
        let conversation_text: String = messages
            .iter()
            .map(|m| format!("[{}]: {}", m.role, m.content))
            .collect::<Vec<_>>()
            .join("\n");

        format!(
            "Please summarize the following conversation segment concisely, \
             preserving key decisions, facts, and action items:\n\n{conversation_text}\n\nSummary:"
        )
    }

    /// Create a summary message that replaces compacted messages.
    pub fn create_summary_message(segment: &CompactedSegment) -> Message {
        Message {
            role: "system".to_string(),
            content: format!(
                "[Context Summary — messages {}-{} — {} tokens → {} tokens]: {}",
                segment.original_range.0,
                segment.original_range.1,
                segment.original_tokens,
                segment.summary_tokens,
                segment.summary
            ),
            tool_calls: None,
        }
    }

    /// Compact a conversation by replacing old messages with summaries.
    /// Returns the compacted message list.
    pub fn compact(
        &mut self,
        messages: &[Message],
        summary_fn: impl FnOnce(&[Message]) -> Option<String>,
    ) -> Vec<Message> {
        let Some((start, end)) = self.find_compact_segment(messages) else {
            return messages.to_vec();
        };

        let segment_messages = &messages[start..end];
        let original_tokens = Self::estimate_total_tokens(segment_messages);

        let summary = match summary_fn(segment_messages) {
            Some(s) => s,
            None => {
                // Fallback: simple concatenation-based summary
                let roles: Vec<&str> = segment_messages.iter().map(|m| m.role.as_str()).collect();
                let user_count = roles.iter().filter(|r| **r == "user").count();
                let assistant_count = roles.iter().filter(|r| **r == "assistant").count();
                format!(
                    "[{user_count} user messages and {assistant_count} assistant messages compacted]"
                )
            }
        };

        let summary_tokens = Self::estimate_tokens(&summary);

        let segment = CompactedSegment {
            original_range: (start, end),
            summary: summary.clone(),
            summary_tokens,
            original_tokens,
        };

        self.segments.push(segment.clone());

        // Build compacted message list: summary + recent messages
        let mut compacted = Vec::new();
        compacted.push(Self::create_summary_message(&segment));
        compacted.extend_from_slice(&messages[end..]);

        compacted
    }

    /// Get all compaction segments for auditing.
    pub fn segments(&self) -> &[CompactedSegment] {
        &self.segments
    }

    /// Get the total tokens saved by compaction.
    pub fn tokens_saved(&self) -> usize {
        self.segments
            .iter()
            .map(|s| s.original_tokens.saturating_sub(s.summary_tokens))
            .sum()
    }

    /// Reset compaction state.
    pub fn reset(&mut self) {
        self.segments.clear();
        self.compacting = false;
    }

    /// Set whether compaction is in progress.
    pub fn set_compacting(&mut self, compacting: bool) {
        self.compacting = compacting;
    }

    /// Check if compaction is in progress.
    pub fn is_compacting(&self) -> bool {
        self.compacting
    }
}

impl Default for CompactionManager {
    fn default() -> Self {
        Self::new(200_000)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_messages(count: usize, base_len: usize) -> Vec<Message> {
        (0..count)
            .map(|i| Message {
                role: if i % 2 == 0 { "user" } else { "assistant" }.to_string(),
                content: "x".repeat(base_len),
                tool_calls: None,
            })
            .collect()
    }

    #[test]
    fn estimate_tokens_simple() {
        assert_eq!(CompactionManager::estimate_tokens("hello"), 1); // 5/4 = 1
        assert_eq!(CompactionManager::estimate_tokens("hello world"), 2); // 11/4 = 2
        assert_eq!(CompactionManager::estimate_tokens(""), 1); // max(0, 1)
    }

    #[test]
    fn estimate_total_tokens() {
        let msgs = vec![
            Message {
                role: "user".to_string(),
                content: "hello world".to_string(),
                tool_calls: None,
            },
            Message {
                role: "assistant".to_string(),
                content: "hi there".to_string(),
                tool_calls: None,
            },
        ];
        let tokens = CompactionManager::estimate_total_tokens(&msgs);
        assert!(tokens > 4);
    }

    #[test]
    fn needs_compaction_when_over_limit() {
        let mgr = CompactionManager::new(100); // small limit
        let msgs = make_messages(50, 100); // many big messages
        assert!(mgr.needs_compaction(&msgs));
    }

    #[test]
    fn no_compaction_when_under_limit() {
        let mgr = CompactionManager::new(1_000_000); // huge limit
        let msgs = make_messages(5, 10);
        assert!(!mgr.needs_compaction(&msgs));
    }

    #[test]
    fn find_compact_segment_identifies_range() {
        let mgr = CompactionManager::with_recent(50, 3); // tiny window, keep 3
        let msgs = make_messages(20, 100); // many messages
        let segment = mgr.find_compact_segment(&msgs);
        assert!(segment.is_some());
        let (start, end) = segment.unwrap();
        assert_eq!(start, 0);
        assert!(end <= 17); // 20 - 3 = 17
    }

    #[test]
    fn no_compact_when_few_messages() {
        let mgr = CompactionManager::with_recent(100, 5);
        let msgs = make_messages(3, 10); // fewer than keep_recent
        assert!(mgr.find_compact_segment(&msgs).is_none());
    }

    #[test]
    fn compact_replaces_old_messages() {
        let mut mgr = CompactionManager::with_recent(30, 2);
        let msgs = make_messages(10, 100);

        let compacted = mgr.compact(&msgs, |_| Some("Summary of old messages".to_string()));

        // Should have: 1 summary message + 2 recent messages = 3 total
        assert!(compacted.len() < msgs.len());
        assert_eq!(compacted[0].role, "system");
        assert!(compacted[0].content.contains("Summary"));
    }

    #[test]
    fn compact_fallback_summary() {
        let mut mgr = CompactionManager::with_recent(30, 2);
        let msgs = make_messages(10, 100);

        let compacted = mgr.compact(&msgs, |_| None); // None = use fallback

        assert!(compacted[0].content.contains("compacted"));
    }

    #[test]
    fn tokens_saved_tracks_savings() {
        let mut mgr = CompactionManager::with_recent(30, 2);
        let msgs = make_messages(10, 100);

        mgr.compact(&msgs, |_| Some("short summary".to_string()));
        assert!(mgr.tokens_saved() > 0);
    }

    #[test]
    fn reset_clears_segments() {
        let mut mgr = CompactionManager::with_recent(30, 2);
        let msgs = make_messages(10, 100);
        mgr.compact(&msgs, |_| Some("summary".to_string()));
        assert!(!mgr.segments().is_empty());

        mgr.reset();
        assert!(mgr.segments().is_empty());
    }

    #[test]
    fn summary_message_format() {
        let segment = CompactedSegment {
            original_range: (0, 10),
            summary: "test summary".to_string(),
            summary_tokens: 3,
            original_tokens: 100,
        };
        let msg = CompactionManager::create_summary_message(&segment);
        assert_eq!(msg.role, "system");
        assert!(msg.content.contains("test summary"));
        assert!(msg.content.contains("100 tokens"));
    }

    #[test]
    fn build_summarization_prompt_formats_messages() {
        let msgs = vec![
            Message {
                role: "user".to_string(),
                content: "hello".to_string(),
                tool_calls: None,
            },
            Message {
                role: "assistant".to_string(),
                content: "hi".to_string(),
                tool_calls: None,
            },
        ];
        let prompt = CompactionManager::build_summarization_prompt(&msgs);
        assert!(prompt.contains("[user]: hello"));
        assert!(prompt.contains("[assistant]: hi"));
        assert!(prompt.contains("summarize"));
    }
}
