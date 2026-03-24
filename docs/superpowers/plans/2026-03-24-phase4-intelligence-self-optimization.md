# Phase 4: Intelligence & Self-Optimization Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** The final push to 5/5 — make every component adaptive, self-correcting, and audit-trailed across ReAct, memory, skills, context engine, autotuner, squads, config, and tools.

**Architecture:** 11 tasks grouped into 4 streams by dependency. Stream A (ReAct + Tools) and Stream B (Memory + Cognitive) and Stream C (Skills + Config) and Stream D (Autotuner + Squad) are independent. Within each stream, tasks are sequential.

**Tech Stack:** Rust, SQLite (sqlx), tokio, serde_json, chrono, regex

---

## File Structure

| Task | Files to Modify/Create | Risk |
|------|----------------------|------|
| 1. Semantic Plan Tracking | `crates/agent/src/execution/scratchpad.rs`, `crates/agent/src/intent_pipeline/engines/reactive.rs` | Low |
| 2. Memory Confirmation | `crates/config/src/schema/cognitive.rs`, `crates/cognitive/src/services/background.rs`, `crates/cognitive/src/repos/` (new pending repo), `crates/bus/src/events.rs` | High |
| 3. Contradiction Detection | `crates/cognitive/src/services/background.rs`, `crates/cognitive/src/repos/semantic_fact.rs` | Medium |
| 4. Embedding Hot-Reload | `crates/skill-system/src/catalog.rs`, `crates/agent/src/agent_loop/builder.rs` | Medium |
| 5. Routing Disambiguation | `crates/skill-system/src/router.rs` | Low |
| 6. Retrieval Audit Trail | `crates/context_engine/src/insight_forge/mod.rs`, `crates/storage/src/repos/` (new audit repo) | Medium |
| 7. Autotuner Fixes | `crates/autotuner/src/cycle.rs`, `crates/storage/src/repos/trial_repo.rs`, `crates/agent/src/autotuner/hooks.rs`, `crates/cognitive/src/services/background.rs` | Medium |
| 8. Autotuner Diagnostics | `crates/autotuner/src/cycle.rs`, `crates/agent/src/autotuner/mod.rs` | Low |
| 9. Squad Cancellation | `crates/agent/src/intent_pipeline/engines/debate.rs`, `crates/agent/src/agent_runtime/runtime.rs` | Medium |
| 10. Config Schema Versioning | `crates/config/src/loader.rs`, `crates/config/src/schema/core.rs` | Medium |
| 11. Structured Tool Output | `crates/tools-core/src/lib.rs`, `crates/agent/src/execution/core.rs`, all tool crates | High |

### Dependency Graph

```
Stream A:                  Stream B:              Stream C:           Stream D:
  Task 1 (Plan Tracking)    Task 3 (Contradict)    Task 4 (Embed)      Task 7 (AT Fixes)
       ↓                    Task 2 (Confirm)       Task 5 (Disambig)   Task 8 (AT Diag)
  Task 11 (Tool Output)          ↓                 Task 10 (Config)    Task 9 (Squad)
                            Task 6 (Audit)
```

---

## Task 1: Semantic Plan Tracking + Loop Detection

**Problem:** `mark_step_completed()` in `scratchpad.rs:L121` matches by exact tool name only. Same tool for different plan steps confuses tracking. No oscillation detection.

**Files:**
- Modify: `crates/agent/src/execution/scratchpad.rs`
- Modify: `crates/agent/src/intent_pipeline/engines/reactive.rs`

- [ ] **Step 1: Write tests for semantic matching**

Add to `scratchpad.rs` tests:

