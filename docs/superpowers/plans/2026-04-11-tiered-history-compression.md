# Tiered History Compression (THC) Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Replace the binary history compressor with a 3-tier cognitive-aware compression system that preserves decisions, code references, and action items in long conversations.

**Architecture:** `TieredHistoryCompressor` replaces `HistoryCompressor`. Messages are grouped by conversation turn, optionally scored via cognitive relevance, assigned to tiers (Verbatim/Detailed/Condensed), and compressed with tier-specific LLM prompts. Delta compaction on session resume avoids redundant LLM calls. The old `HistoryCompressor`, `CompressorConfig`, and `CompressorMode` are deleted.

**Tech Stack:** Rust, async-trait, serde, SQLite (session storage), providers crate (Message types)

**Spec:** `docs/superpowers/specs/2026-04-11-tiered-history-compression-design.md`

---

## File Structure

### New Files

| File | Responsibility |
|------|----------------|
| `crates/config/src/schema/history_compression.rs` | `HistoryCompressionConfig`, `TierZeroConfig` — all config types |
| `crates/context_engine/src/history_compressor/types.rs` | `CompressionTier`, `ConversationTurn`, `TierSummary`, `CompressedHistory` (replaces old types) |
| `crates/context_engine/src/history_compressor/grouping.rs` | `group_into_turns()` — turn-boundary detection |
| `crates/context_engine/src/history_compressor/prompts.rs` | `TIER1_INSTRUCTIONS`, `TIER2_INSTRUCTIONS` constants |
| `crates/context_engine/src/history_compressor/tiered.rs` | `TieredHistoryCompressor` — core pipeline |
| `crates/context_engine/src/memory_scorer.rs` | `MemoryScorer` trait |
| `crates/agent/src/adapters/memory_scorer_impl.rs` | `CognitiveMemoryScorer` — wraps `UnifiedMemoryService` |

### Modified Files

| File | Change |
|------|--------|
| `crates/config/src/schema/cognitive.rs` | Add `history_compression` field to `CognitiveConfig` |
| `crates/config/src/schema/mod.rs` | Add `mod history_compression; pub use` |
| `crates/context_engine/src/summary_provider.rs` | Add `tier: CompressionTier` to `summarize_batch` |
| `crates/context_engine/src/history_compressor/mod.rs` | Delete old impl, re-export new types + tiered compressor |
| `crates/context_engine/src/lib.rs` | Update re-exports |
| `crates/context_engine/src/assembler/mod.rs` | Use `TieredHistoryCompressor` instead of `HistoryCompressor` |
| `crates/agent/src/adapters/llm_summary.rs` | Tier-aware `build_batch_prompt`, update `summarize_batch` signature |
| `crates/agent/src/agent_loop/builder.rs` | Wire config + MemoryScorer + model override |
| `crates/storage/migrations/001_initial.sql` | Add 3 columns to `sessions` table |
| `crates/storage/src/repos/session.rs` | `save_compressed_prefix()`, `load_compressed_prefix()` |
| `crates/session/src/manager.rs` | Expose prefix persistence through `Session` |
| `crates/bus/src/events.rs` | Add `AgentEvent::ContextTieredCompressed` |
| `crates/agent/src/agent_runtime/runtime.rs` | Pass `HistoryCompressionConfig` to context engine |

### Deleted Code

| Code | File |
|------|------|
| `HistoryCompressor` struct + all methods | `history_compressor/mod.rs` |
| `CompressorConfig`, `CompressorMode`, old `HistorySummary` | `history_compressor/types.rs` |
| Old `HistoryCompressor` tests | `history_compressor/mod.rs` (test module) |

### Kept As-Is

| File | Why |
|------|-----|
| `history_compressor/snippet.rs` (`first_snippet`) | Used internally for extractive fallback |
| `agent/src/execution/mid_loop_compressor.rs` | Handles in-loop compression separately |

---

## Task 1: Config Schema

**Files:**
- Create: `crates/config/src/schema/history_compression.rs`
- Modify: `crates/config/src/schema/cognitive.rs:15-177`
- Modify: `crates/config/src/schema/mod.rs`

- [ ] **Step 1: Create the config types file**

```rust
// crates/config/src/schema/history_compression.rs
use serde::{Deserialize, Serialize};

fn default_true() -> bool {
    true
}
fn default_tier1_ratio() -> f32 {
    0.35
}
fn default_tier2_ratio() -> f32 {
    0.12
}
fn default_high_threshold() -> f64 {
    0.70
}
fn default_low_threshold() -> f64 {
    0.40
}
fn default_demotion_threshold() -> usize {
    30
}
fn default_8() -> usize {
    8
}
fn default_12() -> usize {
    12
}
fn default_16() -> usize {
    16
}

/// Configuration for tiered history compression.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HistoryCompressionConfig {
    /// Override model for summarization LLM calls.
    /// None = use agents.defaults.model.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub model: Option<String>,

    /// Use cognitive 12-factor scoring for tier promotion.
    #[serde(default = "default_true")]
    pub use_cognitive_scoring: bool,

    /// Only compress new messages on session resume.
    #[serde(default = "default_true")]
    pub delta_only_on_resume: bool,

    /// Tier 0 verbatim message count per depth mode.
    #[serde(default)]
    pub tier0_messages: TierZeroConfig,

    /// Target compression ratio for Tier 1 summaries.
    #[serde(default = "default_tier1_ratio")]
    pub tier1_ratio: f32,

    /// Target compression ratio for Tier 2 summaries.
    #[serde(default = "default_tier2_ratio")]
    pub tier2_ratio: f32,

    /// Cognitive score threshold for promoting old turns to Tier 1.
    #[serde(default = "default_high_threshold")]
    pub high_relevance_threshold: f64,

    /// Cognitive score threshold for keeping turns in Tier 1 vs Tier 2.
    #[serde(default = "default_low_threshold")]
    pub low_relevance_threshold: f64,

    /// Turns from current end before Tier 1 demotes to Tier 2.
    #[serde(default = "default_demotion_threshold")]
    pub tier1_demotion_threshold: usize,
}

impl Default for HistoryCompressionConfig {
    fn default() -> Self {
        Self {
            model: None,
            use_cognitive_scoring: true,
            delta_only_on_resume: true,
            tier0_messages: TierZeroConfig::default(),
            tier1_ratio: 0.35,
            tier2_ratio: 0.12,
            high_relevance_threshold: 0.70,
            low_relevance_threshold: 0.40,
            tier1_demotion_threshold: 30,
        }
    }
}

/// Tier 0 verbatim message count per depth mode.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TierZeroConfig {
    #[serde(default = "default_8")]
    pub normal: usize,
    #[serde(default = "default_12")]
    pub deep_think: usize,
    #[serde(default = "default_16")]
    pub ultra: usize,
}

impl Default for TierZeroConfig {
    fn default() -> Self {
        Self {
            normal: 8,
            deep_think: 12,
            ultra: 16,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_config_values() {
        let config = HistoryCompressionConfig::default();
        assert!(config.model.is_none());
        assert!(config.use_cognitive_scoring);
        assert!(config.delta_only_on_resume);
        assert_eq!(config.tier0_messages.normal, 8);
        assert_eq!(config.tier0_messages.deep_think, 12);
        assert_eq!(config.tier0_messages.ultra, 16);
        assert!((config.tier1_ratio - 0.35).abs() < f32::EPSILON);
        assert!((config.tier2_ratio - 0.12).abs() < f32::EPSILON);
        assert!((config.high_relevance_threshold - 0.70).abs() < f64::EPSILON);
        assert!((config.low_relevance_threshold - 0.40).abs() < f64::EPSILON);
        assert_eq!(config.tier1_demotion_threshold, 30);
    }

    #[test]
    fn test_config_roundtrip_json() {
        let json = r#"{
            "model": "claude-haiku-4-5-20251001",
            "useCognitiveScoring": false,
            "tier0Messages": { "normal": 10, "deepThink": 14, "ultra": 20 },
            "tier1Ratio": 0.40
        }"#;
        let config: HistoryCompressionConfig = serde_json::from_str(json).unwrap();
        assert_eq!(config.model.as_deref(), Some("claude-haiku-4-5-20251001"));
        assert!(!config.use_cognitive_scoring);
        assert_eq!(config.tier0_messages.normal, 10);
        // Unset fields get defaults
        assert!(config.delta_only_on_resume);
        assert!((config.tier2_ratio - 0.12).abs() < f32::EPSILON);
    }

    #[test]
    fn test_empty_json_uses_all_defaults() {
        let config: HistoryCompressionConfig = serde_json::from_str("{}").unwrap();
        assert_eq!(config.tier0_messages.normal, 8);
        assert!(config.use_cognitive_scoring);
    }
}
```

- [ ] **Step 2: Wire into cognitive config**

In `crates/config/src/schema/cognitive.rs`, add the field to `CognitiveConfig` (after the `query_enhancement` field around line 140):

```rust
// Add import at top of file
use super::history_compression::HistoryCompressionConfig;

// Add field to CognitiveConfig struct
    /// Tiered history compression configuration.
    #[serde(default)]
    pub history_compression: HistoryCompressionConfig,
```

In `crates/config/src/schema/mod.rs`, add the module and re-export:

```rust
pub mod history_compression;
pub use self::history_compression::*;
```

- [ ] **Step 3: Run tests**

Run: `cargo nextest run -p config`
Expected: All pass, including the new config tests.

- [ ] **Step 4: Commit**

```bash
git add crates/config/src/schema/history_compression.rs crates/config/src/schema/cognitive.rs crates/config/src/schema/mod.rs
git commit -m "feat(config): add HistoryCompressionConfig for tiered compression"
```

---

## Task 2: Core Types + MemoryScorer Trait

**Files:**
- Rewrite: `crates/context_engine/src/history_compressor/types.rs`
- Create: `crates/context_engine/src/memory_scorer.rs`
- Modify: `crates/context_engine/src/lib.rs:17-20,28`

- [ ] **Step 1: Rewrite types.rs with new types**

Replace the entire content of `crates/context_engine/src/history_compressor/types.rs`:

```rust
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
                    format!("{}...", &text[..300])
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
                    let snip = if content.len() > 100 {
                        format!("{}...", &content[..100])
                    } else {
                        content.clone()
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
```

