use providers::Message;
use serde::{Deserialize, Serialize};

/// Default snippet length for extractive summaries (characters).
pub(crate) const DEFAULT_SNIPPET_LENGTH: usize = 200;

/// Compression tier for a conversation turn.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum CompressionTier {
    /// Tier 1: preserves decisions, code refs, action items.
    Detailed,
    /// Tier 2: outcomes only, maximum compression.
    Condensed,
}

/// A single conversation turn: one user message + the assistant response
/// (including tool calls and results) that follows.
#[derive(Debug, Clone)]
pub struct ConversationTurn {
    /// The raw messages in this turn.
    pub messages: Vec<Message>,
    /// Index of this turn in the conversation (0 = oldest).
    pub turn_index: usize,
    /// Estimated token count for all messages in this turn.
    pub token_count: usize,
    /// Cognitive relevance score (0.0–1.0). Filled by `MemoryScorer`.
    pub cognitive_score: Option<f64>,
    /// Which tier this turn was assigned to after scoring.
    pub assigned_tier: Option<AssignedTier>,
}

/// Tier assignment for a turn.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AssignedTier {
    /// Tier 0: kept verbatim.
    Verbatim,
    /// Tier 1: detailed LLM summary.
    Detailed,
    /// Tier 2: condensed gist.
    Condensed,
}

impl ConversationTurn {
    /// Build a lightweight text representation for the cognitive scorer.
    ///
    /// Concatenates the user message + final assistant text + tool result
    /// snippets. Used only for embedding-based scoring.
    pub fn scoring_content(&self) -> String {
        let mut parts = Vec::new();

        // First user message
        for msg in &self.messages {
            if let Message::User { content } = msg {
                let text = match content {
                    providers::UserContent::Text(t) => t.clone(),
                    providers::UserContent::MultiPart(_) => "[multipart]".to_string(),
                };
                parts.push(format!("User: {}", text));
                break;
            }
        }

        // Last assistant message
        for msg in self.messages.iter().rev() {
            if let Message::Assistant {
                content: Some(text),
                ..
            } = msg
            {
                let snippet = if text.len() > 300 {
                    let end = text.floor_char_boundary(300);
                    format!("{}...", &text[..end])
                } else {
                    text.clone()
                };
                parts.push(format!("Assistant: {}", snippet));
                break;
            }
        }

        // Key tool outcomes (first 100 chars of each tool result)
        let tool_snippets: Vec<String> = self
            .messages
            .iter()
            .filter_map(|m| {
                if let Message::Tool { name, content, .. } = m {
                    let text = content.as_text();
                    let snip = if text.len() > 100 {
                        let end = text.floor_char_boundary(100);
                        format!("{}...", &text[..end])
                    } else {
                        text
                    };
                    Some(format!("{}: {}", name, snip))
                } else {
                    None
                }
            })
            .take(3)
            .collect();

        if !tool_snippets.is_empty() {
            parts.push(format!("Tools: {}", tool_snippets.join("; ")));
        }

        parts.join("\n")
    }
}

/// A persisted summary of compressed turns.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TierSummary {
    /// Which tier this summary was compressed at.
    pub tier: CompressionTier,
    /// The summary text.
    pub content: String,
    /// Original turn indices this summary covers (start, end exclusive).
    pub turn_range: (usize, usize),
    /// Estimated token count.
    pub token_count: usize,
    /// Cognitive score at compression time (for demotion decisions).
    pub cognitive_score: Option<f64>,
}

/// Result of tiered history compression.
pub struct CompressedHistory {
    /// Tier 1 + Tier 2 summaries of older turns.
    pub summaries: Vec<TierSummary>,
    /// Tier 0: recent messages kept verbatim.
    pub recent_messages: Vec<Message>,
    /// Preamble system messages (never compressed).
    pub preamble: Vec<Message>,
    /// Estimated total token count across all tiers.
    pub total_tokens: usize,
}

impl CompressedHistory {
    /// Create a verbatim result (no compression needed).
    pub fn verbatim(history: Vec<Message>) -> Self {
        Self {
            summaries: vec![],
            recent_messages: history,
            preamble: vec![],
            total_tokens: 0,
        }
    }
}