```rust
#[test]
fn test_mark_step_completed_semantic_with_args() {
    let mut scratchpad = Scratchpad::new();
    scratchpad.plan = Some(ExecutionPlan {
        steps: vec![
            PlanStep { index: 0, description: "Search for rust tutorials".into(), expected_tool: Some("notes".into()), completed: false },
            PlanStep { index: 1, description: "Search for python guides".into(), expected_tool: Some("notes".into()), completed: false },
        ],
        raw_text: String::new(),
    });

    // Should match step 0 because args contain "rust tutorials"
    let result = scratchpad.mark_step_completed_semantic("notes", &serde_json::json!({"query": "rust tutorials"}), "");
    assert_eq!(result.map(|r| r.0), Some(0));

    // Should match step 1 because args contain "python guides"
    let result = scratchpad.mark_step_completed_semantic("notes", &serde_json::json!({"query": "python guides"}), "");
    assert_eq!(result.map(|r| r.0), Some(1));
}

#[test]
fn test_mark_step_completed_semantic_fallback() {
    let mut scratchpad = Scratchpad::new();
    scratchpad.plan = Some(ExecutionPlan {
        steps: vec![
            PlanStep { index: 0, description: "Do something".into(), expected_tool: Some("tasks".into()), completed: false },
        ],
        raw_text: String::new(),
    });

    // No word overlap — falls back to name-only matching
    let result = scratchpad.mark_step_completed_semantic("tasks", &serde_json::json!({"action": "list"}), "");
    assert_eq!(result.map(|r| r.0), Some(0));
}
```

- [ ] **Step 2: Implement `mark_step_completed_semantic()`**

Add to `impl Scratchpad` in `scratchpad.rs`:

```rust
/// Enhanced plan step matching using tool name + argument/result word overlap.
/// Falls back to name-only matching if no semantic match is found.
pub fn mark_step_completed_semantic(
    &mut self,
    tool_name: &str,
    tool_args: &serde_json::Value,
    tool_result: &str,
) -> Option<(usize, String, String)> {
    let plan = self.plan.as_mut()?;
    let arg_text = tool_args.to_string().to_lowercase();
    let result_lower = tool_result.to_lowercase();

    // Try semantic match: tool name + word overlap with step description
    let mut best_match: Option<(usize, usize)> = None; // (step_idx, overlap_count)
    for (idx, step) in plan.steps.iter().enumerate() {
        if step.completed { continue; }
        if step.expected_tool.as_deref() != Some(tool_name) { continue; }

        let desc_words: std::collections::HashSet<String> = step.description
            .to_lowercase()
            .split_whitespace()
            .filter(|w| w.len() > 2) // skip short words
            .map(|w| w.to_string())
            .collect();

        let overlap = desc_words.iter()
            .filter(|w| arg_text.contains(w.as_str()) || result_lower.contains(w.as_str()))
            .count();

        if overlap > desc_words.len() / 3 && overlap > 0 {
            match best_match {
                Some((_, best_overlap)) if overlap > best_overlap => {
                    best_match = Some((idx, overlap));
                }
                None => best_match = Some((idx, overlap)),
                _ => {}
            }
        }
    }

    if let Some((idx, _)) = best_match {
        let step = &mut plan.steps[idx];
        step.completed = true;
        return Some((step.index, step.description.clone(), tool_name.to_string()));
    }

    // Fallback to name-only matching
    self.mark_step_completed(tool_name)
}
```

- [ ] **Step 3: Wire into ReactiveEngine**

In `reactive.rs`, find where `scratchpad.mark_step_completed(tool_name)` is called (around L240) and replace with:

```rust
// Replace:
scratchpad.mark_step_completed(&tool_name)
// With:
scratchpad.mark_step_completed_semantic(&tool_name, &tool_args, &tool_result)
```

Note: `tool_args` and `tool_result` need to be available at this call site. Read the surrounding code to see what variables hold the tool arguments and results from the current cycle. They should be in the `CycleOutcome::ToolsExecuted` match arm.

- [ ] **Step 4: Add oscillation detection**

In `scratchpad.rs`, add:

```rust
/// Detect if the agent is oscillating between tool calls without progress.
/// Returns true if the last N traces show a repeating pattern.
pub fn detect_oscillation(&self, window: usize) -> bool {
    if self.traces.len() < window * 2 { return false; }
    let recent = &self.traces[self.traces.len() - window * 2..];
    let first_half: Vec<&str> = recent[..window].iter().map(|t| t.actual_action.as_str()).collect();
    let second_half: Vec<&str> = recent[window..].iter().map(|t| t.actual_action.as_str()).collect();
    first_half == second_half
}
```

In `reactive.rs`, after recording a `ReasoningTrace`, check for oscillation:

```rust
if scratchpad.detect_oscillation(3) {
    tracing::warn!("Oscillation detected — agent is repeating the same tool pattern");
    // Break the loop early or inject a reflection prompt
    break;
}
```

- [ ] **Step 5: Run tests and clippy**

Run: `cargo nextest run -p agent -E 'test(mark_step_completed_semantic)' -E 'test(oscillation)'`
Run: `cargo clippy -p agent --all-targets`

- [ ] **Step 6: Commit**

---

## Task 2: Memory Confirmation (Human-in-the-Loop)

**Problem:** Memory writes are fully automated. Extraction hallucinations become permanent facts.

**Scope note:** This is a large task spanning backend config, cognitive pipeline, domain events, and desktop UI. The backend portion (config + pending queue + event) is included here. The desktop UI panel is noted as a follow-up.

**Files:**
- Modify: `crates/config/src/schema/cognitive.rs`
- Modify: `crates/cognitive/src/services/background.rs`
- Create: `crates/cognitive/src/repos/pending_memory.rs`
- Modify: `crates/bus/src/events.rs`

- [ ] **Step 1: Add config fields**

In the `CognitiveConfig` struct, add:

```rust
/// Require user confirmation for memory writes below this confidence threshold.
/// Set to 0.0 to auto-approve all. Set to 1.0 to require confirmation for everything.
#[serde(default = "default_confirm_threshold")]
pub confirm_threshold: f32,

fn default_confirm_threshold() -> f32 { 0.0 } // disabled by default
```

- [ ] **Step 2: Create pending memory repo**

Create `crates/cognitive/src/repos/pending_memory.rs`:

```rust
use sqlx::SqlitePool;
use crate::types::SemanticFact;

pub struct PendingMemoryRepo {
    pool: SqlitePool,
}

impl PendingMemoryRepo {
    pub fn new(pool: SqlitePool) -> Self { Self { pool } }

    pub async fn insert(&self, fact: &SemanticFact, reason: &str) -> Result<(), sqlx::Error> {
        sqlx::query(
            "INSERT INTO pending_memories (id, fact_json, reason, created_at) \
             VALUES (?1, ?2, ?3, datetime('now'))"
        )
        .bind(&fact.id)
        .bind(serde_json::to_string(fact).unwrap_or_default())
        .bind(reason)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    pub async fn list_pending(&self, limit: i64) -> Vec<PendingMemoryRow> {
        sqlx::query_as::<_, PendingMemoryRow>(
            "SELECT * FROM pending_memories ORDER BY created_at DESC LIMIT ?1"
        )
        .bind(limit)
        .fetch_all(&self.pool)
        .await
        .unwrap_or_default()
    }

    pub async fn approve(&self, id: &str) -> Result<(), sqlx::Error> {
        sqlx::query("DELETE FROM pending_memories WHERE id = ?1")
            .bind(id)
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    pub async fn reject(&self, id: &str) -> Result<(), sqlx::Error> {
        sqlx::query("DELETE FROM pending_memories WHERE id = ?1")
            .bind(id)
            .execute(&self.pool)
            .await?;
        Ok(())
    }
}

#[derive(Debug, Clone, sqlx::FromRow)]
pub struct PendingMemoryRow {
    pub id: String,
    pub fact_json: String,
    pub reason: String,
    pub created_at: String,
}
```

- [ ] **Step 3: Add migration for pending_memories table**

Add to the cognitive migrations:

```sql
CREATE TABLE IF NOT EXISTS pending_memories (
    id         TEXT PRIMARY KEY,
    fact_json  TEXT NOT NULL,
    reason     TEXT NOT NULL,
    created_at TEXT NOT NULL DEFAULT (datetime('now'))
);
```

- [ ] **Step 4: Add domain event**

In `crates/bus/src/events.rs`, add a variant to `DomainEvent`:

```rust
MemoryPendingConfirmation {
    fact_id: String,
    subject: String,
    predicate: String,
    object: String,
},
```

- [ ] **Step 5: Wire into consolidation**

In `background.rs`, after `execute_memory_ops` (L430-L436), add confirmation gating:

```rust
// Before executing ops, check if confirmation is required
if config.confirm_threshold > 0.0 {
    for (op, candidate) in ops.iter().zip(candidates.iter()) {
        if matches!(op, MemoryOp::Add { .. } | MemoryOp::Update { .. }) {
            if candidate.candidate.confidence < config.confirm_threshold {
                // Queue for confirmation instead of auto-writing
                if let Some(ref pending_repo) = pending_memory_repo {
                    pending_repo.insert(&candidate.candidate, "low confidence").await;
                    bus.publish(DomainEvent::MemoryPendingConfirmation { ... });
                }
                continue; // Skip this op
            }
        }
    }
}
```

Note: The exact wiring depends on how `pending_memory_repo` and `confirm_threshold` are threaded into the consolidation loop. Read the builder and consolidation service construction to find the injection points.

- [ ] **Step 6: Run tests and build**

---

## Task 3: Contradiction Detection

**Problem:** No mechanism to detect when a new fact contradicts an existing one.

**Files:**
- Modify: `crates/cognitive/src/services/background.rs`

- [ ] **Step 1: Read existing contradiction handling**

The explorer found that `background.rs:L439-L466` already has contradiction detection for `Update` ops! It compares old vs new object and publishes `DomainEvent::ContradictionDetected`. Read this code to understand what already exists.

- [ ] **Step 2: Extend contradiction detection to `Add` ops**

Currently contradictions are only detected for `Update` ops. For `Add` ops, check `find_similar()` results:

In the consolidation path, after `prefetch_existing` and before `execute_memory_ops`, add:

```rust
// Check for contradictions on Add ops
for (op, candidate) in ops.iter().zip(candidates.iter()) {
    if matches!(op, MemoryOp::Add { .. }) {
        // existing facts already prefetched in candidate.existing
        for existing in &candidate.existing {
            if existing.object != candidate.candidate.object
                && existing.superseded_at.is_none()
            {
                tracing::info!(
                    "Contradiction detected: existing '{}' vs new '{}'",
                    existing.object, candidate.candidate.object
                );
                // NOTE: Check the actual DomainEvent::ContradictionDetected field names!
                // The real variant (bus/domain_events.rs:L369) uses:
                //   existing_subject, existing_predicate, existing_object, new_object, confidence
                // Match those exact field names when constructing the event.
                if let Some(ref bus) = domain_bus {
                    let _ = bus.publish(DomainEvent::ContradictionDetected {
                        existing_subject: existing.subject.clone(),
                        existing_predicate: existing.predicate.clone(),
                        existing_object: existing.object.clone(),
                        new_object: candidate.candidate.object.clone(),
                        confidence: candidate.candidate.confidence,
                    });
                }
            }
        }
    }
}
```

Note: Read the existing `DomainEvent::ContradictionDetected` variant to match the exact field names.

- [ ] **Step 3: Run tests**

---

## Task 4: Embedding Hot-Reload

**Problem:** Skill embeddings computed once at startup. Hot-reloaded skills use stale embeddings.

**Key finding:** Embeddings are on `SkillCatalog` (not `SkillRouter`). The catalog stores embeddings in its internal state.

**Files:**
- Modify: `crates/skill-system/src/catalog.rs` (or wherever `SkillCatalog` stores embeddings)

- [ ] **Step 1: Read `SkillCatalog`** to find the embeddings storage

```bash
grep -n "embedding" crates/skill-system/src/catalog.rs crates/skill-system/src/discovery.rs
```

- [ ] **Step 2: Add `recompute_embedding()` method**

**IMPORTANT:** `SkillCatalog.embeddings` is `HashMap<String, Vec<f32>>` (NOT behind `RwLock`). Since `SkillCatalog` itself is held behind `Arc<RwLock<SkillCatalog>>` in the runtime, the method needs `&mut self`:

```rust
impl SkillCatalog {
    pub fn recompute_embedding(&mut self, skill_name: &str, embedder: &dyn TextEmbedder) {
        // Note: TextEmbedder might be async. If so, this needs to be async too
        // and the caller must hold a write lock on the catalog.
        if let Some(skill) = self.get(skill_name) {
            // Compute embedding synchronously or via block_in_place
            // Alternatively, accept the pre-computed embedding as a parameter:
        }
    }

    /// Simpler approach: accept pre-computed embedding
    pub fn update_embedding(&mut self, skill_name: &str, embedding: Vec<f32>) {
        self.embeddings.insert(skill_name.to_string(), embedding);
    }
}
```