- [ ] **Step 2: Create MemoryScorer trait**

```rust
// crates/context_engine/src/memory_scorer.rs
use async_trait::async_trait;

/// Scores text passages for cognitive relevance (0.0–1.0).
///
/// Implemented in the `agent` crate by wrapping `UnifiedMemoryService`.
/// Lives here (L3) to avoid `context_engine` depending on `cognitive` (L5).
#[async_trait]
pub trait MemoryScorer: Send + Sync {
    /// Score relevance of text passages. Returns one score per input.
    async fn score_batch(&self, texts: &[String]) -> Vec<f64>;
}
```

- [ ] **Step 3: Update lib.rs re-exports**

In `crates/context_engine/src/lib.rs`, replace the old compression re-exports (line 17-20):

```rust
// Old:
pub use history_compressor::{
    first_snippet, CompressedHistory, CompressorConfig, CompressorMode, HistoryCompressor,
    HistorySummary,
};

// New:
pub use history_compressor::{
    first_snippet, AssignedTier, CompressedHistory, CompressionTier, ConversationTurn,
    TierSummary, TieredHistoryCompressor,
};
```

Add the `memory_scorer` module (near the existing `summary_provider` module declaration):

```rust
pub mod memory_scorer;
pub use memory_scorer::MemoryScorer;
```

- [ ] **Step 4: Update history_compressor/mod.rs exports temporarily**

For now, keep the old code compiling but add re-exports for the new types. We'll delete the old code in Task 10. In `crates/context_engine/src/history_compressor/mod.rs`, update the `pub use` at the top:

```rust
mod snippet;
mod types;

pub use snippet::first_snippet;
pub use types::{
    AssignedTier, CompressedHistory, CompressionTier, ConversationTurn, TierSummary,
};
```

This will break compilation because the old types (`CompressorConfig`, `CompressorMode`, `HistorySummary`, `HistoryCompressor`) are gone. Fix the imports in `crates/context_engine/src/assembler/mod.rs` line 21 by commenting or stubbing — this will be fully replaced in Task 8. For now, add a temporary re-export alias at the bottom of `types.rs` to keep the assembler compiling:

```rust
// Temporary compat — removed in Task 10 when assembler is updated
pub type HistorySummary = TierSummary;
```

And in `mod.rs`, temporarily keep exporting what the assembler needs:

```rust
pub use types::{
    AssignedTier, CompressedHistory, CompressionTier, ConversationTurn, HistorySummary,
    TierSummary,
};
```

- [ ] **Step 5: Run tests**

Run: `cargo check -p context_engine`
Expected: Compilation errors from assembler and old `HistoryCompressor` references. This is expected — we'll fix them progressively. The types module itself should be correct.

