# Tiered History Compression (THC) — Design Spec

> Date: 2026-04-11
> Status: Approved
> Author: Jayden + Claude
> Scope: `context_engine`, `agent`, `config`, `session`, `bus`

## 1. Problem Statement

When conversations exceed the LLM's token budget, older messages must be compressed. Today's `HistoryCompressor` uses a binary approach:

- **Recent messages** (last 4+): kept verbatim.
- **Everything else**: chunked into fixed groups of 5, each reduced to a first-200-char extractive snippet.

An `LlmSummaryProvider` exists and is wired into the builder (`builder.rs:550`), but `CompressorConfig` defaults to `CompressorMode::Extractive` and nothing overrides it — so the abstractive path never runs. The provider is dead code in production.

**Problems with the current approach:**

1. **Extractive snippets lose critical context** — decisions buried mid-message, reasoning chains, and code references all get dropped.
2. **Fixed chunk size (5 messages)** splits mid-conversation-turn — a user question ends up in one chunk, the assistant's answer in another.
3. **No relevance awareness** — a critical decision from 50 messages ago gets the same (poor) treatment as idle chatter from 3 messages ago.
4. **No delta optimization** — every `compress_async` call reprocesses the entire history from scratch, wasting LLM calls on content that hasn't changed.
5. **Generic LLM prompt** — "summarize in 2-3 sentences" produces low-quality summaries that don't preserve what matters.

## 2. Solution: 3-Tier Cognitive-Aware Compression

Replace the old binary compressor entirely with a unified tiered system. There is no backward-compatibility mode — the old `HistoryCompressor`, `CompressorConfig`, and `CompressorMode` enum are deleted and consolidated into `TieredHistoryCompressor`.

```
┌─────────────────────────────────────────────────────┐
│                  Conversation History                │
│  [msg0] [msg1] ... [msg_N-30] ... [msg_N-10] [msg_N]│
└────────┬──────────────┬───────────────┬─────────────┘
         │              │               │
    ┌────▼────┐   ┌─────▼─────┐   ┌────▼────┐
    │ Tier 2  │   │  Tier 1   │   │ Tier 0  │
    │Condensed│   │ Detailed  │   │Verbatim │
    │10–15%   │   │ 30–40%    │   │  100%   │
    │outcomes │   │ decisions │   │  full   │
    │only     │   │ + code    │   │fidelity │
    └────┬────┘   └─────┬─────┘   └────┬────┘
         │              │               │
         └──────────────┼───────────────┘
                        ▼
              AssembledContext.messages
```

### 2.1 What Gets Deleted

| Old code | Replacement |
|----------|-------------|
| `HistoryCompressor` struct | `TieredHistoryCompressor` |
| `CompressorConfig` struct | `HistoryCompressionConfig` (in `config` crate) |
| `CompressorMode` enum (`Extractive` / `Abstractive`) | Deleted — tiered is the only mode |
| `SummaryProvider` trait | `SummaryProvider` kept but now always receives a `CompressionTier` |
| `history_compressor/mod.rs` (old impl) | Rewritten as thin re-export module |
| `history_compressor/types.rs` (old types) | Replaced with new types (`ConversationTurn`, `TierSummary`, `CompressionTier`, `CompressedHistory`) |
| `history_compressor/snippet.rs` (`first_snippet`) | Kept — used internally for extractive fallback and hybrid optimization |

The extractive snippet logic (`first_snippet()`) survives as an internal helper for the hybrid extractive-first optimization (Section 5.5) and LLM failure fallback (Section 5.6). It is no longer a user-facing compression mode.

### 2.2 Pipeline (within `assemble_uncached_with_memory`)

1. Allocate system prompt + tools + memory (unchanged).
2. **Microcompact** stale tool results in older messages.
3. **Group** messages by conversation turn (not fixed-5).
4. **Score** each turn group via cognitive relevance (optional).
5. **Assign tiers** based on recency + score.
6. **Compress** Tier 1 (structured LLM) and Tier 2 (aggressive LLM or extractive fallback).
7. **Delta optimization** — skip re-compression of already-summarized prefix.

### 2.3 Early Exit

