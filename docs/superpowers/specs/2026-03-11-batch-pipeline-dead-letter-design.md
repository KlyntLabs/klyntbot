# Batch LLM Operations & Dead-Letter Queue

> Design spec for recommendations #3 and #4 from system-architecture-analysis.md
> Date: 2026-03-11
> Scope: Micro-batch cognitive pipeline + self-healing dead-letter retry

## Problem

The cognitive memory pipeline processes domain events one-at-a-time. Each `Extract`-verdict event triggers 1 LLM extraction call + N consolidation calls (one per extracted fact with existing matches). A burst of 5 events producing 2 facts each makes 15 LLM calls. This is wasteful and slow.

Additionally, when LLM calls fail (network error or JSON parse failure), the pipeline silently falls back to heuristic handlers. These produce weaker facts, and the original observation is permanently lost — no retry, no record of the failure.

## Solution

### Part 1: Micro-Batch Event Loop

Replace the single-event `recv()` loop in `BackgroundConsolidationService` with a collect-then-process pattern:

1. **Collect:** Buffer incoming events for up to 3 seconds or until 10 events accumulate (whichever comes first)
2. **Classify:** Run salience evaluation on each event, split into extract/accumulate/discard buckets
3. **Batch extract:** One LLM call for all `Extract` observations — prompt contains N observations, response contains facts grouped by observation index
4. **Pre-fetch:** Concurrently `find_similar()` for all extracted fact candidates via `join_all`
5. **Batch consolidate:** One LLM call for all candidate-vs-existing pairs — prompt contains all decisions, response contains one `MemoryOp` per candidate
6. **Execute:** Apply all `MemoryOp`s to the repo and embedder
7. **Accumulate:** Process `Accumulate`-verdict events through the existing accumulator logic

```
loop {
    let batch = collect_batch(&mut event_rx, &cancel, Duration::from_secs(3), 10).await;
    if batch.is_empty() { continue; }

    let (to_extract, to_accumulate) = classify_batch(batch);

    if !to_extract.is_empty() {
        let result = extraction_handler.extract_facts_batch(&to_extract).await;

        // Queue fallback observations to dead-letter
        for idx in &result.fallback_indices {
            failed_obs_repo.insert(&to_extract[*idx], "extraction", &reason).await;
        }

        // Episodic memory for high-importance observations
        for obs in &to_extract {
            if obs.importance >= 0.7 {
                insert_episodic_memory(obs, &episodic_repo).await;
            }
        }

        let candidates = prefetch_existing(&result.extractions, &repo).await;
        let ops = consolidation_handler.decide_batch(&candidates).await;
        execute_memory_ops(&ops, &candidates, &repo, embedder.as_deref()).await;

        // Self-healing: drain dead-letter if LLM is healthy
        if result.fallback_indices.is_empty() {
            drain_dead_letter(&failed_obs_repo, &mut reprocess_queue).await;
        }
    }

    // Accumulate events; promoted observations queue for the NEXT batch cycle
    handle_accumulation(to_accumulate, &mut accumulator, &mut reprocess_queue);
}
```

**`collect_batch` behavior:**
- Uses `tokio::select!` with a `tokio::time::sleep(3s)` timeout
- Handles `RecvError::Lagged(n)` by logging the count and continuing to collect
- Handles `RecvError::Closed` by returning whatever has been collected so far
- Checks the cancellation token; if cancelled during collection, returns the partial batch
- Single events still process within 3 seconds (timer fires after first event)

**`execute_memory_ops` replaces `consolidate_fact`/`consolidate_batch`:** The existing `consolidate_fact()` function combines repo lookup + LLM decision + repo mutation in one call. In the batch model, these are separated: pre-fetch happens before the LLM call, decisions come from the batch handler, and `execute_memory_ops` applies the results. This function replicates the match-arm logic from current `consolidate_fact` lines 80-107:

```rust
async fn execute_memory_ops(
    ops: &[MemoryOp],
    candidates: &[ConsolidationCandidate],
    repo: &SemanticFactRepo,
    embedder: Option<&dyn SemanticFactEmbedder>,
) {
    for (op, candidate) in ops.iter().zip(candidates.iter()) {
        match op {
            MemoryOp::Add { .. } => {
                repo.upsert(&candidate.candidate).await;
                try_embed(embedder, &candidate.candidate).await;
            }
            MemoryOp::Update { id, old_id } => {
                repo.supersede(old_id, id).await;
                repo.upsert(&candidate.candidate).await;
                try_remove_embedding(embedder, old_id).await;
                try_embed(embedder, &candidate.candidate).await;
            }
            MemoryOp::Delete { id, superseded_by } => {
                repo.supersede(id, superseded_by).await;
                try_remove_embedding(embedder, id).await;
            }
            MemoryOp::Noop => {}
        }
    }
}
```