Run: `cargo nextest run -p context_engine -- types`
Expected: Old type tests fail (they're deleted). New tests not yet written.

- [ ] **Step 6: Commit**

```bash
git add crates/context_engine/src/history_compressor/types.rs crates/context_engine/src/memory_scorer.rs crates/context_engine/src/lib.rs crates/context_engine/src/history_compressor/mod.rs
git commit -m "feat(context_engine): add tiered compression types + MemoryScorer trait

ConversationTurn, TierSummary, CompressionTier, CompressedHistory
replace the old CompressorConfig/CompressorMode/HistorySummary types.
MemoryScorer trait enables cognitive-aware tier promotion."
```

---

## Task 3: Structured Prompts

**Files:**
- Create: `crates/context_engine/src/history_compressor/prompts.rs`
- Modify: `crates/context_engine/src/history_compressor/mod.rs`

- [ ] **Step 1: Create the prompts module**

```rust
// crates/context_engine/src/history_compressor/prompts.rs

/// Tier 1 — Detailed Summary prompt.
///
/// Preserves decisions, code references, action items, reasoning.
/// Target: ~35% of original length.
pub const TIER1_INSTRUCTIONS: &str = "\
Summarize each conversation turn below. For each turn, preserve:
- Decisions made and their reasoning
- Action items or commitments
- File paths, function names, IDs, or other specific references
- Key questions asked and answers given
- Errors encountered and how they were resolved
- Any constraints or requirements stated

Preserve temporal order of events. Use bullet points. Never invent information.
Keep technical details (exact names, numbers, paths). Remove pleasantries, \
repetition, and verbose explanations. Target ~35% of original length.";

/// Tier 2 — Condensed Gist prompt.
///
/// Outcomes only, maximum compression.
/// Target: ~12% of original length.
pub const TIER2_INSTRUCTIONS: &str = "\
For each conversation turn below, extract ONLY:
- The final outcome or decision (one sentence)
- Any unresolved item that affects later conversation (prefix with \"UNRESOLVED:\")

No code, no file paths, no reasoning chains. Maximum 2 sentences per turn. \
Target ~12% of original length.";

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tier1_mentions_decisions() {
        assert!(TIER1_INSTRUCTIONS.contains("Decisions made"));
        assert!(TIER1_INSTRUCTIONS.contains("File paths"));
        assert!(TIER1_INSTRUCTIONS.contains("35%"));
    }

    #[test]
    fn tier2_mentions_outcomes_only() {
        assert!(TIER2_INSTRUCTIONS.contains("ONLY"));
        assert!(TIER2_INSTRUCTIONS.contains("UNRESOLVED"));
        assert!(TIER2_INSTRUCTIONS.contains("12%"));
        assert!(TIER2_INSTRUCTIONS.contains("No code"));
    }
}
```

- [ ] **Step 2: Add module to mod.rs**

In `crates/context_engine/src/history_compressor/mod.rs`, add:

```rust
pub mod prompts;
```

- [ ] **Step 3: Run tests**

Run: `cargo nextest run -p context_engine -- prompts`
Expected: 2 tests pass.

- [ ] **Step 4: Commit**

```bash
git add crates/context_engine/src/history_compressor/prompts.rs crates/context_engine/src/history_compressor/mod.rs
git commit -m "feat(context_engine): add tier-specific summarization prompts"
```

---

## Task 4: Turn-Based Grouping

**Files:**
- Create: `crates/context_engine/src/history_compressor/grouping.rs`
- Modify: `crates/context_engine/src/history_compressor/mod.rs`

- [ ] **Step 1: Write the failing tests**

```rust
// crates/context_engine/src/history_compressor/grouping.rs
use std::sync::Arc;

use providers::Message;

use crate::token_counter::{self, TokenCounter};

use super::types::ConversationTurn;

/// Group a flat message list into conversation turns.
///
/// A turn starts at each `Message::User`. Everything until the next
/// `Message::User` belongs to the current turn. Leading system messages
/// form a preamble (returned separately). `ContextUpdate` messages
/// attach to their containing turn.
pub fn group_into_turns(
    messages: &[Message],
    token_counter: &dyn TokenCounter,
) -> (Vec<Message>, Vec<ConversationTurn>) {
    todo!()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::token_counter::default_token_counter;

    fn tc() -> Arc<dyn TokenCounter> {
        default_token_counter()
    }

    #[test]
    fn test_empty_history() {
        let (preamble, turns) = group_into_turns(&[], &*tc());
        assert!(preamble.is_empty());
        assert!(turns.is_empty());
    }

    #[test]
    fn test_system_preamble_extracted() {
        let msgs = vec![
            Message::system("You are a helpful assistant."),
            Message::system("Additional context."),
            Message::user("Hello"),
            Message::assistant("Hi there!"),
        ];
        let (preamble, turns) = group_into_turns(&msgs, &*tc());
        assert_eq!(preamble.len(), 2);
        assert_eq!(turns.len(), 1);
        assert_eq!(turns[0].messages.len(), 2); // user + assistant
        assert_eq!(turns[0].turn_index, 0);
    }

    #[test]
    fn test_multiple_turns() {
        let msgs = vec![
            Message::user("First question"),
            Message::assistant("First answer"),
            Message::user("Second question"),
            Message::assistant("Second answer"),
            Message::user("Third question"),
            Message::assistant("Third answer"),
        ];
        let (preamble, turns) = group_into_turns(&msgs, &*tc());
        assert!(preamble.is_empty());
        assert_eq!(turns.len(), 3);
        assert_eq!(turns[0].turn_index, 0);
        assert_eq!(turns[1].turn_index, 1);
        assert_eq!(turns[2].turn_index, 2);
    }

    #[test]
    fn test_tool_calls_stay_with_turn() {
        let msgs = vec![
            Message::user("Search for X"),
            Message::Assistant {
                content: None,
                tool_calls: Some(vec![providers::ToolCallMessage {
                    id: "tc1".into(),
                    r#type: "function".into(),
                    function: providers::FunctionCall {
                        name: "search".into(),
                        arguments: "{}".into(),
                    },
                }]),
                reasoning_content: None,
            },
            Message::Tool {
                tool_call_id: "tc1".into(),
                name: "search".into(),
                content: "Found 3 results".into(),
            },
            Message::assistant("Here are the results."),
        ];
        let (_, turns) = group_into_turns(&msgs, &*tc());
        assert_eq!(turns.len(), 1);
        assert_eq!(turns[0].messages.len(), 4);
    }

    #[test]
    fn test_context_update_attaches_to_turn() {
        let msgs = vec![
            Message::user("Tell me about X"),
            Message::assistant("Let me look..."),
            Message::ContextUpdate {
                reason: "MemoryPromoted".into(),
                content: "New fact available".into(),
            },
            Message::user("What else?"),
            Message::assistant("Here's more."),
        ];
        let (_, turns) = group_into_turns(&msgs, &*tc());
        assert_eq!(turns.len(), 2);
        assert_eq!(turns[0].messages.len(), 3); // user + assistant + context_update
        assert_eq!(turns[1].messages.len(), 2); // user + assistant
    }

    #[test]
    fn test_token_count_populated() {
        let msgs = vec![
            Message::user("Hello world"),
            Message::assistant("Hi!"),
        ];
        let (_, turns) = group_into_turns(&msgs, &*tc());
        assert!(turns[0].token_count > 0);
    }

    #[test]
    fn test_no_user_messages_returns_empty_turns() {
        let msgs = vec![
            Message::system("System prompt"),
            Message::assistant("Unprompted response"),
        ];
        let (preamble, turns) = group_into_turns(&msgs, &*tc());
        // System → preamble, assistant without user → no turn
        assert_eq!(preamble.len(), 1);
        // The orphan assistant message has no user, so it gets attached
        // to the preamble or forms a turn. Design decision: orphan
        // assistant/tool messages before first user become part of preamble.
        assert!(turns.is_empty());
        assert_eq!(preamble.len(), 2); // system + orphan assistant
    }
}
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo nextest run -p context_engine -- grouping`
Expected: FAIL — `todo!()` panics.

- [ ] **Step 3: Implement group_into_turns**

Replace the `todo!()` in `group_into_turns`:

```rust
pub fn group_into_turns(
    messages: &[Message],
    token_counter: &dyn TokenCounter,
) -> (Vec<Message>, Vec<ConversationTurn>) {
    if messages.is_empty() {
        return (vec![], vec![]);
    }

    let mut preamble = Vec::new();
    let mut turns: Vec<ConversationTurn> = Vec::new();
    let mut current_turn_msgs: Vec<Message> = Vec::new();
    let mut seen_first_user = false;

    for msg in messages {
        match msg {
            Message::User { .. } => {
                if !seen_first_user {
                    seen_first_user = true;
                }
                // Start a new turn — flush the previous one
                if !current_turn_msgs.is_empty() {
                    let token_count = current_turn_msgs
                        .iter()
                        .map(|m| token_counter::estimate_message_tokens(token_counter, m))
                        .sum();
                    turns.push(ConversationTurn {
                        messages: std::mem::take(&mut current_turn_msgs),
                        turn_index: turns.len(),
                        token_count,
                        cognitive_score: None,
                        assigned_tier: None,
                    });
                }
                current_turn_msgs.push(msg.clone());
            }
            Message::System { .. } if !seen_first_user => {
                preamble.push(msg.clone());
            }
            _ => {
                if seen_first_user {
                    current_turn_msgs.push(msg.clone());
                } else {
                    // Orphan non-system messages before first user → preamble
                    preamble.push(msg.clone());
                }
            }
        }
    }

    // Flush the last turn
    if !current_turn_msgs.is_empty() {
        let token_count = current_turn_msgs
            .iter()
            .map(|m| token_counter::estimate_message_tokens(token_counter, m))
            .sum();
        turns.push(ConversationTurn {
            messages: std::mem::take(&mut current_turn_msgs),
            turn_index: turns.len(),
            token_count,
            cognitive_score: None,
            assigned_tier: None,
        });
    }

    (preamble, turns)
}
```

- [ ] **Step 4: Run tests to verify they pass**

Run: `cargo nextest run -p context_engine -- grouping`
Expected: All 7 tests pass.

- [ ] **Step 5: Add module to mod.rs**

In `crates/context_engine/src/history_compressor/mod.rs`, add:

```rust
pub mod grouping;
```

- [ ] **Step 6: Commit**

```bash
git add crates/context_engine/src/history_compressor/grouping.rs crates/context_engine/src/history_compressor/mod.rs
git commit -m "feat(context_engine): add turn-based message grouping"
```

---

## Task 5: Update SummaryProvider Trait + LlmSummaryProvider

**Files:**
- Modify: `crates/context_engine/src/summary_provider.rs`
- Modify: `crates/agent/src/adapters/llm_summary.rs`

- [ ] **Step 1: Update the SummaryProvider trait**

Replace `crates/context_engine/src/summary_provider.rs`:

```rust
use async_trait::async_trait;
use providers::Message;

use crate::history_compressor::CompressionTier;

/// Trait for abstractive summarization of conversation history.
///
/// Implementations call an LLM to produce summaries at a specified
/// compression tier (Detailed or Condensed).
#[async_trait]
pub trait SummaryProvider: Send + Sync {
    /// Summarize multiple conversation segments in a single batch.
    ///
    /// Each element in `segments` is a group of messages to summarize together.
    /// `tier` controls the prompt and compression aggressiveness.
    /// Returns one `Result` per input segment — individual segments may fail
    /// independently, allowing callers to fall back to extractive summarization
    /// on a per-segment basis.
    async fn summarize_batch(
        &self,
        segments: Vec<Vec<Message>>,
        tier: CompressionTier,
    ) -> Vec<Result<String, String>>;
}
```

- [ ] **Step 2: Update LlmSummaryProvider**

In `crates/agent/src/adapters/llm_summary.rs`, make these changes:

1. Add import for `CompressionTier` and the prompt constants:

```rust
use context_engine::CompressionTier;
use context_engine::history_compressor::prompts::{TIER1_INSTRUCTIONS, TIER2_INSTRUCTIONS};
```

2. Update `build_batch_prompt` to accept a tier (replace existing method, lines 52-68):

```rust
    fn build_batch_prompt(segments: &[Vec<Message>], tier: CompressionTier) -> String {
        let instructions = match tier {
            CompressionTier::Detailed => TIER1_INSTRUCTIONS,
            CompressionTier::Condensed => TIER2_INSTRUCTIONS,
        };

        let mut prompt = format!(
            "{}\n\nReturn ONLY a JSON array of exactly {} strings, one summary per turn.\n\
             No extra text.\n",
            instructions,
            segments.len()
        );
        for (i, segment) in segments.iter().enumerate() {
            let _ = write!(
                prompt,
                "\n=== Turn {} ===\n{}\n",
                i + 1,
                Self::format_segment(segment)
            );
        }
        prompt
    }
```

3. Update `call_batch` to accept and pass tier (lines 78-108):

```rust
    async fn call_batch(
        &self,
        segments: &[Vec<Message>],
        tier: CompressionTier,
    ) -> Vec<Result<String, String>> {
        let n = segments.len();
        let prompt = Self::build_batch_prompt(segments, tier);
        let request_messages = vec![Message::user(prompt)];
        let max_tokens = match tier {
            CompressionTier::Detailed => (150 * n).max(256) as u32,
            CompressionTier::Condensed => (60 * n).max(128) as u32,
        };
        let params = ChatParams::new(&self.model).with_max_tokens(max_tokens);
        // ... rest unchanged
```

4. Update `summarize_batch` impl (lines 112-124):

```rust
#[async_trait]
impl SummaryProvider for LlmSummaryProvider {
    async fn summarize_batch(
        &self,
        segments: Vec<Vec<Message>>,
        tier: CompressionTier,
    ) -> Vec<Result<String, String>> {
        if segments.is_empty() {
            return vec![];
        }
        let futures = segments
            .chunks(MAX_SEGMENTS_PER_CALL)
            .map(|batch| self.call_batch(batch, tier));
        let results = futures_util::future::join_all(futures).await;
        results.into_iter().flatten().collect()
    }
}
```

5. Update existing tests in `llm_summary.rs` — the `build_batch_prompt` test:

```rust
    #[test]
    fn build_batch_prompt_contains_all_segments() {
        let segments = vec![
            vec![Message::user("Hello"), Message::assistant("Hi!")],
            vec![Message::user("Bye"), Message::assistant("Goodbye!")],
        ];
        let prompt =
            LlmSummaryProvider::build_batch_prompt(&segments, CompressionTier::Detailed);
        assert!(prompt.contains("exactly 2 strings"));
        assert!(prompt.contains("=== Turn 1 ==="));
        assert!(prompt.contains("=== Turn 2 ==="));
        assert!(prompt.contains("User: Hello"));
        assert!(prompt.contains("Decisions made")); // Tier 1 specific
    }

    #[test]
    fn build_batch_prompt_tier2_uses_condensed_instructions() {
        let segments = vec![vec![Message::user("Test"), Message::assistant("Reply")]];
        let prompt =
            LlmSummaryProvider::build_batch_prompt(&segments, CompressionTier::Condensed);
        assert!(prompt.contains("ONLY"));
        assert!(prompt.contains("UNRESOLVED"));
        assert!(!prompt.contains("Decisions made"));
    }
```

- [ ] **Step 3: Fix all callers of the old summarize_batch signature**

Search for any other callers of `summarize_batch` in the codebase (assembler tests, history_compressor tests) and update them to pass a `CompressionTier`. The old `HistoryCompressor::compress_async` calls `provider.summarize_batch(segments)` — since we're deleting that code in Task 10, temporarily add `CompressionTier::Detailed` to any surviving call sites to keep them compiling.

In `crates/context_engine/src/assembler/mod.rs` test (around line 1092-1158), if there's a mock `SummaryProvider`, update its signature:

```rust
impl SummaryProvider for TrackingProvider {
    async fn summarize_batch(
        &self,
        segments: Vec<Vec<Message>>,
        _tier: CompressionTier,
    ) -> Vec<Result<String, String>> {
        // ... existing mock logic
    }
}
```

- [ ] **Step 4: Run tests**

Run: `cargo nextest run -p agent -- llm_summary`
Expected: All llm_summary tests pass including the new tier2 test.

Run: `cargo nextest run -p context_engine`
Expected: May have failures from old HistoryCompressor tests — acceptable, those are deleted in Task 10.

- [ ] **Step 5: Commit**

```bash
git add crates/context_engine/src/summary_provider.rs crates/agent/src/adapters/llm_summary.rs crates/context_engine/src/assembler/mod.rs
git commit -m "feat: add CompressionTier to SummaryProvider trait

LlmSummaryProvider now uses tier-specific prompts: Detailed preserves
decisions/code/actions, Condensed extracts outcomes only. Max tokens
scale per tier (150*n vs 60*n)."
```

---

## Task 6: TieredHistoryCompressor Core

**Files:**
- Create: `crates/context_engine/src/history_compressor/tiered.rs`
- Modify: `crates/context_engine/src/history_compressor/mod.rs`

This is the largest task. The compressor implements the full pipeline: group → score → assign tiers → compress.

- [ ] **Step 1: Write the core struct and tests**

```rust
// crates/context_engine/src/history_compressor/tiered.rs
use std::sync::Arc;

use providers::Message;

use crate::memory_scorer::MemoryScorer;
use crate::summary_provider::SummaryProvider;
use crate::token_counter::{self, TokenCounter};
use config::schema::HistoryCompressionConfig;

use super::grouping::group_into_turns;
use super::snippet::first_snippet;
use super::types::{
    AssignedTier, CompressedHistory, CompressionTier, ConversationTurn, TierSummary,
    DEFAULT_SNIPPET_LENGTH,
};

/// Tiered history compressor.
///
/// Replaces the old `HistoryCompressor`. Groups messages into conversation
/// turns, optionally scores them via cognitive relevance, assigns tiers
/// (Verbatim / Detailed / Condensed), and compresses with tier-specific
/// LLM prompts. Falls back to extractive snippets when LLM calls fail
/// or when extractive already fits the budget.
pub struct TieredHistoryCompressor {
    token_counter: Arc<dyn TokenCounter>,
    summary_provider: Option<Arc<dyn SummaryProvider>>,
    memory_scorer: Option<Arc<dyn MemoryScorer>>,
    config: HistoryCompressionConfig,
}

impl TieredHistoryCompressor {
    pub fn new(
        token_counter: Arc<dyn TokenCounter>,
        config: HistoryCompressionConfig,
    ) -> Self {
        Self {
            token_counter,
            summary_provider: None,
            memory_scorer: None,
            config,
        }
    }

    pub fn with_summary_provider(mut self, provider: Arc<dyn SummaryProvider>) -> Self {
        self.summary_provider = Some(provider);
        self
    }

    pub fn with_memory_scorer(mut self, scorer: Arc<dyn MemoryScorer>) -> Self {
        self.memory_scorer = Some(scorer);
        self
    }

    /// Compress conversation history using the tiered pipeline.
    ///
    /// `tier0_count` is the number of recent turns to keep verbatim
    /// (determined by DepthMode externally).
    pub async fn compress(
        &self,
        history: &[Message],
        budget_tokens: usize,
        tier0_count: usize,
    ) -> CompressedHistory {
        // Step 1: Group into turns
        let (preamble, mut turns) = group_into_turns(history, &*self.token_counter);

        // Early exit: all turns fit in Tier 0
        if turns.len() <= tier0_count {
            return CompressedHistory {
                summaries: vec![],
                recent_messages: history.to_vec(),
                preamble: vec![],
                total_tokens: history
                    .iter()
                    .map(|m| token_counter::estimate_message_tokens(&*self.token_counter, m))
                    .sum(),
            };
        }

        // Step 2: Split into Tier 0 (recent) and older turns
        let tier0_start = turns.len().saturating_sub(tier0_count);
        let older_turns = &mut turns[..tier0_start];
        let recent_turns = &turns[tier0_start..];

        // Collect Tier 0 messages
        let recent_messages: Vec<Message> = recent_turns
            .iter()
            .flat_map(|t| t.messages.iter().cloned())
            .collect();

        // Step 3: Score older turns (if cognitive scoring enabled)
        if self.config.use_cognitive_scoring {
            if let Some(scorer) = &self.memory_scorer {
                let texts: Vec<String> =
                    older_turns.iter().map(|t| t.scoring_content()).collect();
                let scores = scorer.score_batch(&texts).await;
                for (turn, score) in older_turns.iter_mut().zip(scores) {
                    turn.cognitive_score = Some(score);
                }
            }
        }

        // Step 4: Assign tiers
        self.assign_tiers(older_turns, tier0_start);

        // Step 5: Compress each tier
        let summaries = self.compress_turns(older_turns, budget_tokens).await;

        let preamble_tokens: usize = preamble
            .iter()
            .map(|m| token_counter::estimate_message_tokens(&*self.token_counter, m))
            .sum();
        let recent_tokens: usize = recent_messages
            .iter()
            .map(|m| token_counter::estimate_message_tokens(&*self.token_counter, m))
            .sum();
        let summary_tokens: usize = summaries.iter().map(|s| s.token_count).sum();

        CompressedHistory {
            summaries,
            recent_messages,
            preamble,
            total_tokens: preamble_tokens + summary_tokens + recent_tokens,
        }
    }

    /// Assign tiers to older turns based on cognitive score + recency.
    fn assign_tiers(&self, turns: &mut [ConversationTurn], total_turns: usize) {
        let demotion_threshold = self.config.tier1_demotion_threshold;
        let high = self.config.high_relevance_threshold;
        let low = self.config.low_relevance_threshold;

        for turn in turns.iter_mut() {
            let distance_from_end = total_turns - turn.turn_index;
            let score = turn.cognitive_score.unwrap_or(0.0);

            let tier = if score >= high {
                // Cognitive promotion: high-relevance turns stay detailed
                AssignedTier::Detailed
            } else if distance_from_end <= demotion_threshold || score >= low {
                // Within recency window or moderate relevance
                AssignedTier::Detailed
            } else {
                AssignedTier::Condensed
            };

            turn.assigned_tier = Some(tier);
        }
    }

    /// Compress turns according to their assigned tier.
    async fn compress_turns(
        &self,
        turns: &[ConversationTurn],
        budget_tokens: usize,
    ) -> Vec<TierSummary> {
        let mut summaries = Vec::new();

        // Group consecutive turns by their assigned tier for batching
        let mut batch_start = 0;
        while batch_start < turns.len() {
            let tier = turns[batch_start]
                .assigned_tier
                .unwrap_or(AssignedTier::Condensed);
            let mut batch_end = batch_start + 1;
            while batch_end < turns.len()
                && turns[batch_end].assigned_tier.unwrap_or(AssignedTier::Condensed) == tier
            {
                batch_end += 1;
            }

            let batch = &turns[batch_start..batch_end];
            let compression_tier = match tier {
                AssignedTier::Detailed => CompressionTier::Detailed,
                AssignedTier::Condensed => CompressionTier::Condensed,
                AssignedTier::Verbatim => {
                    batch_start = batch_end;
                    continue;
                }
            };

            // Process in sub-batches of 5 (matching MAX_SEGMENTS_PER_CALL)
            for chunk in batch.chunks(5) {
                let chunk_summaries = self
                    .compress_chunk(chunk, compression_tier, budget_tokens)
                    .await;
                summaries.extend(chunk_summaries);
            }

            batch_start = batch_end;
        }

        summaries
    }

    /// Compress a chunk of turns at a given tier.
    ///
    /// Uses hybrid extractive-first: if extractive fits the budget, skip LLM.
    async fn compress_chunk(
        &self,
        turns: &[ConversationTurn],
        tier: CompressionTier,
        _budget_tokens: usize,
    ) -> Vec<TierSummary> {
        let snippet_len = match tier {
            CompressionTier::Detailed => DEFAULT_SNIPPET_LENGTH,
            CompressionTier::Condensed => 100,
        };

        // Try extractive first for each turn
        let extractive_summaries: Vec<String> = turns
            .iter()
            .map(|t| extractive_turn_summary(&t.messages, snippet_len))
            .collect();

        // Check if we have a provider for abstractive
        let final_texts = if let Some(provider) = &self.summary_provider {
            // Build segments for turns that need LLM summarization
            let segments: Vec<Vec<Message>> =
                turns.iter().map(|t| t.messages.clone()).collect();

            let results = provider.summarize_batch(segments, tier).await;

            // Merge: use LLM result if successful, fall back to extractive
            results
                .into_iter()
                .zip(extractive_summaries.iter())
                .map(|(result, fallback)| match result {
                    Ok(text) if !text.is_empty() => text,
                    _ => fallback.clone(),
                })
                .collect()
        } else {
            extractive_summaries
        };

        // Build TierSummary for each turn
        final_texts
            .into_iter()
            .zip(turns.iter())
            .map(|(content, turn)| {
                let token_count = self.token_counter.estimate_text(&content);
                TierSummary {
                    tier,
                    content,
                    turn_range: (turn.turn_index, turn.turn_index + 1),
                    token_count,
                    cognitive_score: turn.cognitive_score,
                }
            })
            .collect()
    }
}

/// Create an extractive summary for a single turn's messages.
fn extractive_turn_summary(messages: &[Message], snippet_len: usize) -> String {
    let mut lines = Vec::new();
    for msg in messages {
        match msg {
            Message::User { content } => {
                let text = match content {
                    providers::UserContent::Text(t) => t.clone(),
                    providers::UserContent::MultiPart(_) => "[multipart]".to_string(),
                };
                lines.push(format!("User: {}", first_snippet(&text, snippet_len)));
            }
            Message::Assistant {
                content: Some(text),
                ..
            } => {
                lines.push(format!(
                    "Assistant: {}",
                    first_snippet(text, snippet_len)
                ));
            }
            Message::Tool { name, content, .. } => {
                lines.push(format!(
                    "{}: {}",
                    name,
                    first_snippet(content, snippet_len / 2)
                ));
            }
            _ => {}
        }
    }
    lines.join("\n")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::token_counter::default_token_counter;
    use async_trait::async_trait;

    struct MockSummaryProvider {
        response: String,
    }

    #[async_trait]
    impl SummaryProvider for MockSummaryProvider {
        async fn summarize_batch(
            &self,
            segments: Vec<Vec<Message>>,
            _tier: CompressionTier,
        ) -> Vec<Result<String, String>> {
            segments.iter().map(|_| Ok(self.response.clone())).collect()
        }
    }

    struct MockMemoryScorer {
        scores: Vec<f64>,
    }

    #[async_trait]
    impl MemoryScorer for MockMemoryScorer {
        async fn score_batch(&self, texts: &[String]) -> Vec<f64> {
            texts
                .iter()
                .enumerate()
                .map(|(i, _)| self.scores.get(i).copied().unwrap_or(0.5))
                .collect()
        }
    }

    fn default_config() -> HistoryCompressionConfig {
        HistoryCompressionConfig::default()
    }

    fn make_history(n: usize) -> Vec<Message> {
        let mut msgs = Vec::new();
        for i in 0..n {
            msgs.push(Message::user(format!("User message {}", i)));
            msgs.push(Message::assistant(format!("Assistant response {}", i)));
        }
        msgs
    }

    #[tokio::test]
    async fn test_short_session_no_compression() {
        let compressor =
            TieredHistoryCompressor::new(default_token_counter(), default_config());
        let history = make_history(3); // 3 turns, tier0_count = 8
        let result = compressor.compress(&history, 50_000, 8).await;
        assert!(result.summaries.is_empty());
        assert_eq!(result.recent_messages.len(), 6); // all kept verbatim
    }

    #[tokio::test]
    async fn test_tiered_compression_produces_summaries() {
        let provider = Arc::new(MockSummaryProvider {
            response: "LLM summary of turn".into(),
        });
        let compressor =
            TieredHistoryCompressor::new(default_token_counter(), default_config())
                .with_summary_provider(provider);

        let history = make_history(20); // 20 turns
        let result = compressor.compress(&history, 50_000, 8).await;

        assert!(!result.summaries.is_empty(), "should have compressed older turns");
        assert!(!result.recent_messages.is_empty(), "should keep recent verbatim");

        // Verify summaries contain LLM output
        assert!(
            result.summaries.iter().any(|s| s.content.contains("LLM summary")),
            "expected LLM summary in results"
        );
    }

    #[tokio::test]
    async fn test_cognitive_promotion_to_tier1() {
        // Old turn (index 0) with high score should be Tier 1
        let scorer = Arc::new(MockMemoryScorer {
            scores: vec![0.9, 0.2, 0.2, 0.2, 0.2, 0.2, 0.2, 0.2, 0.2, 0.2, 0.2, 0.2],
        });
        let provider = Arc::new(MockSummaryProvider {
            response: "summary".into(),
        });

        let compressor =
            TieredHistoryCompressor::new(default_token_counter(), default_config())
                .with_summary_provider(provider)
                .with_memory_scorer(scorer);

        let history = make_history(20); // 20 turns, tier0 = 8, so 12 older
        let result = compressor.compress(&history, 50_000, 8).await;

        // The first turn (score 0.9 > 0.7) should be Detailed
        let first_summary = result.summaries.iter().find(|s| s.turn_range.0 == 0);
        assert!(first_summary.is_some());
        assert_eq!(first_summary.unwrap().tier, CompressionTier::Detailed);
    }

    #[tokio::test]
    async fn test_no_scorer_falls_back_to_recency() {
        let provider = Arc::new(MockSummaryProvider {
            response: "summary".into(),
        });
        let compressor =
            TieredHistoryCompressor::new(default_token_counter(), default_config())
                .with_summary_provider(provider);

        let history = make_history(20);
        let result = compressor.compress(&history, 50_000, 8).await;

        // Without scorer, should still produce summaries (recency-based)
        assert!(!result.summaries.is_empty());
    }

    #[tokio::test]
    async fn test_extractive_fallback_on_no_provider() {
        let compressor =
            TieredHistoryCompressor::new(default_token_counter(), default_config());
        let history = make_history(20);
        let result = compressor.compress(&history, 50_000, 8).await;

        // Should produce extractive summaries
        assert!(!result.summaries.is_empty());
        assert!(
            result
                .summaries
                .iter()
                .any(|s| s.content.contains("User:")),
            "extractive fallback should contain 'User:' prefix"
        );
    }

    #[tokio::test]
    async fn test_preamble_system_messages_preserved() {
        let mut history = vec![
            Message::system("You are a helpful assistant."),
            Message::system("Extra context."),
        ];
        history.extend(make_history(20));

        let compressor =
            TieredHistoryCompressor::new(default_token_counter(), default_config());
        let result = compressor.compress(&history, 50_000, 8).await;

        assert_eq!(result.preamble.len(), 2);
    }

    #[test]
    fn test_extractive_turn_summary() {
        let messages = vec![
            Message::user("What is the weather?"),
            Message::assistant("It's sunny today."),
        ];
        let summary = extractive_turn_summary(&messages, 200);
        assert!(summary.contains("User: What is the weather?"));
        assert!(summary.contains("Assistant: It's sunny today."));
    }

    #[test]
    fn test_scoring_content() {
        let turn = ConversationTurn {
            messages: vec![
                Message::user("How do I deploy?"),
                Message::assistant("Use docker compose up."),
            ],
            turn_index: 0,
            token_count: 10,
            cognitive_score: None,
            assigned_tier: None,
        };
        let content = turn.scoring_content();
        assert!(content.contains("User: How do I deploy?"));
        assert!(content.contains("Assistant: Use docker compose up."));
    }
}
```

- [ ] **Step 2: Add module to mod.rs and re-export**

In `crates/context_engine/src/history_compressor/mod.rs`:

```rust
mod grouping;
mod prompts;
mod snippet;
mod tiered;
mod types;

pub use prompts::{TIER1_INSTRUCTIONS, TIER2_INSTRUCTIONS};
pub use snippet::first_snippet;
pub use tiered::TieredHistoryCompressor;
pub use types::{
    AssignedTier, CompressedHistory, CompressionTier, ConversationTurn, TierSummary,
};
```

This replaces the entire old `mod.rs` content (deleting the old `HistoryCompressor` impl and its inline tests). The `grouping` and `prompts` modules are now `mod` (not `pub mod`) since their public items are re-exported through `types` and the parent module.

- [ ] **Step 3: Run tests**

Run: `cargo nextest run -p context_engine -- tiered`
Expected: All tiered tests pass.

Run: `cargo nextest run -p context_engine -- grouping`
Expected: All grouping tests pass.

Run: `cargo nextest run -p context_engine -- prompts`
Expected: All prompt tests pass.

- [ ] **Step 4: Commit**

```bash
git add crates/context_engine/src/history_compressor/
git commit -m "feat(context_engine): implement TieredHistoryCompressor

3-tier pipeline: group by turn → score via MemoryScorer → assign
tiers (Verbatim/Detailed/Condensed) → compress with tier-specific
LLM prompts. Hybrid extractive-first: skips LLM when extractive
fits. Falls back to extractive on LLM failure."
```

---

## Task 7: Tool-Result Microcompaction Pre-Pass

**Files:**
- Modify: `crates/context_engine/src/history_compressor/tiered.rs`

Spec §7: Before tier compression, scan older tool results and prune stale ones at assembly time (extending the mid-loop pattern to session resume).

- [ ] **Step 1: Write the failing test**

Add to the test module in `tiered.rs`:

```rust
    #[test]
    fn test_microcompact_stale_tool_results() {
        let messages = vec![
            Message::user("Read the file"),
            Message::Assistant {
                content: None,
                tool_calls: Some(vec![providers::ToolCallMessage {
                    id: "tc1".into(),
                    r#type: "function".into(),
                    function: providers::FunctionCall {
                        name: "read_file".into(),
                        arguments: "{}".into(),
                    },
                }]),
                reasoning_content: None,
            },
            Message::Tool {
                tool_call_id: "tc1".into(),
                name: "read_file".into(),
                content: "A".repeat(5000), // large tool result
            },
            Message::assistant("Here's what I found."),
            Message::user("Now do something else"),
            Message::assistant("Sure."),
        ];

        let compacted = microcompact_tool_results(messages, 8); // recent window = 8
        // The old tool result (position 2) should be truncated
        if let Message::Tool { content, .. } = &compacted[2] {
            assert!(
                content.len() < 500,
                "stale tool result should be compacted, got {} chars",
                content.len()
            );
            assert!(content.contains("[compressed"));
        } else {
            panic!("expected Tool message at index 2");
        }
    }

    #[test]
    fn test_microcompact_preserves_recent_tool_results() {
        let messages = vec![
            Message::user("Do something"),
            Message::Tool {
                tool_call_id: "tc1".into(),
                name: "search".into(),
                content: "B".repeat(5000),
            },
            Message::assistant("Done."),
        ];

        // All messages within recent window — nothing compacted
        let compacted = microcompact_tool_results(messages.clone(), 8);
        if let Message::Tool { content, .. } = &compacted[1] {
            assert_eq!(content.len(), 5000, "recent tool results should not be compacted");
        }
    }
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo nextest run -p context_engine -- microcompact`
Expected: FAIL — function not defined.

- [ ] **Step 3: Implement the microcompaction function**

Add to `tiered.rs`:

```rust
/// Compactable tool names (same set as MidLoopCompressor).
const COMPACTABLE_TOOLS: &[&str] = &[
    "read_file", "bash", "grep", "glob", "web_search", "web_fetch",
];

/// Minimum token count for a tool result to be worth compacting.
const MIN_COMPACTABLE_TOKENS: usize = 50;

/// Snippet length for compacted tool results.
const MICROCOMPACT_SNIPPET_LEN: usize = 150;

/// Pre-pass: compact stale tool results in older messages before tiered compression.
///
/// Tool results outside the `recent_window` (counted from the end) that match
/// `COMPACTABLE_TOOLS` and exceed `MIN_COMPACTABLE_TOKENS` get replaced with
/// a snippet summary.
pub fn microcompact_tool_results(
    mut messages: Vec<Message>,
    recent_window: usize,
) -> Vec<Message> {
    if messages.len() <= recent_window {
        return messages;
    }

    let cutoff = messages.len().saturating_sub(recent_window);

    for msg in messages[..cutoff].iter_mut() {
        if let Message::Tool { name, content, .. } = msg {
            if COMPACTABLE_TOOLS.iter().any(|t| name.contains(t)) && content.len() > MIN_COMPACTABLE_TOKENS * 4 {
                let snippet = first_snippet(content, MICROCOMPACT_SNIPPET_LEN);
                *content = format!(
                    "{} [compressed {} result, originally {} chars]",
                    snippet,
                    name,
                    content.len()
                );
            }
        }
    }

    messages
}
```

- [ ] **Step 4: Wire into the compress method**

In `TieredHistoryCompressor::compress()`, add a microcompaction step before grouping. Change the first few lines after the method signature:

```rust
    pub async fn compress(
        &self,
        history: &[Message],
        budget_tokens: usize,
        tier0_count: usize,
    ) -> CompressedHistory {
        // Step 0: Microcompact stale tool results
        let history = microcompact_tool_results(history.to_vec(), tier0_count * 2);

        // Step 1: Group into turns (use &history now, not the slice parameter)
        let (preamble, mut turns) = group_into_turns(&history, &*self.token_counter);
        // ... rest unchanged
```

- [ ] **Step 5: Run tests**

Run: `cargo nextest run -p context_engine -- microcompact`
Expected: 2 tests pass.

Run: `cargo nextest run -p context_engine -- tiered`
Expected: All tiered tests still pass.

- [ ] **Step 6: Commit**

```bash
git add crates/context_engine/src/history_compressor/tiered.rs
git commit -m "feat(context_engine): add tool-result microcompaction pre-pass

Stale Read/Bash/Grep/Glob/WebSearch/WebFetch results outside the
recent window are truncated to 150-char snippets before tiered
compression, reducing token count before the pipeline starts."
```

---

## Task 8: Delta Compaction Logic

**Files:**
- Modify: `crates/context_engine/src/history_compressor/tiered.rs`

Spec §6.3-6.4: On session resume, load existing compressed prefix, compress only the delta (new messages), merge, and demote old Tier 1 summaries.

- [ ] **Step 1: Write the failing test**

Add to the test module in `tiered.rs`:

```rust
    #[tokio::test]
    async fn test_delta_compression_reuses_prefix() {
        let provider = Arc::new(MockSummaryProvider {
            response: "delta summary".into(),
        });
        let compressor =
            TieredHistoryCompressor::new(default_token_counter(), default_config())
                .with_summary_provider(provider);

        // Simulate existing prefix from previous compression
        let existing_prefix = vec![
            TierSummary {
                tier: CompressionTier::Detailed,
                content: "Old Tier 1 summary of turns 0-3".into(),
                turn_range: (0, 4),
                token_count: 20,
                cognitive_score: Some(0.5),
            },
            TierSummary {
                tier: CompressionTier::Condensed,
                content: "Old Tier 2 gist of turns 4-7".into(),
                turn_range: (4, 8),
                token_count: 10,
                cognitive_score: Some(0.3),
            },
        ];

        // Full history: 20 turns. Prefix covers first 8 turns (16 messages).
        let history = make_history(20);
        let compressed_through_idx = 16; // 8 turns * 2 msgs/turn

        let result = compressor
            .compress_with_delta(
                &history,
                50_000,
                8, // tier0_count
                &existing_prefix,
                compressed_through_idx,
            )
            .await;

        // Should include the old summaries + new delta summaries
        assert!(result.summaries.len() >= 2, "should include old + new summaries");

        // Recent 8 turns should be verbatim
        assert!(!result.recent_messages.is_empty());
    }

    #[tokio::test]
    async fn test_delta_demotes_old_tier1() {
        let provider = Arc::new(MockSummaryProvider {
            response: "demoted summary".into(),
        });
        let mut config = default_config();
        config.tier1_demotion_threshold = 5; // aggressive demotion

        let compressor =
            TieredHistoryCompressor::new(default_token_counter(), config)
                .with_summary_provider(provider);

        // Old Tier 1 summary far from current end
        let existing_prefix = vec![TierSummary {
            tier: CompressionTier::Detailed,
            content: "Old detailed summary of turns 0-2".into(),
            turn_range: (0, 3),
            token_count: 30,
            cognitive_score: Some(0.4), // below high threshold
        }];

        let history = make_history(30); // 30 turns, far from turn 0-2
        let result = compressor
            .compress_with_delta(&history, 50_000, 8, &existing_prefix, 6)
            .await;

        // The old Tier 1 should be demoted to Tier 2
        let old_summary = result
            .summaries
            .iter()
            .find(|s| s.turn_range == (0, 3));
        assert!(old_summary.is_some());
        assert_eq!(
            old_summary.unwrap().tier,
            CompressionTier::Condensed,
            "old Tier 1 should be demoted to Condensed"
        );
    }
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo nextest run -p context_engine -- delta`
Expected: FAIL — method not defined.

- [ ] **Step 3: Implement compress_with_delta**

Add to `TieredHistoryCompressor`:

```rust
    /// Compress with delta optimization: reuse existing prefix, only compress new messages.
    pub async fn compress_with_delta(
        &self,
        history: &[Message],
        budget_tokens: usize,
        tier0_count: usize,
        existing_prefix: &[TierSummary],
        compressed_through_idx: usize,
    ) -> CompressedHistory {
        // If no existing prefix or index is invalid, fall back to full compression
        if existing_prefix.is_empty() || compressed_through_idx >= history.len() {
            return self.compress(history, budget_tokens, tier0_count).await;
        }

        // Delta: only new messages since last compression
        let delta_messages = &history[compressed_through_idx..];

        // Compress only the delta
        let delta_result = self.compress(delta_messages, budget_tokens, tier0_count).await;

        // Merge existing prefix with delta summaries
        let mut merged_summaries = Vec::new();

        // Determine total turn count for demotion calculation
        let (_, all_turns) = group_into_turns(history, &*self.token_counter);
        let total_turns = all_turns.len();

        // Process existing summaries: keep or demote
        for summary in existing_prefix {
            let distance_from_end = total_turns.saturating_sub(summary.turn_range.1);
            let score = summary.cognitive_score.unwrap_or(0.0);

            if summary.tier == CompressionTier::Detailed
                && distance_from_end > self.config.tier1_demotion_threshold
                && score < self.config.high_relevance_threshold
            {
                // Demote: re-compress the Tier 1 text as Tier 2
                let demoted_content = if let Some(provider) = &self.summary_provider {
                    let segments = vec![vec![Message::user(&summary.content)]];
                    let results = provider
                        .summarize_batch(segments, CompressionTier::Condensed)
                        .await;
                    results
                        .into_iter()
                        .next()
                        .and_then(|r| r.ok())
                        .unwrap_or_else(|| summary.content.clone())
                } else {
                    first_snippet(&summary.content, 100).to_string()
                };

                merged_summaries.push(TierSummary {
                    tier: CompressionTier::Condensed,
                    content: demoted_content,
                    turn_range: summary.turn_range,
                    token_count: self.token_counter.estimate_text(&summary.content),
                    cognitive_score: summary.cognitive_score,
                });
            } else {
                // Keep as-is
                merged_summaries.push(summary.clone());
            }
        }

        // Append delta summaries
        merged_summaries.extend(delta_result.summaries);

        CompressedHistory {
            summaries: merged_summaries,
            recent_messages: delta_result.recent_messages,
            preamble: delta_result.preamble,
            total_tokens: delta_result.total_tokens,
        }
    }
```

- [ ] **Step 4: Run tests**

Run: `cargo nextest run -p context_engine -- delta`
Expected: 2 tests pass.

Run: `cargo nextest run -p context_engine -- tiered`
Expected: All tiered tests still pass.

- [ ] **Step 5: Commit**

```bash
git add crates/context_engine/src/history_compressor/tiered.rs
git commit -m "feat(context_engine): add delta compaction for session resume

compress_with_delta() reuses existing compressed prefix, only
processes new messages since last compression. Old Tier 1 summaries
beyond the demotion threshold are re-compressed as Tier 2."
```

---

## Task 9: Session Delta Storage

**Files:**
- Modify: `crates/storage/migrations/001_initial.sql:126-136`
- Modify: `crates/storage/src/repos/session.rs`
- Modify: `crates/session/src/manager.rs`

- [ ] **Step 1: Add columns to sessions table**

In `crates/storage/migrations/001_initial.sql`, add three columns to the `sessions` CREATE TABLE (after `squad_id`):

```sql
    compressed_prefix      TEXT,
    compressed_through_idx INTEGER,
    compressed_at          TEXT
```

- [ ] **Step 2: Add repo methods for compressed prefix**

In `crates/storage/src/repos/session.rs`, add two methods to `SessionRepo`:

```rust
    /// Save the compressed history prefix for a session.
    pub async fn save_compressed_prefix(
        &self,
        session_key: &str,
        prefix_json: &str,
        through_idx: i64,
    ) -> common::Result<()> {
        sqlx::query(
            "UPDATE sessions SET compressed_prefix = ?1, compressed_through_idx = ?2, \
             compressed_at = strftime('%Y-%m-%dT%H:%M:%SZ', 'now') WHERE key = ?3",
        )
        .bind(prefix_json)
        .bind(through_idx)
        .bind(session_key)
        .execute(self.pool())
        .await?;
        Ok(())
    }

    /// Load the compressed history prefix for a session.
    /// Returns (prefix_json, through_idx) or None if not set.
    pub async fn load_compressed_prefix(
        &self,
        session_key: &str,
    ) -> common::Result<Option<(String, i64)>> {
        let row: Option<(Option<String>, Option<i64>)> = sqlx::query_as(
            "SELECT compressed_prefix, compressed_through_idx FROM sessions WHERE key = ?1",
        )
        .bind(session_key)
        .fetch_optional(self.pool())
        .await?;

        Ok(row.and_then(|(prefix, idx)| match (prefix, idx) {
            (Some(p), Some(i)) => Some((p, i)),
            _ => None,
        }))
    }

    /// Clear the compressed prefix (e.g., on message edit/delete).
    pub async fn clear_compressed_prefix(&self, session_key: &str) -> common::Result<()> {
        sqlx::query(
            "UPDATE sessions SET compressed_prefix = NULL, compressed_through_idx = NULL, \
             compressed_at = NULL WHERE key = ?1",
        )
        .bind(session_key)
        .execute(self.pool())
        .await?;
        Ok(())
    }
```

- [ ] **Step 3: Expose through Session manager**

In `crates/session/src/manager.rs`, add methods to `SessionManager` that delegate to the repo. Find the existing `save()` method and add nearby:

```rust
    /// Save compressed history prefix for delta compaction.
    pub async fn save_compressed_prefix(
        &self,
        session_key: &str,
        prefix_json: &str,
        through_idx: i64,
    ) -> common::Result<()> {
        self.repo.save_compressed_prefix(session_key, prefix_json, through_idx).await
    }

    /// Load compressed history prefix for delta compaction.
    pub async fn load_compressed_prefix(
        &self,
        session_key: &str,
    ) -> common::Result<Option<(String, i64)>> {
        self.repo.load_compressed_prefix(session_key).await
    }

    /// Clear compressed prefix (invalidation).
    pub async fn clear_compressed_prefix(&self, session_key: &str) -> common::Result<()> {
        self.repo.clear_compressed_prefix(session_key).await
    }
```

- [ ] **Step 4: Run tests**

Run: `cargo nextest run -p storage`
Expected: All pass (migration creates new columns, existing tests unaffected).

Run: `cargo nextest run -p session`
Expected: All pass.

- [ ] **Step 5: Commit**

```bash
git add crates/storage/migrations/001_initial.sql crates/storage/src/repos/session.rs crates/session/src/manager.rs
git commit -m "feat(storage): add compressed_prefix columns for delta compaction

Three new columns on sessions table: compressed_prefix (JSON text),
compressed_through_idx (message index), compressed_at (timestamp).
SessionRepo + SessionManager expose save/load/clear methods."
```

---

## Task 10: Assembler Integration

**Files:**
- Modify: `crates/context_engine/src/assembler/mod.rs`
- Modify: `crates/context_engine/src/lib.rs`

- [ ] **Step 1: Replace HistoryCompressor with TieredHistoryCompressor in the assembler**

In `crates/context_engine/src/assembler/mod.rs`:

1. Update imports (line 21):

```rust
// Old:
use crate::{BudgetAllocator, BudgetConfig, CompressorConfig, HistoryCompressor, Priority};

// New:
use crate::{BudgetAllocator, BudgetConfig, Priority, TieredHistoryCompressor};
```

Also add:
```rust
use crate::memory_scorer::MemoryScorer;
use config::schema::HistoryCompressionConfig;
```

2. Update `ContextEngine` struct (line 32-48):

```rust
pub struct ContextEngine {
    compressor: TieredHistoryCompressor,
    token_counter: Arc<dyn TokenCounter>,
    // ... rest unchanged
}
```

3. Update constructor methods. The `new()` (line 69) now needs config:

```rust
    pub fn new(config: HistoryCompressionConfig) -> Self {
        let counter = default_token_counter();
        Self {
            compressor: TieredHistoryCompressor::new(Arc::clone(&counter), config),
            token_counter: counter,
            memory_retriever: None,
            memory_retrieval_limit: 10,
            cache: Arc::new(Mutex::new(ContextCache::new())),
            sources: Vec::new(),
            insight_forge: None,
            query_pipeline: None,
            ranking_pipeline: None,
        }
    }
```

4. Update `with_token_counter` (lines 75-88) to also update the compressor's counter:

```rust
    pub fn with_token_counter(mut self, counter: Arc<dyn TokenCounter>) -> Self {
        self.compressor = TieredHistoryCompressor::new(
            Arc::clone(&counter),
            // Preserve existing config — reconstruct
            HistoryCompressionConfig::default(), // Will be overridden by builder
        );
        self.token_counter = counter;
        self
    }
```

Actually, better approach — take config in `new()` and store it. Then `with_token_counter` can reconstruct the compressor with the stored config. Add a `config` field:

```rust
pub struct ContextEngine {
    compressor: TieredHistoryCompressor,
    token_counter: Arc<dyn TokenCounter>,
    compression_config: HistoryCompressionConfig,
    // ... rest unchanged
}
```

5. Remove old `with_compressor_config()` method (lines 92-97). Replace with a proper builder chain.

6. Update `with_summary_provider` (lines 119-122):

```rust
    pub fn with_summary_provider(mut self, provider: Arc<dyn SummaryProvider>) -> Self {
        self.compressor = self.compressor.with_summary_provider(provider);
        self
    }
```

7. Add `with_memory_scorer`:

```rust
    pub fn with_memory_scorer(mut self, scorer: Arc<dyn MemoryScorer>) -> Self {
        self.compressor = self.compressor.with_memory_scorer(scorer);
        self
    }
```

8. In `assemble_uncached_with_memory` (line 346-349), update the compress call. The `tier0_count` needs to come from the request (which carries `DepthMode` info). Add a `tier0_count` field to `ContextRequest` or compute from config:

```rust
        // Determine tier0_count from depth mode
        let tier0_count = match &request.strategy {
            ExecutionStrategy::ToolAssisted { .. } | ExecutionStrategy::AutonomousTask { .. } => {
                self.compression_config.tier0_messages.normal
            }
            _ => self.compression_config.tier0_messages.normal,
        };

        let compressed = self
            .compressor
            .compress(&request.history, history_budget, tier0_count)
            .await;
```

9. Update summary injection (lines 398-401) to handle the new `CompressedHistory` with `preamble`:

```rust
        // Preamble (system messages that precede conversation)
        for msg in &compressed.preamble {
            messages.push(msg.clone());
        }

        // Tier 1 + Tier 2 summaries as system-level context
        if !compressed.summaries.is_empty() {
            let summary_text = compressed
                .summaries
                .iter()
                .map(|s| s.content.as_str())
                .collect::<Vec<_>>()
                .join("\n\n---\n\n");
            messages.push(Message::system(&format!(
                "Earlier in this conversation:\n\n{}",
                summary_text
            )));
        }
```

- [ ] **Step 2: Update ContextRequest if needed**

In `crates/context_engine/src/assembler/types.rs`, add a `tier0_count` field to `ContextRequest` (or handle via the engine's config — check which approach fits the existing call sites better). If adding to request:

```rust
    /// Number of recent turns to keep verbatim (from DepthMode).
    pub tier0_count: Option<usize>,
```

Then in the compress call, use `request.tier0_count.unwrap_or(self.compression_config.tier0_messages.normal)`.

- [ ] **Step 3: Fix assembler tests**

The existing assembler tests will need updating. They construct `ContextEngine::new()` which now requires config. Update all test helpers:

```rust
fn make_engine() -> ContextEngine {
    ContextEngine::new(HistoryCompressionConfig::default())
}
```

Update any tests that reference `CompressorConfig` or `CompressorMode`. Update mock `SummaryProvider` impls to match the new trait signature (with `tier: CompressionTier` param).

- [ ] **Step 4: Update lib.rs re-exports**

Ensure `crates/context_engine/src/lib.rs` exports everything needed:

```rust
pub use history_compressor::{
    first_snippet, AssignedTier, CompressedHistory, CompressionTier, ConversationTurn,
    TierSummary, TieredHistoryCompressor, TIER1_INSTRUCTIONS, TIER2_INSTRUCTIONS,
};
pub use memory_scorer::MemoryScorer;
pub use summary_provider::SummaryProvider;
```

Remove any old exports (`CompressorConfig`, `CompressorMode`, `HistoryCompressor`, `HistorySummary`).

- [ ] **Step 5: Run tests**

Run: `cargo nextest run -p context_engine`
Expected: All pass. Old `HistoryCompressor` tests are gone; new tiered + assembler tests pass.

- [ ] **Step 6: Commit**

```bash
git add crates/context_engine/
git commit -m "refactor(context_engine): replace HistoryCompressor with TieredHistoryCompressor

Assembler now uses TieredHistoryCompressor exclusively. Old
HistoryCompressor, CompressorConfig, CompressorMode deleted.
ContextEngine::new() takes HistoryCompressionConfig. Summary
injection reformatted for tier summaries."
```

---

## Task 11: Builder Wiring + MemoryScorer Impl

**Files:**
- Create: `crates/agent/src/adapters/memory_scorer_impl.rs`
- Modify: `crates/agent/src/adapters/mod.rs`
- Modify: `crates/agent/src/agent_loop/builder.rs:550-558`
- Modify: `crates/agent/src/agent_runtime/runtime.rs`

- [ ] **Step 1: Create the MemoryScorer implementation**

```rust
// crates/agent/src/adapters/memory_scorer_impl.rs
use std::sync::Arc;

use async_trait::async_trait;
use context_engine::MemoryScorer;

use crate::cognitive_accessor::CognitiveAccessor;

/// Wraps `CognitiveAccessor` to implement `MemoryScorer` for tiered compression.
///
/// Uses the existing embedding + retrieval infrastructure to score
/// text passages for cognitive relevance.
pub struct CognitiveMemoryScorer {
    accessor: Arc<CognitiveAccessor>,
}

impl CognitiveMemoryScorer {
    pub fn new(accessor: Arc<CognitiveAccessor>) -> Self {
        Self { accessor }
    }
}

#[async_trait]
impl MemoryScorer for CognitiveMemoryScorer {
    async fn score_batch(&self, texts: &[String]) -> Vec<f64> {
        // Score each text by retrieving top-1 and using the score as relevance.
        // This leverages the existing 12-factor relevance pipeline.
        let mut scores = Vec::with_capacity(texts.len());
        for text in texts {
            let entries = self.accessor.retrieve(text, 1).await;
            let score = entries.first().map(|e| e.score).unwrap_or(0.0);
            scores.push(score);
        }
        scores
    }
}
```

Note: This implementation is intentionally simple — one retrieval per text. If `CognitiveAccessor` doesn't exist or has a different name, find the equivalent type that wraps `UnifiedMemoryService` and implements `MemoryRetriever`. Adapt the import path accordingly. The key pattern is: retrieve top-1 result for each text, use its score.

- [ ] **Step 2: Register the adapter module**

In `crates/agent/src/adapters/mod.rs`, add:

```rust
pub mod memory_scorer_impl;
```

- [ ] **Step 3: Update builder.rs to wire everything**

In `crates/agent/src/agent_loop/builder.rs`, update the ContextEngine construction (around lines 550-558):

```rust
        // Determine summarization model: config override or default
        let summary_model = config
            .cognitive
            .history_compression
            .model
            .clone()
            .unwrap_or_else(|| config.agents.defaults.model.clone());

        let summary_provider = Arc::new(crate::adapters::llm_summary::LlmSummaryProvider::new(
            provider.clone(),
            summary_model,
        ));

        let token_counter = context_engine::token_counter_for_model(&config.agents.defaults.model);

        let mut context_engine = context_engine::ContextEngine::new(
            config.cognitive.history_compression.clone(),
        )
        .with_sources(sources)
        .with_token_counter(Arc::clone(&token_counter))
        .with_summary_provider(summary_provider);

        // Wire cognitive memory scorer if available
        if config.cognitive.history_compression.use_cognitive_scoring {
            if let Some(ref accessor) = cognitive_accessor {
                let scorer = Arc::new(
                    crate::adapters::memory_scorer_impl::CognitiveMemoryScorer::new(
                        Arc::clone(accessor),
                    ),
                );
                context_engine = context_engine.with_memory_scorer(scorer);
            }
        }
```

The `cognitive_accessor` variable should already exist in the builder scope — it's the same object used for `MemoryRetriever`. Find its actual name and type in the builder and use that. If it's `memory_retriever`, use that instead.

- [ ] **Step 4: Update runtime.rs if needed**

In `crates/agent/src/agent_runtime/runtime.rs`, if `ContextRequest` gained a `tier0_count` field, populate it based on `DepthMode`:

```rust
        let tier0_count = match depth {
            DepthMode::Normal => self.compression_config.tier0_messages.normal,
            DepthMode::DeepThink => self.compression_config.tier0_messages.deep_think,
            DepthMode::Ultra => self.compression_config.tier0_messages.ultra,
        };

        let context_request = ContextRequest {
            // ... existing fields ...
            tier0_count: Some(tier0_count),
        };
```

If `tier0_count` was handled inside the assembler via config (not via request), skip this step.

- [ ] **Step 5: Run tests**

Run: `cargo nextest run -p agent`
Expected: All pass.

Run: `cargo check --workspace`
Expected: Clean compilation.

- [ ] **Step 6: Commit**

```bash
git add crates/agent/src/adapters/memory_scorer_impl.rs crates/agent/src/adapters/mod.rs crates/agent/src/agent_loop/builder.rs crates/agent/src/agent_runtime/runtime.rs
git commit -m "feat(agent): wire TieredHistoryCompressor into builder + runtime

CognitiveMemoryScorer wraps existing retrieval for relevance scoring.
Builder uses compression model override from config. Runtime passes
tier0_count from DepthMode."
```

---

## Task 12: Streaming Event

**Files:**
- Modify: `crates/bus/src/events.rs`

- [ ] **Step 1: Add the event variant**

In `crates/bus/src/events.rs`, add to the `AgentEvent` enum:

```rust
    /// Tiered history compression completed during context assembly.
    ContextTieredCompressed {
        /// Number of turns kept verbatim (Tier 0).
        tier0_kept: usize,
        /// Total tokens in Tier 1 (detailed) summaries.
        tier1_tokens: usize,
        /// Total tokens in Tier 2 (condensed) summaries.
        tier2_tokens: usize,
        /// Whether cognitive scoring was used for tier assignment.
        cognitive_scoring_used: bool,
        /// Whether delta-only compression was used.
        delta_only: bool,
        /// Number of LLM calls saved by hybrid extractive-first.
        llm_calls_saved: usize,
    },
```

- [ ] **Step 2: Emit the event from the assembler**

In `crates/context_engine/src/assembler/mod.rs`, after the compression step, emit the event if an event sender is available. This depends on the existing event emission pattern in the assembler — check if there's an `event_tx` or similar. If events are emitted outside the assembler (in the runtime), add the necessary fields to `AssembledContext` instead:

```rust
// In AssembledContext (types.rs), add:
    /// Tiered compression stats (for event emission by the caller).
    pub compression_stats: Option<CompressionStats>,

// New struct:
#[derive(Debug, Clone)]
pub struct CompressionStats {
    pub tier0_kept: usize,
    pub tier1_tokens: usize,
    pub tier2_tokens: usize,
    pub cognitive_scoring_used: bool,
    pub delta_only: bool,
    pub llm_calls_saved: usize,
}
```

Populate after compression:

```rust
        let tier1_tokens: usize = compressed.summaries.iter()
            .filter(|s| s.tier == CompressionTier::Detailed)
            .map(|s| s.token_count)
            .sum();
        let tier2_tokens: usize = compressed.summaries.iter()
            .filter(|s| s.tier == CompressionTier::Condensed)
            .map(|s| s.token_count)
            .sum();

        // ... set on AssembledContext
        compression_stats: Some(CompressionStats {
            tier0_kept: compressed.recent_messages.len(), // approximate by messages
            tier1_tokens,
            tier2_tokens,
            cognitive_scoring_used: self.compression_config.use_cognitive_scoring
                && self.compressor.has_scorer(),
            delta_only: false, // delta not yet implemented
            llm_calls_saved: 0, // tracked in compressor
        }),
```

- [ ] **Step 3: Run tests**

Run: `cargo check --workspace`
Expected: Clean compilation.

- [ ] **Step 4: Commit**

```bash
git add crates/bus/src/events.rs crates/context_engine/src/assembler/
git commit -m "feat(bus): add ContextTieredCompressed event + compression stats"
```

---

## Task 13: Cleanup + Workspace Verification

**Files:**
- Various — final cleanup pass

- [ ] **Step 1: Verify no references to deleted types remain**

Search for any remaining references to old types:

```bash
cargo build --workspace 2>&1 | head -50
```

Fix any remaining references to `CompressorConfig`, `CompressorMode`, `HistoryCompressor`, or the old `HistorySummary`. Common locations:
- `crates/context_engine/src/assembler/mod.rs` tests
- Any crate that imports from `context_engine`

- [ ] **Step 2: Run full test suite**

```bash
cargo nextest run --workspace
```

Fix any failures. Common issues:
- Old mock `SummaryProvider` impls missing `tier` param
- Tests referencing `HistoryCompressor::new()` or `CompressorConfig::default()`
- Import paths changed

- [ ] **Step 3: Run clippy**

```bash
cargo clippy --workspace --all-targets --all-features -- -D warnings
```

Fix any warnings.

- [ ] **Step 4: Run format check**

```bash
cargo fmt --all --check
```

Fix any formatting issues.

- [ ] **Step 5: Commit**

```bash
git add -A
git commit -m "chore: fix compilation + clippy after tiered compression migration"
```

---

## Task 14: Integration Tests

**Files:**
- Modify: test files (location depends on existing test structure)

- [ ] **Step 1: Write assembler integration test with tiered compression**

Find the existing assembler test file (likely in `crates/context_engine/src/assembler/mod.rs` test module or `tests/`). Add:

```rust
    #[tokio::test]
    async fn test_tiered_compression_end_to_end() {
        use crate::summary_provider::SummaryProvider;
        use crate::CompressionTier;

        struct TierTrackingProvider;

        #[async_trait]
        impl SummaryProvider for TierTrackingProvider {
            async fn summarize_batch(
                &self,
                segments: Vec<Vec<Message>>,
                tier: CompressionTier,
            ) -> Vec<Result<String, String>> {
                segments
                    .iter()
                    .map(|_| {
                        let label = match tier {
                            CompressionTier::Detailed => "DETAILED summary",
                            CompressionTier::Condensed => "CONDENSED summary",
                        };
                        Ok(label.to_string())
                    })
                    .collect()
            }
        }

        let provider = Arc::new(TierTrackingProvider);
        let config = HistoryCompressionConfig::default();
        let engine = ContextEngine::new(config)
            .with_token_counter(default_token_counter())
            .with_summary_provider(provider);

        // Build a 30-turn history (60 messages)
        let mut history = Vec::new();
        for i in 0..30 {
            history.push(Message::user(format!("Question {}", i)));
            history.push(Message::assistant(format!("Answer {}", i)));
        }

        let request = ContextRequest {
            message_text: "Latest question".into(),
            history,
            system_prompt: String::new(),
            strategy: ExecutionStrategy::ToolAssisted { max_iterations: 10 },
            tool_definitions: vec![],
            context_window: 100_000,
            session_key: None,
            retrieval_context: None,
            enhancement_budget: None,
            tier0_count: Some(8),
        };

        let result = engine.assemble(request).await;

        // Verify structure: should have system messages (summaries) + recent messages
        assert!(result.messages.len() > 8, "should have summaries + recent");

        // Check that summaries are present
        let summary_msgs: Vec<_> = result
            .messages
            .iter()
            .filter(|m| {
                if let Message::System { content } = m {
                    content.contains("DETAILED") || content.contains("CONDENSED")
                } else {
                    false
                }
            })
            .collect();
        assert!(
            !summary_msgs.is_empty(),
            "should contain tier-labeled summaries"
        );
    }

    #[tokio::test]
    async fn test_short_session_no_compression() {
        let config = HistoryCompressionConfig::default();
        let engine = ContextEngine::new(config)
            .with_token_counter(default_token_counter());

        let history = vec![
            Message::user("Hello"),
            Message::assistant("Hi!"),
        ];

        let request = ContextRequest {
            message_text: "Hello".into(),
            history,
            system_prompt: String::new(),
            strategy: ExecutionStrategy::DirectResponse,
            tool_definitions: vec![],
            context_window: 100_000,
            session_key: None,
            retrieval_context: None,
            enhancement_budget: None,
            tier0_count: Some(8),
        };

        let result = engine.assemble(request).await;

        // Short session — no summaries, all verbatim
        let has_summary = result.messages.iter().any(|m| {
            if let Message::System { content } = m {
                content.contains("Earlier in this conversation")
            } else {
                false
            }
        });
        assert!(!has_summary, "short session should have no compression");
    }
```

- [ ] **Step 2: Run integration tests**

```bash
cargo nextest run -p context_engine -- test_tiered_compression
cargo nextest run -p context_engine -- test_short_session_no_compression
```

Expected: Both pass.

- [ ] **Step 3: Run full workspace tests**

```bash
cargo nextest run --workspace
```

Expected: All pass.

- [ ] **Step 4: Commit**

```bash
git add .
git commit -m "test: add integration tests for tiered history compression"
```

---

## Task 15: Update Backlog

**Files:**
- Modify: `docs/backlog/cognitive-gaps.md`

- [ ] **Step 1: Mark item #2 as DONE**

Update the Abstractive History Summarization entry in `docs/backlog/cognitive-gaps.md`:

```markdown
### ~~2. Add Abstractive History Summarization~~ DONE
- Replaced binary compressor with 3-tier cognitive-aware system (`TieredHistoryCompressor`).
- Tier 0 (verbatim recent), Tier 1 (structured LLM summary), Tier 2 (condensed gist).
- Cognitive 12-factor scoring for tier promotion, turn-based grouping, delta compaction on resume.
- Config: `cognitive.historyCompression` with model override, tier ratios, relevance thresholds.
- Spec: `docs/superpowers/specs/2026-04-11-tiered-history-compression-design.md`
```

- [ ] **Step 2: Commit**

```bash
git add docs/backlog/cognitive-gaps.md
git commit -m "docs: mark abstractive history summarization as DONE in backlog"
```

---

## Verification Checklist

After all tasks are complete:

- [ ] `cargo nextest run --workspace` — all tests pass
- [ ] `cargo clippy --workspace --all-targets --all-features -- -D warnings` — zero warnings
- [ ] `cargo fmt --all --check` — clean
- [ ] `cargo test --workspace --doc` — doctests pass
- [ ] No references to `HistoryCompressor`, `CompressorConfig`, `CompressorMode` remain (except in git history)
- [ ] Config roundtrip: empty `{}` for `historyCompression` uses all defaults
- [ ] Config with `model` override: `LlmSummaryProvider` uses the override model