The caller (file watcher/reload path) computes the embedding async, then acquires the catalog write lock and calls `update_embedding()`.
```

- [ ] **Step 3: Wire into PersonaManager::reload() or file watcher**

Find where skills are reloaded and call `recompute_embedding()` after.

- [ ] **Step 4: Run tests**

---

## Task 5: Routing Disambiguation

**Problem:** Trigger phrase overlap causes ambiguous routing. No tie-breaking beyond iteration order.

**Files:**
- Modify: `crates/skill-system/src/router.rs`

- [ ] **Step 1: Write test for disambiguation**

```rust
#[test]
fn test_disambiguation_picks_more_specific() {
    // Test that when two skills have very close scores,
    // the more specific one (fewer triggers) wins
}
```

- [ ] **Step 2: Add AMBIGUITY_THRESHOLD and specificity tiebreaker**

**IMPORTANT:** The current `select_orchestrator_blended()` (router.rs ~L82-L125) tracks only a single `best: Option<(&str, f64)>` — there is NO candidates vector. You MUST first restructure the function to collect all passing candidates into a `Vec`, sort by score descending, then apply disambiguation.

Restructure approach:

```rust
const AMBIGUITY_THRESHOLD: f64 = 0.05;

// Step 1: Collect ALL candidates that pass the candidacy gate (not just the best)
let mut candidates: Vec<(&str, f64)> = Vec::new();
for skill in catalog.orchestrators() {
    let kw_score = self.keyword_scores(message, skill);
    let sem_score = /* compute semantic score */;
    if kw_score == 0.0 && sem_score < 0.5 { continue; } // existing gate
    let blended = kw_score * kw_w + sem_score * sem_w;
    candidates.push((skill.name(), blended));
}

// Step 2: Sort descending by score
candidates.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));

// Step 3: Disambiguation — if top two are close, prefer the more specific
if candidates.len() >= 2 {
    let gap = candidates[0].1 - candidates[1].1;
    if gap < AMBIGUITY_THRESHOLD {
        let a_triggers = catalog.get(candidates[0].0).map_or(0, |s| s.triggers().len());
        let b_triggers = catalog.get(candidates[1].0).map_or(0, |s| s.triggers().len());
        if b_triggers < a_triggers {
            candidates.swap(0, 1); // more specific wins
        }
    }
}

// Step 4: Return the winner (or fallback to "general")
```

- [ ] **Step 3: Run tests**

---

## Task 6: Retrieval Quality Audit Trail

**Problem:** No visibility into retrieval decisions. Autotuner and debugging lack data.

**Files:**
- Create: `crates/context_engine/src/insight_forge/audit.rs`
- Modify: `crates/context_engine/src/insight_forge/mod.rs`
- Create: `crates/storage/src/repos/retrieval_audit.rs`

- [ ] **Step 1: Define the audit entry struct**

Create `crates/context_engine/src/insight_forge/audit.rs`:

```rust
use std::collections::HashMap;

#[derive(Debug, Clone, serde::Serialize)]
pub struct RetrievalAuditEntry {
    pub query: String,
    pub enriched_query: Option<String>,
    pub sub_queries: Vec<String>,
    pub sources_queried: Vec<String>,
    pub results_per_source: HashMap<String, usize>,
    pub final_results: usize,
    pub diversity_cap_applied: bool,
    pub circuit_breaker_fallback: bool,
    pub total_latency_ms: u64,
}
```

- [ ] **Step 2: Add timing and audit collection in InsightForge**

In `retrieve_with_enrichment()` (insight_forge/mod.rs), add:

```rust
let start = std::time::Instant::now();
// ... existing logic ...
let elapsed = start.elapsed().as_millis() as u64;