`consolidate_fact()` and `consolidate_batch()` are removed — their logic is split across `prefetch_existing` (repo lookup), `decide_batch` (LLM decision), and `execute_memory_ops` (repo mutation).

**Accumulator promotion timing:** When an accumulated event promotes, the summary observation is added to the `reprocess_queue`, which feeds into the *next* batch cycle's extraction. It does not enter the current batch. This keeps the batch boundaries clean and avoids re-entering the extraction handler mid-cycle.

### Part 2: Batch Handler Traits

Breaking change to both handler traits — they become batch-native:

```rust
#[async_trait]
pub trait ExtractionHandler: Send + Sync {
    async fn extract_facts_batch(
        &self,
        observations: &[Observation],
    ) -> common::Result<BatchExtractionResult>;
}

#[async_trait]
pub trait ConsolidationHandler: Send + Sync {
    async fn decide_batch(
        &self,
        candidates: &[ConsolidationCandidate],
    ) -> common::Result<Vec<MemoryOp>>;
}
```

New supporting types:

```rust
/// Maps extracted facts back to their source observation.
pub struct BatchExtraction {
    pub observation_index: usize,
    pub facts: Vec<ExtractedFact>,
}

/// Result of batch extraction, including fallback tracking.
pub struct BatchExtractionResult {
    pub extractions: Vec<BatchExtraction>,
    pub fallback_indices: Vec<usize>,  // observations that used heuristic fallback
}

/// A candidate fact paired with its existing matches for consolidation.
pub struct ConsolidationCandidate {
    pub candidate: SemanticFact,
    pub existing: Vec<SemanticFact>,
}
```

### Part 3: LLM Batch Prompts

**Extraction prompt:** The system prompt stays the same (extract structured semantic facts). The user message changes from a single observation to a numbered list:

```
Observation 1:
Domain: productivity
Source: ProductivityScoreComputed
Importance: 0.5
Content: Productivity score for 2026-03-10: 82.5

Observation 2:
Domain: general
Source: ChatTurnCompleted
Content: I prefer working with TypeScript over JavaScript

Extract facts from ALL observations. Return JSON:
{"results": [{"observation_index": 1, "facts": [...]}, {"observation_index": 2, "facts": [...]}]}
```

**Consolidation prompt:** Similar batching — all candidate-vs-existing pairs in one prompt:

```
Decision 1:
Candidate: user.peak_hours = 9am-11am (confidence: 0.8)
Existing: [{"id": "f1", "subject": "user", "predicate": "peak_hours", "object": "10am-12pm"}]

Decision 2:
Candidate: user.prefers_typescript = true (confidence: 0.9)
Existing: []

Decide for ALL candidates. Return JSON:
{"decisions": [{"index": 1, "action": "update", "target_id": "f1"}, {"index": 2, "action": "add", "target_id": null}]}
```

**Prompt indexing:** Prompt indices are 1-based (for LLM readability). When mapping the JSON response back to the `observations`/`candidates` Vec, decrement by 1. Missing indices in the response are treated as empty extractions or Noop decisions.

**Heuristic handlers:** Loop internally over the batch, applying existing single-item logic per observation/candidate. Wrapped in the batch interface.

**Fallback behavior:** If the batch LLM call fails entirely, all observations in the batch fall back to heuristics and all are queued to the dead-letter table. If the LLM succeeds but JSON parsing fails for specific observations, only those fall back.

### Part 4: Dead-Letter Queue

**Table (cognitive migration):**

```sql
CREATE TABLE IF NOT EXISTS failed_observations (
    id TEXT PRIMARY KEY,
    observation_json TEXT NOT NULL,
    failure_reason TEXT NOT NULL,      -- 'llm_error' or 'parse_error'
    failed_stage TEXT NOT NULL,        -- 'extraction' or 'consolidation'
    retry_count INTEGER NOT NULL DEFAULT 0,
    max_retries INTEGER NOT NULL DEFAULT 3,
    created_at TEXT NOT NULL DEFAULT (datetime('now')),
    next_retry_at TEXT
);

CREATE INDEX IF NOT EXISTS idx_failed_observations_eligible
    ON failed_observations(retry_count, next_retry_at);
```