```rust
if turns.len() <= config.tier0_messages(depth_mode) {
    // All turns fit in Tier 0 — return history verbatim, no compression
    return CompressedHistory::verbatim(history);
}
```

Short sessions skip the entire pipeline.

## 3. Turn-Based Grouping

### 3.1 ConversationTurn

```rust
pub struct ConversationTurn {
    pub messages: Vec<Message>,
    pub turn_index: usize,
    pub token_count: usize,
    pub cognitive_score: Option<f64>,
}
```

### 3.2 Grouping Rules

- A **turn** starts at each `Message::User`.
- Everything until the next `Message::User` belongs to the current turn (assistant response, tool calls, tool results).
- System messages at the start form a **preamble group** — never compressed (always Tier 0).
- Adjacent tool-call + tool-result pairs are never split.
- `ContextUpdate` messages are attached to the turn they appear in.

### 3.3 Scoring Content

Each turn produces a lightweight text representation for the cognitive scorer:

```rust
impl ConversationTurn {
    pub fn scoring_content(&self) -> String {
        // Concatenate: user message + final assistant text + key tool outcomes
        // Lightweight — used only for embedding + 12-factor scoring
    }
}
```

### 3.4 Batching for LLM Summarization

Multiple turns are batched into one LLM call (up to 5 turns per call, matching `MAX_SEGMENTS_PER_CALL`). Batch boundaries always align to turn boundaries.

## 4. Cognitive-Aware Tier Assignment

### 4.1 Algorithm

```
Input: Vec<ConversationTurn> (grouped, scored)
Output: each turn tagged as Tier0 / Tier1 / Tier2

1. Last `tier0_count` turns → Tier 0 (always verbatim)

2. For remaining turns (oldest to newest):
   a. cognitive_score >= HIGH_RELEVANCE_THRESHOLD (0.70)
      → Tier 1 (detailed, even if old — cognitive promotion)
   b. Within Tier 1 recency window (last `tier1_demotion_threshold` turns
      before Tier 0, i.e. turns that are not yet old enough to demote)
      OR score >= LOW_RELEVANCE_THRESHOLD (0.40)
      → Tier 1 (detailed)
   c. Else
      → Tier 2 (condensed gist)
```

### 4.2 Cognitive Scoring

The `MemoryScorer` trait lives in `context_engine` (L3) to avoid depending on `cognitive` (L5):

```rust
// crates/context_engine/src/memory_scorer.rs
#[async_trait]
pub trait MemoryScorer: Send + Sync {
    /// Score relevance of text passages (0.0–1.0).
    async fn score_batch(&self, texts: &[String]) -> Vec<f64>;
}
```

Implemented in the `agent` crate by wrapping `UnifiedMemoryService`. Injected via `ContextEngine::with_memory_scorer()`.

When `use_cognitive_scoring = false`: skip the scoring step. All non-Tier-0 turns assigned by pure recency: newer half → Tier 1, older half → Tier 2.

### 4.3 DepthMode Interaction

| DepthMode | `tier0_count` | Tier 1 ratio | Tier 2 ratio |
|-----------|--------------|--------------|--------------|
| Normal    | 8            | 35%          | 12%          |
| DeepThink | 12           | 40%          | 15%          |
| Ultra     | 16           | 45%          | 20%          |

Values are configurable via `cognitive.historyCompression`.

## 5. Structured Prompts (Per-Tier)

### 5.1 Tier 1 — Detailed Summary

```
Summarize each conversation turn below. For each turn, preserve:
- Decisions made and their reasoning
- Action items or commitments
- File paths, function names, IDs, or other specific references
- Key questions asked and answers given
- Errors encountered and how they were resolved
- Any constraints or requirements stated

Preserve temporal order of events. Use bullet points. Never invent information.
Keep technical details (exact names, numbers, paths). Remove pleasantries,
repetition, and verbose explanations. Target ~35% of original length.

Return ONLY a JSON array of exactly {N} strings, one summary per turn.
No extra text.
```

### 5.2 Tier 2 — Condensed Gist

```
For each conversation turn below, extract ONLY:
- The final outcome or decision (one sentence)
- Any unresolved item that affects later conversation (prefix with "UNRESOLVED:")

No code, no file paths, no reasoning chains. Maximum 2 sentences per turn.
Target ~12% of original length.

Return ONLY a JSON array of exactly {N} strings, one summary per turn.
No extra text.
```