let audit = RetrievalAuditEntry {
    query: query.to_string(),
    enriched_query: enriched.map(|r| r.enriched_query.clone()),
    sub_queries: sub_queries.iter().map(|q| q.to_string()).collect(),
    sources_queried: /* collect source names */,
    results_per_source: /* count per source */,
    final_results: budgeted.len(),
    diversity_cap_applied: /* true if budget trimming occurred */,
    circuit_breaker_fallback: /* true if circuit breaker fired */,
    total_latency_ms: elapsed,
};

tracing::debug!(?audit, "Retrieval audit");
```

- [ ] **Step 3: Create rolling storage table**

Create `crates/storage/src/repos/retrieval_audit.rs` with INSERT and cleanup (keep 7 days).

- [ ] **Step 4: Run tests**

---

## Task 7: Autotuner Fixes (3 Sub-Items)

**Files:**
- Modify: `crates/autotuner/src/cycle.rs` (diversity bonus)
- Modify: `crates/storage/src/repos/trial_repo.rs` (add message_id column)
- Modify: `crates/agent/src/autotuner/hooks.rs` (pass message_id)
- Modify: `crates/cognitive/src/services/background.rs` (live accumulation params)

### 7a: Increase diversity bonus

In `cycle.rs`, change the `diversity_bonus` function (L259-L264):

```rust
// From:
0.1 * (distance / max_distance)
// To:
0.3 * (distance / max_distance)
```

### 7b: Add message_id to shadow log

1. Add `message_id TEXT` column to `autotuner_shadow_log` table DDL in `trial_repo.rs`
2. Update `insert_shadow_log` to accept and bind `message_id`
3. Update `update_shadow_log_ground_truth` to match on `(chat_id, message_id)` instead of just `chat_id`
4. Update `on_message_received` in `hooks.rs` to pass the message_id
5. Update `on_message_completed` to pass the message_id

### 7c: Live accumulation params

In `background.rs`, replace the static `promote_threshold`/`min_days` with live-readable values from the champion override:

```rust
// Change BackgroundConsolidationConfig to hold Arc<RwLock<Option<TrialParams>>>
// On each batch cycle, resolve params:
let (threshold, min_days) = if let Some(ref champion) = champion_override {
    if let Ok(guard) = champion.read() {
        if let Some(ref params) = *guard {
            (
                params.accumulate_promote_threshold.unwrap_or(config.promote_threshold) as usize,
                params.accumulate_min_days.unwrap_or(config.min_days as u32) as usize,
            )
        } else { (config.promote_threshold, config.min_days) }
    } else { (config.promote_threshold, config.min_days) }
} else { (config.promote_threshold, config.min_days) };
```

Wire the `champion_override: Arc<std::sync::RwLock<Option<TrialParams>>>` from the autotuner through the builder into the consolidation service.

- [ ] **Step 1-6: Implement all 3 sub-items, run tests, clippy**

---

## Task 8: Autotuner Self-Diagnostic

**Files:**
- Modify: `crates/autotuner/src/cycle.rs`
- Modify: `crates/agent/src/autotuner/mod.rs`

- [ ] **Step 1: Add diagnostic structs**

In `cycle.rs`:

```rust
#[derive(Debug, Clone)]
pub struct AutotunerHealth {
    pub champion_age_days: u32,
    pub shadow_log_volume_24h: usize,
    pub ground_truth_match_rate: f32,
    pub last_promotion_days_ago: u32,
    pub consecutive_no_improvement: u32,
    pub experiment_pace: String,
}

#[derive(Debug, Clone)]
pub enum HealthWarning {
    LowGroundTruthMatch,
    StagnantOptimization,
    InsufficientData,
}