**Repo:** `FailedObservationRepo` with:
- `insert(observation, stage, reason)` — serialize observation to JSON, generate UUID
- `list_eligible(limit)` — `WHERE retry_count < max_retries AND (next_retry_at IS NULL OR next_retry_at <= datetime('now')) ORDER BY created_at ASC LIMIT ?`
- `mark_succeeded(id)` — delete the row
- `mark_failed(id)` — increment `retry_count`, set `next_retry_at = datetime('now', '+' || (retry_count * 5) || ' minutes')` (5min, 10min, 15min backoff)
- `count_pending()` — for observability

**Self-healing drain:** After each successful batch LLM extraction (no fallbacks), the background service pulls up to 5 eligible dead-letter items and adds them to the next micro-batch's observation list for reprocessing. On success, the heuristic-generated facts get naturally superseded by consolidation (same subject+predicate → UPDATE). On failure, `retry_count` increments. After 3 retries, the item stays in the table but is no longer eligible.

### Part 5: Pipeline Events

Updated `PipelineEvent` for batch observability:

```rust
#[derive(Debug, Clone, Serialize)]
#[serde(tag = "type")]
pub enum PipelineEvent {
    BatchStarted {
        #[serde(rename = "observationCount")]
        observation_count: usize,
    },
    Extraction {
        observation: String,
        #[serde(rename = "factsExtracted")]
        facts_extracted: usize,
        #[serde(rename = "usedFallback")]
        used_fallback: bool,
    },
    Consolidation {
        operation: String,
        fact: String,
    },
    DeadLetterQueued {
        observation: String,
        #[serde(rename = "failureReason")]
        failure_reason: String,
    },
    DeadLetterReprocessed {
        observation: String,
        #[serde(rename = "factsExtracted")]
        facts_extracted: usize,
    },
}
```

Note: The existing `Extraction` variant's `#[serde(rename = "factsExtracted")]` is preserved. New fields use camelCase serde renames to match the desktop-ui convention.

## What Stays Unchanged

- **Salience evaluation** — pure match, no change
- **Accumulator logic** — same promotion rules (threshold + min_days), processes inside the batch window
- **Episodic memory insertion** — same importance >= 0.7 threshold
- **`event_to_observation` mapping** — same conversions
- **Reflection handler** — separate weekly cycle, not part of this pipeline
- **`event_type_key` and `summarize_accumulated`** — unchanged

## Files Changed

| Action | File |
|--------|------|
| Rewrite | `crates/cognitive/src/background.rs` — batch event loop, dead-letter drain |
| Rewrite | `crates/cognitive/src/extraction.rs` — batch trait, `BatchExtraction` types |
| Rewrite | `crates/cognitive/src/consolidation.rs` — batch trait, `ConsolidationCandidate` type |
| Rewrite | `crates/agent/src/cognitive_handlers.rs` — batch LLM prompts, fallback tracking |
| Modify | `crates/agent/src/agent_loop/builder.rs` — wire new handler signatures |
| Add | `crates/cognitive/src/repos/failed_observation.rs` — `FailedObservationRepo` |
| Modify | `crates/cognitive/src/repos/mod.rs` — add `FailedObservationRepo` export |
| Modify | `crates/cognitive/src/lib.rs` — re-export new types |
| Add | Cognitive migration for `failed_observations` table |

## LLM Call Reduction

| Scenario | Before | After |
|----------|--------|-------|
| 5 events, 2 facts each | 5 extraction + 10 consolidation = **15 calls** | 1 extraction + 1 consolidation = **2 calls** |
| 1 event, 1 fact | 1 + 1 = **2 calls** | 1 + 1 = **2 calls** (no regression) |
| 10 events burst | 10 + N = **10+ calls** | 1 + 1 = **2 calls** |

## Testing Strategy

| Test | Location | Validates |
|------|----------|-----------|
| `collect_batch` drains within window | `background.rs` | 3s timeout, 10-event cap, cancellation |
| Batch extraction groups facts by index | `extraction.rs` | `BatchExtraction` mapping correct |
| Batch consolidation returns per-candidate ops | `consolidation.rs` | `ConsolidationCandidate` → `MemoryOp` |
| Heuristic fallback indices tracked | `cognitive_handlers.rs` | `fallback_indices` populated correctly |
| Dead-letter insert on fallback | `background.rs` | Observation persisted to `failed_observations` |
| Dead-letter drain on LLM success | `background.rs` | Eligible items reprocessed, removed on success |
| Dead-letter max retries respected | `failed_observation.rs` | Items with 3 retries excluded from `list_eligible` |
| Pipeline events reflect batch context | `background.rs` | `BatchStarted`, `used_fallback`, `DeadLetterQueued` emitted |
| Full pipeline integration | `background.rs` | Events → batch → extract → consolidate → facts in repo |