### 5.3 Integration with LlmSummaryProvider

The existing `LlmSummaryProvider` is updated (not extended) to always accept a tier:

```rust
pub enum CompressionTier {
    Detailed,   // Tier 1
    Condensed,  // Tier 2
}

impl LlmSummaryProvider {
    fn build_batch_prompt(
        segments: &[Vec<Message>],
        tier: CompressionTier,
    ) -> String {
        let instructions = match tier {
            CompressionTier::Detailed => TIER1_INSTRUCTIONS,
            CompressionTier::Condensed => TIER2_INSTRUCTIONS,
        };
        // Same batching structure, different preamble
    }
}
```

The old single-prompt `build_batch_prompt(segments)` signature is removed. `SummaryProvider::summarize_batch` gains a `tier: CompressionTier` parameter:

```rust
#[async_trait]
pub trait SummaryProvider: Send + Sync {
    async fn summarize_batch(
        &self,
        segments: Vec<Vec<Message>>,
        tier: CompressionTier,
    ) -> Vec<Result<String, String>>;
}
```

Prompts stored as constants in `crates/context_engine/src/history_compressor/prompts.rs`.

### 5.4 Max Tokens Per LLM Call

- Tier 1: `150 * n_segments` (room for detail).
- Tier 2: `60 * n_segments` (tight budget).

### 5.5 Hybrid Extractive-First Optimization

Before calling the LLM, check if extractive summaries already fit within the tier's token budget:

```rust
let extractive = first_snippet_summary(chunk, snippet_len);
let extractive_tokens = token_counter.estimate_text(&extractive);

if extractive_tokens <= tier_budget {
    return extractive; // no LLM call needed
}
// Otherwise, call LLM for abstractive summary
```

Saves 40–60% of LLM calls for sessions where history isn't very long. Very short turns (< 30 tokens) always use extractive regardless of tier.

### 5.6 Fallback

On LLM failure, fall back to extractive per-segment. The pipeline never breaks.

## 6. Delta Compaction (Anchored Summarization)

### 6.1 Persistent Storage

New columns on `sessions` table (in-place migration, pre-release):

```sql
ALTER TABLE sessions ADD COLUMN compressed_prefix TEXT;
ALTER TABLE sessions ADD COLUMN compressed_through_idx INTEGER;
ALTER TABLE sessions ADD COLUMN compressed_at TEXT;
```

### 6.2 TierSummary (Persisted as JSON)

```rust
#[derive(Serialize, Deserialize)]
pub struct TierSummary {
    pub tier: CompressionTier,
    pub content: String,
    pub turn_range: (usize, usize),
    pub token_count: usize,
    pub cognitive_score: Option<f64>,
}
```

`compressed_prefix` stores `Vec<TierSummary>` serialized as JSON.

### 6.3 Resume Flow

```
Session resumes with 100 messages total.
compressed_through_idx = 60

1. Load compressed_prefix → Vec<TierSummary>
2. Delta = messages[60..100] (40 new messages)
3. Group delta into turns
4. Score delta turns (if cognitive scoring enabled)
5. Assign tiers for delta turns
6. Compress delta Tier 1 & Tier 2
7. Merge:
   a. Existing Tier 2 summaries → keep as-is
   b. Existing Tier 1 summaries where turn_range end
      is > tier1_demotion_threshold turns from current end
      → demote to Tier 2 (re-compress the Tier 1 text, not raw messages)
   c. New summaries appended
8. Update compressed_through_idx = 100, persist
9. Assemble: [Tier 2] + [Tier 1] + [Tier 0 verbatim]
```

### 6.4 Tier Demotion

Existing Tier 1 summaries that are now far from the recent window get demoted. Since we already have the Tier 1 summary text (not raw messages), the Tier 2 LLM call processes the summary — much cheaper than re-processing raw messages. The persisted `cognitive_score` enables demotion decisions without re-scoring.

Default `tier1_demotion_threshold`: 30 turns from current end.

### 6.5 Invalidation

If the user edits or deletes a message (rare in chat): clear `compressed_prefix` and recompute from scratch.