impl AutotunerHealth {
    pub fn diagnose(&self) -> Vec<HealthWarning> {
        let mut warnings = vec![];
        if self.ground_truth_match_rate < 0.8 {
            warnings.push(HealthWarning::LowGroundTruthMatch);
        }
        if self.consecutive_no_improvement > 7 {
            warnings.push(HealthWarning::StagnantOptimization);
        }
        if self.shadow_log_volume_24h < 10 {
            warnings.push(HealthWarning::InsufficientData);
        }
        warnings
    }
}
```

- [ ] **Step 2: Wire into nightly cycle**

In the nightly cycle's `run_evaluation_and_promotion`, after evaluation:

```rust
let health = AutotunerHealth { /* collect metrics */ };
let warnings = health.diagnose();
for warning in &warnings {
    tracing::warn!(?warning, "Autotuner health issue detected");
}
// If stagnant > 7 days, auto-switch pace to "bold"
```

- [ ] **Step 3: Run tests**

---

## Task 9: Squad Cancellation + Resource Limits

**Files:**
- Modify: `crates/agent/src/intent_pipeline/engines/debate.rs`
- Modify: `crates/agent/src/agent_runtime/runtime.rs`

- [ ] **Step 1: Add CancellationToken to `run_room_debate`**

Update the signature:

```rust
pub async fn run_room_debate(
    // ... existing params ...
    cancel_token: Option<CancellationToken>,  // NEW
) -> DebateRounds
```

In the main round loop, check cancellation:

```rust
for round in 1..=MAX_ROUNDS {
    if let Some(ref token) = cancel_token {
        if token.is_cancelled() {
            return DebateRounds::cancelled(round);
        }
    }
    // ... existing round logic ...
}
```

- [ ] **Step 2: Add DebateConfig with resource limits**

```rust
pub struct DebateConfig {
    pub max_rounds: u32,           // default: MAX_ROUNDS (6)
    pub max_time: Duration,        // default: 120s
    pub consensus_threshold: f64,  // default: CONSENSUS_THRESHOLD (85.0)
}
```

Add a timeout wrapper around the debate call in `runtime.rs`:

```rust
let debate_result = tokio::time::timeout(
    debate_config.max_time,
    run_room_debate(/* ... */, cancel_token),
).await;
```

- [ ] **Step 3: Update callers**

Find where `run_room_debate` is called (in `runtime.rs`'s `run_squad_execution`) and pass the cancellation token from the runtime's `cancel_token`.

- [ ] **Step 4: Run tests**

---

## Task 10: Config Schema Versioning

**Files:**
- Modify: `crates/config/src/loader.rs`
- Modify: `crates/config/src/schema/core.rs`

- [ ] **Step 1: Add schemaVersion to Config**

In `core.rs`, add to the `Config` struct:

```rust
#[serde(default = "default_schema_version", rename = "schemaVersion")]
pub schema_version: u32,

fn default_schema_version() -> u32 { 1 }
```

- [ ] **Step 2: Add version constant and migration function**

In `loader.rs`:

```rust
const CURRENT_SCHEMA_VERSION: u32 = 1;

fn migrate_config(mut raw: Value, from: u32, to: u32) -> Result<Value> {
    // For now, no migrations needed (version 1 is the initial version)
    // Future migrations will be added here as match arms:
    // if from < 2 { /* migrate from v1 to v2 */ }
    raw["schemaVersion"] = serde_json::json!(to);
    Ok(raw)
}
```

- [ ] **Step 3: Wire into load()**

In `load()`, after parsing the raw JSON but before deserializing into `Config`:

```rust
pub async fn load() -> Result<Config> {
    let klyntbot_path = config_path()?;
    if klyntbot_path.exists() {
        let content = fs::read_to_string(&klyntbot_path).await.map_err(ConfigError::Io)?;
        let mut raw: Value = serde_json::from_str(&content)
            .map_err(|e| ConfigError::Invalid(format!("Failed to parse config: {}", e)))?;

        let file_version = raw["schemaVersion"].as_u64().unwrap_or(1) as u32;
        if file_version < CURRENT_SCHEMA_VERSION {
            raw = migrate_config(raw, file_version, CURRENT_SCHEMA_VERSION)?;
            // Auto-save migrated config (write back to disk)
            let config: Config = serde_json::from_value(raw)
                .map_err(|e| ConfigError::Invalid(format!("Migration produced invalid config: {}", e)))?;
            save(&config).await?;
            return Ok(config);
        }

        let config: Config = serde_json::from_value(raw)
            .map_err(|e| ConfigError::Invalid(format!("Failed to parse config: {}", e)))?;
        return Ok(config);
    }
    Ok(Config::default())
}
```

- [ ] **Step 4: Add tests**

```rust
#[test]
fn test_migrate_config_adds_version() {
    let raw = serde_json::json!({});
    let migrated = migrate_config(raw, 1, 1).unwrap();
    assert_eq!(migrated["schemaVersion"], 1);
}