### 6.6 Cost Savings

For a 100-turn session resuming 5 times (20 new turns each):
- Today: 100 turns × 5 = 500 turn-compressions.
- With delta: 100 total (20 new per resume × 5).
- **80% reduction** in LLM summarization calls.

## 7. Tool-Result Microcompaction

Before tier compression, scan older tool results and prune stale ones. This extends the mid-loop `MidLoopCompressor` pattern to assembly time for sessions that resume after hours/days.

**Target tools:** Read, Bash, Grep, Glob, WebSearch, WebFetch (same as mid-loop compressor's compactable set).

**Replacement format:**
```
[Tool {name} result compressed – key outcome: {first_snippet}]
```

This runs as a pre-pass before turn grouping, reducing token count before the tiered pipeline even starts.

## 8. Configuration

### 8.1 Config Schema

Added to `CognitiveConfig` as `history_compression: HistoryCompressionConfig`:

```rust
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
    pub tier1_ratio: f32,  // 0.35

    /// Target compression ratio for Tier 2 summaries.
    #[serde(default = "default_tier2_ratio")]
    pub tier2_ratio: f32,  // 0.12

    /// Cognitive score threshold for promoting old turns to Tier 1.
    #[serde(default = "default_high_threshold")]
    pub high_relevance_threshold: f64,  // 0.70

    /// Cognitive score threshold for keeping turns in Tier 1 vs Tier 2.
    #[serde(default = "default_low_threshold")]
    pub low_relevance_threshold: f64,  // 0.40

    /// Turns from current end before Tier 1 demotes to Tier 2.
    #[serde(default = "default_demotion_threshold")]
    pub tier1_demotion_threshold: usize,  // 30
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TierZeroConfig {
    #[serde(default = "default_8")]
    pub normal: usize,     // 8
    #[serde(default = "default_12")]
    pub deep_think: usize, // 12
    #[serde(default = "default_16")]
    pub ultra: usize,      // 16
}
```

No `enabled` field — compression always runs (with early exit for short sessions).
No `mode` field — tiered is the only behavior.

### 8.2 JSON Example

```json
{
  "cognitive": {
    "historyCompression": {
      "model": "claude-haiku-4-5-20251001",
      "useCognitiveScoring": true,
      "deltaOnlyOnResume": true,
      "tier0Messages": { "normal": 8, "deepThink": 12, "ultra": 16 },
      "tier1Ratio": 0.35,
      "tier2Ratio": 0.12,
      "highRelevanceThreshold": 0.70,
      "lowRelevanceThreshold": 0.40,
      "tier1DemotionThreshold": 30
    }
  }
}
```

## 9. Integration Map

### 9.1 New Files

| File | Contents |
|------|----------|
| `context_engine/src/history_compressor/tiered.rs` | `TieredHistoryCompressor` — core pipeline |
| `context_engine/src/history_compressor/grouping.rs` | Turn-based grouping logic |
| `context_engine/src/history_compressor/prompts.rs` | Tier 1 & Tier 2 prompt constants |
| `context_engine/src/memory_scorer.rs` | `MemoryScorer` trait |
| `agent/src/adapters/memory_scorer_impl.rs` | `MemoryScorer` impl wrapping `UnifiedMemoryService` |

### 9.2 Modified Files

| File | Change |
|------|--------|
| `context_engine/src/history_compressor/mod.rs` | Delete old `HistoryCompressor` impl; re-export `TieredHistoryCompressor` as the public API |
| `context_engine/src/history_compressor/types.rs` | Delete `CompressorConfig`, `CompressorMode`; replace with `ConversationTurn`, `TierSummary`, `CompressionTier`, `CompressedHistory` |
| `context_engine/src/history_compressor/snippet.rs` | Kept as-is — internal helper for extractive fallback |
| `context_engine/src/summary_provider.rs` | Add `tier: CompressionTier` parameter to `summarize_batch` |
| `context_engine/src/assembler/mod.rs` | Replace `HistoryCompressor` usage with `TieredHistoryCompressor`; accept `MemoryScorer` |
| `context_engine/src/lib.rs` | Update re-exports: remove old types, add new |
| `agent/src/adapters/llm_summary.rs` | Update `build_batch_prompt` to accept `CompressionTier`; add two prompt constants; remove old generic prompt |
| `agent/src/agent_loop/builder.rs` | Wire `MemoryScorer` + `HistoryCompressionConfig` into `ContextEngine`; use config model override for `LlmSummaryProvider` |
| `config/src/schema/cognitive.rs` | Add `HistoryCompressionConfig`, `TierZeroConfig` |
| `session/src/manager.rs` | Add `save_compressed_prefix()`, `load_compressed_prefix()` methods |
| `storage` crate | In-place migration: 3 new columns on `sessions` |
| `bus/src/events.rs` | Add `AgentEvent::ContextTieredCompressed` variant |

### 9.3 Deleted Code

| Code | Reason |
|------|--------|
| `HistoryCompressor` struct (`history_compressor/mod.rs`) | Replaced by `TieredHistoryCompressor` |
| `CompressorConfig` struct (`history_compressor/types.rs`) | Replaced by `HistoryCompressionConfig` in `config` crate |
| `CompressorMode` enum (`Extractive` / `Abstractive`) | No mode switch — tiered is the only behavior |
| `HistoryCompressor::compress()` (sync method) | Replaced by `TieredHistoryCompressor::compress_async()` |
| `HistoryCompressor::extractive_summary()` (public) | `first_snippet` kept as internal helper; public extractive API removed |
| Old `compress_async` impl | Replaced entirely by tiered pipeline |
| All old `HistoryCompressor` tests | Rewritten for tiered behavior |

### 9.4 Unchanged

- `MidLoopCompressor` — continues to handle in-loop tool-result compression.
- `LiveContextRefresher` — unchanged, injects updates post-assembly.
- Budget waterfall priorities — `CompressedHistory` stays lowest priority.
- All non-compression context sources (skills, persona, memory retrieval).

## 10. Streaming Event

```rust
AgentEvent::ContextTieredCompressed {
    tier0_kept: usize,
    tier1_tokens: usize,
    tier2_tokens: usize,
    cognitive_scoring_used: bool,
    delta_only: bool,
    llm_calls_saved: usize,
}
```

Emitted after successful tiered compression in the assembler.

## 11. Testing Strategy

### 11.1 Unit Tests

- **Grouping** (`grouping.rs`): turn boundary detection with `Message::User` splits, preamble handling for leading system messages, tool-call + tool-result pairing stays together, `ContextUpdate` attachment.
- **Tier assignment** (`tiered.rs`): pure recency fallback (no scorer), cognitive promotion of old high-score turns, DepthMode tier0_count variation, demotion threshold boundary.
- **Prompts** (`llm_summary.rs`): Tier 1 vs Tier 2 prompt selection, max-token scaling per tier.
- **Delta merge** (`tiered.rs`): resume with existing prefix, Tier 1 → Tier 2 demotion using persisted cognitive_score, invalidation clears prefix.
- **Hybrid extractive-first** (`tiered.rs`): extractive fits budget → no LLM call; exceeds budget → LLM called.

### 11.2 Integration Tests

- Assemble with mock `SummaryProvider` + mock `MemoryScorer` → verify 3-tier output structure.
- Delta resume: assemble → persist prefix → add messages → re-assemble → verify only delta compressed.
- Hybrid extractive-first: verify LLM not called when extractive fits budget.
- Fallback: LLM failure → extractive per-segment, pipeline completes successfully.
- Short session early exit: fewer turns than `tier0_count` → no compression, all verbatim.

## 12. Non-Goals

- **Desktop UI toggle** — config.json edit is sufficient for now.
- **Autotuner integration** — no `TrialParams` for compression in this spec.
- **Per-session config override** — same config for all sessions.
- **Multi-modal compression** — text only.
- **Observability dashboard** — streaming event + logs are sufficient.

## 13. Future Work

- **Reforge signal**: Add "compression quality" metric to nightly reforge cycle for auto-tuning thresholds.
- **Prompt cache optimization**: Partial compaction that preserves cached prefix for prompt-cache-friendly APIs.
- **Embedding-based stale detection**: Use LanceDB cosine similarity to detect redundant tool results in microcompaction (beyond simple snippet extraction).