#[test]
fn test_default_config_has_version() {
    let config = Config::default();
    assert_eq!(config.schema_version, 1);
}
```

- [ ] **Step 5: Run tests**

---

## Task 11: Structured Tool Output

**Problem:** Tools return `Result<String>`. No structured data for UI/MCP. No retryable error classification.

**Scope note:** This is a major breaking change. The approach is incremental — add `ToolOutput` enum but keep backward compatibility via `From<String>`.

**Files:**
- Modify: `crates/tools-core/src/lib.rs`
- Modify: `crates/agent/src/execution/core.rs`

- [ ] **Step 1: Add `ToolOutput` enum (backward compatible)**

In `tools-core/src/lib.rs`:

```rust
/// Structured tool output — backward compatible with plain String.
#[derive(Debug, Clone)]
pub enum ToolOutput {
    /// Plain text (current behavior)
    Text(String),
    /// Structured: summary for LLM, data for UI/MCP
    Structured { summary: String, data: serde_json::Value },
}

impl From<String> for ToolOutput {
    fn from(s: String) -> Self { ToolOutput::Text(s) }
}

impl From<&str> for ToolOutput {
    fn from(s: &str) -> Self { ToolOutput::Text(s.to_string()) }
}

impl ToolOutput {
    pub fn as_text(&self) -> &str {
        match self {
            ToolOutput::Text(s) => s,
            ToolOutput::Structured { summary, .. } => summary,
        }
    }

    pub fn into_string(self) -> String {
        match self {
            ToolOutput::Text(s) => s,
            ToolOutput::Structured { summary, .. } => summary,
        }
    }
}
```

- [ ] **Step 2: Keep Tool trait returning `Result<String>` for now**

DO NOT change the `Tool` trait signature yet. Instead, add the `ToolOutput` type as an opt-in enhancement. Tools that want structured output can return JSON-encoded `ToolOutput::Structured` in the string, and the execution core can detect and parse it.

This avoids a breaking change to 20+ tools while providing the infrastructure for gradual adoption.

- [ ] **Step 3: Update execution core to detect structured output**

In `core.rs`, where tool results are appended to messages, add detection:

```rust
fn parse_tool_output(result: &str) -> ToolOutput {
    // Check if the result starts with a structured marker
    if let Some(stripped) = result.strip_prefix("__STRUCTURED__") {
        if let Ok(value) = serde_json::from_str::<serde_json::Value>(stripped) {
            if let (Some(summary), Some(data)) = (value["summary"].as_str(), value.get("data")) {
                return ToolOutput::Structured {
                    summary: summary.to_string(),
                    data: data.clone(),
                };
            }
        }
    }
    ToolOutput::Text(result.to_string())
}
```

- [ ] **Step 4: Run tests**

---

## Verification

After all 11 tasks:

- [ ] `cargo nextest run --workspace`
- [ ] `cargo clippy --workspace --all-targets --all-features`
- [ ] `cargo fmt --all --check`

---

## Summary

| Task | Crate(s) | Risk | Complexity |
|------|----------|------|------------|
| 1. Semantic Plan Tracking | `agent` | Low | Low |
| 2. Memory Confirmation | `cognitive`, `config`, `bus` | High | High |
| 3. Contradiction Detection | `cognitive` | Medium | Medium |
| 4. Embedding Hot-Reload | `skill-system` | Medium | Medium |
| 5. Routing Disambiguation | `skill-system` | Low | Low |
| 6. Retrieval Audit Trail | `context_engine`, `storage` | Medium | Medium |
| 7. Autotuner Fixes | `autotuner`, `storage`, `agent`, `cognitive` | Medium | Medium |
| 8. Autotuner Diagnostics | `autotuner`, `agent` | Low | Low |
| 9. Squad Cancellation | `agent` | Medium | Medium |
| 10. Config Schema Versioning | `config` | Medium | Low |
| 11. Structured Tool Output | `tools-core`, `agent` | High | High (incremental) |
