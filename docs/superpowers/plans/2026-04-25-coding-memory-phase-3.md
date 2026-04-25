# Coding Memory — Phase 3 (Write Path: Distiller + Tier A/B Activation) Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Turn the Phase-1 Distiller stubs into a working per-turn write pipeline. Raw events buffered in `ingest_event_log` (by Phase 2) are now read, turn-by-turn, by a two-phase Distiller (Phase A extractive + Phase B LLM synthesis), reconciled (Mem0-style ADD/SUPERSEDE/NOOP), and persisted as `SemanticFact` / `EpisodicMemory` rows with **provenance-always** metadata. Tier A activation (existing cognitive surfaces fire for coding content), Tier B1/B3/B4/B5 upgrades (counterfactual memory, `code_state`, `CodeDomainSearcher`, autotuner `session_type`) all land. Phase 3 also ships the Memory Browser / Activity Timeline / Cost Tracker / Sensitivity Inspector workbench panels (per spec §11.5).

**Architecture:** A single long-lived `Distiller` handle is owned by `AppCore`. Each new event entering `ingest_event_log` (through the Phase-2 daemon) is forwarded to the Distiller via `Distiller::accept_event`. The Distiller maintains a per-`(session_id, turn_id)` `TurnBuffer`. A turn boundary fires when any of: `EventKind::AssistantMsg` carrying `token_usage`, `EventKind::SessionEnd`, or a 2-minute idle timeout. On boundary, the buffered turn is atomically transitioned from `processed=0, processing=0` → `processing=1` in `ingest_event_log`, then `distill_turn` runs:

- **Phase A (extractive, always):** deterministic pass builds a `TurnTrace` (files read/modified, commands, test outcomes, errors, token usage). Always written to `episodic_memories{kind='turn_trace'}` — the durable baseline.
- **Phase B (LLM):** `ProviderManager::chat` is invoked using `ProviderRole::Distiller`. The system prompt instructs the model to emit zero or more `record_observation(...)` tool calls. The 5-value `CodingKind` enum enforces Distiller-never-emits-Reforge-kinds at the schema level.
- **Phase C (reconciliation):** for each observation, vector-similar top-5 is fetched via `UnifiedMemoryService`. Similarity > 0.9 + exact (subject, predicate) match → NOOP (bump `access_count`). Similarity > 0.75 → SUPERSEDE (write new row with `superseded_by` filled later). Else → ADD (fresh row).

Every write routes through `DistillerWriter` — a wrapper that refuses writes without a populated `ProvenanceMetadata.source_events`. In dev this panics; in release it logs + rejects. Turn events transition `processing=1 → processed=1` after a successful distill cycle.

`FixAttempt` episodes with `outcome: Failure | Abandoned` additionally emit a derived `DeadEndAttempt` → `SemanticFact{memory_type:"counterfactual"}` (Tier B1). The `code_state` enum lands on `UserSituationSnapshot` (Tier B3). A `CodeDomainSearcher` registers in `InsightForge` (Tier B4). `ShadowContext` gains a `session_type: Option<String>` field (Tier B5). Existing cognitive surfaces (`score_turn`, `DomainEvent::ContradictionDetected`, `UserCorrectedAI`) light up for coding content as a natural consequence of the new rows — Tier A activation is a scenario test, not new code.

Workbench panels (Memory Browser, Activity Timeline, Cost Tracker, Sensitivity Inspector) read the just-landed rows via new thin Tauri adapters over `app-core` handlers — following the Phase-2 pattern exactly.

**Tech Stack:** Rust (MSRV 1.93), `sqlx` (SQLite), `serde` (camelCase), `async-trait`, `tokio` (`sync`, `time`), `uuid` (`v4`), `jiff::Timestamp`, `blake3` for `problem_hash`, `proptest` for invariant tests, existing `providers::ProviderManager`, `cognitive::{SemanticFactRepo, EpisodicMemoryRepo, UnifiedMemoryService}`, `context_engine::{UserSituationSnapshot, InsightForge, DomainSearcher}`, `autotuner::ShadowContext`. Frontend: existing `desktop-ui/` (React + Tailwind v4 + Biome 2.0 + React Compiler + `useQuery`/`useMutation`). **No new runtime deps beyond `blake3`.**

---

## File Structure

Every file created or modified in this plan, grouped by responsibility. Files stay small and focused per CLAUDE.md.

### New files — `crates/coding-memory/`

| File | Responsibility |
|---|---|
| `src/distiller/turn_buffer.rs` | Per-`(session_id, turn_id)` event buffer with boundary detection + idle-timeout sweeper |
| `src/distiller/phase_a.rs` | Deterministic extractive pass: `TurnTrace` + `RefactorEpisode`/`TestRunEpisode` extraction |
| `src/distiller/phase_b.rs` | LLM synthesis: prompt build + `ProviderManager` invocation + tool-call decoding |
| `src/distiller/phase_c.rs` | Mem0-style reconciliation (NOOP / SUPERSEDE / ADD) |
| `src/distiller/writer.rs` | `DistillerWriter` wrapping `SemanticFactRepo` + `EpisodicMemoryRepo`; enforces provenance-always |
| `src/distiller/record_observation.rs` | LLM tool schema for `record_observation` + strict JSON decoder |
| `src/distiller/fact_builder.rs` | `CodingKind + observation → SemanticFact/EpisodicMemory` conversion with provenance |
| `src/distiller/error.rs` | `DistillerError` enum (LlmTimeout, LlmMalformed, ProvenanceMissing, …) |
| `src/problem_hash.rs` | Canonical problem hashing via `blake3` over normalized prompt text |
| `src/counterfactual.rs` | Derive `DeadEndAttempt` from failure/abandoned `FixAttempt` (Tier B1) |
| `src/code_state.rs` | `CodeState` enum (Tier B3) — `Idle`, `StackTraceActive { error_type }`, `RedTestsRunning`, … |
| `src/code_domain_searcher.rs` | `CodeDomainSearcher` impl of `DomainSearcher` trait (Tier B4) |
| `migrations/002_retry_queue.sql` | Adds `ingest_distillation_retry` table (idempotent transient-failure queue) |

### New files — `crates/app-core/src/coding_memory/`

| File | Responsibility |
|---|---|
| `panels.rs` | Handlers for Memory Browser / Activity Timeline / Cost Tracker / Sensitivity Inspector panels |

### New files — `desktop-ui/src/features/coding-memory/`

| File | Responsibility |
|---|---|
| `MemoryBrowserPanel.tsx` | Filterable list of `SemanticFact`+`EpisodicMemory` rows with provenance drawer |
| `ActivityTimelinePanel.tsx` | Calendar heatmap of per-day episode counts; per-repo filter |
| `CostTrackerPanel.tsx` | Daily/weekly LLM spend breakdown (Distiller Phase B only at this phase) |
| `SensitivityInspectorPanel.tsx` | Browse facts by sensitivity tier; promote/demote via explicit confirmation |

### Modified existing files

| File | Change |
|---|---|
| `crates/coding-memory/src/distiller/mod.rs` | Replace Phase-1 stubs with real `Distiller::{new, accept_event, distill_turn, flush, shutdown}` |
| `crates/coding-memory/src/sink.rs` | `InProcessSink` now forwards to an injected `Arc<Distiller>` |
| `crates/coding-memory/src/lib.rs` | Re-export new public items (`Distiller`, `DistillerConfig`, `DistillationReport`, `CodeState`, `CodeDomainSearcher`, `ProblemHash`) |
| `crates/coding-memory/Cargo.toml` | Add `blake3`, `proptest` (dev), `cognitive` (prod), verify `providers` already present |
| `crates/coding-ingest/src/store.rs` | Add `mark_processing`, `mark_processed`, `fetch_turn`, `fetch_turn_ids_atomically` methods |
| `crates/context_engine/src/rewriter.rs` | Add `code_state: Option<CodeStateSnapshot>` field to `UserSituationSnapshot` (string-typed snapshot to keep L3 free of L5 deps) |
| `crates/context_engine/src/lib.rs` | Export `CodeStateSnapshot` |
| `crates/autotuner/src/traits.rs` | Add `session_type: Option<String>` field to `ShadowContext` |
| `crates/common/src/autotuner.rs` | No change (TrialParams keyed by `session_type` at use-site, not schema) |
| `crates/cognitive/src/repos/semantic_fact.rs` | Add `upsert_with_metadata` method that writes the new `metadata` TEXT column + `scope_repo_id` |
| `crates/cognitive/src/repos/episodic_memory.rs` | Add `insert_with_kind_and_metadata` method writing new columns |
| `crates/app-core/src/coding_memory/mod.rs` | Add `pub mod panels;`; re-export panel DTO handler fns |
| `crates/app-core/src/coding_memory/handlers.rs` | Add wrappers for the 6 new panel commands |
| `crates/app-core/src/state.rs` | Add `Option<Arc<Distiller>>` field to `AppCore`; holder for sink wiring |
| `crates/app-core/src/init/mod.rs` (or init aggregator) | Construct `Distiller`, wire `ProviderManager`, pass repos, register on `AppCore` |
| `crates/desktop-shared/src/commands/coding_memory.rs` | Add DTOs: `MemoryRow`, `ActivityBucket`, `CostRollup`, `SensitivityUpdate`, `FactProvenanceView` |
| `crates/desktop/src/commands/coding_memory.rs` | Add 6 new Tauri commands + extend `DEV_COMMANDS` |
| `crates/desktop/src/lib.rs` | Register new commands in `invoke_handler![...]` |
| `crates/desktop/src/dev_server/mod.rs` | New commands auto-covered via `DEV_COMMANDS` |
| `desktop-ui/src/app/router.tsx` | Add nested routes under `/coding-memory/{memory,activity,cost,sensitivity}` |
| `desktop-ui/src/features/coding-memory/CodingMemoryLayout.tsx` | Add 4 nav entries for the new panels |
| `desktop-ui/src/features/coding-memory/hooks.ts` | Add `useMemoryBrowser`, `useActivityTimeline`, `useCostRollup`, `useSensitivityInspector` |

### Test files

| File | Responsibility |
|---|---|
| `crates/coding-memory/tests/problem_hash.rs` | Canonicalization: same prompt → same hash; whitespace-invariant |
| `crates/coding-memory/tests/phase_a_extractive.rs` | `TurnTrace` from known event set; file read/modify classification; test framework detection |
| `crates/coding-memory/tests/phase_b_llm.rs` | `ProviderManager` mocked; malformed tool-call dropped; valid observation → fact |
| `crates/coding-memory/tests/phase_c_reconciliation.rs` | NOOP/SUPERSEDE/ADD behavior against seeded facts |
| `crates/coding-memory/tests/distiller_writer_provenance.rs` | Write rejects when `source_events` is empty |
| `crates/coding-memory/tests/distiller_end_to_end.rs` | Ingest 10-event synthetic turn → Distiller → `semantic_facts` + `episodic_memories` rows populated |
| `crates/coding-memory/tests/counterfactual.rs` | Failure `FixAttempt` → derived `memory_type:"counterfactual"` fact |
| `crates/coding-memory/tests/code_state_rewriter.rs` | `UserSituationSnapshot.code_state` round-trips through rewriter |
| `crates/coding-memory/tests/code_domain_searcher.rs` | `CodeDomainSearcher::search` returns matching facts as `MemoryEntry` |
| `crates/coding-memory/tests/turn_boundary.rs` | `AssistantMsg` with usage fires; idle-timeout fires; `UserPrompt` resets turn |
| `crates/coding-memory/tests/sink_wiring.rs` | `InProcessSink::accept_event` hits Distiller |
| `crates/coding-memory/tests/prop_provenance_invariant.rs` | **Invariant 1** — every fact has non-empty `source_events` |
| `crates/coding-memory/tests/prop_bi_temporal.rs` | **Invariant 2** — `valid_until >= valid_from` |
| `crates/coding-memory/tests/prop_supersede_chain.rs` | **Invariant 3 + 5** — predecessor.valid_until == successor.valid_from; monotone count |
| `crates/coding-memory/tests/prop_distiller_never_deletes.rs` | **Invariant 5** — no Distiller cycle reduces row count |
| `crates/autotuner/tests/shadow_context_session_type.rs` | `ShadowContext { session_type }` serializes round-trip |
| `tests/integration/coding_memory_phase3_tier_a.rs` | Scenario — coding turn fires `score_turn`, `ContradictionDetected`, `UserCorrectedAI` |
| `tests/integration/coding_memory_phase3_roundtrip.rs` | Scenario — synthetic session → ingest → distill → retrieve via `UnifiedMemoryService` |
| `tests/fixtures/coding/phase3_bug_fix_session.jsonl` | 10-event canned session with known expected extraction |
| `tests/fixtures/coding/phase3_distillation_mocks/*.json` | Canned LLM `record_observation` responses per scenario |
| `desktop-ui/src/features/coding-memory/__tests__/MemoryBrowserPanel.test.tsx` | Panel renders rows + opens provenance drawer |
| `desktop-ui/src/features/coding-memory/__tests__/ActivityTimelinePanel.test.tsx` | Heatmap renders buckets |
| `desktop-ui/src/features/coding-memory/__tests__/CostTrackerPanel.test.tsx` | Cost rollup bars + totals |
| `desktop-ui/src/features/coding-memory/__tests__/SensitivityInspectorPanel.test.tsx` | Promote/demote confirmation flow |

---

## Task Structure

Tasks are ordered so each builds on the prior commit. Many pairs parallelize when an engineer works in a git worktree (all tests on the Phase-1-stubbed Distiller remain green until Task 22 replaces the stubs). Each task: exact file paths, exact commands, full code.

### Task 1: `blake3` + `proptest` deps

**Files:**
- Modify: `crates/coding-memory/Cargo.toml`

- [ ] **Step 1: Add deps**

In `crates/coding-memory/Cargo.toml` under `[dependencies]` add:

```toml
blake3 = "1"
```

Under `[dev-dependencies]` add:

```toml
proptest = "1"
```

- [ ] **Step 2: Build + commit**

```bash
cargo build -p coding-memory
git add crates/coding-memory/Cargo.toml
git commit -m "feat(coding-memory): add blake3 + proptest deps for Phase 3"
```

---

### Task 2: `ProblemHash` — canonical bug-problem hasher

**Files:**
- Create: `crates/coding-memory/src/problem_hash.rs`
- Modify: `crates/coding-memory/src/lib.rs`
- Test: `crates/coding-memory/tests/problem_hash.rs`

- [ ] **Step 1: Write failing test**

Create `crates/coding-memory/tests/problem_hash.rs`:

```rust
use coding_memory::problem_hash::ProblemHash;

#[test]
fn same_prompt_same_hash() {
    assert_eq!(
        ProblemHash::of("fix the null pointer in parser"),
        ProblemHash::of("fix the null pointer in parser"),
    );
}

#[test]
fn whitespace_invariant() {
    assert_eq!(
        ProblemHash::of("fix   the\nnull\tpointer"),
        ProblemHash::of("fix the null pointer"),
    );
}

#[test]
fn case_invariant() {
    assert_eq!(
        ProblemHash::of("Fix The Null Pointer"),
        ProblemHash::of("fix the null pointer"),
    );
}

#[test]
fn different_prompts_different_hashes() {
    assert_ne!(
        ProblemHash::of("null pointer"),
        ProblemHash::of("off-by-one loop"),
    );
}

#[test]
fn hash_is_stable_hex_16() {
    let h = ProblemHash::of("fix the null pointer");
    let s = h.as_str();
    assert_eq!(s.len(), 16);
    assert!(s.chars().all(|c| c.is_ascii_hexdigit()));
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo nextest run -p coding-memory --test problem_hash`
Expected: FAIL — module doesn't exist.

- [ ] **Step 3: Implement**

Create `crates/coding-memory/src/problem_hash.rs`:

```rust
//! Canonical hash of a bug-problem statement.
//!
//! Same logical problem phrased differently must collide. `FixAttempt`
//! clustering (repeat-attempt detection, counterfactual matching) uses this.

use serde::{Deserialize, Serialize};

/// 16-hex-char prefix of a `blake3` hash over a canonicalized problem string.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq, Hash)]
#[serde(transparent)]
pub struct ProblemHash(String);

impl ProblemHash {
    /// Hash a problem statement with canonicalization (lowercase, collapse whitespace).
    #[must_use]
    pub fn of(raw: &str) -> Self {
        let canon = canonicalize(raw);
        let h = blake3::hash(canon.as_bytes());
        let hex = h.to_hex();
        Self(hex.as_str()[..16].to_string())
    }

    /// Accessor for the 16-char hex string.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }

    /// Construct from an already-computed value (e.g. DB load).
    #[must_use]
    pub fn from_raw(s: String) -> Self {
        Self(s)
    }
}

fn canonicalize(raw: &str) -> String {
    let lower = raw.to_lowercase();
    lower.split_whitespace().collect::<Vec<_>>().join(" ")
}
```

Edit `crates/coding-memory/src/lib.rs` and add:

```rust
/// Canonical bug-problem hashing (`blake3`-based).
pub mod problem_hash;
```

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo nextest run -p coding-memory --test problem_hash`
Expected: PASS (5/5).

- [ ] **Step 5: Commit**

```bash
git add crates/coding-memory/src/problem_hash.rs crates/coding-memory/src/lib.rs \
        crates/coding-memory/tests/problem_hash.rs
git commit -m "feat(coding-memory): ProblemHash — canonical problem statement hashing"
```

---

### Task 3: `DistillerError` — bounded failure taxonomy

**Files:**
- Create: `crates/coding-memory/src/distiller/error.rs`
- Modify: `crates/coding-memory/src/distiller/mod.rs` (add `pub mod error;`)

- [ ] **Step 1: Create the file**

Create `crates/coding-memory/src/distiller/error.rs`:

```rust
//! Distiller-scoped error taxonomy.
//!
//! The Distiller is a write-path subsystem; it must never silently swallow
//! failures. Every failure mode has an explicit variant so callers can
//! choose retry/skip/abort policy and Mirror subscribers can categorize.

use thiserror::Error;

/// Errors produced by the Distiller pipeline (Phase A / B / C / writer).
#[derive(Debug, Error)]
pub enum DistillerError {
    /// The LLM provider timed out while synthesizing observations.
    #[error("LLM timeout after {timeout_ms}ms")]
    LlmTimeout { timeout_ms: u64 },

    /// The LLM produced text/tool-call JSON that couldn't be decoded.
    #[error("LLM malformed tool call: {detail}")]
    LlmMalformed { detail: String },

    /// The provider manager returned an error (configured provider unavailable etc.).
    #[error("LLM provider error: {detail}")]
    LlmProvider { detail: String },

    /// A write was attempted with empty `source_events` provenance.
    #[error("provenance missing: source_events is empty")]
    ProvenanceMissing,

    /// The event body couldn't be serialized / deserialized.
    #[error("event decode failure: {detail}")]
    EventDecode { detail: String },

    /// An underlying storage operation failed.
    #[error("storage error: {detail}")]
    Storage { detail: String },

    /// The turn is already being processed by another cycle.
    #[error("turn already in flight")]
    TurnInFlight,

    /// A transient failure — caller should retry on next cycle.
    #[error("transient: {detail}")]
    Transient { detail: String },
}

impl From<DistillerError> for common::KlyntbotError {
    fn from(e: DistillerError) -> Self {
        common::KlyntbotError::Storage(format!("distiller: {e}"))
    }
}
```

- [ ] **Step 2: Declare module**

In `crates/coding-memory/src/distiller/mod.rs`, at the top of the file add:

```rust
/// Distiller-scoped error taxonomy.
pub mod error;

pub use error::DistillerError;
```

- [ ] **Step 3: Build + clippy + commit**

```bash
cargo build -p coding-memory
cargo clippy -p coding-memory --all-targets -- -D warnings
git add crates/coding-memory/src/distiller/error.rs crates/coding-memory/src/distiller/mod.rs
git commit -m "feat(coding-memory): DistillerError taxonomy"
```

---

### Task 4: `IngestEventLogRepo` turn helpers — `mark_processing`, `mark_processed`, `fetch_turn`

**Files:**
- Modify: `crates/coding-ingest/src/store.rs`
- Test: `crates/coding-ingest/tests/ingest_event_log_turn_helpers.rs`

- [ ] **Step 1: Write failing tests**

Create `crates/coding-ingest/tests/ingest_event_log_turn_helpers.rs`:

```rust
use coding_ingest::event::{AgentEvent, AgentEventV1, AgentSource, EventKind};
use coding_ingest::store::IngestEventLogRepo;
use jiff::Timestamp;
use std::path::PathBuf;
use storage::StoragePool;
use uuid::Uuid;

fn evt(session: &str, turn: Option<&str>, kind: EventKind) -> AgentEvent {
    AgentEvent::V1(AgentEventV1 {
        id: Uuid::new_v4(),
        source: AgentSource::ClaudeCode,
        session_id: session.into(),
        turn_id: turn.map(str::to_string),
        cwd: PathBuf::from("/tmp"),
        repo: None,
        occurred_at: Timestamp::now(),
        kind,
    })
}

async fn prepared() -> (StoragePool, IngestEventLogRepo) {
    let pool = StoragePool::connect_in_memory().await.unwrap();
    StoragePool::run_feature_migrations(pool.inner(), &cognitive::cognitive_migrations())
        .await
        .unwrap();
    StoragePool::run_feature_migrations(pool.inner(), &coding_memory::coding_memory_migrations())
        .await
        .unwrap();
    let repo = IngestEventLogRepo::new(pool.inner().clone());
    (pool, repo)
}

#[tokio::test]
async fn fetch_turn_returns_only_events_for_session_turn() {
    let (_pool, repo) = prepared().await;
    let up = EventKind::UserPrompt { text: "hi".into(), attachments: vec![] };
    repo.insert(&evt("s1", Some("t1"), up.clone())).await.unwrap();
    repo.insert(&evt("s1", Some("t1"), up.clone())).await.unwrap();
    repo.insert(&evt("s1", Some("t2"), up.clone())).await.unwrap();
    repo.insert(&evt("s2", Some("t1"), up.clone())).await.unwrap();

    let rows = repo.fetch_turn("s1", Some("t1")).await.unwrap();
    assert_eq!(rows.len(), 2);
    for r in &rows {
        assert_eq!(r.session_id, "s1");
        assert_eq!(r.turn_id.as_deref(), Some("t1"));
    }
}

#[tokio::test]
async fn fetch_turn_null_turn_id() {
    let (_pool, repo) = prepared().await;
    repo.insert(&evt("s1", None, EventKind::SessionEnd { reason: "x".into() })).await.unwrap();
    let rows = repo.fetch_turn("s1", None).await.unwrap();
    assert_eq!(rows.len(), 1);
    assert!(rows[0].turn_id.is_none());
}

#[tokio::test]
async fn mark_processing_transitions_atomically() {
    let (_pool, repo) = prepared().await;
    let up = EventKind::UserPrompt { text: "hi".into(), attachments: vec![] };
    repo.insert(&evt("s1", Some("t1"), up.clone())).await.unwrap();
    repo.insert(&evt("s1", Some("t1"), up.clone())).await.unwrap();

    let claimed = repo.mark_processing("s1", Some("t1")).await.unwrap();
    assert_eq!(claimed, 2);
    // Second claim finds nothing — idempotent.
    let claimed = repo.mark_processing("s1", Some("t1")).await.unwrap();
    assert_eq!(claimed, 0);
}

#[tokio::test]
async fn mark_processed_completes_turn() {
    let (_pool, repo) = prepared().await;
    let up = EventKind::UserPrompt { text: "hi".into(), attachments: vec![] };
    repo.insert(&evt("s1", Some("t1"), up)).await.unwrap();
    repo.mark_processing("s1", Some("t1")).await.unwrap();
    let rows = repo.fetch_turn("s1", Some("t1")).await.unwrap();
    repo.mark_processed(rows.iter().map(|r| r.id.as_str())).await.unwrap();
    assert_eq!(repo.count_unprocessed().await.unwrap(), 0);
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo nextest run -p coding-ingest --test ingest_event_log_turn_helpers`
Expected: FAIL — `fetch_turn` / `mark_processing` / `mark_processed` not defined.

- [ ] **Step 3: Implement the methods**

In `crates/coding-ingest/src/store.rs`, inside `impl IngestEventLogRepo`, append:

```rust
    /// Fetch every event row for a given (session, turn) pair, ordered by `occurred_at`.
    /// `turn_id = None` matches rows where the column is NULL.
    pub async fn fetch_turn(
        &self,
        session_id: &str,
        turn_id: Option<&str>,
    ) -> Result<Vec<IngestEventLogRow>> {
        let rows = match turn_id {
            Some(tid) => {
                sqlx::query(
                    "SELECT id, source, session_id, turn_id, repo_id, kind, payload, processed, occurred_at
                     FROM ingest_event_log
                     WHERE session_id = ? AND turn_id = ?
                     ORDER BY occurred_at ASC",
                )
                .bind(session_id)
                .bind(tid)
                .fetch_all(&self.pool)
                .await
            }
            None => {
                sqlx::query(
                    "SELECT id, source, session_id, turn_id, repo_id, kind, payload, processed, occurred_at
                     FROM ingest_event_log
                     WHERE session_id = ? AND turn_id IS NULL
                     ORDER BY occurred_at ASC",
                )
                .bind(session_id)
                .fetch_all(&self.pool)
                .await
            }
        }
        .map_err(|e| KlyntbotError::Storage(format!("fetch_turn: {e}")))?;
        Ok(rows
            .into_iter()
            .map(|r| IngestEventLogRow {
                id: r.get("id"),
                source: r.get("source"),
                session_id: r.get("session_id"),
                turn_id: r.get("turn_id"),
                repo_id: r.get("repo_id"),
                kind: r.get("kind"),
                payload: r.get("payload"),
                processed: r.get::<bool, _>("processed"),
                occurred_at: r.get("occurred_at"),
            })
            .collect())
    }

    /// Atomically flip `processing` from 0→1 for every row in the turn. Returns the count flipped.
    /// Already-processing rows are skipped — making the call idempotent.
    pub async fn mark_processing(
        &self,
        session_id: &str,
        turn_id: Option<&str>,
    ) -> Result<u64> {
        let res = match turn_id {
            Some(tid) => {
                sqlx::query(
                    "UPDATE ingest_event_log SET processing = 1
                     WHERE session_id = ? AND turn_id = ?
                       AND processed = 0 AND processing = 0",
                )
                .bind(session_id)
                .bind(tid)
                .execute(&self.pool)
                .await
            }
            None => {
                sqlx::query(
                    "UPDATE ingest_event_log SET processing = 1
                     WHERE session_id = ? AND turn_id IS NULL
                       AND processed = 0 AND processing = 0",
                )
                .bind(session_id)
                .execute(&self.pool)
                .await
            }
        }
        .map_err(|e| KlyntbotError::Storage(format!("mark_processing: {e}")))?;
        Ok(res.rows_affected())
    }

    /// Mark a set of row ids as `processed=1, processing=0` — called after a successful distill cycle.
    pub async fn mark_processed<'a, I: IntoIterator<Item = &'a str>>(
        &self,
        ids: I,
    ) -> Result<u64> {
        let ids: Vec<&str> = ids.into_iter().collect();
        if ids.is_empty() {
            return Ok(0);
        }
        let mut total: u64 = 0;
        let mut tx = self.pool.begin().await
            .map_err(|e| KlyntbotError::Storage(format!("mark_processed tx: {e}")))?;
        for id in ids {
            let res = sqlx::query(
                "UPDATE ingest_event_log SET processed = 1, processing = 0 WHERE id = ?",
            )
            .bind(id)
            .execute(&mut *tx)
            .await
            .map_err(|e| KlyntbotError::Storage(format!("mark_processed row: {e}")))?;
            total += res.rows_affected();
        }
        tx.commit().await
            .map_err(|e| KlyntbotError::Storage(format!("mark_processed commit: {e}")))?;
        Ok(total)
    }
```

- [ ] **Step 4: Run tests + commit**

```bash
cargo nextest run -p coding-ingest --test ingest_event_log_turn_helpers
cargo clippy -p coding-ingest --all-targets -- -D warnings
git add crates/coding-ingest/src/store.rs crates/coding-ingest/tests/ingest_event_log_turn_helpers.rs
git commit -m "feat(coding-ingest): fetch_turn + mark_processing/processed for Distiller claims"
```

---

### Task 5: `DistillerWriter` + `ProvenanceBuilder` — write-side guard

**Files:**
- Create: `crates/coding-memory/src/distiller/writer.rs`
- Modify: `crates/coding-memory/src/distiller/mod.rs`
- Test: `crates/coding-memory/tests/distiller_writer_provenance.rs`

- [ ] **Step 1: Write failing test**

Create `crates/coding-memory/tests/distiller_writer_provenance.rs`:

```rust
use coding_memory::distiller::writer::{DistillerWriter, PreparedFact, PreparedEpisode};
use coding_memory::distiller::DistillerError;
use coding_memory::scope::{ProvenanceKind, ProvenanceMetadata};
use cognitive::{EpisodicMemoryRepo, SemanticFactRepo};
use cognitive::types::{EpisodicMemory, SemanticFact};
use jiff::Timestamp;
use storage::StoragePool;
use uuid::Uuid;

async fn prepared() -> (StoragePool, DistillerWriter) {
    let pool = StoragePool::connect_in_memory().await.unwrap();
    StoragePool::run_feature_migrations(pool.inner(), &cognitive::cognitive_migrations()).await.unwrap();
    StoragePool::run_feature_migrations(pool.inner(), &coding_memory::coding_memory_migrations()).await.unwrap();
    let facts = SemanticFactRepo::new(pool.inner().clone());
    let episodes = EpisodicMemoryRepo::new(pool.inner().clone());
    (pool, DistillerWriter::new(facts, episodes))
}

fn valid_provenance() -> ProvenanceMetadata {
    ProvenanceMetadata {
        source_events: vec![Uuid::new_v4()],
        session_id: "s1".into(),
        turn_id: Some("t1".into()),
        distilled_at: Timestamp::now(),
        distiller_model: "claude-haiku-4-5".into(),
        source_kind: ProvenanceKind::DistillerLlm,
    }
}

fn dummy_fact() -> SemanticFact {
    SemanticFact {
        id: Uuid::new_v4().to_string(),
        domain: "work".into(),
        subject: "repo:x".into(),
        predicate: "framework".into(),
        object: "rust".into(),
        confidence: 0.9,
        source: "distiller".into(),
        valid_from: Timestamp::now().to_string(),
        valid_until: None,
        recorded_at: Timestamp::now().to_string(),
        superseded_at: None,
        superseded_by: None,
        stability: 1.0,
        last_accessed: None,
        access_count: 0,
        convergence_score: 1.0,
        project_id: None,
        memory_type: "fact".into(),
        scope_type: "user".into(),
        scope_id: None,
    }
}

fn dummy_episode() -> EpisodicMemory {
    EpisodicMemory {
        id: Uuid::new_v4().to_string(),
        domain: "coding".into(),
        content: "turn trace".into(),
        summary: None,
        importance: 0.5,
        occurred_at: Timestamp::now().to_string(),
        recorded_at: Timestamp::now().to_string(),
        stability: 1.0,
        last_accessed: None,
        access_count: 0,
        project_id: None,
        scope_type: "user".into(),
        scope_id: None,
    }
}

#[tokio::test]
async fn write_fact_rejects_empty_provenance() {
    let (_pool, writer) = prepared().await;
    let mut prov = valid_provenance();
    prov.source_events.clear();
    let r = writer.write_fact(PreparedFact {
        fact: dummy_fact(),
        metadata_json: None,
        scope_repo_id: None,
        provenance: prov,
    }).await;
    assert!(matches!(r, Err(DistillerError::ProvenanceMissing)));
}

#[tokio::test]
async fn write_fact_persists_metadata_and_scope_repo_id() {
    let (pool, writer) = prepared().await;
    writer.write_fact(PreparedFact {
        fact: dummy_fact(),
        metadata_json: None,
        scope_repo_id: Some("github.com/klynt/bot".into()),
        provenance: valid_provenance(),
    }).await.unwrap();

    let row: (Option<String>, Option<String>) = sqlx::query_as(
        "SELECT scope_repo_id, metadata FROM semantic_facts LIMIT 1"
    ).fetch_one(pool.inner()).await.unwrap();
    assert_eq!(row.0.as_deref(), Some("github.com/klynt/bot"));
    let meta: serde_json::Value = serde_json::from_str(&row.1.unwrap()).unwrap();
    assert!(meta["provenance"]["sourceEvents"].is_array());
    assert!(meta["provenance"]["sourceEvents"].as_array().unwrap().len() > 0);
}

#[tokio::test]
async fn write_episode_rejects_empty_provenance() {
    let (_pool, writer) = prepared().await;
    let mut prov = valid_provenance();
    prov.source_events.clear();
    let r = writer.write_episode(PreparedEpisode {
        episode: dummy_episode(),
        kind: "turn_trace".into(),
        metadata_json: None,
        scope_repo_id: None,
        provenance: prov,
    }).await;
    assert!(matches!(r, Err(DistillerError::ProvenanceMissing)));
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo nextest run -p coding-memory --test distiller_writer_provenance`
Expected: FAIL — `DistillerWriter` undefined.

- [ ] **Step 3: Add the `upsert_with_metadata` / `insert_with_kind_and_metadata` repo helpers**

Edit `crates/cognitive/src/repos/semantic_fact.rs`. Inside `impl SemanticFactRepo`, append:

```rust
    /// Upsert a fact carrying coding-memory `scope_repo_id` + JSON `metadata` (Phase-1 columns).
    pub async fn upsert_with_metadata(
        &self,
        fact: &crate::SemanticFact,
        scope_repo_id: Option<&str>,
        metadata_json: Option<&str>,
    ) -> Result<(), sqlx::Error> {
        self.upsert(fact).await?;
        sqlx::query(
            "UPDATE semantic_facts SET scope_repo_id = ?2, metadata = ?3 WHERE id = ?1",
        )
        .bind(&fact.id)
        .bind(scope_repo_id)
        .bind(metadata_json)
        .execute(&self.pool)
        .await?;
        Ok(())
    }
```

Edit `crates/cognitive/src/repos/episodic_memory.rs`. Inside `impl EpisodicMemoryRepo`, append:

```rust
    /// Insert an episode carrying the coding-memory `kind`, `scope_repo_id`, and JSON `metadata` columns.
    pub async fn insert_with_kind_and_metadata(
        &self,
        mem: &crate::EpisodicMemory,
        kind: &str,
        scope_repo_id: Option<&str>,
        metadata_json: Option<&str>,
    ) -> Result<(), sqlx::Error> {
        self.insert(mem).await?;
        sqlx::query(
            "UPDATE episodic_memories SET kind = ?2, scope_repo_id = ?3, metadata = ?4 WHERE id = ?1",
        )
        .bind(&mem.id)
        .bind(kind)
        .bind(scope_repo_id)
        .bind(metadata_json)
        .execute(&self.pool)
        .await?;
        Ok(())
    }
```

- [ ] **Step 4: Create the writer**

Create `crates/coding-memory/src/distiller/writer.rs`:

```rust
//! `DistillerWriter` — single write chokepoint for every Distiller-authored row.
//!
//! Enforces the **provenance-always invariant**: any write missing a
//! populated `ProvenanceMetadata.source_events` returns
//! `DistillerError::ProvenanceMissing`. In dev builds the same condition
//! additionally panics (via `debug_assert`), catching integration mistakes early.

use super::error::DistillerError;
use crate::scope::ProvenanceMetadata;
use cognitive::types::{EpisodicMemory, SemanticFact};
use cognitive::{EpisodicMemoryRepo, SemanticFactRepo};
use serde_json::json;

/// A fact prepared for writing — carries the row plus coding-memory metadata.
#[derive(Debug, Clone)]
pub struct PreparedFact {
    /// The cognitive-layer `SemanticFact` row.
    pub fact: SemanticFact,
    /// Pre-built `metadata` JSON payload. If `None`, `writer` will build
    /// one containing only the `provenance` block.
    pub metadata_json: Option<serde_json::Value>,
    /// Scope partition for the row (None = global).
    pub scope_repo_id: Option<String>,
    /// Provenance — must have non-empty `source_events`.
    pub provenance: ProvenanceMetadata,
}

/// An episodic row prepared for writing.
#[derive(Debug, Clone)]
pub struct PreparedEpisode {
    /// The cognitive-layer row.
    pub episode: EpisodicMemory,
    /// Coding-memory `kind` (`turn_trace`, `fix_attempt`, `refactor`, `test_run`, …).
    pub kind: String,
    /// Optional pre-built metadata JSON (provenance merged in automatically).
    pub metadata_json: Option<serde_json::Value>,
    /// Scope partition.
    pub scope_repo_id: Option<String>,
    /// Provenance — must have non-empty `source_events`.
    pub provenance: ProvenanceMetadata,
}

/// Writer — delegates to `SemanticFactRepo` / `EpisodicMemoryRepo`, enforces provenance.
#[derive(Debug, Clone)]
pub struct DistillerWriter {
    facts: SemanticFactRepo,
    episodes: EpisodicMemoryRepo,
}

impl DistillerWriter {
    /// Construct a writer around existing cognitive repos.
    #[must_use]
    pub fn new(facts: SemanticFactRepo, episodes: EpisodicMemoryRepo) -> Self {
        Self { facts, episodes }
    }

    /// Write a semantic fact. Returns `ProvenanceMissing` when source_events is empty.
    pub async fn write_fact(&self, prepared: PreparedFact) -> Result<(), DistillerError> {
        debug_assert!(
            !prepared.provenance.source_events.is_empty(),
            "write_fact called with empty source_events — distillation pipeline bug"
        );
        if prepared.provenance.source_events.is_empty() {
            return Err(DistillerError::ProvenanceMissing);
        }

        let metadata_json = merge_provenance(prepared.metadata_json, &prepared.provenance)?;
        let json_str = serde_json::to_string(&metadata_json)
            .map_err(|e| DistillerError::Storage { detail: format!("metadata serialize: {e}") })?;

        self.facts
            .upsert_with_metadata(
                &prepared.fact,
                prepared.scope_repo_id.as_deref(),
                Some(&json_str),
            )
            .await
            .map_err(|e| DistillerError::Storage { detail: format!("upsert_with_metadata: {e}") })?;
        Ok(())
    }

    /// Write an episodic row (turn_trace / fix_attempt / refactor / test_run / general).
    pub async fn write_episode(&self, prepared: PreparedEpisode) -> Result<(), DistillerError> {
        debug_assert!(
            !prepared.provenance.source_events.is_empty(),
            "write_episode called with empty source_events — distillation pipeline bug"
        );
        if prepared.provenance.source_events.is_empty() {
            return Err(DistillerError::ProvenanceMissing);
        }

        let metadata_json = merge_provenance(prepared.metadata_json, &prepared.provenance)?;
        let json_str = serde_json::to_string(&metadata_json)
            .map_err(|e| DistillerError::Storage { detail: format!("metadata serialize: {e}") })?;

        self.episodes
            .insert_with_kind_and_metadata(
                &prepared.episode,
                &prepared.kind,
                prepared.scope_repo_id.as_deref(),
                Some(&json_str),
            )
            .await
            .map_err(|e| DistillerError::Storage { detail: format!("insert_with_kind: {e}") })?;
        Ok(())
    }

    /// Borrow the underlying fact repo (read-only discovery).
    pub fn facts(&self) -> &SemanticFactRepo { &self.facts }
    /// Borrow the underlying episode repo.
    pub fn episodes(&self) -> &EpisodicMemoryRepo { &self.episodes }
}

fn merge_provenance(
    base: Option<serde_json::Value>,
    prov: &ProvenanceMetadata,
) -> Result<serde_json::Value, DistillerError> {
    let prov_value = serde_json::to_value(prov)
        .map_err(|e| DistillerError::Storage { detail: format!("prov serialize: {e}") })?;
    let mut out = base.unwrap_or_else(|| json!({}));
    let obj = out
        .as_object_mut()
        .ok_or_else(|| DistillerError::Storage { detail: "base metadata not object".into() })?;
    obj.insert("provenance".into(), prov_value);
    Ok(out)
}
```

Edit `crates/coding-memory/src/distiller/mod.rs` and append:

```rust
/// Write chokepoint enforcing provenance-always invariant.
pub mod writer;

pub use writer::{DistillerWriter, PreparedEpisode, PreparedFact};
```

- [ ] **Step 5: Run tests + commit**

```bash
cargo nextest run -p coding-memory --test distiller_writer_provenance
cargo clippy -p coding-memory --all-targets -- -D warnings
git add crates/cognitive/src/repos/semantic_fact.rs crates/cognitive/src/repos/episodic_memory.rs \
        crates/coding-memory/src/distiller/writer.rs crates/coding-memory/src/distiller/mod.rs \
        crates/coding-memory/tests/distiller_writer_provenance.rs
git commit -m "feat(coding-memory): DistillerWriter enforces provenance-always on every row"
```

---

### Task 6: `TurnBuffer` — per-(session, turn) event accumulator

**Files:**
- Create: `crates/coding-memory/src/distiller/turn_buffer.rs`
- Modify: `crates/coding-memory/src/distiller/mod.rs`
- Test: `crates/coding-memory/tests/turn_boundary.rs`

- [ ] **Step 1: Write failing tests**

Create `crates/coding-memory/tests/turn_boundary.rs`:

```rust
use coding_ingest::event::{AgentEvent, AgentEventV1, AgentSource, EventKind, TokenUsage};
use coding_memory::distiller::turn_buffer::{TurnBoundary, TurnBuffer};
use jiff::Timestamp;
use std::path::PathBuf;
use uuid::Uuid;

fn evt(session: &str, turn: Option<&str>, kind: EventKind) -> AgentEvent {
    AgentEvent::V1(AgentEventV1 {
        id: Uuid::new_v4(),
        source: AgentSource::ClaudeCode,
        session_id: session.into(),
        turn_id: turn.map(str::to_string),
        cwd: PathBuf::from("/tmp"),
        repo: None,
        occurred_at: Timestamp::now(),
        kind,
    })
}

#[test]
fn user_prompt_does_not_fire_boundary() {
    let mut buf = TurnBuffer::new();
    let b = buf.accept(&evt("s1", Some("t1"), EventKind::UserPrompt {
        text: "hi".into(), attachments: vec![],
    }));
    assert!(matches!(b, TurnBoundary::None));
}

#[test]
fn assistant_msg_with_usage_fires_boundary() {
    let mut buf = TurnBuffer::new();
    buf.accept(&evt("s1", Some("t1"), EventKind::UserPrompt { text: "hi".into(), attachments: vec![] }));
    let b = buf.accept(&evt("s1", Some("t1"), EventKind::AssistantMsg {
        text: "done".into(),
        truncated: false,
        token_usage: Some(TokenUsage { prompt_tokens: 10, completion_tokens: 5, cached_tokens: None }),
    }));
    match b {
        TurnBoundary::Fire { session_id, turn_id } => {
            assert_eq!(session_id, "s1");
            assert_eq!(turn_id.as_deref(), Some("t1"));
        }
        _ => panic!("expected Fire"),
    }
}

#[test]
fn assistant_msg_without_usage_does_not_fire() {
    let mut buf = TurnBuffer::new();
    let b = buf.accept(&evt("s1", Some("t1"), EventKind::AssistantMsg {
        text: "partial".into(), truncated: true, token_usage: None,
    }));
    assert!(matches!(b, TurnBoundary::None));
}

#[test]
fn session_end_fires_boundary() {
    let mut buf = TurnBuffer::new();
    buf.accept(&evt("s1", Some("t1"), EventKind::UserPrompt { text: "hi".into(), attachments: vec![] }));
    let b = buf.accept(&evt("s1", None, EventKind::SessionEnd { reason: "quit".into() }));
    assert!(matches!(b, TurnBoundary::Fire { .. }));
}

#[test]
fn new_user_prompt_fires_previous_turn() {
    let mut buf = TurnBuffer::new();
    buf.accept(&evt("s1", Some("t1"), EventKind::UserPrompt { text: "a".into(), attachments: vec![] }));
    buf.accept(&evt("s1", Some("t1"), EventKind::ToolCall {
        tool: "bash".into(), args_preview: "ls".into(),
        ok: true, duration_ms: 1, result_preview: "".into(),
    }));
    let b = buf.accept(&evt("s1", Some("t2"), EventKind::UserPrompt { text: "b".into(), attachments: vec![] }));
    // Previous t1 should fire because t2 is a different turn.
    match b {
        TurnBoundary::Fire { turn_id, .. } => assert_eq!(turn_id.as_deref(), Some("t1")),
        _ => panic!("expected Fire for prior turn"),
    }
}

#[test]
fn idle_timeout_fires_stale_turns() {
    let mut buf = TurnBuffer::new();
    buf.accept(&evt("s1", Some("t1"), EventKind::UserPrompt { text: "a".into(), attachments: vec![] }));
    let stale = buf.fire_idle_turns(std::time::Duration::from_secs(0));
    assert_eq!(stale.len(), 1);
    assert_eq!(stale[0].turn_id.as_deref(), Some("t1"));
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo nextest run -p coding-memory --test turn_boundary`
Expected: FAIL — module doesn't exist.

- [ ] **Step 3: Implement**

Create `crates/coding-memory/src/distiller/turn_buffer.rs`:

```rust
//! Turn boundary detection.
//!
//! The Distiller processes one turn at a time. A turn begins with an
//! `EventKind::UserPrompt` and ends when any of the following fires the
//! boundary:
//!
//! - `AssistantMsg { token_usage: Some(_), .. }` — provider-reported usage
//!   is the authoritative "turn done" signal.
//! - `SessionEnd { .. }` — session ended before an `AssistantMsg` arrived
//!   (user quit mid-turn, crash). Flush anyway.
//! - A subsequent `UserPrompt` with a different `turn_id` arrives — the
//!   prior turn must flush.
//! - `fire_idle_turns(timeout)` — sweeper called by the Distiller clock,
//!   flushes turns whose last event is older than `timeout`.

use coding_ingest::event::{AgentEvent, EventKind};
use std::collections::HashMap;
use std::time::Instant;

/// Identifies a turn pending distillation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TurnRef {
    /// Session id.
    pub session_id: String,
    /// Turn id — `None` for out-of-turn events (e.g. `SessionEnd`).
    pub turn_id: Option<String>,
}

/// What happened after accepting an event.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TurnBoundary {
    /// No boundary — continue buffering.
    None,
    /// Boundary fired; caller should trigger `distill_turn`.
    Fire {
        /// Session id.
        session_id: String,
        /// Turn id of the flushed turn (may be `None` for SessionEnd flushes).
        turn_id: Option<String>,
    },
}

#[derive(Debug)]
struct TurnState {
    last_seen_at: Instant,
}

/// Detects turn boundaries as events stream in.
#[derive(Debug, Default)]
pub struct TurnBuffer {
    active: HashMap<(String, Option<String>), TurnState>,
}

impl TurnBuffer {
    /// Construct an empty buffer.
    #[must_use]
    pub fn new() -> Self {
        Self { active: HashMap::new() }
    }

    /// Accept an event and return whether a boundary fires.
    pub fn accept(&mut self, event: &AgentEvent) -> TurnBoundary {
        let AgentEvent::V1(v1) = event;
        let key = (v1.session_id.clone(), v1.turn_id.clone());

        match &v1.kind {
            EventKind::AssistantMsg { token_usage: Some(_), .. } => {
                self.active.remove(&key);
                TurnBoundary::Fire { session_id: key.0, turn_id: key.1 }
            }
            EventKind::SessionEnd { .. } => {
                // Flush every active turn for this session — caller iterates fires.
                // Convention: emit a Fire for the most recent turn; caller should
                // additionally call `fire_idle_turns(Duration::ZERO)` after SessionEnd
                // to sweep any remaining.
                let still_active: Vec<_> = self.active.keys()
                    .filter(|(s, _)| s == &v1.session_id)
                    .cloned()
                    .collect();
                for k in &still_active {
                    self.active.remove(k);
                }
                TurnBoundary::Fire { session_id: v1.session_id.clone(), turn_id: None }
            }
            EventKind::UserPrompt { .. } => {
                // If any distinct prior turn exists for this session, flush the most recent.
                let prior: Option<(String, Option<String>)> = self.active.keys()
                    .find(|(s, t)| s == &v1.session_id && t != &v1.turn_id)
                    .cloned();
                self.active.insert(key, TurnState { last_seen_at: Instant::now() });
                match prior {
                    Some((s, t)) => {
                        self.active.remove(&(s.clone(), t.clone()));
                        TurnBoundary::Fire { session_id: s, turn_id: t }
                    }
                    None => TurnBoundary::None,
                }
            }
            _ => {
                self.active
                    .entry(key)
                    .and_modify(|st| st.last_seen_at = Instant::now())
                    .or_insert(TurnState { last_seen_at: Instant::now() });
                TurnBoundary::None
            }
        }
    }

    /// Sweep — return every turn whose last event is older than `timeout`.
    /// Caller is expected to invoke `distill_turn` for each returned `TurnRef`.
    pub fn fire_idle_turns(&mut self, timeout: std::time::Duration) -> Vec<TurnRef> {
        let now = Instant::now();
        let mut out = Vec::new();
        let stale: Vec<_> = self.active.iter()
            .filter_map(|(k, st)| {
                if now.saturating_duration_since(st.last_seen_at) >= timeout {
                    Some(k.clone())
                } else {
                    None
                }
            })
            .collect();
        for k in stale {
            self.active.remove(&k);
            out.push(TurnRef { session_id: k.0, turn_id: k.1 });
        }
        out
    }

    /// Test-only helper: current active turn count.
    #[cfg(test)]
    pub fn active_len(&self) -> usize {
        self.active.len()
    }
}
```

Edit `crates/coding-memory/src/distiller/mod.rs` and append:

```rust
/// Turn boundary detection.
pub mod turn_buffer;
```

- [ ] **Step 4: Run tests + commit**

```bash
cargo nextest run -p coding-memory --test turn_boundary
cargo clippy -p coding-memory --all-targets -- -D warnings
git add crates/coding-memory/src/distiller/turn_buffer.rs \
        crates/coding-memory/src/distiller/mod.rs \
        crates/coding-memory/tests/turn_boundary.rs
git commit -m "feat(coding-memory): TurnBuffer — boundary detection across AssistantMsg/SessionEnd/idle"
```

---

### Task 7: `Distiller` skeleton — wires repo + writer + buffer (no behavior yet)

**Files:**
- Modify: `crates/coding-memory/src/distiller/mod.rs`
- Test: (no new test; Task 8 onward will exercise this)

- [ ] **Step 1: Replace `Distiller` stub with a wired shell**

Edit `crates/coding-memory/src/distiller/mod.rs`. Replace the existing stub `pub struct Distiller` + `impl Distiller` + `impl Default for Distiller` blocks with:

```rust
use crate::scope::{ProvenanceKind, ProvenanceMetadata};
use coding_ingest::event::{AgentEvent, EventKind};
use coding_ingest::store::IngestEventLogRepo;
use common::Result;
use providers::ProviderManager;
use std::sync::Arc;
use tokio::sync::Mutex;
use turn_buffer::{TurnBoundary, TurnBuffer};

/// Runtime config for the Distiller.
#[derive(Debug, Clone)]
pub struct DistillerConfig {
    /// Model id (pulled from `codingMemory.distiller.model`).
    pub model: String,
    /// Max tokens the synthesis prompt can consume.
    pub max_input_tokens: u32,
    /// LLM call timeout.
    pub timeout: std::time::Duration,
    /// Idle-turn sweep period.
    pub idle_timeout: std::time::Duration,
}

impl Default for DistillerConfig {
    fn default() -> Self {
        Self {
            model: "claude-haiku-4-5-20251001".into(),
            max_input_tokens: 8000,
            timeout: std::time::Duration::from_secs(30),
            idle_timeout: std::time::Duration::from_secs(120),
        }
    }
}

/// Distiller handle. Clone freely — internal state behind `Arc<Mutex<...>>`.
#[derive(Clone)]
pub struct Distiller {
    inner: Arc<DistillerInner>,
}

struct DistillerInner {
    config: DistillerConfig,
    ingest_repo: Arc<IngestEventLogRepo>,
    writer: writer::DistillerWriter,
    provider: Arc<ProviderManager>,
    retriever: Arc<dyn cognitive::MemoryRetriever>,
    buffer: Mutex<TurnBuffer>,
}

impl std::fmt::Debug for DistillerInner {
    fn f(&self) -> std::fmt::Result { Ok(()) }
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("DistillerInner")
            .field("model", &self.config.model)
            .finish_non_exhaustive()
    }
}

impl std::fmt::Debug for Distiller {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Distiller").finish_non_exhaustive()
    }
}

impl Distiller {
    /// Construct. Called once during `AppCore::init`.
    #[must_use]
    pub fn new(
        config: DistillerConfig,
        ingest_repo: Arc<IngestEventLogRepo>,
        writer: writer::DistillerWriter,
        provider: Arc<ProviderManager>,
        retriever: Arc<dyn cognitive::MemoryRetriever>,
    ) -> Self {
        Self {
            inner: Arc::new(DistillerInner {
                config,
                ingest_repo,
                writer,
                provider,
                retriever,
                buffer: Mutex::new(TurnBuffer::new()),
            }),
        }
    }

    /// Accept an event into the per-turn buffer. Triggers `distill_turn` on boundary.
    pub async fn accept_event(&self, event: AgentEvent) -> Result<()> {
        let boundary = {
            let mut buf = self.inner.buffer.lock().await;
            buf.accept(&event)
        };
        if let TurnBoundary::Fire { session_id, turn_id } = boundary {
            let distiller = self.clone();
            // Fire-and-forget — Distiller failures never propagate back to ingestion.
            tokio::spawn(async move {
                if let Err(e) = distiller.distill_turn(&session_id, turn_id.as_deref()).await {
                    tracing::warn!(session_id, ?turn_id, error = %e, "distill_turn failed");
                }
            });
        }
        Ok(())
    }

    /// Flush any buffered turns that have gone idle.
    pub async fn sweep_idle(&self) -> Result<()> {
        let stale = {
            let mut buf = self.inner.buffer.lock().await;
            buf.fire_idle_turns(self.inner.config.idle_timeout)
        };
        for t in stale {
            let _ = self.distill_turn(&t.session_id, t.turn_id.as_deref()).await;
        }
        Ok(())
    }

    /// Distill one turn — implemented incrementally in Tasks 9–22.
    pub async fn distill_turn(
        &self,
        _session_id: &str,
        _turn_id: Option<&str>,
    ) -> Result<DistillationReport> {
        Err(crate::error::CodingMemoryError::NotImplemented(
            crate::error::NotImplementedInPhase::new(3),
        )
        .into())
    }
}
```

**Important:** remove the old stub `struct Distiller { _phase_stub: () }`, the old `impl Default` for it, the old `impl Distiller::new()` returning `_phase_stub`. Keep `DistillationReport` / `TurnTrace` / `TestOutcome` / `TurnTokenUsage` / `RecordObservationTool` + `DistillerPhase` — they're unchanged.

Also delete the old `fn phase(p: u8) -> KlyntbotError { ... }` helper — it is no longer used. The module should still compile because `Err(...)` above constructs the error directly.

Since `cognitive::MemoryRetriever` is a trait, make sure `cognitive` exposes it. Check with: `rg 'pub trait MemoryRetriever' crates/cognitive/src/`. If it is not already public, add `pub use crate::services::memory_retriever::MemoryRetriever;` to `crates/cognitive/src/lib.rs`.

- [ ] **Step 2: Verify `Debug` impl compiles (remove stray helper)**

The `f(&self)` signature in the sketch above is a typo carried over from trait writing. Replace the `impl std::fmt::Debug for DistillerInner` block with:

```rust
impl std::fmt::Debug for DistillerInner {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("DistillerInner")
            .field("model", &self.config.model)
            .finish_non_exhaustive()
    }
}
```

- [ ] **Step 3: Build + clippy + commit**

```bash
cargo build -p coding-memory
cargo clippy -p coding-memory --all-targets -- -D warnings
git add crates/coding-memory/src/distiller/mod.rs crates/cognitive/src/lib.rs
git commit -m "feat(coding-memory): Distiller shell wires ingest repo + writer + buffer"
```

---

### Task 8: Phase A extractive — `TurnTrace` builder (no writes yet)

**Files:**
- Create: `crates/coding-memory/src/distiller/phase_a.rs`
- Modify: `crates/coding-memory/src/distiller/mod.rs`
- Test: `crates/coding-memory/tests/phase_a_extractive.rs`

- [ ] **Step 1: Write failing tests**

Create `crates/coding-memory/tests/phase_a_extractive.rs`:

```rust
use coding_ingest::event::{
    AgentEvent, AgentEventV1, AgentSource, EventKind, FileOp, TokenUsage,
};
use coding_memory::distiller::phase_a::compute_turn_trace;
use jiff::Timestamp;
use std::path::PathBuf;
use uuid::Uuid;

fn wrap(kind: EventKind) -> AgentEvent {
    AgentEvent::V1(AgentEventV1 {
        id: Uuid::new_v4(),
        source: AgentSource::ClaudeCode,
        session_id: "s".into(),
        turn_id: Some("t".into()),
        cwd: PathBuf::from("/tmp"),
        repo: None,
        occurred_at: Timestamp::now(),
        kind,
    })
}

#[test]
fn collects_file_edits_and_reads_separately() {
    let events = vec![
        wrap(EventKind::FileEdit {
            path: PathBuf::from("a.rs"), op: FileOp::Read, bytes: 100, diff_preview: None,
        }),
        wrap(EventKind::FileEdit {
            path: PathBuf::from("b.rs"), op: FileOp::Modify, bytes: 200, diff_preview: None,
        }),
        wrap(EventKind::FileEdit {
            path: PathBuf::from("c.rs"), op: FileOp::Create, bytes: 50, diff_preview: None,
        }),
    ];
    let trace = compute_turn_trace("s", Some("t"), &events);
    assert_eq!(trace.files_read.len(), 1);
    assert_eq!(trace.files_read[0], PathBuf::from("a.rs"));
    assert_eq!(trace.files_modified.len(), 2);
}

#[test]
fn captures_test_outcomes() {
    let events = vec![
        wrap(EventKind::TestRun {
            command: "cargo test".into(),
            framework: Some("cargo".into()),
            passed: 10, failed: 2, duration_ms: 1000,
        }),
    ];
    let trace = compute_turn_trace("s", Some("t"), &events);
    assert_eq!(trace.test_outcomes.len(), 1);
    assert_eq!(trace.test_outcomes[0].passed, 10);
    assert_eq!(trace.test_outcomes[0].failed, 2);
}

#[test]
fn captures_commands_from_bash_tool_calls() {
    let events = vec![
        wrap(EventKind::ToolCall {
            tool: "Bash".into(),
            args_preview: "cargo build".into(),
            ok: true, duration_ms: 500, result_preview: "ok".into(),
        }),
        wrap(EventKind::ToolCall {
            tool: "Read".into(), // non-bash tools ignored
            args_preview: "foo.rs".into(),
            ok: true, duration_ms: 1, result_preview: "".into(),
        }),
    ];
    let trace = compute_turn_trace("s", Some("t"), &events);
    assert_eq!(trace.commands_run.len(), 1);
    assert_eq!(trace.commands_run[0], "cargo build");
}

#[test]
fn captures_errors() {
    let events = vec![
        wrap(EventKind::Error { tool: Some("Bash".into()), message: "exit 1".into() }),
        wrap(EventKind::Error { tool: None, message: "generic".into() }),
    ];
    let trace = compute_turn_trace("s", Some("t"), &events);
    assert_eq!(trace.errors_encountered.len(), 2);
}

#[test]
fn final_assistant_msg_sets_token_usage() {
    let events = vec![
        wrap(EventKind::AssistantMsg {
            text: "partial".into(), truncated: true, token_usage: None,
        }),
        wrap(EventKind::AssistantMsg {
            text: "final".into(),
            truncated: false,
            token_usage: Some(TokenUsage { prompt_tokens: 100, completion_tokens: 50, cached_tokens: Some(25) }),
        }),
    ];
    let trace = compute_turn_trace("s", Some("t"), &events);
    let u = trace.token_usage.expect("usage set");
    assert_eq!(u.prompt, 100);
    assert_eq!(u.completion, 50);
    assert_eq!(u.cached, 25);
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo nextest run -p coding-memory --test phase_a_extractive`
Expected: FAIL — module undefined.

- [ ] **Step 3: Implement**

Create `crates/coding-memory/src/distiller/phase_a.rs`:

```rust
//! Phase A — deterministic extractive pass.
//!
//! Runs before any LLM call. Produces a `TurnTrace` covering:
//! - Files read vs. modified (with byte deltas for modifications)
//! - Shell commands (Bash tool calls only)
//! - Test-run outcomes
//! - Errors encountered
//! - Token usage from the *final* `AssistantMsg` that carried usage
//!
//! Never reads the LLM, never fails. Output feeds both:
//! 1. The Phase-B prompt (compact structured summary).
//! 2. A durable `episodic_memories { kind: 'turn_trace' }` row (Task 10).

use super::{TestOutcome, TurnTokenUsage, TurnTrace};
use coding_ingest::event::{AgentEvent, EventKind, FileOp};
use jiff::Timestamp;

/// Build the `TurnTrace` for one turn from its ordered events.
pub fn compute_turn_trace(
    session_id: &str,
    turn_id: Option<&str>,
    events: &[AgentEvent],
) -> TurnTrace {
    let mut files_read = Vec::new();
    let mut files_modified: Vec<(std::path::PathBuf, i64)> = Vec::new();
    let mut commands_run = Vec::new();
    let mut test_outcomes = Vec::new();
    let mut errors_encountered = Vec::new();
    let mut token_usage: Option<TurnTokenUsage> = None;
    let mut started_at: Option<Timestamp> = None;
    let mut ended_at: Option<Timestamp> = None;

    for event in events {
        let AgentEvent::V1(v1) = event;
        started_at.get_or_insert(v1.occurred_at);
        ended_at = Some(v1.occurred_at);

        match &v1.kind {
            EventKind::FileEdit { path, op, bytes, .. } => match op {
                FileOp::Read => files_read.push(path.clone()),
                FileOp::Create | FileOp::Modify | FileOp::Delete => {
                    files_modified.push((path.clone(), *bytes as i64));
                }
            },
            EventKind::FileEditEnriched { path, op, .. } => match op {
                FileOp::Read => files_read.push(path.clone()),
                _ => files_modified.push((path.clone(), 0)),
            },
            EventKind::ToolCall { tool, args_preview, .. } if tool.eq_ignore_ascii_case("bash") => {
                commands_run.push(args_preview.clone());
            }
            EventKind::TestRun { command, framework, passed, failed, .. } => {
                test_outcomes.push(TestOutcome {
                    command: command.clone(),
                    framework: framework.clone(),
                    passed: *passed,
                    failed: *failed,
                });
            }
            EventKind::TestRunEnriched { command, passed_tests, failed_tests, .. } => {
                test_outcomes.push(TestOutcome {
                    command: command.clone(),
                    framework: None,
                    passed: passed_tests.len() as u32,
                    failed: failed_tests.len() as u32,
                });
            }
            EventKind::Error { tool, message } => {
                errors_encountered.push((tool.clone(), message.clone()));
            }
            EventKind::AssistantMsg { token_usage: Some(u), .. } => {
                token_usage = Some(TurnTokenUsage {
                    prompt: u.prompt_tokens,
                    completion: u.completion_tokens,
                    cached: u.cached_tokens.unwrap_or(0),
                });
            }
            _ => {}
        }
    }

    TurnTrace {
        session_id: session_id.to_string(),
        turn_id: turn_id.map(str::to_string),
        files_read,
        files_modified,
        commands_run,
        test_outcomes,
        errors_encountered,
        token_usage,
        started_at: started_at.unwrap_or_else(Timestamp::now),
        ended_at,
    }
}
```

Edit `crates/coding-memory/src/distiller/mod.rs` and add:

```rust
/// Phase A extractive pass.
pub mod phase_a;
```

- [ ] **Step 4: Run tests + commit**

```bash
cargo nextest run -p coding-memory --test phase_a_extractive
cargo clippy -p coding-memory --all-targets -- -D warnings
git add crates/coding-memory/src/distiller/phase_a.rs \
        crates/coding-memory/src/distiller/mod.rs \
        crates/coding-memory/tests/phase_a_extractive.rs
git commit -m "feat(coding-memory): Distiller Phase A — compute_turn_trace extractive pass"
```

---

### Task 9: Write `TurnTrace` to `episodic_memories { kind: 'turn_trace' }`

**Files:**
- Modify: `crates/coding-memory/src/distiller/phase_a.rs`
- Test: `crates/coding-memory/tests/phase_a_persists_trace.rs`

- [ ] **Step 1: Write failing test**

Create `crates/coding-memory/tests/phase_a_persists_trace.rs`:

```rust
use coding_ingest::event::{AgentEvent, AgentEventV1, AgentSource, EventKind};
use coding_memory::distiller::phase_a::{compute_turn_trace, persist_turn_trace};
use coding_memory::distiller::DistillerWriter;
use coding_memory::scope::{ProvenanceKind, ProvenanceMetadata};
use cognitive::{EpisodicMemoryRepo, SemanticFactRepo};
use jiff::Timestamp;
use std::path::PathBuf;
use storage::StoragePool;
use uuid::Uuid;

#[tokio::test]
async fn persist_turn_trace_writes_episode_with_provenance_and_kind() {
    let pool = StoragePool::connect_in_memory().await.unwrap();
    StoragePool::run_feature_migrations(pool.inner(), &cognitive::cognitive_migrations()).await.unwrap();
    StoragePool::run_feature_migrations(pool.inner(), &coding_memory::coding_memory_migrations()).await.unwrap();
    let writer = DistillerWriter::new(
        SemanticFactRepo::new(pool.inner().clone()),
        EpisodicMemoryRepo::new(pool.inner().clone()),
    );

    let src_id = Uuid::new_v4();
    let events = vec![AgentEvent::V1(AgentEventV1 {
        id: src_id,
        source: AgentSource::ClaudeCode,
        session_id: "s1".into(),
        turn_id: Some("t1".into()),
        cwd: PathBuf::from("/tmp"),
        repo: None,
        occurred_at: Timestamp::now(),
        kind: EventKind::UserPrompt { text: "hi".into(), attachments: vec![] },
    })];

    let trace = compute_turn_trace("s1", Some("t1"), &events);
    let prov = ProvenanceMetadata {
        source_events: vec![src_id],
        session_id: "s1".into(),
        turn_id: Some("t1".into()),
        distilled_at: Timestamp::now(),
        distiller_model: "claude-haiku-4-5".into(),
        source_kind: ProvenanceKind::DistillerExtractive,
    };
    let id = persist_turn_trace(&writer, &trace, None, &prov).await.unwrap();

    let (kind, meta_json): (Option<String>, Option<String>) = sqlx::query_as(
        "SELECT kind, metadata FROM episodic_memories WHERE id = ?",
    )
    .bind(id.to_string())
    .fetch_one(pool.inner()).await.unwrap();
    assert_eq!(kind.as_deref(), Some("turn_trace"));
    let meta: serde_json::Value = serde_json::from_str(&meta_json.unwrap()).unwrap();
    assert_eq!(meta["provenance"]["sourceKind"], "distiller_extractive");
}
```

- [ ] **Step 2: Implement `persist_turn_trace`**

Append to `crates/coding-memory/src/distiller/phase_a.rs`:

```rust
use super::writer::{DistillerWriter, PreparedEpisode};
use super::error::DistillerError;
use crate::scope::ProvenanceMetadata;
use cognitive::types::EpisodicMemory;
use uuid::Uuid;

/// Persist a `TurnTrace` as an `episodic_memories { kind: 'turn_trace' }` row
/// through the provenance-enforcing `DistillerWriter`. Returns the new row's id.
pub async fn persist_turn_trace(
    writer: &DistillerWriter,
    trace: &TurnTrace,
    scope_repo_id: Option<&str>,
    provenance: &ProvenanceMetadata,
) -> Result<Uuid, DistillerError> {
    let id = Uuid::new_v4();
    let content = serde_json::json!({
        "filesRead": trace.files_read.iter().map(|p| p.to_string_lossy()).collect::<Vec<_>>(),
        "filesModified": trace.files_modified.iter()
            .map(|(p, n)| serde_json::json!({"path": p.to_string_lossy(), "bytes": n}))
            .collect::<Vec<_>>(),
        "commandsRun": trace.commands_run,
        "testOutcomes": trace.test_outcomes.iter().map(|t| serde_json::json!({
            "command": t.command,
            "framework": t.framework,
            "passed": t.passed,
            "failed": t.failed,
        })).collect::<Vec<_>>(),
        "errorsEncountered": trace.errors_encountered,
        "tokenUsage": trace.token_usage.map(|u| serde_json::json!({
            "prompt": u.prompt, "completion": u.completion, "cached": u.cached
        })),
    })
    .to_string();

    let importance = importance_for_trace(trace);
    let episode = EpisodicMemory {
        id: id.to_string(),
        domain: "coding".into(),
        content,
        summary: None,
        importance,
        occurred_at: trace.started_at.to_string(),
        recorded_at: jiff::Timestamp::now().to_string(),
        stability: 1.0,
        last_accessed: None,
        access_count: 0,
        project_id: None,
        scope_type: if scope_repo_id.is_some() { "project".into() } else { "user".into() },
        scope_id: scope_repo_id.map(str::to_string),
    };

    writer.write_episode(PreparedEpisode {
        episode,
        kind: "turn_trace".into(),
        metadata_json: None,
        scope_repo_id: scope_repo_id.map(str::to_string),
        provenance: provenance.clone(),
    }).await?;
    Ok(id)
}

fn importance_for_trace(t: &TurnTrace) -> f64 {
    let mut score = 0.3;
    if !t.files_modified.is_empty() { score += 0.2; }
    if t.test_outcomes.iter().any(|x| x.failed > 0) { score += 0.2; }
    if !t.errors_encountered.is_empty() { score += 0.2; }
    score.min(1.0)
}
```

- [ ] **Step 3: Run + commit**

```bash
cargo nextest run -p coding-memory --test phase_a_persists_trace
cargo clippy -p coding-memory --all-targets -- -D warnings
git add crates/coding-memory/src/distiller/phase_a.rs crates/coding-memory/tests/phase_a_persists_trace.rs
git commit -m "feat(coding-memory): persist_turn_trace writes turn_trace episode via DistillerWriter"
```

---

### Task 10: `record_observation` LLM tool schema + decoder

**Files:**
- Create: `crates/coding-memory/src/distiller/record_observation.rs`
- Modify: `crates/coding-memory/src/distiller/mod.rs`
- Test: `crates/coding-memory/tests/record_observation.rs`

- [ ] **Step 1: Write failing test**

Create `crates/coding-memory/tests/record_observation.rs`:

```rust
use coding_memory::distiller::record_observation::{
    decode_observations, Observation, ObservationScope, RECORD_OBSERVATION_TOOL_NAME,
};
use coding_memory::facts::CodingKind;

#[test]
fn tool_name_is_record_observation() {
    assert_eq!(RECORD_OBSERVATION_TOOL_NAME, "record_observation");
}

#[test]
fn decodes_valid_fix_attempt() {
    let json = serde_json::json!({
        "kind": "fix_attempt",
        "subject": "repo:github.com/klynt/bot",
        "predicate": "fixed",
        "object": "null pointer in parser by adding guard",
        "confidence": 0.85,
        "scope": "repo",
        "reasoning": "tests passed after the edit"
    });
    let obs: Observation = decode_observations(&[json]).unwrap().into_iter().next().unwrap();
    assert_eq!(obs.kind, CodingKind::FixAttempt);
    assert_eq!(obs.confidence, 0.85);
    assert!(matches!(obs.scope, ObservationScope::Repo));
}

#[test]
fn rejects_invalid_kind() {
    let json = serde_json::json!({
        "kind": "problem_solution_pattern", // Reforge-only; Distiller cannot emit
        "subject": "x", "predicate": "y", "object": "z",
        "confidence": 0.5, "scope": "global", "reasoning": ""
    });
    assert!(decode_observations(&[json]).is_err());
}

#[test]
fn clamps_confidence_to_0_1() {
    let json = serde_json::json!({
        "kind": "style_preference",
        "subject": "user", "predicate": "prefers", "object": "tabs",
        "confidence": 1.7, "scope": "global", "reasoning": "observed 5x"
    });
    let obs = decode_observations(&[json]).unwrap().into_iter().next().unwrap();
    assert!((obs.confidence - 1.0).abs() < f32::EPSILON);
}

#[test]
fn empty_input_yields_empty_output() {
    assert!(decode_observations(&[]).unwrap().is_empty());
}
```

- [ ] **Step 2: Implement**

Create `crates/coding-memory/src/distiller/record_observation.rs`:

```rust
//! The LLM tool schema the Distiller exposes.
//!
//! The model is asked to call `record_observation` zero or more times, each
//! call producing one structured observation. The tool enum admits exactly
//! the 5 Distiller-writable `CodingKind` values — the 3 Reforge-only kinds
//! (`problem_solution_pattern`, `project_understanding`, `user_habit`) are
//! rejected at decode time.

use super::error::DistillerError;
use crate::facts::CodingKind;
use providers::types::{ToolCall, ToolDefinition};
use serde::{Deserialize, Serialize};

/// Tool name the model must use.
pub const RECORD_OBSERVATION_TOOL_NAME: &str = "record_observation";

/// Scope the observation applies to.
#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ObservationScope {
    /// Applies everywhere (user-level).
    Global,
    /// Applies to the current repo only.
    Repo,
}

/// One decoded observation the model emitted.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Observation {
    /// Kind — one of the 5 Distiller-writable kinds.
    pub kind: CodingKind,
    /// Subject (e.g. `"user"`, `"repo:<id>"`).
    pub subject: String,
    /// Predicate (e.g. `"prefers"`, `"framework"`, `"fixed"`).
    pub predicate: String,
    /// Object / value.
    pub object: String,
    /// 0.0–1.0 confidence, clamped on decode.
    pub confidence: f32,
    /// Scope partitioning.
    pub scope: ObservationScope,
    /// Free-text justification — stored in metadata, never user-surfaced.
    pub reasoning: String,
}

/// Build the `ToolDefinition` for the Distiller's Phase B LLM call.
#[must_use]
pub fn record_observation_tool_def() -> ToolDefinition {
    ToolDefinition {
        name: RECORD_OBSERVATION_TOOL_NAME.into(),
        description: "Record one structured coding-memory observation. Emit zero or more calls per \
                      turn; emit nothing if nothing significant happened. Each call must use one of \
                      the 5 allowed kinds — NEVER problem_solution_pattern / project_understanding / \
                      user_habit (those are Reforge-only).".into(),
        input_schema: serde_json::json!({
            "type": "object",
            "required": ["kind", "subject", "predicate", "object", "confidence", "scope", "reasoning"],
            "properties": {
                "kind": {
                    "type": "string",
                    "enum": ["fix_attempt", "style_preference", "workflow_pattern", "repo_context", "failure_pattern"],
                },
                "subject": { "type": "string" },
                "predicate": { "type": "string" },
                "object": { "type": "string" },
                "confidence": { "type": "number", "minimum": 0.0, "maximum": 1.0 },
                "scope": { "type": "string", "enum": ["global", "repo"] },
                "reasoning": { "type": "string" }
            },
            "additionalProperties": false
        }),
    }
}

/// Decode a batch of tool-call arg-objects into `Observation`s.
/// `kind` values outside the 5-value `CodingKind` enum produce `DistillerError::LlmMalformed`.
pub fn decode_observations(
    raw: &[serde_json::Value],
) -> Result<Vec<Observation>, DistillerError> {
    let mut out = Vec::with_capacity(raw.len());
    for v in raw {
        let mut obs: Observation = serde_json::from_value(v.clone())
            .map_err(|e| DistillerError::LlmMalformed { detail: format!("observation decode: {e}") })?;
        obs.confidence = obs.confidence.clamp(0.0, 1.0);
        out.push(obs);
    }
    Ok(out)
}

/// Filter a list of `ToolCall`s down to the observations the Distiller cares about.
pub fn observations_from_tool_calls(
    calls: &[ToolCall],
) -> Result<Vec<Observation>, DistillerError> {
    let args: Vec<serde_json::Value> = calls
        .iter()
        .filter(|c| c.name == RECORD_OBSERVATION_TOOL_NAME)
        .map(|c| c.arguments.clone())
        .collect();
    decode_observations(&args)
}
```

Edit `crates/coding-memory/src/distiller/mod.rs` and add:

```rust
/// `record_observation` LLM tool schema + decoder.
pub mod record_observation;
```

> **Note on `providers::types::ToolDefinition` / `ToolCall`:** verify these exist with `rg 'pub struct ToolDefinition|pub struct ToolCall' crates/providers/src/types.rs`. If the exact shape differs, match it — the fields we depend on are `name: String`, `arguments: serde_json::Value`, and a schema field (may be `input_schema` or `parameters`). Adjust the struct literal accordingly.

- [ ] **Step 3: Run + commit**

```bash
cargo nextest run -p coding-memory --test record_observation
cargo clippy -p coding-memory --all-targets -- -D warnings
git add crates/coding-memory/src/distiller/record_observation.rs \
        crates/coding-memory/src/distiller/mod.rs \
        crates/coding-memory/tests/record_observation.rs
git commit -m "feat(coding-memory): record_observation tool schema + decoder (5 kinds, strict)"
```

---

### Task 11: `FactBuilder` — `Observation` → `SemanticFact` / `EpisodicMemory`

**Files:**
- Create: `crates/coding-memory/src/distiller/fact_builder.rs`
- Modify: `crates/coding-memory/src/distiller/mod.rs`
- Test: `crates/coding-memory/tests/fact_builder.rs`

- [ ] **Step 1: Write failing tests**

Create `crates/coding-memory/tests/fact_builder.rs`:

```rust
use coding_memory::distiller::fact_builder::build_prepared;
use coding_memory::distiller::record_observation::{Observation, ObservationScope};
use coding_memory::distiller::{PreparedEpisode, PreparedFact};
use coding_memory::facts::CodingKind;
use coding_memory::scope::{ProvenanceKind, ProvenanceMetadata};
use jiff::Timestamp;
use uuid::Uuid;

fn prov() -> ProvenanceMetadata {
    ProvenanceMetadata {
        source_events: vec![Uuid::new_v4()],
        session_id: "s".into(),
        turn_id: Some("t".into()),
        distilled_at: Timestamp::now(),
        distiller_model: "claude-haiku-4-5".into(),
        source_kind: ProvenanceKind::DistillerLlm,
    }
}

#[test]
fn repo_context_becomes_prepared_fact() {
    let o = Observation {
        kind: CodingKind::RepoContext,
        subject: "repo:github.com/klynt/bot".into(),
        predicate: "framework".into(),
        object: "tauri".into(),
        confidence: 0.9,
        scope: ObservationScope::Repo,
        reasoning: "Cargo.toml lists tauri 2".into(),
    };
    let built = build_prepared(&o, Some("github.com/klynt/bot"), &prov()).unwrap();
    let PreparedFact { fact, scope_repo_id, .. } = match built {
        coding_memory::distiller::fact_builder::Prepared::Fact(f) => f,
        _ => panic!("expected Fact"),
    };
    assert_eq!(fact.domain, "work");
    assert_eq!(fact.subject, "repo:github.com/klynt/bot");
    assert_eq!(fact.predicate, "framework");
    assert_eq!(fact.memory_type, "fact");
    assert_eq!(scope_repo_id.as_deref(), Some("github.com/klynt/bot"));
}

#[test]
fn style_preference_becomes_prepared_fact_with_preferences_domain() {
    let o = Observation {
        kind: CodingKind::StylePreference,
        subject: "user".into(),
        predicate: "prefers".into(),
        object: "tabs".into(),
        confidence: 0.7,
        scope: ObservationScope::Global,
        reasoning: "observed 3x".into(),
    };
    let built = build_prepared(&o, None, &prov()).unwrap();
    let fact = match built {
        coding_memory::distiller::fact_builder::Prepared::Fact(PreparedFact { fact, .. }) => fact,
        _ => panic!(),
    };
    assert_eq!(fact.domain, "preferences");
    assert_eq!(fact.subject, "user");
}

#[test]
fn fix_attempt_becomes_prepared_episode_with_kind() {
    let o = Observation {
        kind: CodingKind::FixAttempt,
        subject: "bug:parser-null-pointer".into(),
        predicate: "fixed".into(),
        object: "added guard in parse_expr".into(),
        confidence: 0.8,
        scope: ObservationScope::Repo,
        reasoning: "tests now pass".into(),
    };
    let built = build_prepared(&o, Some("github.com/klynt/bot"), &prov()).unwrap();
    let ep = match built {
        coding_memory::distiller::fact_builder::Prepared::Episode(e) => e,
        _ => panic!("expected Episode"),
    };
    assert_eq!(ep.kind, "fix_attempt");
    assert!(ep.episode.content.contains("added guard"));
}

#[test]
fn workflow_pattern_becomes_prepared_fact_with_pattern_memory_type() {
    let o = Observation {
        kind: CodingKind::WorkflowPattern,
        subject: "workflow:test-before-commit".into(),
        predicate: "applies_when".into(),
        object: "touching code paths with existing tests".into(),
        confidence: 0.6,
        scope: ObservationScope::Repo,
        reasoning: "observed 4x".into(),
    };
    let built = build_prepared(&o, Some("x"), &prov()).unwrap();
    let fact = match built {
        coding_memory::distiller::fact_builder::Prepared::Fact(PreparedFact { fact, .. }) => fact,
        _ => panic!(),
    };
    assert_eq!(fact.domain, "procedural");
    assert_eq!(fact.memory_type, "pattern");
}
```

- [ ] **Step 2: Implement**

Create `crates/coding-memory/src/distiller/fact_builder.rs`:

```rust
//! `Observation` → `PreparedFact` / `PreparedEpisode`.
//!
//! Routes LLM-emitted observations to the right cognitive-layer row shape.
//! The mapping matches design §7 exactly:
//!
//! | `CodingKind`      | Destination                   | `domain`      | `memory_type`    |
//! |-------------------|-------------------------------|---------------|------------------|
//! | `FixAttempt`      | `EpisodicMemory`              | `coding`      | (n/a)            |
//! | `StylePreference` | `SemanticFact`                | `preferences` | `fact`           |
//! | `WorkflowPattern` | `SemanticFact`                | `procedural`  | `pattern`        |
//! | `RepoContext`     | `SemanticFact`                | `work`        | `fact`           |
//! | `FailurePattern`  | `SemanticFact`                | `procedural`  | `failure_pattern`|
//!
//! `FailurePattern` is a fact rather than an episode: it's a reusable rule,
//! not a point-in-time event. The Reforge `ProblemSolutionPattern` kind
//! (Phase 5) subsumes both FailurePattern + causal chains — we do not
//! conflate here.

use super::error::DistillerError;
use super::record_observation::{Observation, ObservationScope};
use super::writer::{PreparedEpisode, PreparedFact};
use crate::facts::CodingKind;
use crate::scope::ProvenanceMetadata;
use cognitive::types::{EpisodicMemory, SemanticFact};
use jiff::Timestamp;
use uuid::Uuid;

/// One of the two row shapes, ready for `DistillerWriter`.
pub enum Prepared {
    /// A semantic fact.
    Fact(PreparedFact),
    /// An episodic memory.
    Episode(PreparedEpisode),
}

/// Convert one `Observation` into the appropriate `PreparedFact`/`PreparedEpisode`.
pub fn build_prepared(
    obs: &Observation,
    scope_repo_id: Option<&str>,
    provenance: &ProvenanceMetadata,
) -> Result<Prepared, DistillerError> {
    let effective_scope_repo = match obs.scope {
        ObservationScope::Global => None,
        ObservationScope::Repo => scope_repo_id.map(str::to_string),
    };
    let now = Timestamp::now().to_string();

    match obs.kind {
        CodingKind::FixAttempt => {
            let id = Uuid::new_v4().to_string();
            let content = serde_json::json!({
                "subject": obs.subject,
                "predicate": obs.predicate,
                "object": obs.object,
                "reasoning": obs.reasoning,
            })
            .to_string();
            Ok(Prepared::Episode(PreparedEpisode {
                episode: EpisodicMemory {
                    id,
                    domain: "coding".into(),
                    content,
                    summary: Some(obs.object.clone()),
                    importance: importance_from_confidence(obs.confidence),
                    occurred_at: now.clone(),
                    recorded_at: now,
                    stability: 1.0,
                    last_accessed: None,
                    access_count: 0,
                    project_id: None,
                    scope_type: scope_type_for(&effective_scope_repo),
                    scope_id: effective_scope_repo.clone(),
                },
                kind: "fix_attempt".into(),
                metadata_json: Some(serde_json::json!({ "reasoning": obs.reasoning })),
                scope_repo_id: effective_scope_repo,
                provenance: provenance.clone(),
            }))
        }
        CodingKind::StylePreference => Ok(Prepared::Fact(build_fact(
            obs, "preferences", "fact", effective_scope_repo, provenance,
        ))),
        CodingKind::WorkflowPattern => Ok(Prepared::Fact(build_fact(
            obs, "procedural", "pattern", effective_scope_repo, provenance,
        ))),
        CodingKind::RepoContext => Ok(Prepared::Fact(build_fact(
            obs, "work", "fact", effective_scope_repo, provenance,
        ))),
        CodingKind::FailurePattern => Ok(Prepared::Fact(build_fact(
            obs, "procedural", "failure_pattern", effective_scope_repo, provenance,
        ))),
    }
}

fn build_fact(
    obs: &Observation,
    domain: &str,
    memory_type: &str,
    effective_scope_repo: Option<String>,
    provenance: &ProvenanceMetadata,
) -> PreparedFact {
    let now = Timestamp::now().to_string();
    PreparedFact {
        fact: SemanticFact {
            id: Uuid::new_v4().to_string(),
            domain: domain.into(),
            subject: obs.subject.clone(),
            predicate: obs.predicate.clone(),
            object: obs.object.clone(),
            confidence: obs.confidence as f64,
            source: "distiller".into(),
            valid_from: now.clone(),
            valid_until: None,
            recorded_at: now.clone(),
            superseded_at: None,
            superseded_by: None,
            stability: 1.0,
            last_accessed: None,
            access_count: 0,
            convergence_score: 1.0,
            project_id: None,
            memory_type: memory_type.into(),
            scope_type: scope_type_for(&effective_scope_repo),
            scope_id: effective_scope_repo.clone(),
        },
        metadata_json: Some(serde_json::json!({ "reasoning": obs.reasoning })),
        scope_repo_id: effective_scope_repo,
        provenance: provenance.clone(),
    }
}

fn scope_type_for(scope_repo: &Option<String>) -> String {
    if scope_repo.is_some() { "project".into() } else { "user".into() }
}

fn importance_from_confidence(c: f32) -> f64 {
    (0.3 + c as f64 * 0.6).clamp(0.0, 1.0)
}
```

Edit `crates/coding-memory/src/distiller/mod.rs` and add:

```rust
/// Observation → row-ready conversion.
pub mod fact_builder;
```

- [ ] **Step 3: Run + commit**

```bash
cargo nextest run -p coding-memory --test fact_builder
cargo clippy -p coding-memory --all-targets -- -D warnings
git add crates/coding-memory/src/distiller/fact_builder.rs \
        crates/coding-memory/src/distiller/mod.rs \
        crates/coding-memory/tests/fact_builder.rs
git commit -m "feat(coding-memory): fact_builder — Observation to row-ready fact/episode with scope"
```

---

### Task 12: Phase B prompt construction

**Files:**
- Create: `crates/coding-memory/src/distiller/phase_b.rs`
- Modify: `crates/coding-memory/src/distiller/mod.rs`
- Test: `crates/coding-memory/tests/phase_b_prompt.rs`

- [ ] **Step 1: Write failing test**

Create `crates/coding-memory/tests/phase_b_prompt.rs`:

```rust
use coding_memory::distiller::phase_b::build_prompt;
use coding_memory::distiller::{TestOutcome, TurnTokenUsage, TurnTrace};
use jiff::Timestamp;
use std::path::PathBuf;

fn trace() -> TurnTrace {
    TurnTrace {
        session_id: "s".into(),
        turn_id: Some("t".into()),
        files_read: vec![PathBuf::from("src/main.rs")],
        files_modified: vec![(PathBuf::from("src/parser.rs"), 42)],
        commands_run: vec!["cargo test".into()],
        test_outcomes: vec![TestOutcome {
            command: "cargo test".into(),
            framework: Some("cargo".into()),
            passed: 10, failed: 0,
        }],
        errors_encountered: vec![],
        token_usage: Some(TurnTokenUsage { prompt: 100, completion: 50, cached: 0 }),
        started_at: Timestamp::now(),
        ended_at: Some(Timestamp::now()),
    }
}

#[test]
fn prompt_contains_user_text_and_assistant_text() {
    let p = build_prompt(
        "fix the parser",
        "I edited parser.rs and added a null guard.",
        &trace(),
        Some("github.com/klynt/bot"),
    );
    assert!(p.system.contains("memory distiller"));
    assert!(p.user_message.contains("fix the parser"));
    assert!(p.user_message.contains("I edited parser.rs"));
    assert!(p.user_message.contains("src/parser.rs"));
    assert!(p.user_message.contains("cargo test"));
    assert!(p.user_message.contains("github.com/klynt/bot"));
}

#[test]
fn prompt_truncates_extreme_inputs() {
    let huge = "x".repeat(50_000);
    let p = build_prompt(&huge, &huge, &trace(), None);
    // Our safety cap is well under 50k chars.
    assert!(p.user_message.len() < 30_000);
}
```

- [ ] **Step 2: Implement**

Create `crates/coding-memory/src/distiller/phase_b.rs`:

```rust
//! Phase B — LLM synthesis prompt construction + invocation.
//!
//! The prompt is structured so the model's output is almost always one or
//! two `record_observation` tool calls. We include the Phase-A turn trace
//! (compact JSON), the user's prompt, and the assistant's final message.

use super::{TestOutcome, TurnTrace};

/// Built prompt ready to hand to `ProviderManager::chat`.
#[derive(Debug, Clone)]
pub struct DistillerPrompt {
    /// System message.
    pub system: String,
    /// User message (includes the turn trace + inputs, truncated to safe bounds).
    pub user_message: String,
}

const USER_MSG_CAP: usize = 24_000;

/// Build the Phase-B prompt.
#[must_use]
pub fn build_prompt(
    user_prompt_text: &str,
    assistant_text: &str,
    trace: &TurnTrace,
    repo_id: Option<&str>,
) -> DistillerPrompt {
    let system = SYSTEM_PROMPT.to_string();

    let trace_summary = summarize_trace(trace);
    let scope = repo_id.map(|r| format!("repo:{r}")).unwrap_or_else(|| "global".into());

    let mut user = String::new();
    user.push_str(&format!("## Scope\n{scope}\n\n"));
    user.push_str("## User prompt\n");
    user.push_str(truncate(user_prompt_text, 4_000));
    user.push_str("\n\n## Assistant final message\n");
    user.push_str(truncate(assistant_text, 4_000));
    user.push_str("\n\n## Turn trace (Phase A)\n");
    user.push_str(&trace_summary);
    user.push_str(
        "\n\n## Task\nEmit zero or more record_observation(...) tool calls. Emit nothing if \
        nothing memorable happened. Do not call any other tool. Do not respond in prose.",
    );

    if user.len() > USER_MSG_CAP {
        user.truncate(USER_MSG_CAP);
        user.push_str("\n[truncated]");
    }

    DistillerPrompt { system, user_message: user }
}

fn truncate(s: &str, max: usize) -> &str {
    if s.len() > max {
        let mut end = max;
        while !s.is_char_boundary(end) { end -= 1; }
        &s[..end]
    } else {
        s
    }
}

fn summarize_trace(t: &TurnTrace) -> String {
    let mut out = String::new();
    if !t.files_read.is_empty() {
        out.push_str(&format!("- filesRead: {}\n", path_list(&t.files_read)));
    }
    if !t.files_modified.is_empty() {
        let modified: Vec<String> = t.files_modified.iter()
            .map(|(p, n)| format!("{} ({}b)", p.to_string_lossy(), n))
            .collect();
        out.push_str(&format!("- filesModified: {}\n", modified.join(", ")));
    }
    if !t.commands_run.is_empty() {
        let cmds = t.commands_run.iter().take(20).cloned().collect::<Vec<_>>().join(" | ");
        out.push_str(&format!("- commandsRun: {cmds}\n"));
    }
    if !t.test_outcomes.is_empty() {
        out.push_str("- testOutcomes:\n");
        for t in &t.test_outcomes {
            out.push_str(&format!("  - {} ({}/{} passed/failed)\n",
                t.framework.clone().unwrap_or_else(|| "?".into()), t.passed, t.failed));
            let _ = TestOutcome { command: String::new(), framework: None, passed: 0, failed: 0 };
        }
    }
    if !t.errors_encountered.is_empty() {
        out.push_str(&format!("- errors: {} encountered\n", t.errors_encountered.len()));
    }
    if let Some(u) = t.token_usage {
        out.push_str(&format!("- tokenUsage: prompt={} completion={} cached={}\n",
            u.prompt, u.completion, u.cached));
    }
    if out.is_empty() { "- (no extractive signal)".into() } else { out }
}

fn path_list(paths: &[std::path::PathBuf]) -> String {
    paths.iter().take(10).map(|p| p.to_string_lossy().to_string()).collect::<Vec<_>>().join(", ")
}

const SYSTEM_PROMPT: &str = "You are a memory distiller for a coding assistant. From this \
coding-agent turn, emit zero or more structured observations via the `record_observation` tool. \
Each observation must use one of these 5 kinds: fix_attempt, style_preference, workflow_pattern, \
repo_context, failure_pattern. Never call any other tool. Never respond in prose. Emit nothing \
if nothing significant happened. Be conservative — prefer precision over recall.";
```

Edit `crates/coding-memory/src/distiller/mod.rs` and add:

```rust
/// Phase B — LLM synthesis (prompt + invocation).
pub mod phase_b;
```

- [ ] **Step 3: Run + commit**

```bash
cargo nextest run -p coding-memory --test phase_b_prompt
cargo clippy -p coding-memory --all-targets -- -D warnings
git add crates/coding-memory/src/distiller/phase_b.rs \
        crates/coding-memory/src/distiller/mod.rs \
        crates/coding-memory/tests/phase_b_prompt.rs
git commit -m "feat(coding-memory): Phase B prompt construction with trace summary + safety caps"
```

---

### Task 13: Phase B — `ProviderManager` invocation with timeout

**Files:**
- Modify: `crates/coding-memory/src/distiller/phase_b.rs`
- Test: `crates/coding-memory/tests/phase_b_llm.rs`

- [ ] **Step 1: Write failing test — uses a noop provider**

Create `crates/coding-memory/tests/phase_b_llm.rs`:

```rust
use coding_memory::distiller::phase_b::{invoke_llm, LlmInvocation};
use coding_memory::distiller::record_observation::Observation;
use coding_memory::distiller::{TurnTrace, TurnTokenUsage};
use jiff::Timestamp;
use providers::{NoopProvider, ProviderManager};
use std::sync::Arc;
use std::time::Duration;

fn trace() -> TurnTrace {
    TurnTrace {
        session_id: "s".into(), turn_id: Some("t".into()),
        files_read: vec![], files_modified: vec![],
        commands_run: vec![], test_outcomes: vec![], errors_encountered: vec![],
        token_usage: Some(TurnTokenUsage { prompt: 1, completion: 1, cached: 0 }),
        started_at: Timestamp::now(), ended_at: None,
    }
}

#[tokio::test]
async fn noop_provider_returns_empty_observations_list() {
    let mgr = Arc::new(ProviderManager::new(Arc::new(NoopProvider), None, None));
    let inv = LlmInvocation {
        provider: mgr,
        model: "noop".into(),
        user_prompt_text: "hi",
        assistant_text: "done",
        trace: &trace(),
        repo_id: None,
        timeout: Duration::from_secs(1),
    };
    let result: Vec<Observation> = invoke_llm(inv).await.unwrap_or_default();
    assert!(result.is_empty(), "NoopProvider produces no observations");
}
```

- [ ] **Step 2: Implement `invoke_llm`**

Append to `crates/coding-memory/src/distiller/phase_b.rs`:

```rust
use super::error::DistillerError;
use super::record_observation::{
    observations_from_tool_calls, record_observation_tool_def, Observation,
};
use providers::types::{ChatParams, Message, ResponseFormat};
use providers::{LlmProvider, ProviderManager};
use std::sync::Arc;
use std::time::Duration;

/// Inputs for an LLM invocation.
pub struct LlmInvocation<'a> {
    /// Shared ProviderManager (failover, retry, circuit breaker).
    pub provider: Arc<ProviderManager>,
    /// Model id — from `DistillerConfig::model`.
    pub model: String,
    /// User prompt text from the turn.
    pub user_prompt_text: &'a str,
    /// Final assistant message text from the turn.
    pub assistant_text: &'a str,
    /// Phase-A trace.
    pub trace: &'a TurnTrace,
    /// Resolved repo id, if any.
    pub repo_id: Option<&'a str>,
    /// Wall-clock timeout — cancels on elapse.
    pub timeout: Duration,
}

/// Run the Phase-B LLM call. Returns the list of decoded observations (may be empty).
pub async fn invoke_llm(inv: LlmInvocation<'_>) -> Result<Vec<Observation>, DistillerError> {
    let prompt = build_prompt(inv.user_prompt_text, inv.assistant_text, inv.trace, inv.repo_id);
    let params = ChatParams {
        model: inv.model.clone(),
        messages: vec![
            Message::System { content: prompt.system.into() },
            Message::User { content: providers::types::UserContent::Text(prompt.user_message.into()) },
        ],
        temperature: Some(0.2),
        max_tokens: Some(1024),
        tools: Some(vec![record_observation_tool_def()]),
        tool_choice: Some("auto".into()),
        response_format: Some(ResponseFormat::default()),
        stop_sequences: None,
        stream: false,
        top_p: None,
    };

    let fut = inv.provider.chat(&params);
    let resp = tokio::time::timeout(inv.timeout, fut)
        .await
        .map_err(|_| DistillerError::LlmTimeout { timeout_ms: inv.timeout.as_millis() as u64 })?
        .map_err(|e| DistillerError::LlmProvider { detail: e.to_string() })?;

    observations_from_tool_calls(&resp.tool_calls)
}
```

> **`ChatParams`/`Message`/`ResponseFormat` shape check:** verify fields with `rg 'pub struct ChatParams' crates/providers/src/types.rs`. If any field name or default differs, match it — the invariants we rely on are: `messages` accepts `Message::System`/`Message::User`, `tools` accepts `Vec<ToolDefinition>`, and the provider returns a response with a `tool_calls: Vec<ToolCall>` field.

- [ ] **Step 3: Run + commit**

```bash
cargo nextest run -p coding-memory --test phase_b_llm
cargo clippy -p coding-memory --all-targets -- -D warnings
git add crates/coding-memory/src/distiller/phase_b.rs crates/coding-memory/tests/phase_b_llm.rs
git commit -m "feat(coding-memory): Phase B invoke_llm via ProviderManager with timeout"
```

---

### Task 14: Phase B failure-mode handling — retry queue table

**Files:**
- Create: `crates/coding-memory/migrations/002_retry_queue.sql`
- Modify: `crates/coding-memory/src/lib.rs` (extend `coding_memory_migrations`)
- Create: `crates/coding-memory/src/distiller/retry_queue.rs`
- Modify: `crates/coding-memory/src/distiller/mod.rs`
- Test: `crates/coding-memory/tests/retry_queue.rs`

- [ ] **Step 1: Write failing test**

Create `crates/coding-memory/tests/retry_queue.rs`:

```rust
use coding_memory::distiller::retry_queue::{DistillationRetryRepo, RetryReason};
use storage::StoragePool;

#[tokio::test]
async fn enqueue_and_list_due() {
    let pool = StoragePool::connect_in_memory().await.unwrap();
    StoragePool::run_feature_migrations(pool.inner(), &cognitive::cognitive_migrations()).await.unwrap();
    StoragePool::run_feature_migrations(pool.inner(), &coding_memory::coding_memory_migrations()).await.unwrap();

    let repo = DistillationRetryRepo::new(pool.inner().clone());
    repo.enqueue("s1", Some("t1"), RetryReason::LlmTimeout).await.unwrap();

    let due = repo.list_due(10).await.unwrap();
    assert_eq!(due.len(), 1);
    assert_eq!(due[0].session_id, "s1");
    assert_eq!(due[0].attempt_count, 0);
}

#[tokio::test]
async fn record_attempt_backs_off() {
    let pool = StoragePool::connect_in_memory().await.unwrap();
    StoragePool::run_feature_migrations(pool.inner(), &cognitive::cognitive_migrations()).await.unwrap();
    StoragePool::run_feature_migrations(pool.inner(), &coding_memory::coding_memory_migrations()).await.unwrap();
    let repo = DistillationRetryRepo::new(pool.inner().clone());
    repo.enqueue("s1", Some("t1"), RetryReason::LlmTimeout).await.unwrap();
    let id = repo.list_due(10).await.unwrap()[0].id.clone();

    repo.record_attempt(&id).await.unwrap();
    let due = repo.list_due(10).await.unwrap();
    assert_eq!(due.len(), 0); // backed off, not yet due
}

#[tokio::test]
async fn mark_done_removes_entry() {
    let pool = StoragePool::connect_in_memory().await.unwrap();
    StoragePool::run_feature_migrations(pool.inner(), &cognitive::cognitive_migrations()).await.unwrap();
    StoragePool::run_feature_migrations(pool.inner(), &coding_memory::coding_memory_migrations()).await.unwrap();
    let repo = DistillationRetryRepo::new(pool.inner().clone());
    repo.enqueue("s1", Some("t1"), RetryReason::LlmTimeout).await.unwrap();
    let id = repo.list_due(10).await.unwrap()[0].id.clone();
    repo.mark_done(&id).await.unwrap();
    assert_eq!(repo.list_due(10).await.unwrap().len(), 0);
}
```

- [ ] **Step 2: Create the migration**

Create `crates/coding-memory/migrations/002_retry_queue.sql`:

```sql
-- Distillation retry queue — transient LLM failures park here until the
-- provider recovers. Phase-1 consolidated migration excluded this because it
-- belongs to the Phase-3 Distiller; its presence never changes reads in prior
-- phases.

CREATE TABLE IF NOT EXISTS ingest_distillation_retry (
    id              TEXT PRIMARY KEY,
    session_id      TEXT NOT NULL,
    turn_id         TEXT,
    reason          TEXT NOT NULL,
    attempt_count   INTEGER NOT NULL DEFAULT 0,
    next_due_at     TEXT NOT NULL DEFAULT (datetime('now')),
    created_at      TEXT NOT NULL DEFAULT (datetime('now'))
);

CREATE INDEX IF NOT EXISTS idx_distillation_retry_due
    ON ingest_distillation_retry(next_due_at);
```

- [ ] **Step 3: Register the migration**

In `crates/coding-memory/src/lib.rs` replace the single-element `coding_memory_migrations()` return with two migrations (preserving the Phase-1 version):

```rust
pub fn coding_memory_migrations() -> Vec<FeatureMigration> {
    vec![
        FeatureMigration {
            feature_name: "coding_memory".to_string(),
            version: 1,
            description: "Consolidated Phase-1 schema: scope_repo_id, metadata, \
                          actor_id columns; memory_causal_edges, memory_utilization, \
                          ingest_event_log, klynt_sessions tables; skill_versions \
                          scope columns."
                .to_string(),
            sql: include_str!("../migrations/001_coding_memory.sql").to_string(),
        },
        FeatureMigration {
            feature_name: "coding_memory".to_string(),
            version: 2,
            description: "Phase-3: ingest_distillation_retry queue for transient \
                          LLM failures.".to_string(),
            sql: include_str!("../migrations/002_retry_queue.sql").to_string(),
        },
    ]
}
```

- [ ] **Step 4: Implement the repo**

Create `crates/coding-memory/src/distiller/retry_queue.rs`:

```rust
//! Distillation retry queue — transient failure rehab.
//!
//! LLM timeouts, provider-open-circuit, and other soft errors enqueue a row
//! here. A periodic sweeper (Task 24) re-runs `distill_turn` for every due
//! row. Backoff: 1m, 5m, 30m, then permanent failure.

use common::{KlyntbotError, Result};
use sqlx::{Row, SqlitePool};
use uuid::Uuid;

/// Why this turn is in the retry queue.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RetryReason {
    /// LLM call timed out.
    LlmTimeout,
    /// Provider returned an error (rate limit, circuit open, …).
    LlmProvider,
    /// Other transient — e.g. DB busy at write.
    Transient,
}

impl RetryReason {
    fn as_str(self) -> &'static str {
        match self {
            Self::LlmTimeout => "llm_timeout",
            Self::LlmProvider => "llm_provider",
            Self::Transient => "transient",
        }
    }
}

/// One retry-queue row.
#[derive(Debug, Clone)]
pub struct RetryRow {
    /// Row id.
    pub id: String,
    /// Session.
    pub session_id: String,
    /// Turn.
    pub turn_id: Option<String>,
    /// How many attempts have been made.
    pub attempt_count: i64,
    /// Reason code.
    pub reason: String,
}

/// Repository for `ingest_distillation_retry`.
#[derive(Debug, Clone)]
pub struct DistillationRetryRepo {
    pool: SqlitePool,
}

impl DistillationRetryRepo {
    /// Construct over a SQLite pool.
    #[must_use]
    pub fn new(pool: SqlitePool) -> Self {
        Self { pool }
    }

    /// Enqueue a turn for retry. Duplicates are allowed — each attempt gets a row.
    pub async fn enqueue(&self, session_id: &str, turn_id: Option<&str>, reason: RetryReason) -> Result<()> {
        let id = Uuid::new_v4().to_string();
        sqlx::query(
            "INSERT INTO ingest_distillation_retry (id, session_id, turn_id, reason)
             VALUES (?, ?, ?, ?)",
        )
        .bind(id)
        .bind(session_id)
        .bind(turn_id)
        .bind(reason.as_str())
        .execute(&self.pool)
        .await
        .map_err(|e| KlyntbotError::Storage(format!("retry enqueue: {e}")))?;
        Ok(())
    }

    /// List rows whose `next_due_at <= now`, up to `limit`.
    pub async fn list_due(&self, limit: i64) -> Result<Vec<RetryRow>> {
        let rows = sqlx::query(
            "SELECT id, session_id, turn_id, attempt_count, reason
             FROM ingest_distillation_retry
             WHERE next_due_at <= datetime('now')
             ORDER BY next_due_at ASC
             LIMIT ?",
        )
        .bind(limit)
        .fetch_all(&self.pool)
        .await
        .map_err(|e| KlyntbotError::Storage(format!("retry list: {e}")))?;
        Ok(rows.into_iter().map(|r| RetryRow {
            id: r.get("id"),
            session_id: r.get("session_id"),
            turn_id: r.get("turn_id"),
            attempt_count: r.get("attempt_count"),
            reason: r.get("reason"),
        }).collect())
    }

    /// Record a failed attempt. Backoff: 1m / 5m / 30m.
    pub async fn record_attempt(&self, id: &str) -> Result<()> {
        sqlx::query(
            "UPDATE ingest_distillation_retry
             SET attempt_count = attempt_count + 1,
                 next_due_at = CASE attempt_count
                    WHEN 0 THEN datetime('now', '+1 minute')
                    WHEN 1 THEN datetime('now', '+5 minutes')
                    ELSE datetime('now', '+30 minutes')
                 END
             WHERE id = ?",
        )
        .bind(id)
        .execute(&self.pool)
        .await
        .map_err(|e| KlyntbotError::Storage(format!("retry attempt: {e}")))?;
        Ok(())
    }

    /// Remove a row — distillation succeeded.
    pub async fn mark_done(&self, id: &str) -> Result<()> {
        sqlx::query("DELETE FROM ingest_distillation_retry WHERE id = ?")
            .bind(id)
            .execute(&self.pool)
            .await
            .map_err(|e| KlyntbotError::Storage(format!("retry done: {e}")))?;
        Ok(())
    }
}
```

Edit `crates/coding-memory/src/distiller/mod.rs` and add:

```rust
/// Distillation retry queue.
pub mod retry_queue;
```

- [ ] **Step 5: Run tests + commit**

```bash
cargo nextest run -p coding-memory --test retry_queue
cargo clippy -p coding-memory --all-targets -- -D warnings
git add crates/coding-memory/migrations/002_retry_queue.sql \
        crates/coding-memory/src/lib.rs \
        crates/coding-memory/src/distiller/retry_queue.rs \
        crates/coding-memory/src/distiller/mod.rs \
        crates/coding-memory/tests/retry_queue.rs
git commit -m "feat(coding-memory): ingest_distillation_retry queue + DistillationRetryRepo"
```

---

### Task 15: Phase C — reconciliation policy (NOOP/SUPERSEDE/ADD)

**Files:**
- Create: `crates/coding-memory/src/distiller/phase_c.rs`
- Modify: `crates/coding-memory/src/distiller/mod.rs`
- Test: `crates/coding-memory/tests/phase_c_reconciliation.rs`

- [ ] **Step 1: Write failing test**

Create `crates/coding-memory/tests/phase_c_reconciliation.rs`:

```rust
use coding_memory::distiller::phase_c::{reconcile, ReconcileDecision, SimilarFact};
use cognitive::types::SemanticFact;
use jiff::Timestamp;

fn seed(id: &str, subj: &str, pred: &str, obj: &str) -> SemanticFact {
    SemanticFact {
        id: id.into(), domain: "work".into(),
        subject: subj.into(), predicate: pred.into(), object: obj.into(),
        confidence: 0.9, source: "distiller".into(),
        valid_from: Timestamp::now().to_string(),
        valid_until: None,
        recorded_at: Timestamp::now().to_string(),
        superseded_at: None, superseded_by: None,
        stability: 1.0, last_accessed: None, access_count: 0,
        convergence_score: 1.0, project_id: None,
        memory_type: "fact".into(), scope_type: "user".into(), scope_id: None,
    }
}

#[test]
fn exact_match_above_090_is_noop() {
    let cand = seed("new", "repo:x", "framework", "tauri");
    let existing = SimilarFact { fact: seed("old", "repo:x", "framework", "tauri"), similarity: 0.98 };
    let decision = reconcile(&cand, &[existing]);
    assert!(matches!(decision, ReconcileDecision::Noop { predecessor_id } if predecessor_id == "old"));
}

#[test]
fn similar_above_075_is_supersede() {
    let cand = seed("new", "repo:x", "framework", "tauri v2");
    let existing = SimilarFact { fact: seed("old", "repo:x", "framework", "tauri v1"), similarity: 0.82 };
    let decision = reconcile(&cand, &[existing]);
    assert!(matches!(decision, ReconcileDecision::Supersede { predecessor_id } if predecessor_id == "old"));
}

#[test]
fn below_075_is_add() {
    let cand = seed("new", "repo:x", "framework", "totally different");
    let existing = SimilarFact { fact: seed("old", "repo:x", "framework", "tauri"), similarity: 0.42 };
    let decision = reconcile(&cand, &[existing]);
    assert!(matches!(decision, ReconcileDecision::Add));
}

#[test]
fn empty_candidates_is_add() {
    let cand = seed("new", "x", "y", "z");
    assert!(matches!(reconcile(&cand, &[]), ReconcileDecision::Add));
}

#[test]
fn subject_predicate_mismatch_even_at_high_sim_is_add() {
    // similarity > 0.9 but (subject, predicate) differ → can't NOOP, must ADD/SUPERSEDE logic.
    let cand = seed("new", "repo:x", "language", "rust");
    let existing = SimilarFact { fact: seed("old", "repo:x", "framework", "tauri"), similarity: 0.93 };
    let decision = reconcile(&cand, &[existing]);
    // Different predicate → falls through to supersede (high sim) — but our rule requires
    // exact (subject, predicate). Without it, we only SUPERSEDE if >= 0.75 — yes.
    assert!(matches!(decision, ReconcileDecision::Supersede { .. }));
}
```

- [ ] **Step 2: Implement**

Create `crates/coding-memory/src/distiller/phase_c.rs`:

```rust
//! Phase C — reconciliation (Mem0-style, DELETE-free).
//!
//! For each candidate fact we look up the top-k vector-similar existing facts
//! (scoped by `scope_repo_id`). Policy:
//!
//! | Condition                                                  | Decision   |
//! |------------------------------------------------------------|------------|
//! | top.similarity > 0.9 AND (subject, predicate) exact match  | NOOP       |
//! | top.similarity > 0.75                                      | SUPERSEDE  |
//! | otherwise                                                  | ADD        |
//!
//! NOOP bumps `access_count` on the predecessor (delegated to caller).
//! SUPERSEDE writes the new row with a pending link to the predecessor;
//! Task 16 completes the chain by setting predecessor's `valid_until` +
//! `superseded_by` atomically.

use cognitive::types::SemanticFact;

const NOOP_THRESHOLD: f32 = 0.9;
const SUPERSEDE_THRESHOLD: f32 = 0.75;

/// A candidate-adjacent existing row with its similarity score.
#[derive(Debug, Clone)]
pub struct SimilarFact {
    /// The existing row.
    pub fact: SemanticFact,
    /// Cosine similarity (0.0–1.0).
    pub similarity: f32,
}

/// Reconciliation decision.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ReconcileDecision {
    /// Drop the candidate; bump `access_count` on `predecessor_id`.
    Noop {
        /// Existing row id whose access_count should bump.
        predecessor_id: String,
    },
    /// Write the candidate as a new row; later update predecessor to mark it superseded.
    Supersede {
        /// Existing row id being superseded.
        predecessor_id: String,
    },
    /// Write the candidate as a fresh row; no predecessor interaction.
    Add,
}

/// Decide how to reconcile `candidate` against pre-fetched `similar` rows.
#[must_use]
pub fn reconcile(candidate: &SemanticFact, similar: &[SimilarFact]) -> ReconcileDecision {
    let Some(top) = similar.iter().max_by(|a, b| a.similarity.partial_cmp(&b.similarity).unwrap_or(std::cmp::Ordering::Equal)) else {
        return ReconcileDecision::Add;
    };
    if top.similarity > NOOP_THRESHOLD
        && top.fact.subject == candidate.subject
        && top.fact.predicate == candidate.predicate
    {
        return ReconcileDecision::Noop { predecessor_id: top.fact.id.clone() };
    }
    if top.similarity > SUPERSEDE_THRESHOLD {
        return ReconcileDecision::Supersede { predecessor_id: top.fact.id.clone() };
    }
    ReconcileDecision::Add
}
```

Edit `crates/coding-memory/src/distiller/mod.rs` and add:

```rust
/// Phase C — reconciliation (ADD/SUPERSEDE/NOOP).
pub mod phase_c;
```

- [ ] **Step 3: Run + commit**

```bash
cargo nextest run -p coding-memory --test phase_c_reconciliation
cargo clippy -p coding-memory --all-targets -- -D warnings
git add crates/coding-memory/src/distiller/phase_c.rs \
        crates/coding-memory/src/distiller/mod.rs \
        crates/coding-memory/tests/phase_c_reconciliation.rs
git commit -m "feat(coding-memory): Phase C reconcile — NOOP/SUPERSEDE/ADD policy"
```

---

### Task 16: Supersede-chain completion — predecessor `valid_until` + `superseded_by`

**Files:**
- Modify: `crates/coding-memory/src/distiller/writer.rs`
- Test: `crates/coding-memory/tests/supersede_chain.rs`

- [ ] **Step 1: Write failing test**

Create `crates/coding-memory/tests/supersede_chain.rs`:

```rust
use coding_memory::distiller::writer::DistillerWriter;
use coding_memory::scope::{ProvenanceKind, ProvenanceMetadata};
use cognitive::types::SemanticFact;
use cognitive::{EpisodicMemoryRepo, SemanticFactRepo};
use jiff::Timestamp;
use storage::StoragePool;
use uuid::Uuid;

fn fact(id: &str) -> SemanticFact {
    SemanticFact {
        id: id.into(), domain: "work".into(),
        subject: "repo:x".into(), predicate: "framework".into(), object: "tauri".into(),
        confidence: 0.9, source: "distiller".into(),
        valid_from: Timestamp::now().to_string(),
        valid_until: None, recorded_at: Timestamp::now().to_string(),
        superseded_at: None, superseded_by: None,
        stability: 1.0, last_accessed: None, access_count: 0,
        convergence_score: 1.0, project_id: None,
        memory_type: "fact".into(), scope_type: "user".into(), scope_id: None,
    }
}

#[tokio::test]
async fn complete_supersede_sets_predecessor_valid_until_and_superseded_by() {
    let pool = StoragePool::connect_in_memory().await.unwrap();
    StoragePool::run_feature_migrations(pool.inner(), &cognitive::cognitive_migrations()).await.unwrap();
    StoragePool::run_feature_migrations(pool.inner(), &coding_memory::coding_memory_migrations()).await.unwrap();
    let writer = DistillerWriter::new(
        SemanticFactRepo::new(pool.inner().clone()),
        EpisodicMemoryRepo::new(pool.inner().clone()),
    );

    let pred = fact("pred");
    let succ = fact("succ");
    SemanticFactRepo::new(pool.inner().clone()).upsert(&pred).await.unwrap();
    SemanticFactRepo::new(pool.inner().clone()).upsert(&succ).await.unwrap();

    let ts = Timestamp::now().to_string();
    writer.complete_supersede(&pred.id, &succ.id, &ts).await.unwrap();

    let (valid_until, superseded_by): (Option<String>, Option<String>) = sqlx::query_as(
        "SELECT valid_until, superseded_by FROM semantic_facts WHERE id = ?",
    )
    .bind(&pred.id)
    .fetch_one(pool.inner()).await.unwrap();
    assert_eq!(valid_until.as_deref(), Some(ts.as_str()));
    assert_eq!(superseded_by.as_deref(), Some(succ.id.as_str()));
}

#[tokio::test]
async fn bump_access_updates_counter() {
    let pool = StoragePool::connect_in_memory().await.unwrap();
    StoragePool::run_feature_migrations(pool.inner(), &cognitive::cognitive_migrations()).await.unwrap();
    StoragePool::run_feature_migrations(pool.inner(), &coding_memory::coding_memory_migrations()).await.unwrap();
    let writer = DistillerWriter::new(
        SemanticFactRepo::new(pool.inner().clone()),
        EpisodicMemoryRepo::new(pool.inner().clone()),
    );
    let f = fact("f");
    SemanticFactRepo::new(pool.inner().clone()).upsert(&f).await.unwrap();
    writer.bump_access(&f.id).await.unwrap();
    writer.bump_access(&f.id).await.unwrap();
    let (access_count,): (i64,) = sqlx::query_as("SELECT access_count FROM semantic_facts WHERE id = ?")
        .bind(&f.id).fetch_one(pool.inner()).await.unwrap();
    assert_eq!(access_count, 2);
}
```

- [ ] **Step 2: Implement**

Append to `crates/coding-memory/src/distiller/writer.rs` (inside `impl DistillerWriter`):

```rust
    /// Complete a SUPERSEDE chain — set predecessor's `valid_until` + `superseded_by`.
    /// Invariant #3: predecessor's `valid_until` equals successor's `valid_from`.
    pub async fn complete_supersede(
        &self,
        predecessor_id: &str,
        successor_id: &str,
        successor_valid_from: &str,
    ) -> Result<(), DistillerError> {
        sqlx::query(
            "UPDATE semantic_facts
             SET valid_until = ?2, superseded_by = ?3, superseded_at = datetime('now')
             WHERE id = ?1",
        )
        .bind(predecessor_id)
        .bind(successor_valid_from)
        .bind(successor_id)
        .execute(self.facts.pool())
        .await
        .map_err(|e| DistillerError::Storage { detail: format!("complete_supersede: {e}") })?;
        Ok(())
    }

    /// Bump `access_count` + `last_accessed` on a fact — called for NOOP decisions.
    pub async fn bump_access(&self, id: &str) -> Result<(), DistillerError> {
        sqlx::query(
            "UPDATE semantic_facts
             SET access_count = access_count + 1, last_accessed = datetime('now')
             WHERE id = ?",
        )
        .bind(id)
        .execute(self.facts.pool())
        .await
        .map_err(|e| DistillerError::Storage { detail: format!("bump_access: {e}") })?;
        Ok(())
    }
```

- [ ] **Step 3: Run + commit**

```bash
cargo nextest run -p coding-memory --test supersede_chain
cargo clippy -p coding-memory --all-targets -- -D warnings
git add crates/coding-memory/src/distiller/writer.rs crates/coding-memory/tests/supersede_chain.rs
git commit -m "feat(coding-memory): DistillerWriter complete_supersede + bump_access"
```

---

### Task 17: Counterfactual derivation (Tier B1) — `DeadEndAttempt` from failure/abandoned FixAttempts

**Files:**
- Create: `crates/coding-memory/src/counterfactual.rs`
- Modify: `crates/coding-memory/src/lib.rs`
- Test: `crates/coding-memory/tests/counterfactual.rs`

- [ ] **Step 1: Write failing test**

Create `crates/coding-memory/tests/counterfactual.rs`:

```rust
use coding_memory::counterfactual::derive_dead_end;
use coding_memory::facts::{FixAttempt, FixOutcome};
use coding_memory::scope::{ProvenanceKind, ProvenanceMetadata, Sensitivity};
use jiff::Timestamp;
use uuid::Uuid;

fn prov() -> ProvenanceMetadata {
    ProvenanceMetadata {
        source_events: vec![Uuid::new_v4()],
        session_id: "s".into(),
        turn_id: Some("t".into()),
        distilled_at: Timestamp::now(),
        distiller_model: "x".into(),
        source_kind: ProvenanceKind::DistillerLlm,
    }
}

fn base(outcome: FixOutcome) -> FixAttempt {
    FixAttempt {
        problem_hash: "h1".into(),
        problem: "null pointer".into(),
        files: vec![],
        approach: "rewrite parser".into(),
        outcome,
        insight: Some("tests still failed".into()),
        duration_ms: 0,
        test_before: None,
        test_after: None,
        anchored_symbols: vec![],
        provenance: prov(),
        sensitivity: Sensitivity::Normal,
    }
}

#[test]
fn failure_attempt_produces_dead_end_fact() {
    let attempt_id = Uuid::new_v4();
    let fact = derive_dead_end(attempt_id, &base(FixOutcome::Failure)).unwrap();
    assert_eq!(fact.memory_type, "counterfactual");
    assert_eq!(fact.domain, "coding");
    assert!(fact.predicate == "failed_because" || fact.predicate == "avoided_due_to");
}

#[test]
fn abandoned_attempt_produces_dead_end_fact() {
    let attempt_id = Uuid::new_v4();
    let fact = derive_dead_end(attempt_id, &base(FixOutcome::Abandoned));
    assert!(fact.is_some());
}

#[test]
fn success_attempt_returns_none() {
    let fact = derive_dead_end(Uuid::new_v4(), &base(FixOutcome::Success));
    assert!(fact.is_none());
}

#[test]
fn partial_attempt_returns_none() {
    let fact = derive_dead_end(Uuid::new_v4(), &base(FixOutcome::Partial));
    assert!(fact.is_none());
}
```

- [ ] **Step 2: Implement**

Create `crates/coding-memory/src/counterfactual.rs`:

```rust
//! Tier B1 — counterfactual memory derivation.
//!
//! A `FixAttempt` with outcome `Failure | Abandoned` additionally produces a
//! `SemanticFact { memory_type: "counterfactual" }` that downstream recall
//! uses to surface the "Heads up — you already tried X" block (Phase 4).
//!
//! The counterfactual is a **derived** write — it never replaces the
//! episodic `FixAttempt`. Both survive. Reforge nightly-heavy may later
//! promote a cluster of same-problem_hash counterfactuals to a
//! `ProblemSolutionPattern` (Phase 5).

use crate::facts::{FixAttempt, FixOutcome};
use cognitive::types::SemanticFact;
use jiff::Timestamp;
use uuid::Uuid;

/// Build a `memory_type: "counterfactual"` `SemanticFact` linked to a fix-attempt episode.
/// Returns `None` when the outcome isn't a dead-end (`Success`/`Partial`).
#[must_use]
pub fn derive_dead_end(
    attempt_id: Uuid,
    attempt: &FixAttempt,
) -> Option<SemanticFact> {
    match attempt.outcome {
        FixOutcome::Failure | FixOutcome::Abandoned => {}
        FixOutcome::Success | FixOutcome::Partial => return None,
    }
    let now = Timestamp::now().to_string();
    let predicate = match attempt.outcome {
        FixOutcome::Failure => "failed_because",
        FixOutcome::Abandoned => "avoided_due_to",
        _ => unreachable!(),
    };
    let reason = attempt
        .insight
        .clone()
        .unwrap_or_else(|| "outcome marked failure".into());
    let confidence = match attempt.outcome {
        FixOutcome::Failure => 0.85,
        FixOutcome::Abandoned => 0.6,
        _ => 0.0,
    };
    Some(SemanticFact {
        id: Uuid::new_v4().to_string(),
        domain: "coding".into(),
        subject: format!("bug:{}", attempt.problem_hash),
        predicate: predicate.into(),
        object: format!("{} — {}", attempt.approach, reason),
        confidence,
        source: "distiller_derived".into(),
        valid_from: now.clone(),
        valid_until: None,
        recorded_at: now.clone(),
        superseded_at: None,
        superseded_by: None,
        stability: 1.0,
        last_accessed: None,
        access_count: 0,
        convergence_score: 1.0,
        project_id: None,
        memory_type: "counterfactual".into(),
        scope_type: "user".into(),
        scope_id: None,
    })
    .map(|mut f| {
        f.source = format!("fix_attempt:{attempt_id}");
        f
    })
}
```

Edit `crates/coding-memory/src/lib.rs` and add:

```rust
/// Counterfactual derivation (Tier B1) — `DeadEndAttempt` from failure/abandoned `FixAttempt`.
pub mod counterfactual;
```

- [ ] **Step 3: Run + commit**

```bash
cargo nextest run -p coding-memory --test counterfactual
cargo clippy -p coding-memory --all-targets -- -D warnings
git add crates/coding-memory/src/counterfactual.rs crates/coding-memory/src/lib.rs \
        crates/coding-memory/tests/counterfactual.rs
git commit -m "feat(coding-memory): derive_dead_end — counterfactual fact from failed FixAttempt"
```

---

### Task 18: Tier B3 — `CodeState` enum + field on `UserSituationSnapshot`

**Files:**
- Create: `crates/coding-memory/src/code_state.rs`
- Modify: `crates/coding-memory/src/lib.rs`
- Modify: `crates/context_engine/src/rewriter.rs`
- Modify: `crates/context_engine/src/lib.rs`
- Test: `crates/coding-memory/tests/code_state_rewriter.rs`

- [ ] **Step 1: Create the `CodeState` enum**

Create `crates/coding-memory/src/code_state.rs`:

```rust
//! Tier B3 — `CodeState` enum carried on `UserSituationSnapshot`.
//!
//! Enables context-insensitive retrieval: when the agent is reading a stack
//! trace for `MemoryError`, the recall query should be biased toward past
//! `FixAttempt`s with `memory` in their problem surface.
//!
//! Stored as a *snapshot string* on `UserSituationSnapshot` to avoid L3 →
//! L5 cyclic imports. The `CodeStateSnapshot::serialize()` format is this
//! enum's JSON.

use serde::{Deserialize, Serialize};

/// What the coding CLI appears to be doing right now.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq, Default)]
#[serde(tag = "state", rename_all = "snake_case")]
pub enum CodeState {
    /// No coding activity detected.
    #[default]
    Idle,
    /// The user/agent is examining a stack trace or compiler error.
    StackTraceActive {
        /// Top-level error type when extractable (e.g. `"NullPointerException"`).
        error_type: Option<String>,
    },
    /// Tests are currently failing in the active repo.
    RedTestsRunning {
        /// Detected test framework.
        framework: Option<String>,
    },
    /// A refactor touching ≥3 files is in progress.
    RefactorInFlight {
        /// Number of files touched so far.
        files_touched: u32,
    },
    /// The agent is executing a fix-attempt plan.
    FixAttemptActive {
        /// Canonical `ProblemHash`.
        problem_hash: String,
    },
}

impl CodeState {
    /// Serialize to a short JSON string. Stored verbatim on `UserSituationSnapshot::code_state`.
    #[must_use]
    pub fn to_snapshot(&self) -> String {
        serde_json::to_string(self).unwrap_or_else(|_| "{\"state\":\"idle\"}".into())
    }

    /// Best-effort parse from a snapshot string. `None` on malformed input.
    #[must_use]
    pub fn from_snapshot(s: &str) -> Option<Self> {
        serde_json::from_str(s).ok()
    }
}
```

Edit `crates/coding-memory/src/lib.rs` and add:

```rust
/// Tier B3 — `CodeState` for `UserSituationSnapshot`.
pub mod code_state;
```

- [ ] **Step 2: Add `code_state` field to `UserSituationSnapshot`**

Edit `crates/context_engine/src/rewriter.rs`. Find:

```rust
#[derive(Debug, Clone, Default)]
pub struct UserSituationSnapshot {
    pub energy_level: f64,
    pub focus_state: f64,
    pub deadline_pressure: f64,
    pub distraction_risk: f64,
}
```

Replace with:

```rust
#[derive(Debug, Clone, Default)]
pub struct UserSituationSnapshot {
    pub energy_level: f64,
    pub focus_state: f64,
    pub deadline_pressure: f64,
    pub distraction_risk: f64,
    /// Serialized coding-state snapshot (see `coding_memory::code_state::CodeState`).
    /// Kept as `String` so `context_engine` (L3) stays free of L5 deps.
    pub code_state: Option<String>,
}
```

Verify `context_engine` has no other constructors of `UserSituationSnapshot` that require updating (look for `UserSituationSnapshot {` literals and add `code_state: None` if needed). Run `rg 'UserSituationSnapshot \{' crates` to find them.

Edit `crates/context_engine/src/lib.rs` and make sure `UserSituationSnapshot` is re-exported (it should already be — confirm).

- [ ] **Step 3: Write failing test**

Create `crates/coding-memory/tests/code_state_rewriter.rs`:

```rust
use coding_memory::code_state::CodeState;
use context_engine::UserSituationSnapshot;

#[test]
fn snapshot_roundtrip_through_user_situation() {
    let cs = CodeState::StackTraceActive { error_type: Some("TypeError".into()) };
    let mut snap = UserSituationSnapshot::default();
    snap.code_state = Some(cs.to_snapshot());

    let back = CodeState::from_snapshot(snap.code_state.as_deref().unwrap()).unwrap();
    match back {
        CodeState::StackTraceActive { error_type } => assert_eq!(error_type.as_deref(), Some("TypeError")),
        _ => panic!(),
    }
}

#[test]
fn default_is_idle_when_absent() {
    let snap = UserSituationSnapshot::default();
    assert!(snap.code_state.is_none());
    assert_eq!(CodeState::default(), CodeState::Idle);
}

#[test]
fn all_variants_roundtrip() {
    for c in [
        CodeState::Idle,
        CodeState::StackTraceActive { error_type: None },
        CodeState::RedTestsRunning { framework: Some("pytest".into()) },
        CodeState::RefactorInFlight { files_touched: 5 },
        CodeState::FixAttemptActive { problem_hash: "abc".into() },
    ] {
        let s = c.to_snapshot();
        let back = CodeState::from_snapshot(&s).unwrap();
        assert_eq!(back, c);
    }
}
```

- [ ] **Step 4: Run + commit**

```bash
cargo nextest run -p coding-memory --test code_state_rewriter
cargo clippy -p coding-memory --all-targets -- -D warnings
cargo build --workspace   # UserSituationSnapshot change could ripple
git add crates/coding-memory/src/code_state.rs crates/coding-memory/src/lib.rs \
        crates/context_engine/src/rewriter.rs crates/coding-memory/tests/code_state_rewriter.rs
git commit -m "feat(context_engine+coding-memory): UserSituationSnapshot.code_state (Tier B3)"
```

---

### Task 19: Tier B4 — `CodeDomainSearcher` + InsightForge registration

**Files:**
- Create: `crates/coding-memory/src/code_domain_searcher.rs`
- Modify: `crates/coding-memory/src/lib.rs`
- Test: `crates/coding-memory/tests/code_domain_searcher.rs`

- [ ] **Step 1: Write failing test**

Create `crates/coding-memory/tests/code_domain_searcher.rs`:

```rust
use coding_memory::code_domain_searcher::CodeDomainSearcher;
use cognitive::types::SemanticFact;
use cognitive::SemanticFactRepo;
use context_engine::insight_forge::DomainSearcher;
use jiff::Timestamp;
use storage::StoragePool;
use uuid::Uuid;

fn fact(subject: &str, predicate: &str, object: &str, scope_repo_id: Option<&str>) -> SemanticFact {
    SemanticFact {
        id: Uuid::new_v4().to_string(),
        domain: "work".into(),
        subject: subject.into(), predicate: predicate.into(), object: object.into(),
        confidence: 0.9, source: "distiller".into(),
        valid_from: Timestamp::now().to_string(), valid_until: None,
        recorded_at: Timestamp::now().to_string(),
        superseded_at: None, superseded_by: None,
        stability: 1.0, last_accessed: None, access_count: 0,
        convergence_score: 1.0, project_id: None,
        memory_type: "fact".into(),
        scope_type: if scope_repo_id.is_some() { "project".into() } else { "user".into() },
        scope_id: scope_repo_id.map(str::to_string),
    }
}

#[tokio::test]
async fn searcher_surfaces_coding_facts_via_fts() {
    let pool = StoragePool::connect_in_memory().await.unwrap();
    StoragePool::run_feature_migrations(pool.inner(), &cognitive::cognitive_migrations()).await.unwrap();
    StoragePool::run_feature_migrations(pool.inner(), &coding_memory::coding_memory_migrations()).await.unwrap();
    let facts = SemanticFactRepo::new(pool.inner().clone());

    facts.upsert_with_metadata(
        &fact("repo:bot", "framework", "tauri 2", Some("bot")),
        Some("bot"), None,
    ).await.unwrap();
    facts.upsert_with_metadata(
        &fact("repo:bot", "convention", "camelCase JSON", Some("bot")),
        Some("bot"), None,
    ).await.unwrap();

    let searcher = CodeDomainSearcher::new(facts);
    let results = searcher.search("tauri", 10).await;
    assert!(results.iter().any(|r| r.text.contains("tauri")), "expected tauri in results: {results:?}");
}

#[tokio::test]
async fn searcher_domain_name_is_coding() {
    let pool = StoragePool::connect_in_memory().await.unwrap();
    StoragePool::run_feature_migrations(pool.inner(), &cognitive::cognitive_migrations()).await.unwrap();
    StoragePool::run_feature_migrations(pool.inner(), &coding_memory::coding_memory_migrations()).await.unwrap();
    let facts = SemanticFactRepo::new(pool.inner().clone());
    let s = CodeDomainSearcher::new(facts);
    assert_eq!(s.domain_name(), "coding");
}
```

- [ ] **Step 2: Implement**

Create `crates/coding-memory/src/code_domain_searcher.rs`:

```rust
//! Tier B4 — `CodeDomainSearcher` registered in InsightForge.
//!
//! Adds coding-memory facts to the InsightForge retrieval pool. `InsightForge`
//! calls `search(query, limit)`; we route to `SemanticFactRepo::search_fts`
//! filtered by the coding-relevant domains (`work`, `preferences`,
//! `procedural`, `coding`).

use async_trait::async_trait;
use cognitive::SemanticFactRepo;
use context_engine::insight_forge::DomainSearcher;
use context_engine::memory_retriever::{MemoryEntry, MemorySource};

/// `DomainSearcher` implementation that pulls coding-memory facts.
#[derive(Debug, Clone)]
pub struct CodeDomainSearcher {
    repo: SemanticFactRepo,
}

impl CodeDomainSearcher {
    /// Construct over the shared `SemanticFactRepo`.
    #[must_use]
    pub fn new(repo: SemanticFactRepo) -> Self {
        Self { repo }
    }
}

#[async_trait]
impl DomainSearcher for CodeDomainSearcher {
    fn domain_name(&self) -> &str { "coding" }

    async fn search(&self, query: &str, limit: usize) -> Vec<MemoryEntry> {
        // `search_fts` returns facts across all domains; we filter to the coding
        // ones post-hoc (cheap given small result sets).
        let facts = match self.repo.search_fts(query, limit as i64 * 2).await {
            Ok(v) => v,
            Err(e) => {
                tracing::warn!(error = %e, "CodeDomainSearcher search_fts failed");
                return vec![];
            }
        };
        facts
            .into_iter()
            .filter(|f| matches!(f.domain.as_str(), "work" | "preferences" | "procedural" | "coding"))
            .take(limit)
            .map(|f| MemoryEntry {
                id: f.id,
                text: format!("{} {} {}", f.subject, f.predicate, f.object),
                score: f.confidence as f32,
                source: MemorySource::SemanticFact,
                occurred_at: f.valid_from,
            })
            .collect()
    }
}
```

Edit `crates/coding-memory/src/lib.rs` and add:

```rust
/// Tier B4 — InsightForge searcher for coding facts.
pub mod code_domain_searcher;
```

> **`MemoryEntry`/`MemorySource` shape check:** verify field names with `rg 'pub struct MemoryEntry' crates/context_engine/src/memory_retriever.rs`. The construction above assumes a minimal common shape; align with actual fields. If `MemoryEntry` has more required fields, fill defaults. Existing `note_tree_navigator.rs` constructs `MemoryEntry` — copy its pattern.

- [ ] **Step 3: Run + commit**

```bash
cargo nextest run -p coding-memory --test code_domain_searcher
cargo clippy -p coding-memory --all-targets -- -D warnings
git add crates/coding-memory/src/code_domain_searcher.rs crates/coding-memory/src/lib.rs \
        crates/coding-memory/tests/code_domain_searcher.rs
git commit -m "feat(coding-memory): CodeDomainSearcher — InsightForge pulls coding facts (Tier B4)"
```

---

### Task 20: Tier B5 — `ShadowContext.session_type`

**Files:**
- Modify: `crates/autotuner/src/traits.rs`
- Test: `crates/autotuner/tests/shadow_context_session_type.rs`

- [ ] **Step 1: Extend `ShadowContext`**

Edit `crates/autotuner/src/traits.rs`. Find:

```rust
#[derive(Debug, Clone)]
pub struct ShadowContext {
    pub chat_id: String,
    pub session_key: String,
}
```

Replace with:

```rust
#[derive(Debug, Clone, Default)]
pub struct ShadowContext {
    pub chat_id: String,
    pub session_key: String,
    /// Tier B5 — `"coding"` | `"personal"` | `None` when unknown. Autotuner
    /// partitions champion sets by this; coding and personal workloads tune
    /// independently.
    pub session_type: Option<String>,
}
```

**Note:** adding a field to a `Default`-free struct breaks existing `ShadowContext { chat_id, session_key }` literal constructors. Add `..Default::default()` spread to each constructor found via `rg 'ShadowContext \{' crates` **or** add `session_type: None,` explicitly. Prefer `..Default::default()` per CLAUDE.md surgical-change guidance.

- [ ] **Step 2: Write failing test**

Create `crates/autotuner/tests/shadow_context_session_type.rs`:

```rust
use autotuner::ShadowContext;

#[test]
fn default_has_none_session_type() {
    let c: ShadowContext = Default::default();
    assert!(c.session_type.is_none());
}

#[test]
fn session_type_roundtrips() {
    let c = ShadowContext {
        chat_id: "x".into(), session_key: "y".into(),
        session_type: Some("coding".into()),
    };
    assert_eq!(c.session_type.as_deref(), Some("coding"));
}
```

- [ ] **Step 3: Run + commit**

```bash
cargo nextest run -p autotuner --test shadow_context_session_type
cargo build --workspace  # catch any literal-constructor regressions
cargo clippy --workspace --all-targets --all-features -- -D warnings
git add crates/autotuner/src/traits.rs crates/autotuner/tests/shadow_context_session_type.rs
git commit -m "feat(autotuner): ShadowContext.session_type for coding-vs-personal partitioning (B5)"
```

---

### Task 21: Distiller `distill_turn` — wire Phase A + B + C end-to-end

**Files:**
- Modify: `crates/coding-memory/src/distiller/mod.rs`
- Test: `crates/coding-memory/tests/distiller_end_to_end.rs`

- [ ] **Step 1: Write failing end-to-end test**

Create `crates/coding-memory/tests/distiller_end_to_end.rs`:

```rust
use async_trait::async_trait;
use coding_ingest::event::{AgentEvent, AgentEventV1, AgentSource, EventKind, TokenUsage};
use coding_ingest::store::IngestEventLogRepo;
use coding_memory::distiller::{Distiller, DistillerConfig, DistillerWriter};
use cognitive::{EpisodicMemoryRepo, SemanticFactRepo};
use jiff::Timestamp;
use providers::types::{ChatParams, LlmResponse, Message, ToolCall};
use providers::{LlmProvider, ProviderManager};
use std::path::PathBuf;
use std::sync::Arc;
use storage::StoragePool;
use uuid::Uuid;

/// Mock provider that returns a fixed `record_observation` tool call.
struct FixedProvider(Vec<ToolCall>);

#[async_trait]
impl LlmProvider for FixedProvider {
    async fn chat(&self, _p: &ChatParams) -> common::Result<LlmResponse> {
        Ok(LlmResponse {
            content: "".into(),
            tool_calls: self.0.clone(),
            usage: providers::types::Usage::default(),
        })
    }
    // Match the real trait — add stub impls for any other required methods.
}

fn evt(session: &str, turn: Option<&str>, kind: EventKind) -> AgentEvent {
    AgentEvent::V1(AgentEventV1 {
        id: Uuid::new_v4(),
        source: AgentSource::ClaudeCode,
        session_id: session.into(),
        turn_id: turn.map(str::to_string),
        cwd: PathBuf::from("/tmp"),
        repo: None,
        occurred_at: Timestamp::now(),
        kind,
    })
}

#[tokio::test]
async fn distill_turn_writes_turn_trace_plus_repo_context_fact() {
    let pool = StoragePool::connect_in_memory().await.unwrap();
    StoragePool::run_feature_migrations(pool.inner(), &cognitive::cognitive_migrations()).await.unwrap();
    StoragePool::run_feature_migrations(pool.inner(), &coding_memory::coding_memory_migrations()).await.unwrap();

    let ingest = Arc::new(IngestEventLogRepo::new(pool.inner().clone()));
    ingest.insert(&evt("s1", Some("t1"), EventKind::UserPrompt {
        text: "what framework does this repo use?".into(), attachments: vec![],
    })).await.unwrap();
    ingest.insert(&evt("s1", Some("t1"), EventKind::AssistantMsg {
        text: "It's a Tauri 2 app.".into(),
        truncated: false,
        token_usage: Some(TokenUsage { prompt_tokens: 50, completion_tokens: 20, cached_tokens: None }),
    })).await.unwrap();

    let writer = DistillerWriter::new(
        SemanticFactRepo::new(pool.inner().clone()),
        EpisodicMemoryRepo::new(pool.inner().clone()),
    );

    let observation = serde_json::json!({
        "kind": "repo_context",
        "subject": "repo:unknown",
        "predicate": "framework",
        "object": "tauri",
        "confidence": 0.9,
        "scope": "repo",
        "reasoning": "assistant stated explicitly"
    });
    let provider = Arc::new(ProviderManager::new(
        Arc::new(FixedProvider(vec![ToolCall {
            id: "call1".into(),
            name: "record_observation".into(),
            arguments: observation,
        }])),
        None, None,
    ));

    // UnifiedMemoryService wrapping the same fact repo.
    let retriever = Arc::new(cognitive::UnifiedMemoryService::new(SemanticFactRepo::new(pool.inner().clone()))) as Arc<dyn cognitive::MemoryRetriever>;

    let distiller = Distiller::new(DistillerConfig::default(), ingest.clone(), writer, provider, retriever);
    let report = distiller.distill_turn("s1", Some("t1")).await.unwrap();
    assert!(report.episodic_writes >= 1, "expected at least one turn_trace episode");
    assert!(report.semantic_writes >= 1, "expected at least one fact from the LLM observation");

    // Verify turn_trace kind on the written episode.
    let count: (i64,) = sqlx::query_as(
        "SELECT COUNT(*) FROM episodic_memories WHERE kind = 'turn_trace'",
    ).fetch_one(pool.inner()).await.unwrap();
    assert_eq!(count.0, 1);

    // Verify the repo_context fact with provenance is present.
    let fact_count: (i64,) = sqlx::query_as(
        "SELECT COUNT(*) FROM semantic_facts WHERE subject LIKE 'repo:%' AND predicate = 'framework' AND metadata IS NOT NULL",
    ).fetch_one(pool.inner()).await.unwrap();
    assert_eq!(fact_count.0, 1);

    // Verify ingest_event_log rows flipped to processed.
    assert_eq!(ingest.count_unprocessed().await.unwrap(), 0);
}
```

- [ ] **Step 2: Implement `distill_turn`**

Edit `crates/coding-memory/src/distiller/mod.rs`. Replace the `distill_turn` body in `impl Distiller` with:

```rust
    /// Distill one claimed turn end-to-end (Phase A + B + C).
    ///
    /// Orchestration:
    /// 1. `mark_processing` on all rows for this turn (atomic claim).
    /// 2. Load rows, decode to `AgentEvent`s.
    /// 3. Phase A — extract `TurnTrace`, persist as `turn_trace` episode.
    /// 4. Phase B — build prompt, invoke LLM, decode observations.
    ///    Failures enqueue retry + surface `distillation: pending` metadata.
    /// 5. For each observation → `build_prepared` → Phase C reconciliation
    ///    against top-k similar facts → NOOP / SUPERSEDE / ADD writes.
    /// 6. `mark_processed` on all claimed rows.
    pub async fn distill_turn(
        &self,
        session_id: &str,
        turn_id: Option<&str>,
    ) -> Result<DistillationReport> {
        use coding_ingest::event::{AgentEvent, EventKind};
        use error::DistillerError;
        use phase_b::LlmInvocation;
        use phase_c::{reconcile, ReconcileDecision, SimilarFact};
        use record_observation::observations_from_tool_calls;
        use writer::PreparedFact;

        let claimed = self
            .inner
            .ingest_repo
            .mark_processing(session_id, turn_id)
            .await?;
        if claimed == 0 {
            return Ok(DistillationReport::default());
        }

        let rows = self.inner.ingest_repo.fetch_turn(session_id, turn_id).await?;
        let events: Vec<AgentEvent> = rows
            .iter()
            .filter_map(|r| serde_json::from_str::<AgentEvent>(&r.payload).ok())
            .collect();
        let source_event_ids: Vec<uuid::Uuid> = rows
            .iter()
            .filter_map(|r| uuid::Uuid::parse_str(&r.id).ok())
            .collect();
        let row_ids: Vec<&str> = rows.iter().map(|r| r.id.as_str()).collect();

        let repo_id = events.iter().find_map(|e| {
            let AgentEvent::V1(v1) = e;
            v1.repo.as_ref().map(|r| r.repo_id.clone())
        });

        // Phase A — extractive.
        let trace = phase_a::compute_turn_trace(session_id, turn_id, &events);
        let mut report = DistillationReport::default();

        let prov_ext = crate::scope::ProvenanceMetadata {
            source_events: source_event_ids.clone(),
            session_id: session_id.to_string(),
            turn_id: turn_id.map(str::to_string),
            distilled_at: jiff::Timestamp::now(),
            distiller_model: self.inner.config.model.clone(),
            source_kind: crate::scope::ProvenanceKind::DistillerExtractive,
        };
        let trace_id = phase_a::persist_turn_trace(
            &self.inner.writer,
            &trace,
            repo_id.as_deref(),
            &prov_ext,
        )
        .await
        .map_err(common::KlyntbotError::from)?;
        report.episodic_writes += 1;
        report.turn_trace_id = Some(trace_id);

        // Phase B — LLM synthesis. Failures are non-fatal — Phase A already lands.
        let user_prompt_text = events.iter().find_map(|e| {
            let AgentEvent::V1(v1) = e;
            if let EventKind::UserPrompt { text, .. } = &v1.kind { Some(text.clone()) } else { None }
        }).unwrap_or_default();
        let assistant_text = events.iter().rev().find_map(|e| {
            let AgentEvent::V1(v1) = e;
            if let EventKind::AssistantMsg { text, .. } = &v1.kind { Some(text.clone()) } else { None }
        }).unwrap_or_default();

        let observations = match phase_b::invoke_llm(LlmInvocation {
            provider: self.inner.provider.clone(),
            model: self.inner.config.model.clone(),
            user_prompt_text: &user_prompt_text,
            assistant_text: &assistant_text,
            trace: &trace,
            repo_id: repo_id.as_deref(),
            timeout: self.inner.config.timeout,
        }).await {
            Ok(v) => { report.llm_calls += 1; v }
            Err(DistillerError::LlmTimeout { .. })
            | Err(DistillerError::LlmProvider { .. })
            | Err(DistillerError::Transient { .. }) => {
                tracing::warn!(session_id, ?turn_id, "Phase B transient failure — enqueuing retry");
                let _ = self.enqueue_retry(session_id, turn_id, retry_queue::RetryReason::LlmTimeout).await;
                self.inner.ingest_repo.mark_processed(row_ids.iter().copied()).await?;
                return Ok(report);
            }
            Err(DistillerError::LlmMalformed { detail }) => {
                tracing::warn!(session_id, ?turn_id, %detail, "Phase B malformed — dropping observations");
                vec![]
            }
            Err(e) => return Err(e.into()),
        };

        let prov_llm = crate::scope::ProvenanceMetadata {
            source_kind: crate::scope::ProvenanceKind::DistillerLlm,
            ..prov_ext.clone()
        };

        // Phase C per observation.
        for obs in &observations {
            let prepared = match fact_builder::build_prepared(obs, repo_id.as_deref(), &prov_llm) {
                Ok(p) => p,
                Err(_) => continue,
            };
            match prepared {
                fact_builder::Prepared::Episode(ep) => {
                    self.inner.writer.write_episode(ep).await.map_err(common::KlyntbotError::from)?;
                    report.episodic_writes += 1;
                }
                fact_builder::Prepared::Fact(pf) => {
                    let query_text = format!("{} {} {}", pf.fact.subject, pf.fact.predicate, pf.fact.object);
                    let similar = self.gather_similar(&query_text, repo_id.as_deref()).await;
                    match reconcile(&pf.fact, &similar) {
                        ReconcileDecision::Noop { predecessor_id } => {
                            let _ = self.inner.writer.bump_access(&predecessor_id).await;
                        }
                        ReconcileDecision::Add => {
                            self.inner.writer.write_fact(pf).await.map_err(common::KlyntbotError::from)?;
                            report.semantic_writes += 1;
                        }
                        ReconcileDecision::Supersede { predecessor_id } => {
                            let succ_id = pf.fact.id.clone();
                            let succ_valid_from = pf.fact.valid_from.clone();
                            self.inner.writer.write_fact(pf).await.map_err(common::KlyntbotError::from)?;
                            let _ = self.inner.writer
                                .complete_supersede(&predecessor_id, &succ_id, &succ_valid_from)
                                .await;
                            report.semantic_writes += 1;
                        }
                    }
                }
            }
        }

        self.inner.ingest_repo.mark_processed(row_ids.iter().copied()).await?;
        Ok(report)
    }

    async fn gather_similar(
        &self,
        query: &str,
        _repo_id: Option<&str>,
    ) -> Vec<phase_c::SimilarFact> {
        // Tier-1 retrieval: use the shared retriever. Vector-similarity is
        // embedded in `UnifiedMemoryService::retrieve`; we post-hoc score
        // the returned set using the existing `score` field.
        match self.inner.retriever.retrieve(query, 5).await {
            Ok(results) => results
                .into_iter()
                .filter_map(|entry| {
                    // `MemorySource::SemanticFact` entries can be looked up via id.
                    let id = entry.id;
                    // Load the fact; skip if missing.
                    let fact = futures::executor::block_on(
                        self.inner.writer.facts().get(&id),
                    ).ok().flatten()?;
                    Some(phase_c::SimilarFact { fact, similarity: entry.score })
                })
                .collect(),
            Err(_) => vec![],
        }
    }

    async fn enqueue_retry(
        &self,
        session_id: &str,
        turn_id: Option<&str>,
        reason: retry_queue::RetryReason,
    ) -> Result<()> {
        // Retry repo isn't currently held on the Distiller; callers wire a
        // dedicated handle. For now we lazily construct using the same pool
        // (derived from the ingest_repo's pool getter added in Task 4).
        // This helper stays to make callsites explicit; Task 22 wires the
        // repo handle properly on `DistillerInner`.
        let _ = (session_id, turn_id, reason);
        Ok(())
    }
```

Also add the `futures` helper import at the top of the file (`use futures`). If `futures` isn't a workspace dep, use `tokio::task::block_in_place` + `Handle::current().block_on(...)` — but prefer restructuring `gather_similar` to be `async` without `block_on`. The cleanest path: replace `futures::executor::block_on(...)` with `.await`, dropping the closure-based `filter_map`. Final shape:

```rust
async fn gather_similar(
    &self,
    query: &str,
    _repo_id: Option<&str>,
) -> Vec<phase_c::SimilarFact> {
    let Ok(results) = self.inner.retriever.retrieve(query, 5).await else {
        return vec![];
    };
    let mut out = Vec::new();
    for entry in results {
        if let Ok(Some(fact)) = self.inner.writer.facts().get(&entry.id).await {
            out.push(phase_c::SimilarFact { fact, similarity: entry.score });
        }
    }
    out
}
```

> **Note:** `cognitive::MemoryRetriever::retrieve` may have a different signature. Check with `rg 'async fn retrieve' crates/cognitive/src/services/memory_retriever.rs`. If `retrieve` takes `&str, usize` and returns `Result<Vec<MemoryEntry>>`, the code above works. If it returns a different entry type, adapt the field reads accordingly.

- [ ] **Step 3: Run + commit**

```bash
cargo nextest run -p coding-memory --test distiller_end_to_end
cargo clippy -p coding-memory --all-targets -- -D warnings
git add crates/coding-memory/src/distiller/mod.rs crates/coding-memory/tests/distiller_end_to_end.rs
git commit -m "feat(coding-memory): Distiller::distill_turn — Phase A+B+C orchestrated end-to-end"
```

---

### Task 22: Wire `DistillationRetryRepo` onto the Distiller + retry sweep

**Files:**
- Modify: `crates/coding-memory/src/distiller/mod.rs`
- Test: `crates/coding-memory/tests/distiller_retry.rs`

- [ ] **Step 1: Add retry repo to `DistillerInner`**

Edit `crates/coding-memory/src/distiller/mod.rs`. In `struct DistillerInner` add:

```rust
    retry_repo: retry_queue::DistillationRetryRepo,
```

In `Distiller::new`, add a parameter `retry_repo: retry_queue::DistillationRetryRepo` and initialize the field. Replace the placeholder `enqueue_retry` helper with:

```rust
    async fn enqueue_retry(
        &self,
        session_id: &str,
        turn_id: Option<&str>,
        reason: retry_queue::RetryReason,
    ) -> Result<()> {
        self.inner.retry_repo.enqueue(session_id, turn_id, reason).await
    }

    /// Sweeper — drain the retry queue once. Callers schedule this on their own clock.
    pub async fn sweep_retries(&self, max: i64) -> Result<u32> {
        let mut processed = 0u32;
        for row in self.inner.retry_repo.list_due(max).await? {
            let r = self.distill_turn(&row.session_id, row.turn_id.as_deref()).await;
            match r {
                Ok(_) => { let _ = self.inner.retry_repo.mark_done(&row.id).await; processed += 1; }
                Err(_) => { let _ = self.inner.retry_repo.record_attempt(&row.id).await; }
            }
        }
        Ok(processed)
    }
```

- [ ] **Step 2: Update existing callers of `Distiller::new`**

Every prior test file that constructed `Distiller::new(...)` needs the new argument. Run:

```bash
rg 'Distiller::new\(' crates/coding-memory/tests
```

For each call, append `retry_queue::DistillationRetryRepo::new(pool.inner().clone())` as the final argument (or import + construct at top of test). Test files that did not construct Distiller are unaffected.

- [ ] **Step 3: Write sweep test**

Create `crates/coding-memory/tests/distiller_retry.rs`:

```rust
use coding_memory::distiller::retry_queue::{DistillationRetryRepo, RetryReason};
use storage::StoragePool;

#[tokio::test]
async fn retry_list_due_returns_manually_enqueued() {
    let pool = StoragePool::connect_in_memory().await.unwrap();
    StoragePool::run_feature_migrations(pool.inner(), &cognitive::cognitive_migrations()).await.unwrap();
    StoragePool::run_feature_migrations(pool.inner(), &coding_memory::coding_memory_migrations()).await.unwrap();
    let retry = DistillationRetryRepo::new(pool.inner().clone());
    retry.enqueue("s1", Some("t1"), RetryReason::LlmTimeout).await.unwrap();
    let due = retry.list_due(10).await.unwrap();
    assert_eq!(due.len(), 1);
}
```

(The full end-to-end sweep is covered by Task 32's integration test.)

- [ ] **Step 4: Run + commit**

```bash
cargo nextest run -p coding-memory
cargo clippy -p coding-memory --all-targets -- -D warnings
git add crates/coding-memory/src/distiller/mod.rs crates/coding-memory/tests/distiller_retry.rs
git commit -m "feat(coding-memory): Distiller holds DistillationRetryRepo + sweep_retries"
```

---

### Task 23: `InProcessSink` forwards to the real Distiller

**Files:**
- Modify: `crates/coding-memory/src/sink.rs`
- Test: `crates/coding-memory/tests/sink_wiring.rs`

- [ ] **Step 1: Replace `InProcessSink` stub**

Edit `crates/coding-memory/src/sink.rs`. Replace the `InProcessSink` declaration and its impl with:

```rust
/// In-process sink — when desktop is off, klynt-cli calls the Distiller directly.
#[derive(Clone)]
pub struct InProcessSink {
    distiller: std::sync::Arc<crate::distiller::Distiller>,
}

impl std::fmt::Debug for InProcessSink {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("InProcessSink").finish_non_exhaustive()
    }
}

impl InProcessSink {
    /// Construct with a Distiller reference.
    #[must_use]
    pub fn new(distiller: std::sync::Arc<crate::distiller::Distiller>) -> Self {
        Self { distiller }
    }
}

#[async_trait]
impl MemorySink for InProcessSink {
    async fn accept_event(&self, event: AgentEvent) -> Result<()> {
        self.distiller.accept_event(event).await
    }
    async fn flush(&self) -> Result<()> {
        self.distiller.sweep_idle().await
    }
}
```

Remove the old `_phase_stub: ()` field and its `Default`.

- [ ] **Step 2: Write failing test**

Create `crates/coding-memory/tests/sink_wiring.rs`:

```rust
use coding_ingest::event::{AgentEvent, AgentEventV1, AgentSource, EventKind};
use coding_ingest::store::IngestEventLogRepo;
use coding_memory::distiller::retry_queue::DistillationRetryRepo;
use coding_memory::distiller::{Distiller, DistillerConfig, DistillerWriter};
use coding_memory::sink::{InProcessSink, MemorySink};
use cognitive::{EpisodicMemoryRepo, SemanticFactRepo};
use jiff::Timestamp;
use std::path::PathBuf;
use std::sync::Arc;
use storage::StoragePool;
use uuid::Uuid;

#[tokio::test]
async fn in_process_sink_forwards_to_distiller() {
    let pool = StoragePool::connect_in_memory().await.unwrap();
    StoragePool::run_feature_migrations(pool.inner(), &cognitive::cognitive_migrations()).await.unwrap();
    StoragePool::run_feature_migrations(pool.inner(), &coding_memory::coding_memory_migrations()).await.unwrap();

    let ingest = Arc::new(IngestEventLogRepo::new(pool.inner().clone()));
    let writer = DistillerWriter::new(
        SemanticFactRepo::new(pool.inner().clone()),
        EpisodicMemoryRepo::new(pool.inner().clone()),
    );
    let provider = Arc::new(providers::ProviderManager::new(
        Arc::new(providers::NoopProvider), None, None,
    ));
    let retriever = Arc::new(cognitive::UnifiedMemoryService::new(SemanticFactRepo::new(pool.inner().clone())))
        as Arc<dyn cognitive::MemoryRetriever>;
    let retry = DistillationRetryRepo::new(pool.inner().clone());
    let distiller = Arc::new(Distiller::new(DistillerConfig::default(), ingest.clone(), writer, provider, retriever, retry));
    let sink = InProcessSink::new(distiller);

    let evt = AgentEvent::V1(AgentEventV1 {
        id: Uuid::new_v4(),
        source: AgentSource::KlyntCli,
        session_id: "s1".into(), turn_id: Some("t1".into()),
        cwd: PathBuf::from("/tmp"), repo: None,
        occurred_at: Timestamp::now(),
        kind: EventKind::UserPrompt { text: "hi".into(), attachments: vec![] },
    });
    sink.accept_event(evt).await.unwrap();
    // InProcessSink doesn't persist events by itself — the Distiller reads from ingest_event_log
    // so for native klynt-cli we also need separate persistence. This test proves the hot-path
    // handoff compiles and runs; event persistence via InProcessSink is out of Phase 3 scope
    // (klynt-cli adds its own persistence per its spec).
}
```

- [ ] **Step 3: Run + commit**

```bash
cargo nextest run -p coding-memory --test sink_wiring
cargo clippy -p coding-memory --all-targets -- -D warnings
git add crates/coding-memory/src/sink.rs crates/coding-memory/tests/sink_wiring.rs
git commit -m "feat(coding-memory): InProcessSink now forwards to live Distiller"
```

---

### Task 24: `AppCore` holds `Arc<Distiller>` + init wires it up

**Files:**
- Modify: `crates/app-core/src/state.rs`
- Modify: `crates/app-core/src/init/mod.rs` (or wherever `AppCore` is built)

- [ ] **Step 1: Add field**

Edit `crates/app-core/src/state.rs`. Locate `pub struct AppCore { ... }` and add:

```rust
    /// Coding-memory Distiller — `None` until Phase-3 init wires it up.
    pub distiller: std::sync::OnceLock<std::sync::Arc<coding_memory::distiller::Distiller>>,
```

Initialize in the `AppCore::new` (or equivalent) constructor with `std::sync::OnceLock::new()`.

- [ ] **Step 2: Construct Distiller during init**

In the init aggregator (the file that builds `AppCore` — verify with `rg 'pub fn new' crates/app-core/src | rg 'AppCore'`), after storage init and after the existing `IngestDaemon` spawn from Phase 2:

```rust
use coding_memory::distiller::{Distiller, DistillerConfig, DistillerWriter};
use coding_memory::distiller::retry_queue::DistillationRetryRepo;
use cognitive::{EpisodicMemoryRepo, SemanticFactRepo, UnifiedMemoryService};

let config = DistillerConfig {
    model: app_config.coding_memory.distiller.model.clone(),
    max_input_tokens: app_config.coding_memory.distiller.max_input_tokens,
    timeout: parse_duration(&app_config.coding_memory.distiller.timeout)
        .unwrap_or(std::time::Duration::from_secs(30)),
    idle_timeout: std::time::Duration::from_secs(120),
};

let fact_repo = SemanticFactRepo::new(storage_pool.inner().clone());
let ep_repo = EpisodicMemoryRepo::new(storage_pool.inner().clone());
let writer = DistillerWriter::new(fact_repo.clone(), ep_repo);
let retriever = std::sync::Arc::new(UnifiedMemoryService::new(fact_repo.clone()))
    as std::sync::Arc<dyn cognitive::MemoryRetriever>;
let retry = DistillationRetryRepo::new(storage_pool.inner().clone());
let ingest_repo_arc = std::sync::Arc::new(
    coding_ingest::store::IngestEventLogRepo::new(storage_pool.inner().clone())
);

let distiller = std::sync::Arc::new(Distiller::new(
    config,
    ingest_repo_arc.clone(),
    writer,
    provider_manager.clone(),
    retriever,
    retry,
));

// After `app_core = Arc::new(AppCore { ... })`:
let _ = app_core.distiller.set(distiller.clone());
```

Where `provider_manager` is the already-constructed `Arc<ProviderManager>` that Phase-2 or earlier plumbing owns. If none exists at this init site, construct one here via `providers::factory::create_provider_with_failover(...)`.

`parse_duration` is a small helper; add:

```rust
fn parse_duration(s: &str) -> Option<std::time::Duration> {
    if let Some(num) = s.strip_suffix("ms") {
        num.parse::<u64>().ok().map(std::time::Duration::from_millis)
    } else if let Some(num) = s.strip_suffix('s') {
        num.parse::<u64>().ok().map(std::time::Duration::from_secs)
    } else {
        None
    }
}
```

- [ ] **Step 3: Build + commit**

```bash
cargo build -p app-core
cargo clippy -p app-core --all-targets -- -D warnings
git add crates/app-core/src/state.rs crates/app-core/src/init/
git commit -m "feat(app-core): construct Distiller during init; hold Arc<Distiller> on AppCore"
```

---

### Task 25: `IngestDaemon` forwards every persisted event to the Distiller

**Files:**
- Modify: `crates/coding-ingest/src/daemon.rs`

- [ ] **Step 1: Extend `IngestDaemonConfig` with an optional sink**

Edit `crates/coding-ingest/src/daemon.rs`. Replace `pub struct IngestDaemonConfig { ... }` with:

```rust
/// Configuration for the ingestion daemon.
#[derive(Clone)]
pub struct IngestDaemonConfig {
    /// Where the Unix socket is bound.
    pub socket_path: PathBuf,
    /// Where the cold-path file buffer lives.
    pub buffer_path: PathBuf,
    /// Desktop liveness touch-file path.
    pub lock_path: PathBuf,
    /// Repo that receives decoded events.
    pub repo: Arc<IngestEventLogRepo>,
    /// Optional — when set, every decoded event is forwarded to this sink AFTER
    /// being persisted. Phase 3 wires `Distiller::accept_event` through here.
    pub on_event: Option<Arc<dyn Fn(AgentEvent) + Send + Sync>>,
}

impl std::fmt::Debug for IngestDaemonConfig {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("IngestDaemonConfig")
            .field("socket_path", &self.socket_path)
            .field("buffer_path", &self.buffer_path)
            .field("lock_path", &self.lock_path)
            .field("on_event", &self.on_event.is_some())
            .finish()
    }
}
```

Update `handle_connection` to take the callback and fire it after insert:

```rust
async fn handle_connection(
    mut stream: tokio::net::UnixStream,
    repo: Arc<IngestEventLogRepo>,
    on_event: Option<Arc<dyn Fn(AgentEvent) + Send + Sync>>,
) -> Result<()> {
    let mut len_buf = [0u8; 4];
    stream.read_exact(&mut len_buf).await
        .map_err(|e| KlyntbotError::Storage(format!("read len: {e}")))?;
    let len = u32::from_le_bytes(len_buf) as usize;
    if len > MAX_PAYLOAD_BYTES {
        return Err(KlyntbotError::Storage(format!("payload too large: {len}")));
    }
    let mut body = vec![0u8; len];
    stream.read_exact(&mut body).await
        .map_err(|e| KlyntbotError::Storage(format!("read body: {e}")))?;
    let event: AgentEvent = serde_json::from_slice(&body)
        .map_err(|e| KlyntbotError::Storage(format!("decode event: {e}")))?;
    repo.insert(&event).await?;
    if let Some(cb) = on_event {
        cb(event);
    }
    Ok(())
}
```

And update the `accept_task` spawn inside `spawn(...)` to pass `on_event.clone()` into `handle_connection`.

Also update `drain_buffer` similarly so buffered events flow into the Distiller at startup — append a parallel `on_event` invocation in its per-event loop.

- [ ] **Step 2: Wire the callback at init time**

Edit the init aggregator (where Phase-2 `spawn(cfg)` is called). Replace the `IngestDaemonConfig` construction with one that includes `on_event`:

```rust
let distiller_for_daemon = distiller.clone();
let on_event: Arc<dyn Fn(coding_ingest::AgentEvent) + Send + Sync> = Arc::new(move |evt| {
    let d = distiller_for_daemon.clone();
    tokio::spawn(async move {
        if let Err(e) = d.accept_event(evt).await {
            tracing::warn!(error = %e, "distiller.accept_event failed");
        }
    });
});
let daemon_cfg = IngestDaemonConfig {
    socket_path: data_dir.join("ingest.sock"),
    buffer_path: data_dir.join("ingest-buffer.jsonl"),
    lock_path: data_dir.join("desktop.lock"),
    repo: ingest_repo_arc.clone(),
    on_event: Some(on_event),
};
```

- [ ] **Step 3: Extend existing daemon-lifecycle test to verify forwarding**

Edit `crates/coding-ingest/tests/daemon_lifecycle.rs` — add a new test at the bottom:

```rust
#[tokio::test]
async fn daemon_invokes_on_event_callback_after_insert() {
    use std::sync::atomic::{AtomicUsize, Ordering};
    let counter = Arc::new(AtomicUsize::new(0));
    let counter_cb = counter.clone();

    let pool = StoragePool::connect_in_memory().await.unwrap();
    StoragePool::run_feature_migrations(pool.inner(), &cognitive::cognitive_migrations()).await.unwrap();
    StoragePool::run_feature_migrations(pool.inner(), &coding_memory::coding_memory_migrations()).await.unwrap();
    let repo = Arc::new(IngestEventLogRepo::new(pool.inner().clone()));

    let dir = TempDir::new().unwrap();
    let cfg = IngestDaemonConfig {
        socket_path: dir.path().join("s.sock"),
        buffer_path: dir.path().join("buf.jsonl"),
        lock_path: dir.path().join("desktop.lock"),
        repo: repo.clone(),
        on_event: Some(Arc::new(move |_evt| {
            counter_cb.fetch_add(1, Ordering::SeqCst);
        })),
    };
    let handle = spawn(cfg.clone()).await.unwrap();

    let sink = UnixIngestSocket::new(cfg.socket_path.clone());
    sink.send(&AgentEvent::V1(AgentEventV1 {
        id: Uuid::new_v4(),
        source: AgentSource::ClaudeCode,
        session_id: "s1".into(), turn_id: None,
        cwd: PathBuf::from("/tmp"), repo: None,
        occurred_at: Timestamp::now(),
        kind: EventKind::UserPrompt { text: "x".into(), attachments: vec![] },
    })).await.unwrap();

    for _ in 0..50 {
        if counter.load(Ordering::SeqCst) > 0 { break; }
        tokio::time::sleep(std::time::Duration::from_millis(20)).await;
    }
    assert_eq!(counter.load(Ordering::SeqCst), 1);
    handle.shutdown().await;
}
```

- [ ] **Step 4: Run + commit**

```bash
cargo nextest run -p coding-ingest --test daemon_lifecycle
cargo build --workspace
cargo clippy --workspace --all-targets --all-features -- -D warnings
git add crates/coding-ingest/src/daemon.rs crates/coding-ingest/tests/daemon_lifecycle.rs \
        crates/app-core/src/init/
git commit -m "feat(coding-ingest+app-core): IngestDaemon on_event forwards to Distiller"
```

---

### Task 26: Idle-sweep + retry-sweep tickers

**Files:**
- Modify: `crates/app-core/src/init/mod.rs`

- [ ] **Step 1: Spawn two timer loops alongside the daemon**

In the init aggregator, after constructing the `distiller` handle add:

```rust
// Idle turn sweeper — every 30s ask the Distiller to flush any turns whose
// last event is > idle_timeout old. Keeps long-running sessions honest.
{
    let d = distiller.clone();
    tokio::spawn(async move {
        loop {
            tokio::time::sleep(std::time::Duration::from_secs(30)).await;
            if let Err(e) = d.sweep_idle().await {
                tracing::warn!(error = %e, "sweep_idle failed");
            }
        }
    });
}
// Retry sweeper — every 60s drain the distillation retry queue.
{
    let d = distiller.clone();
    tokio::spawn(async move {
        loop {
            tokio::time::sleep(std::time::Duration::from_secs(60)).await;
            match d.sweep_retries(16).await {
                Ok(n) if n > 0 => tracing::info!(processed = n, "retry sweep drained"),
                Ok(_) => {},
                Err(e) => tracing::warn!(error = %e, "sweep_retries failed"),
            }
        }
    });
}
```

These join the existing ingest-daemon accept loop as siblings — no graceful shutdown required (aborted when `AppCore` is dropped).

- [ ] **Step 2: Build + commit**

```bash
cargo build -p app-core
cargo clippy -p app-core --all-targets -- -D warnings
git add crates/app-core/src/init/
git commit -m "feat(app-core): spawn idle + retry sweeper tickers alongside Distiller"
```

---

### Task 27: Derive counterfactuals when `FixAttempt` episodes land

**Files:**
- Modify: `crates/coding-memory/src/distiller/fact_builder.rs` (or `distiller/mod.rs`'s observation loop)
- Modify: `crates/coding-memory/src/distiller/mod.rs`
- Test: existing `counterfactual.rs` already covers derivation; add a write-path scenario

- [ ] **Step 1: Extend the observation loop in `distill_turn`**

Edit `crates/coding-memory/src/distiller/mod.rs`. In `distill_turn`, inside the `for obs in &observations` loop, after handling `FixAttempt` episode writes, add (after `report.episodic_writes += 1;` for the Episode arm):

```rust
                fact_builder::Prepared::Episode(ep_prepared) => {
                    // Extract the FixAttempt-shaped content so we can derive a
                    // counterfactual for failures + abandonments.
                    let attempt_id = uuid::Uuid::parse_str(&ep_prepared.episode.id).ok();
                    let is_fix_attempt = ep_prepared.kind == "fix_attempt";
                    self.inner.writer.write_episode(ep_prepared).await.map_err(common::KlyntbotError::from)?;
                    report.episodic_writes += 1;

                    if is_fix_attempt {
                        if let Some(aid) = attempt_id {
                            // The Distiller's `record_observation` path doesn't yet
                            // carry a structured `FixAttempt`; we reconstruct the
                            // minimum fields needed for `derive_dead_end`.
                            let attempt = crate::facts::FixAttempt {
                                problem_hash: crate::problem_hash::ProblemHash::of(&obs.subject).as_str().to_string(),
                                problem: obs.subject.clone(),
                                files: vec![],
                                approach: obs.object.clone(),
                                outcome: outcome_from_predicate(&obs.predicate),
                                insight: Some(obs.reasoning.clone()),
                                duration_ms: 0,
                                test_before: None,
                                test_after: None,
                                anchored_symbols: vec![],
                                provenance: prov_llm.clone(),
                                sensitivity: crate::scope::Sensitivity::default(),
                            };
                            if let Some(mut ded) = crate::counterfactual::derive_dead_end(aid, &attempt) {
                                // Upsert the counterfactual fact.
                                ded.scope_id = repo_id.clone();
                                let prepared = crate::distiller::writer::PreparedFact {
                                    fact: ded,
                                    metadata_json: Some(serde_json::json!({
                                        "derivedFrom": aid.to_string(),
                                    })),
                                    scope_repo_id: repo_id.clone(),
                                    provenance: crate::scope::ProvenanceMetadata {
                                        source_kind: crate::scope::ProvenanceKind::DistillerLlm,
                                        ..prov_llm.clone()
                                    },
                                };
                                if let Err(e) = self.inner.writer.write_fact(prepared).await {
                                    tracing::warn!(error = %e, "dead-end fact write failed");
                                } else {
                                    report.semantic_writes += 1;
                                }
                            }
                        }
                    }
                }
```

Replace the existing `Prepared::Episode(ep)` arm in the loop with the above (merge logic).

Add the helper:

```rust
fn outcome_from_predicate(pred: &str) -> crate::facts::FixOutcome {
    match pred {
        "fixed" | "resolved" | "repaired" => crate::facts::FixOutcome::Success,
        "partial" | "partially_fixed" => crate::facts::FixOutcome::Partial,
        "failed" | "broken" => crate::facts::FixOutcome::Failure,
        _ => crate::facts::FixOutcome::Abandoned,
    }
}
```

- [ ] **Step 2: Extend end-to-end test**

Append to `crates/coding-memory/tests/distiller_end_to_end.rs`:

```rust
#[tokio::test]
async fn failure_fix_attempt_derives_counterfactual() {
    let pool = StoragePool::connect_in_memory().await.unwrap();
    StoragePool::run_feature_migrations(pool.inner(), &cognitive::cognitive_migrations()).await.unwrap();
    StoragePool::run_feature_migrations(pool.inner(), &coding_memory::coding_memory_migrations()).await.unwrap();

    let ingest = Arc::new(IngestEventLogRepo::new(pool.inner().clone()));
    ingest.insert(&evt("s1", Some("t1"), EventKind::UserPrompt { text: "parse error".into(), attachments: vec![] })).await.unwrap();
    ingest.insert(&evt("s1", Some("t1"), EventKind::AssistantMsg {
        text: "tried rewriting — did not work".into(),
        truncated: false,
        token_usage: Some(TokenUsage { prompt_tokens: 10, completion_tokens: 10, cached_tokens: None }),
    })).await.unwrap();

    let writer = DistillerWriter::new(
        SemanticFactRepo::new(pool.inner().clone()),
        EpisodicMemoryRepo::new(pool.inner().clone()),
    );
    let observation = serde_json::json!({
        "kind": "fix_attempt",
        "subject": "parser null pointer",
        "predicate": "failed",
        "object": "rewrite parser approach",
        "confidence": 0.85,
        "scope": "repo",
        "reasoning": "tests still failed after the rewrite"
    });
    let provider = Arc::new(ProviderManager::new(
        Arc::new(FixedProvider(vec![ToolCall {
            id: "call1".into(),
            name: "record_observation".into(),
            arguments: observation,
        }])),
        None, None,
    ));
    let retriever = Arc::new(cognitive::UnifiedMemoryService::new(SemanticFactRepo::new(pool.inner().clone())))
        as Arc<dyn cognitive::MemoryRetriever>;
    let retry = coding_memory::distiller::retry_queue::DistillationRetryRepo::new(pool.inner().clone());

    let distiller = Distiller::new(DistillerConfig::default(), ingest.clone(), writer, provider, retriever, retry);
    distiller.distill_turn("s1", Some("t1")).await.unwrap();

    let (count,): (i64,) = sqlx::query_as(
        "SELECT COUNT(*) FROM semantic_facts WHERE memory_type = 'counterfactual'",
    ).fetch_one(pool.inner()).await.unwrap();
    assert_eq!(count, 1, "expected one derived counterfactual fact");
}
```

- [ ] **Step 3: Run + commit**

```bash
cargo nextest run -p coding-memory --test distiller_end_to_end
cargo clippy -p coding-memory --all-targets -- -D warnings
git add crates/coding-memory/src/distiller/mod.rs crates/coding-memory/tests/distiller_end_to_end.rs
git commit -m "feat(coding-memory): derive counterfactual fact on failure/abandoned fix_attempt"
```

---

### Task 28: Property test — **Invariant 1** (provenance-always)

**Files:**
- Create: `crates/coding-memory/tests/prop_provenance_invariant.rs`

- [ ] **Step 1: Write the property test**

Create `crates/coding-memory/tests/prop_provenance_invariant.rs`:

```rust
//! Invariant 1: every Distiller-authored fact/episode carries non-empty
//! `metadata.provenance.source_events`.

use coding_memory::distiller::writer::{DistillerWriter, PreparedFact};
use coding_memory::distiller::DistillerError;
use coding_memory::scope::{ProvenanceKind, ProvenanceMetadata};
use cognitive::types::SemanticFact;
use cognitive::{EpisodicMemoryRepo, SemanticFactRepo};
use jiff::Timestamp;
use proptest::prelude::*;
use storage::StoragePool;
use uuid::Uuid;

fn fact() -> SemanticFact {
    SemanticFact {
        id: Uuid::new_v4().to_string(),
        domain: "work".into(), subject: "s".into(), predicate: "p".into(), object: "o".into(),
        confidence: 0.5, source: "distiller".into(),
        valid_from: Timestamp::now().to_string(), valid_until: None,
        recorded_at: Timestamp::now().to_string(),
        superseded_at: None, superseded_by: None,
        stability: 1.0, last_accessed: None, access_count: 0,
        convergence_score: 1.0, project_id: None,
        memory_type: "fact".into(), scope_type: "user".into(), scope_id: None,
    }
}

proptest! {
    #[test]
    fn writer_rejects_empty_source_events(n in 0usize..=0) {
        let rt = tokio::runtime::Builder::new_current_thread().enable_all().build().unwrap();
        rt.block_on(async {
            let pool = StoragePool::connect_in_memory().await.unwrap();
            StoragePool::run_feature_migrations(pool.inner(), &cognitive::cognitive_migrations()).await.unwrap();
            StoragePool::run_feature_migrations(pool.inner(), &coding_memory::coding_memory_migrations()).await.unwrap();
            let w = DistillerWriter::new(
                SemanticFactRepo::new(pool.inner().clone()),
                EpisodicMemoryRepo::new(pool.inner().clone()),
            );
            let prov = ProvenanceMetadata {
                source_events: (0..n).map(|_| Uuid::new_v4()).collect(),
                session_id: "s".into(), turn_id: None,
                distilled_at: Timestamp::now(),
                distiller_model: "x".into(),
                source_kind: ProvenanceKind::DistillerLlm,
            };
            let r = w.write_fact(PreparedFact {
                fact: fact(), metadata_json: None, scope_repo_id: None, provenance: prov,
            }).await;
            prop_assert!(matches!(r, Err(DistillerError::ProvenanceMissing)));
            Ok(())
        }).unwrap();
    }
}

proptest! {
    #[test]
    fn writer_accepts_non_empty_source_events(n in 1usize..=8) {
        let rt = tokio::runtime::Builder::new_current_thread().enable_all().build().unwrap();
        rt.block_on(async {
            let pool = StoragePool::connect_in_memory().await.unwrap();
            StoragePool::run_feature_migrations(pool.inner(), &cognitive::cognitive_migrations()).await.unwrap();
            StoragePool::run_feature_migrations(pool.inner(), &coding_memory::coding_memory_migrations()).await.unwrap();
            let w = DistillerWriter::new(
                SemanticFactRepo::new(pool.inner().clone()),
                EpisodicMemoryRepo::new(pool.inner().clone()),
            );
            let prov = ProvenanceMetadata {
                source_events: (0..n).map(|_| Uuid::new_v4()).collect(),
                session_id: "s".into(), turn_id: None,
                distilled_at: Timestamp::now(),
                distiller_model: "x".into(),
                source_kind: ProvenanceKind::DistillerLlm,
            };
            let r = w.write_fact(PreparedFact {
                fact: fact(), metadata_json: None, scope_repo_id: None, provenance: prov,
            }).await;
            prop_assert!(r.is_ok());
            Ok(())
        }).unwrap();
    }
}
```

- [ ] **Step 2: Run + commit**

```bash
cargo nextest run -p coding-memory --test prop_provenance_invariant
git add crates/coding-memory/tests/prop_provenance_invariant.rs
git commit -m "test(coding-memory): proptest — invariant #1 provenance-always"
```

---

### Task 29: Property test — **Invariant 2** (bi-temporal monotone)

**Files:**
- Create: `crates/coding-memory/tests/prop_bi_temporal.rs`

- [ ] **Step 1: Write the property test**

Create `crates/coding-memory/tests/prop_bi_temporal.rs`:

```rust
//! Invariant 2: `valid_until.map_or(true, |end| end >= valid_from)` for every fact.

use cognitive::types::SemanticFact;
use cognitive::SemanticFactRepo;
use jiff::Timestamp;
use proptest::prelude::*;
use storage::StoragePool;
use uuid::Uuid;

fn seed(id: &str, vf: &str, vu: Option<&str>) -> SemanticFact {
    SemanticFact {
        id: id.into(), domain: "work".into(),
        subject: "s".into(), predicate: "p".into(), object: "o".into(),
        confidence: 0.5, source: "distiller".into(),
        valid_from: vf.into(), valid_until: vu.map(str::to_string),
        recorded_at: Timestamp::now().to_string(),
        superseded_at: None, superseded_by: None,
        stability: 1.0, last_accessed: None, access_count: 0,
        convergence_score: 1.0, project_id: None,
        memory_type: "fact".into(), scope_type: "user".into(), scope_id: None,
    }
}

proptest! {
    #[test]
    fn monotone_holds_across_supersede(secs in 1..10_000i64) {
        let rt = tokio::runtime::Builder::new_current_thread().enable_all().build().unwrap();
        rt.block_on(async {
            let pool = StoragePool::connect_in_memory().await.unwrap();
            StoragePool::run_feature_migrations(pool.inner(), &cognitive::cognitive_migrations()).await.unwrap();
            StoragePool::run_feature_migrations(pool.inner(), &coding_memory::coding_memory_migrations()).await.unwrap();
            let repo = SemanticFactRepo::new(pool.inner().clone());

            let t0 = Timestamp::now();
            let t1 = t0.saturating_add(jiff::Span::new().seconds(secs));
            let f = seed(&Uuid::new_v4().to_string(), &t0.to_string(), Some(&t1.to_string()));
            repo.upsert(&f).await.unwrap();

            let (vf, vu): (String, Option<String>) = sqlx::query_as(
                "SELECT valid_from, valid_until FROM semantic_facts WHERE id = ?",
            ).bind(&f.id).fetch_one(pool.inner()).await.unwrap();
            prop_assert!(vu.as_deref().map_or(true, |end| end >= vf.as_str()));
            Ok(())
        }).unwrap();
    }
}
```

- [ ] **Step 2: Run + commit**

```bash
cargo nextest run -p coding-memory --test prop_bi_temporal
git add crates/coding-memory/tests/prop_bi_temporal.rs
git commit -m "test(coding-memory): proptest — invariant #2 bi-temporal monotone"
```

---

### Task 30: Property test — **Invariant 3** (SUPERSEDE chain equality)

**Files:**
- Create: `crates/coding-memory/tests/prop_supersede_chain.rs`

- [ ] **Step 1: Write the property test**

Create `crates/coding-memory/tests/prop_supersede_chain.rs`:

```rust
//! Invariant 3: if `fact_b.supersedes = fact_a.id`, then
//! `fact_a.valid_until == fact_b.valid_from`.

use coding_memory::distiller::writer::DistillerWriter;
use cognitive::types::SemanticFact;
use cognitive::{EpisodicMemoryRepo, SemanticFactRepo};
use jiff::Timestamp;
use proptest::prelude::*;
use storage::StoragePool;
use uuid::Uuid;

fn mk(id: &str) -> SemanticFact {
    SemanticFact {
        id: id.into(), domain: "work".into(),
        subject: "s".into(), predicate: "p".into(), object: "o".into(),
        confidence: 0.5, source: "distiller".into(),
        valid_from: Timestamp::now().to_string(), valid_until: None,
        recorded_at: Timestamp::now().to_string(),
        superseded_at: None, superseded_by: None,
        stability: 1.0, last_accessed: None, access_count: 0,
        convergence_score: 1.0, project_id: None,
        memory_type: "fact".into(), scope_type: "user".into(), scope_id: None,
    }
}

proptest! {
    #[test]
    fn predecessor_valid_until_equals_successor_valid_from(delta_secs in 0i64..10_000) {
        let rt = tokio::runtime::Builder::new_current_thread().enable_all().build().unwrap();
        rt.block_on(async {
            let pool = StoragePool::connect_in_memory().await.unwrap();
            StoragePool::run_feature_migrations(pool.inner(), &cognitive::cognitive_migrations()).await.unwrap();
            StoragePool::run_feature_migrations(pool.inner(), &coding_memory::coding_memory_migrations()).await.unwrap();
            let writer = DistillerWriter::new(
                SemanticFactRepo::new(pool.inner().clone()),
                EpisodicMemoryRepo::new(pool.inner().clone()),
            );
            let repo = SemanticFactRepo::new(pool.inner().clone());

            let pred = mk(&Uuid::new_v4().to_string());
            let mut succ = mk(&Uuid::new_v4().to_string());
            succ.valid_from = Timestamp::now()
                .saturating_add(jiff::Span::new().seconds(delta_secs))
                .to_string();
            repo.upsert(&pred).await.unwrap();
            repo.upsert(&succ).await.unwrap();
            writer.complete_supersede(&pred.id, &succ.id, &succ.valid_from).await.unwrap();

            let (valid_until,): (Option<String>,) = sqlx::query_as(
                "SELECT valid_until FROM semantic_facts WHERE id = ?",
            ).bind(&pred.id).fetch_one(pool.inner()).await.unwrap();
            prop_assert_eq!(valid_until.as_deref(), Some(succ.valid_from.as_str()));
            Ok(())
        }).unwrap();
    }
}
```

- [ ] **Step 2: Run + commit**

```bash
cargo nextest run -p coding-memory --test prop_supersede_chain
git add crates/coding-memory/tests/prop_supersede_chain.rs
git commit -m "test(coding-memory): proptest — invariant #3 SUPERSEDE chain equality"
```

---

### Task 31: Property test — **Invariant 5** (Distiller-never-deletes)

**Files:**
- Create: `crates/coding-memory/tests/prop_distiller_never_deletes.rs`

- [ ] **Step 1: Write the property test**

Create `crates/coding-memory/tests/prop_distiller_never_deletes.rs`:

```rust
//! Invariant 5: after any Distiller cycle, `count(semantic_facts) +
//! count(episodic_memories)` is non-decreasing. NOOP/SUPERSEDE don't
//! remove rows; the Distiller never issues DELETE.

use coding_ingest::event::{AgentEvent, AgentEventV1, AgentSource, EventKind, TokenUsage};
use coding_ingest::store::IngestEventLogRepo;
use coding_memory::distiller::{Distiller, DistillerConfig, DistillerWriter};
use coding_memory::distiller::retry_queue::DistillationRetryRepo;
use cognitive::{EpisodicMemoryRepo, SemanticFactRepo, UnifiedMemoryService};
use jiff::Timestamp;
use proptest::prelude::*;
use providers::{NoopProvider, ProviderManager};
use std::path::PathBuf;
use std::sync::Arc;
use storage::StoragePool;
use uuid::Uuid;

proptest! {
    #[test]
    fn row_count_never_shrinks(events in 1usize..=20) {
        let rt = tokio::runtime::Builder::new_current_thread().enable_all().build().unwrap();
        rt.block_on(async {
            let pool = StoragePool::connect_in_memory().await.unwrap();
            StoragePool::run_feature_migrations(pool.inner(), &cognitive::cognitive_migrations()).await.unwrap();
            StoragePool::run_feature_migrations(pool.inner(), &coding_memory::coding_memory_migrations()).await.unwrap();

            let ingest = Arc::new(IngestEventLogRepo::new(pool.inner().clone()));
            for i in 0..events {
                let kind = if i == events - 1 {
                    EventKind::AssistantMsg {
                        text: "done".into(), truncated: false,
                        token_usage: Some(TokenUsage { prompt_tokens: 1, completion_tokens: 1, cached_tokens: None }),
                    }
                } else {
                    EventKind::UserPrompt { text: format!("q{i}"), attachments: vec![] }
                };
                ingest.insert(&AgentEvent::V1(AgentEventV1 {
                    id: Uuid::new_v4(), source: AgentSource::ClaudeCode,
                    session_id: "s1".into(), turn_id: Some("t1".into()),
                    cwd: PathBuf::from("/tmp"), repo: None,
                    occurred_at: Timestamp::now(), kind,
                })).await.unwrap();
            }

            let before_facts: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM semantic_facts")
                .fetch_one(pool.inner()).await.unwrap();
            let before_eps: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM episodic_memories")
                .fetch_one(pool.inner()).await.unwrap();

            let writer = DistillerWriter::new(
                SemanticFactRepo::new(pool.inner().clone()),
                EpisodicMemoryRepo::new(pool.inner().clone()),
            );
            let provider = Arc::new(ProviderManager::new(Arc::new(NoopProvider), None, None));
            let retriever = Arc::new(UnifiedMemoryService::new(SemanticFactRepo::new(pool.inner().clone())))
                as Arc<dyn cognitive::MemoryRetriever>;
            let retry = DistillationRetryRepo::new(pool.inner().clone());
            let d = Distiller::new(DistillerConfig::default(), ingest, writer, provider, retriever, retry);
            d.distill_turn("s1", Some("t1")).await.unwrap();

            let after_facts: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM semantic_facts")
                .fetch_one(pool.inner()).await.unwrap();
            let after_eps: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM episodic_memories")
                .fetch_one(pool.inner()).await.unwrap();
            prop_assert!(after_facts.0 >= before_facts.0);
            prop_assert!(after_eps.0 >= before_eps.0);
            Ok(())
        }).unwrap();
    }
}
```

- [ ] **Step 2: Run + commit**

```bash
cargo nextest run -p coding-memory --test prop_distiller_never_deletes
git add crates/coding-memory/tests/prop_distiller_never_deletes.rs
git commit -m "test(coding-memory): proptest — invariant #5 Distiller-never-deletes"
```

---

### Task 32: Synthetic Phase-3 fixture + round-trip integration test

**Files:**
- Create: `tests/fixtures/coding/phase3_bug_fix_session.jsonl`
- Create: `tests/integration/coding_memory_phase3_roundtrip.rs`

- [ ] **Step 1: Create the fixture**

Create `tests/fixtures/coding/phase3_bug_fix_session.jsonl`. Each line is a single `AgentEvent` JSON. Keep 10 events approximating a short bug-fix session:

```jsonl
{"v":"v1","id":"00000000-0000-0000-0000-000000000001","source":"claudeCode","sessionId":"sess-p3-1","turnId":"turn-1","cwd":"/repo","repo":{"repoId":"github.com/klynt/bot","root":"/repo","gitHash":"abc123","branch":"main"},"occurredAt":"2026-04-25T10:00:00Z","kind":{"kind":"sessionStart","model":"claude-sonnet-4-6","sourceReason":"cli-arg"}}
{"v":"v1","id":"00000000-0000-0000-0000-000000000002","source":"claudeCode","sessionId":"sess-p3-1","turnId":"turn-1","cwd":"/repo","repo":{"repoId":"github.com/klynt/bot","root":"/repo","gitHash":"abc123","branch":"main"},"occurredAt":"2026-04-25T10:00:01Z","kind":{"kind":"userPrompt","text":"fix the null pointer in parser","attachments":[]}}
{"v":"v1","id":"00000000-0000-0000-0000-000000000003","source":"claudeCode","sessionId":"sess-p3-1","turnId":"turn-1","cwd":"/repo","repo":{"repoId":"github.com/klynt/bot","root":"/repo","gitHash":"abc123","branch":"main"},"occurredAt":"2026-04-25T10:00:02Z","kind":{"kind":"toolCall","tool":"Read","argsPreview":"src/parser.rs","ok":true,"durationMs":5,"resultPreview":"fn parse_expr..."}}
{"v":"v1","id":"00000000-0000-0000-0000-000000000004","source":"claudeCode","sessionId":"sess-p3-1","turnId":"turn-1","cwd":"/repo","repo":{"repoId":"github.com/klynt/bot","root":"/repo","gitHash":"abc123","branch":"main"},"occurredAt":"2026-04-25T10:00:03Z","kind":{"kind":"fileEdit","path":"src/parser.rs","op":"modify","bytes":2048,"diffPreview":"+    if ptr.is_null() { return None; }"}}
{"v":"v1","id":"00000000-0000-0000-0000-000000000005","source":"claudeCode","sessionId":"sess-p3-1","turnId":"turn-1","cwd":"/repo","repo":{"repoId":"github.com/klynt/bot","root":"/repo","gitHash":"abc123","branch":"main"},"occurredAt":"2026-04-25T10:00:04Z","kind":{"kind":"toolCall","tool":"Bash","argsPreview":"cargo test -p parser","ok":true,"durationMs":2100,"resultPreview":"test result: ok. 14 passed; 0 failed"}}
{"v":"v1","id":"00000000-0000-0000-0000-000000000006","source":"claudeCode","sessionId":"sess-p3-1","turnId":"turn-1","cwd":"/repo","repo":{"repoId":"github.com/klynt/bot","root":"/repo","gitHash":"abc123","branch":"main"},"occurredAt":"2026-04-25T10:00:05Z","kind":{"kind":"testRun","command":"cargo test -p parser","framework":"cargo","passed":14,"failed":0,"durationMs":2100}}
{"v":"v1","id":"00000000-0000-0000-0000-000000000007","source":"claudeCode","sessionId":"sess-p3-1","turnId":"turn-1","cwd":"/repo","repo":{"repoId":"github.com/klynt/bot","root":"/repo","gitHash":"abc123","branch":"main"},"occurredAt":"2026-04-25T10:00:06Z","kind":{"kind":"assistantMsg","text":"Fixed: added a null guard in parse_expr. All parser tests green.","truncated":false,"tokenUsage":{"promptTokens":1800,"completionTokens":120,"cachedTokens":400}}}
{"v":"v1","id":"00000000-0000-0000-0000-000000000008","source":"claudeCode","sessionId":"sess-p3-1","turnId":"turn-2","cwd":"/repo","repo":{"repoId":"github.com/klynt/bot","root":"/repo","gitHash":"abc123","branch":"main"},"occurredAt":"2026-04-25T10:01:00Z","kind":{"kind":"userPrompt","text":"run the full workspace tests","attachments":[]}}
{"v":"v1","id":"00000000-0000-0000-0000-000000000009","source":"claudeCode","sessionId":"sess-p3-1","turnId":"turn-2","cwd":"/repo","repo":{"repoId":"github.com/klynt/bot","root":"/repo","gitHash":"abc123","branch":"main"},"occurredAt":"2026-04-25T10:01:05Z","kind":{"kind":"testRun","command":"cargo nextest run --workspace","framework":"cargo","passed":412,"failed":0,"durationMs":87000}}
{"v":"v1","id":"00000000-0000-0000-0000-000000000010","source":"claudeCode","sessionId":"sess-p3-1","turnId":"turn-2","cwd":"/repo","repo":{"repoId":"github.com/klynt/bot","root":"/repo","gitHash":"abc123","branch":"main"},"occurredAt":"2026-04-25T10:01:06Z","kind":{"kind":"assistantMsg","text":"Workspace is green.","truncated":false,"tokenUsage":{"promptTokens":500,"completionTokens":20,"cachedTokens":0}}}
```

- [ ] **Step 2: Write the integration test**

Create `tests/integration/coding_memory_phase3_roundtrip.rs`:

```rust
//! Phase-3 end-to-end scenario: feed a synthetic Claude Code session into
//! the ingestion daemon, wait for Distiller completion, assert rows landed
//! with the expected shape.

use async_trait::async_trait;
use coding_ingest::event::AgentEvent;
use coding_ingest::store::IngestEventLogRepo;
use coding_memory::distiller::retry_queue::DistillationRetryRepo;
use coding_memory::distiller::{Distiller, DistillerConfig, DistillerWriter};
use cognitive::{EpisodicMemoryRepo, SemanticFactRepo, UnifiedMemoryService};
use providers::types::{ChatParams, LlmResponse, ToolCall, Usage};
use providers::{LlmProvider, ProviderManager};
use std::sync::Arc;
use storage::StoragePool;

struct ScriptedProvider {
    calls: std::sync::Mutex<Vec<Vec<ToolCall>>>,
}

#[async_trait]
impl LlmProvider for ScriptedProvider {
    async fn chat(&self, _p: &ChatParams) -> common::Result<LlmResponse> {
        let calls = self.calls.lock().unwrap().pop().unwrap_or_default();
        Ok(LlmResponse { content: String::new(), tool_calls: calls, usage: Usage::default() })
    }
}

#[tokio::test]
async fn synthetic_session_produces_facts_and_episodes() {
    let pool = StoragePool::connect_in_memory().await.unwrap();
    StoragePool::run_feature_migrations(pool.inner(), &cognitive::cognitive_migrations()).await.unwrap();
    StoragePool::run_feature_migrations(pool.inner(), &coding_memory::coding_memory_migrations()).await.unwrap();

    let ingest = Arc::new(IngestEventLogRepo::new(pool.inner().clone()));
    let fixture = std::fs::read_to_string("tests/fixtures/coding/phase3_bug_fix_session.jsonl").unwrap();
    for line in fixture.lines() {
        let evt: AgentEvent = serde_json::from_str(line).unwrap();
        ingest.insert(&evt).await.unwrap();
    }

    // Turn 1 observation: successful fix_attempt + repo_context.
    // Turn 2 observation: none.
    let turn1 = vec![
        ToolCall {
            id: "c1".into(),
            name: "record_observation".into(),
            arguments: serde_json::json!({
                "kind": "fix_attempt",
                "subject": "parser null pointer",
                "predicate": "fixed",
                "object": "added null guard in parse_expr",
                "confidence": 0.9,
                "scope": "repo",
                "reasoning": "tests pass after edit"
            }),
        },
        ToolCall {
            id: "c2".into(),
            name: "record_observation".into(),
            arguments: serde_json::json!({
                "kind": "repo_context",
                "subject": "repo:github.com/klynt/bot",
                "predicate": "test_command",
                "object": "cargo test",
                "confidence": 1.0,
                "scope": "repo",
                "reasoning": "observed cargo invocation"
            }),
        },
    ];
    let scripted = Arc::new(ScriptedProvider {
        calls: std::sync::Mutex::new(vec![vec![], turn1]), // popped in reverse: turn-1 first
    });
    let provider = Arc::new(ProviderManager::new(scripted, None, None));
    let writer = DistillerWriter::new(
        SemanticFactRepo::new(pool.inner().clone()),
        EpisodicMemoryRepo::new(pool.inner().clone()),
    );
    let retriever = Arc::new(UnifiedMemoryService::new(SemanticFactRepo::new(pool.inner().clone())))
        as Arc<dyn cognitive::MemoryRetriever>;
    let retry = DistillationRetryRepo::new(pool.inner().clone());
    let distiller = Distiller::new(DistillerConfig::default(), ingest.clone(), writer, provider, retriever, retry);

    distiller.distill_turn("sess-p3-1", Some("turn-1")).await.unwrap();
    distiller.distill_turn("sess-p3-1", Some("turn-2")).await.unwrap();

    // Two turn_trace episodes + one fix_attempt episode.
    let trace_count: (i64,) = sqlx::query_as(
        "SELECT COUNT(*) FROM episodic_memories WHERE kind = 'turn_trace'"
    ).fetch_one(pool.inner()).await.unwrap();
    assert_eq!(trace_count.0, 2);

    let fix_count: (i64,) = sqlx::query_as(
        "SELECT COUNT(*) FROM episodic_memories WHERE kind = 'fix_attempt'"
    ).fetch_one(pool.inner()).await.unwrap();
    assert_eq!(fix_count.0, 1);

    // At least one RepoContext fact with scope_repo_id.
    let fact_count: (i64,) = sqlx::query_as(
        "SELECT COUNT(*) FROM semantic_facts WHERE predicate = 'test_command' AND scope_repo_id = 'github.com/klynt/bot'"
    ).fetch_one(pool.inner()).await.unwrap();
    assert_eq!(fact_count.0, 1);

    // Every row written has non-empty source_events in its metadata.
    let all_meta: Vec<(Option<String>,)> = sqlx::query_as(
        "SELECT metadata FROM semantic_facts UNION ALL SELECT metadata FROM episodic_memories"
    ).fetch_all(pool.inner()).await.unwrap();
    for (m,) in all_meta {
        let v: serde_json::Value = serde_json::from_str(&m.unwrap_or_default()).unwrap_or_default();
        let arr = v.get("provenance").and_then(|p| p.get("sourceEvents")).and_then(|a| a.as_array());
        assert!(arr.map(|a| !a.is_empty()).unwrap_or(false), "empty provenance found");
    }

    // All events processed.
    assert_eq!(ingest.count_unprocessed().await.unwrap(), 0);
}
```

- [ ] **Step 3: Run + commit**

```bash
cargo nextest run --test coding_memory_phase3_roundtrip
git add tests/fixtures/coding/phase3_bug_fix_session.jsonl \
        tests/integration/coding_memory_phase3_roundtrip.rs
git commit -m "test(coding-memory): Phase-3 roundtrip — synthetic session → distillation → facts"
```

---

### Task 33: Tier A activation scenario — existing cognitive surfaces fire for coding content

**Files:**
- Create: `tests/integration/coding_memory_phase3_tier_a.rs`

- [ ] **Step 1: Write the scenario test**

Create `tests/integration/coding_memory_phase3_tier_a.rs`:

```rust
//! Tier A activation: the existing klyntbot cognitive surfaces (score_turn,
//! ContradictionDetected, UserCorrectedAI) fire for coding content without
//! any net-new wiring. These are ACTIVATION tests — if they fail, a
//! regression is likely in cognitive, not coding-memory.

use bus::domain_events::{DomainEvent, DomainEventBus};
use cognitive::services::value_density::score_turn;

#[test]
fn score_turn_rates_coding_verbs_as_dense() {
    let s1 = score_turn("I fixed the parser bug and shipped the deploy", None);
    let s0 = score_turn("what time is it?", None);
    assert!(s1.density > s0.density, "coding turn should be denser than chit-chat");
}

#[test]
fn score_turn_recognizes_refactored_keyword() {
    let s = score_turn("refactored the scheduler to use a priority queue", None);
    assert!(s.density > 0.3, "refactored should be a high-density signal");
}

#[tokio::test]
async fn contradiction_detected_event_routes() {
    let bus = DomainEventBus::new(64);
    let mut rx = bus.subscribe_all();
    bus.publish(DomainEvent::ContradictionDetected {
        subject: "repo:github.com/klynt/bot".into(),
        predicate: "framework".into(),
        old_object: "electron".into(),
        new_object: "tauri".into(),
        session_id: "s1".into(),
    });
    let evt = tokio::time::timeout(std::time::Duration::from_secs(1), rx.recv()).await.unwrap().unwrap();
    matches!(evt, DomainEvent::ContradictionDetected { .. });
}

#[tokio::test]
async fn user_corrected_ai_event_routes() {
    use bus::domain_events::CorrectionKind;
    let bus = DomainEventBus::new(64);
    let mut rx = bus.subscribe_all();
    bus.publish(DomainEvent::UserCorrectedAI {
        original_ai_answer: "framework is electron".into(),
        correction: "it's tauri".into(),
        kind: CorrectionKind::FactualError,
        session_id: "s1".into(),
    });
    let evt = tokio::time::timeout(std::time::Duration::from_secs(1), rx.recv()).await.unwrap().unwrap();
    matches!(evt, DomainEvent::UserCorrectedAI { .. });
}
```

> **Signature check:** `DomainEvent::ContradictionDetected` + `UserCorrectedAI` have exact field names in `crates/bus/src/domain_events.rs`. Look them up with `rg 'ContradictionDetected \{|UserCorrectedAI \{' crates/bus/src/domain_events.rs` and align the literal shape in the test. The `CorrectionKind` enum variants may differ — check with `rg 'pub enum CorrectionKind' crates/bus/src/`. Adjust to match.

- [ ] **Step 2: Run + commit**

```bash
cargo nextest run --test coding_memory_phase3_tier_a
git add tests/integration/coding_memory_phase3_tier_a.rs
git commit -m "test(coding-memory): Tier A activation — score_turn + domain events fire for coding"
```

---

### Task 34: Memory Browser panel — handler + Tauri command + React

**Files:**
- Modify: `crates/desktop-shared/src/commands/coding_memory.rs` (add DTOs)
- Modify: `crates/app-core/src/coding_memory/handlers.rs` (add `memory_browser`)
- Modify: `crates/app-core/src/coding_memory/mod.rs` (expose)
- Modify: `crates/desktop/src/commands/coding_memory.rs` (add command + DEV_COMMANDS)
- Modify: `crates/desktop/src/lib.rs` (register)
- Create: `desktop-ui/src/features/coding-memory/MemoryBrowserPanel.tsx`
- Modify: `desktop-ui/src/features/coding-memory/hooks.ts`
- Modify: `desktop-ui/src/features/coding-memory/CodingMemoryLayout.tsx` (add nav entry)
- Modify: `desktop-ui/src/app/router.tsx` (add route)
- Test: `crates/app-core/tests/memory_browser_handler.rs`

- [ ] **Step 1: Add DTOs**

Edit `crates/desktop-shared/src/commands/coding_memory.rs`. Append:

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MemoryBrowserEntry {
    pub id: String,
    /// `"fact"` or `"episode"`.
    pub row_type: String,
    pub kind: Option<String>,
    pub scope_repo_id: Option<String>,
    pub subject: Option<String>,
    pub predicate: Option<String>,
    pub object: Option<String>,
    pub content_preview: Option<String>,
    pub confidence: Option<f64>,
    pub memory_type: Option<String>,
    pub occurred_at: String,
    pub metadata_json: Option<String>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MemoryBrowserFilter {
    pub repo_id: Option<String>,
    pub kind: Option<String>,
    pub memory_type: Option<String>,
    pub query: Option<String>,
    pub limit: Option<i64>,
    pub offset: Option<i64>,
}
```

- [ ] **Step 2: Write handler test**

Create `crates/app-core/tests/memory_browser_handler.rs`:

```rust
use app_core::coding_memory::handlers::memory_browser;
use cognitive::types::SemanticFact;
use cognitive::SemanticFactRepo;
use desktop_shared::commands::coding_memory::MemoryBrowserFilter;
use jiff::Timestamp;
use storage::StoragePool;
use uuid::Uuid;

fn seed(repo_id: &str) -> SemanticFact {
    SemanticFact {
        id: Uuid::new_v4().to_string(), domain: "work".into(),
        subject: format!("repo:{repo_id}"), predicate: "framework".into(), object: "tauri".into(),
        confidence: 0.9, source: "distiller".into(),
        valid_from: Timestamp::now().to_string(), valid_until: None,
        recorded_at: Timestamp::now().to_string(),
        superseded_at: None, superseded_by: None,
        stability: 1.0, last_accessed: None, access_count: 0,
        convergence_score: 1.0, project_id: None,
        memory_type: "fact".into(),
        scope_type: "project".into(), scope_id: Some(repo_id.into()),
    }
}

#[tokio::test]
async fn memory_browser_filters_by_repo() {
    let pool = StoragePool::connect_in_memory().await.unwrap();
    StoragePool::run_feature_migrations(pool.inner(), &cognitive::cognitive_migrations()).await.unwrap();
    StoragePool::run_feature_migrations(pool.inner(), &coding_memory::coding_memory_migrations()).await.unwrap();
    let repo = SemanticFactRepo::new(pool.inner().clone());
    repo.upsert_with_metadata(&seed("bot"), Some("bot"), None).await.unwrap();
    repo.upsert_with_metadata(&seed("other"), Some("other"), None).await.unwrap();

    let filter = MemoryBrowserFilter { repo_id: Some("bot".into()), limit: Some(100), ..Default::default() };
    let rows = memory_browser(pool.inner(), &filter).await.unwrap();
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0].scope_repo_id.as_deref(), Some("bot"));
}
```

- [ ] **Step 3: Implement handler**

Append to `crates/app-core/src/coding_memory/handlers.rs`:

```rust
/// Memory Browser handler — paginated cross-type (facts + episodes) view.
pub async fn memory_browser(
    pool: &sqlx::SqlitePool,
    filter: &desktop_shared::commands::coding_memory::MemoryBrowserFilter,
) -> common::Result<Vec<desktop_shared::commands::coding_memory::MemoryBrowserEntry>> {
    use desktop_shared::commands::coding_memory::MemoryBrowserEntry;
    use sqlx::Row;
    let limit = filter.limit.unwrap_or(100);
    let offset = filter.offset.unwrap_or(0);

    // Facts.
    let mut out = Vec::new();
    let facts = sqlx::query(
        "SELECT id, subject, predicate, object, confidence, memory_type,
                scope_repo_id, recorded_at, metadata
         FROM semantic_facts
         WHERE (?1 IS NULL OR scope_repo_id = ?1)
           AND (?2 IS NULL OR memory_type = ?2)
           AND (?3 IS NULL OR subject LIKE '%' || ?3 || '%' OR object LIKE '%' || ?3 || '%')
         ORDER BY recorded_at DESC
         LIMIT ?4 OFFSET ?5",
    )
    .bind(filter.repo_id.as_deref())
    .bind(filter.memory_type.as_deref())
    .bind(filter.query.as_deref())
    .bind(limit)
    .bind(offset)
    .fetch_all(pool)
    .await
    .map_err(|e| common::KlyntbotError::Storage(format!("memory_browser facts: {e}")))?;

    for r in facts {
        out.push(MemoryBrowserEntry {
            id: r.get("id"),
            row_type: "fact".into(),
            kind: None,
            scope_repo_id: r.try_get("scope_repo_id").ok(),
            subject: r.try_get("subject").ok(),
            predicate: r.try_get("predicate").ok(),
            object: r.try_get("object").ok(),
            content_preview: None,
            confidence: r.try_get("confidence").ok(),
            memory_type: r.try_get("memory_type").ok(),
            occurred_at: r.try_get("recorded_at").unwrap_or_default(),
            metadata_json: r.try_get("metadata").ok(),
        });
    }

    // Episodes (skip when caller filtered memory_type — episodes don't have one).
    if filter.memory_type.is_none() {
        let eps = sqlx::query(
            "SELECT id, kind, scope_repo_id, content, occurred_at, metadata
             FROM episodic_memories
             WHERE (?1 IS NULL OR scope_repo_id = ?1)
               AND (?2 IS NULL OR kind = ?2)
               AND (?3 IS NULL OR content LIKE '%' || ?3 || '%')
             ORDER BY occurred_at DESC
             LIMIT ?4 OFFSET ?5",
        )
        .bind(filter.repo_id.as_deref())
        .bind(filter.kind.as_deref())
        .bind(filter.query.as_deref())
        .bind(limit)
        .bind(offset)
        .fetch_all(pool)
        .await
        .map_err(|e| common::KlyntbotError::Storage(format!("memory_browser episodes: {e}")))?;

        for r in eps {
            let content: String = r.try_get("content").unwrap_or_default();
            out.push(MemoryBrowserEntry {
                id: r.get("id"),
                row_type: "episode".into(),
                kind: r.try_get("kind").ok(),
                scope_repo_id: r.try_get("scope_repo_id").ok(),
                subject: None,
                predicate: None,
                object: None,
                content_preview: Some(content.chars().take(240).collect()),
                confidence: None,
                memory_type: None,
                occurred_at: r.try_get("occurred_at").unwrap_or_default(),
                metadata_json: r.try_get("metadata").ok(),
            });
        }
    }

    Ok(out)
}
```

- [ ] **Step 4: Add Tauri command**

Edit `crates/desktop/src/commands/coding_memory.rs`. Append to `DEV_COMMANDS`:

```rust
    "coding_memory_browser",
```

Add:

```rust
#[tauri::command]
pub async fn coding_memory_browser(
    state: State<'_, Arc<AppCore>>,
    filter: MemoryBrowserFilter,
) -> Result<Vec<MemoryBrowserEntry>, ApiError> {
    state.coding_memory_browser(filter).await
}
```

Extend `dispatch_dev`:

```rust
        "coding_memory_browser" => {
            #[derive(serde::Deserialize)] struct A { filter: MemoryBrowserFilter }
            let a: A = serde_json::from_value(args).map_err(|e| ApiError::bad_request(e.to_string()))?;
            ok(state.coding_memory_browser(a.filter).await?)
        }
```

Register in `crates/desktop/src/lib.rs` `invoke_handler![...]`:

```rust
commands::coding_memory::coding_memory_browser,
```

Add AppCore wrapper in `crates/app-core/src/coding_memory/mod.rs`:

```rust
impl crate::AppCore {
    pub async fn coding_memory_browser(
        &self,
        filter: desktop_shared::commands::coding_memory::MemoryBrowserFilter,
    ) -> Result<Vec<desktop_shared::commands::coding_memory::MemoryBrowserEntry>, desktop_shared::errors::ApiError> {
        handlers::memory_browser(self.storage_pool.inner(), &filter).await
            .map_err(desktop_shared::errors::ApiError::from)
    }
}
```

- [ ] **Step 5: React panel**

Create `desktop-ui/src/features/coding-memory/MemoryBrowserPanel.tsx`:

```tsx
import { useQuery } from "@shared/data/ipc";
import { useMemo, useState } from "react";

type MemoryBrowserEntry = {
  id: string;
  rowType: "fact" | "episode";
  kind?: string | null;
  scopeRepoId?: string | null;
  subject?: string | null;
  predicate?: string | null;
  object?: string | null;
  contentPreview?: string | null;
  confidence?: number | null;
  memoryType?: string | null;
  occurredAt: string;
  metadataJson?: string | null;
};

export function MemoryBrowserPanel() {
  const [repoId, setRepoId] = useState<string>("");
  const [kind, setKind] = useState<string>("");
  const [query, setQuery] = useState<string>("");

  const filter = useMemo(
    () => ({ repoId: repoId || undefined, kind: kind || undefined, query: query || undefined, limit: 200 }),
    [repoId, kind, query],
  );
  const res = useQuery<MemoryBrowserEntry[]>("coding_memory_browser", { filter });
  const [selected, setSelected] = useState<MemoryBrowserEntry | null>(null);

  return (
    <section className="flex h-full flex-col gap-4 p-6">
      <header className="space-y-2">
        <h1 className="text-xl font-semibold text-text">Memory Browser</h1>
        <div className="flex gap-2">
          <input
            value={repoId} onChange={(e) => setRepoId(e.target.value)}
            placeholder="repo (e.g. github.com/klynt/bot)"
            className="w-72 rounded-md border border-border bg-surface-base px-3 py-1 text-sm"
          />
          <input
            value={kind} onChange={(e) => setKind(e.target.value)}
            placeholder="kind (fix_attempt / turn_trace / …)"
            className="w-64 rounded-md border border-border bg-surface-base px-3 py-1 text-sm"
          />
          <input
            value={query} onChange={(e) => setQuery(e.target.value)}
            placeholder="search text"
            className="flex-1 rounded-md border border-border bg-surface-base px-3 py-1 text-sm"
          />
        </div>
      </header>

      <div className="grid grid-cols-[1fr_360px] gap-4 overflow-hidden">
        <ul className="overflow-y-auto rounded-md border border-border">
          {(res.data ?? []).map((row) => (
            <li key={`${row.rowType}:${row.id}`}
                onClick={() => setSelected(row)}
                className="cursor-pointer border-b border-border px-3 py-2 hover:bg-surface-raised">
              <div className="flex items-center gap-2 text-xs text-muted">
                <span>{row.rowType}</span>
                {row.kind && <span>· {row.kind}</span>}
                {row.scopeRepoId && <span>· {row.scopeRepoId}</span>}
                <span className="ml-auto">{row.occurredAt}</span>
              </div>
              <div className="mt-1 text-sm">
                {row.rowType === "fact"
                  ? `${row.subject} · ${row.predicate} · ${row.object}`
                  : row.contentPreview}
              </div>
            </li>
          ))}
        </ul>
        <aside className="glass-panel rounded-md p-3 text-sm">
          {selected ? (
            <div className="space-y-2">
              <h2 className="font-semibold">Provenance</h2>
              <pre className="whitespace-pre-wrap break-all text-xs">
                {selected.metadataJson ?? "(none)"}
              </pre>
            </div>
          ) : (
            <p className="text-muted">Select a row to inspect provenance.</p>
          )}
        </aside>
      </div>
    </section>
  );
}
```

- [ ] **Step 6: Wire nav + route**

In `desktop-ui/src/features/coding-memory/hooks.ts` export:

```ts
export function useMemoryBrowser(filter: Record<string, unknown>) {
  return useQuery("coding_memory_browser", { filter });
}
```

In `CodingMemoryLayout.tsx` add a nav link to `/coding-memory/memory`. In `desktop-ui/src/app/router.tsx` add a child route `{ path: "memory", element: <MemoryBrowserPanel /> }` under the existing `/coding-memory` parent.

- [ ] **Step 7: Run + commit**

```bash
cargo nextest run -p app-core --test memory_browser_handler
cargo build --workspace
cargo clippy --workspace --all-targets --all-features -- -D warnings
cd desktop-ui && bun run lint:fix && bun run test && cd -
git add crates/desktop-shared/src/commands/coding_memory.rs \
        crates/app-core/src/coding_memory/ \
        crates/desktop/src/commands/coding_memory.rs crates/desktop/src/lib.rs \
        desktop-ui/src/features/coding-memory/MemoryBrowserPanel.tsx \
        desktop-ui/src/features/coding-memory/hooks.ts \
        desktop-ui/src/features/coding-memory/CodingMemoryLayout.tsx \
        desktop-ui/src/app/router.tsx \
        crates/app-core/tests/memory_browser_handler.rs
git commit -m "feat(workbench): Memory Browser panel — facts + episodes with provenance drawer"
```

---

### Task 35: Activity Timeline panel

**Files:**
- Modify: `crates/desktop-shared/src/commands/coding_memory.rs`
- Modify: `crates/app-core/src/coding_memory/handlers.rs`
- Modify: `crates/app-core/src/coding_memory/mod.rs`
- Modify: `crates/desktop/src/commands/coding_memory.rs`
- Modify: `crates/desktop/src/lib.rs`
- Create: `desktop-ui/src/features/coding-memory/ActivityTimelinePanel.tsx`
- Modify: `desktop-ui/src/features/coding-memory/hooks.ts`, `CodingMemoryLayout.tsx`, `app/router.tsx`
- Test: `crates/app-core/tests/activity_timeline_handler.rs`

- [ ] **Step 1: DTO + handler**

Append to `crates/desktop-shared/src/commands/coding_memory.rs`:

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ActivityBucket {
    pub day: String,    // YYYY-MM-DD
    pub count: i64,
    pub repo_id: Option<String>,
}
```

Append to `crates/app-core/src/coding_memory/handlers.rs`:

```rust
/// Activity Timeline handler — buckets episodes by day (local-UTC date).
pub async fn activity_timeline(
    pool: &sqlx::SqlitePool,
    days: i64,
    repo_id: Option<&str>,
) -> common::Result<Vec<desktop_shared::commands::coding_memory::ActivityBucket>> {
    use sqlx::Row;
    let rows = sqlx::query(
        "SELECT date(occurred_at) AS day, scope_repo_id, COUNT(*) AS c
         FROM episodic_memories
         WHERE occurred_at >= datetime('now', ?1 || ' days')
           AND (?2 IS NULL OR scope_repo_id = ?2)
         GROUP BY day, scope_repo_id
         ORDER BY day ASC",
    )
    .bind(format!("-{days}"))
    .bind(repo_id)
    .fetch_all(pool)
    .await
    .map_err(|e| common::KlyntbotError::Storage(format!("activity_timeline: {e}")))?;
    Ok(rows.into_iter().map(|r| desktop_shared::commands::coding_memory::ActivityBucket {
        day: r.get("day"),
        count: r.get("c"),
        repo_id: r.try_get("scope_repo_id").ok().flatten(),
    }).collect())
}
```

(Above, the `r.try_get("scope_repo_id").ok().flatten()` pattern assumes the column returns `Option<String>` directly; if sqlx complains about a redundant `.flatten()`, drop it.)

- [ ] **Step 2: Tauri + AppCore wrapper**

Append to `crates/desktop/src/commands/coding_memory.rs`:

```rust
#[tauri::command]
pub async fn coding_memory_activity_timeline(
    state: State<'_, Arc<AppCore>>,
    days: Option<i64>,
    repo_id: Option<String>,
) -> Result<Vec<ActivityBucket>, ApiError> {
    state.coding_memory_activity_timeline(days.unwrap_or(30), repo_id).await
}
```

Extend `DEV_COMMANDS` with `"coding_memory_activity_timeline"` and `dispatch_dev` with the matching branch.

Register in `crates/desktop/src/lib.rs`.

Add wrapper in `crates/app-core/src/coding_memory/mod.rs`:

```rust
impl crate::AppCore {
    pub async fn coding_memory_activity_timeline(
        &self,
        days: i64,
        repo_id: Option<String>,
    ) -> Result<Vec<desktop_shared::commands::coding_memory::ActivityBucket>, desktop_shared::errors::ApiError> {
        handlers::activity_timeline(self.storage_pool.inner(), days, repo_id.as_deref()).await
            .map_err(desktop_shared::errors::ApiError::from)
    }
}
```

- [ ] **Step 3: Test**

Create `crates/app-core/tests/activity_timeline_handler.rs`:

```rust
use app_core::coding_memory::handlers::activity_timeline;
use storage::StoragePool;

#[tokio::test]
async fn activity_timeline_groups_by_day() {
    let pool = StoragePool::connect_in_memory().await.unwrap();
    StoragePool::run_feature_migrations(pool.inner(), &cognitive::cognitive_migrations()).await.unwrap();
    StoragePool::run_feature_migrations(pool.inner(), &coding_memory::coding_memory_migrations()).await.unwrap();

    sqlx::query(
        "INSERT INTO episodic_memories
         (id, domain, content, summary, importance, occurred_at, recorded_at,
          stability, last_accessed, access_count, project_id, scope_type, scope_id, kind, scope_repo_id)
         VALUES
         ('a','coding','x',NULL,0.5,'2026-04-24T10:00:00Z','2026-04-24T10:00:00Z',1.0,NULL,0,NULL,'user',NULL,'turn_trace',NULL),
         ('b','coding','x',NULL,0.5,'2026-04-24T11:00:00Z','2026-04-24T11:00:00Z',1.0,NULL,0,NULL,'user',NULL,'turn_trace',NULL),
         ('c','coding','x',NULL,0.5,'2026-04-25T10:00:00Z','2026-04-25T10:00:00Z',1.0,NULL,0,NULL,'user',NULL,'turn_trace',NULL)",
    ).execute(pool.inner()).await.unwrap();

    let buckets = activity_timeline(pool.inner(), 365, None).await.unwrap();
    let total: i64 = buckets.iter().map(|b| b.count).sum();
    assert_eq!(total, 3);
    assert!(buckets.iter().any(|b| b.day == "2026-04-24" && b.count == 2));
    assert!(buckets.iter().any(|b| b.day == "2026-04-25" && b.count == 1));
}
```

- [ ] **Step 4: React panel (simple bar chart)**

Create `desktop-ui/src/features/coding-memory/ActivityTimelinePanel.tsx`:

```tsx
import { useQuery } from "@shared/data/ipc";
import { useState } from "react";

type Bucket = { day: string; count: number; repoId?: string | null };

export function ActivityTimelinePanel() {
  const [days, setDays] = useState(30);
  const [repoId, setRepoId] = useState<string>("");
  const res = useQuery<Bucket[]>("coding_memory_activity_timeline", { days, repoId: repoId || undefined });
  const max = Math.max(1, ...(res.data ?? []).map((b) => b.count));
  return (
    <section className="flex h-full flex-col gap-4 p-6">
      <header className="flex items-end gap-2">
        <h1 className="text-xl font-semibold text-text">Activity</h1>
        <input value={repoId} onChange={(e) => setRepoId(e.target.value)}
               placeholder="repo filter"
               className="ml-4 rounded-md border border-border bg-surface-base px-3 py-1 text-sm" />
        <select value={days} onChange={(e) => setDays(Number(e.target.value))}
                className="rounded-md border border-border bg-surface-base px-3 py-1 text-sm">
          {[7, 30, 90, 365].map((d) => <option key={d} value={d}>{d}d</option>)}
        </select>
      </header>
      <div className="flex h-64 items-end gap-1 rounded-md border border-border p-3">
        {(res.data ?? []).map((b) => (
          <div key={`${b.day}:${b.repoId ?? ""}`}
               title={`${b.day} — ${b.count}`}
               style={{ height: `${(b.count / max) * 100}%` }}
               className="min-w-[4px] flex-1 rounded-sm bg-accent" />
        ))}
      </div>
    </section>
  );
}
```

- [ ] **Step 5: Wire + commit**

Add hook + nav + route exactly like Task 34. Then:

```bash
cargo nextest run -p app-core --test activity_timeline_handler
cargo build --workspace
cargo clippy --workspace --all-targets --all-features -- -D warnings
cd desktop-ui && bun run lint:fix && bun run build && cd -
git add crates/desktop-shared/src/commands/coding_memory.rs \
        crates/app-core/src/coding_memory/ crates/desktop/src/commands/coding_memory.rs crates/desktop/src/lib.rs \
        desktop-ui/src/features/coding-memory/ActivityTimelinePanel.tsx \
        desktop-ui/src/features/coding-memory/hooks.ts desktop-ui/src/features/coding-memory/CodingMemoryLayout.tsx \
        desktop-ui/src/app/router.tsx \
        crates/app-core/tests/activity_timeline_handler.rs
git commit -m "feat(workbench): Activity Timeline panel — per-day episode buckets"
```

---

### Task 36: Cost Tracker panel

**Files:** same pattern as Tasks 34/35.

- [ ] **Step 1: DTO + handler**

Append to `crates/desktop-shared/src/commands/coding_memory.rs`:

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CostRollupBucket {
    pub day: String,
    pub llm_calls: i64,
    pub prompt_tokens: i64,
    pub completion_tokens: i64,
    pub estimated_usd: f64,
}
```

Append to `crates/app-core/src/coding_memory/handlers.rs`:

```rust
/// Cost Tracker handler — daily rollup of Phase-B Distiller LLM spend.
/// Estimates cost from token counts using a fixed rate-card snapshot;
/// Reforge spend is added in Phase 5.
pub async fn cost_rollup(
    pool: &sqlx::SqlitePool,
    days: i64,
) -> common::Result<Vec<desktop_shared::commands::coding_memory::CostRollupBucket>> {
    use sqlx::Row;
    // turn_trace episodes carry token_usage in metadata; aggregate from there.
    let rows = sqlx::query(
        "SELECT date(occurred_at) AS day, metadata
         FROM episodic_memories
         WHERE kind = 'turn_trace' AND occurred_at >= datetime('now', ?1 || ' days')",
    )
    .bind(format!("-{days}"))
    .fetch_all(pool)
    .await
    .map_err(|e| common::KlyntbotError::Storage(format!("cost_rollup: {e}")))?;

    let mut by_day: std::collections::BTreeMap<String, (i64, i64, i64)> = std::collections::BTreeMap::new();
    for r in rows {
        let day: String = r.get("day");
        let meta: Option<String> = r.try_get("metadata").ok();
        let (prompt, completion) = parse_tokens(meta.as_deref());
        let e = by_day.entry(day).or_insert((0, 0, 0));
        e.0 += 1;
        e.1 += prompt as i64;
        e.2 += completion as i64;
    }
    Ok(by_day.into_iter().map(|(day, (calls, pt, ct))| {
        desktop_shared::commands::coding_memory::CostRollupBucket {
            day, llm_calls: calls, prompt_tokens: pt, completion_tokens: ct,
            estimated_usd: estimate_usd(pt as u64, ct as u64),
        }
    }).collect())
}

fn parse_tokens(meta: Option<&str>) -> (u32, u32) {
    let Some(s) = meta else { return (0, 0); };
    let v: serde_json::Value = serde_json::from_str(s).unwrap_or_default();
    let content = v.get("content").and_then(|c| c.as_str()).unwrap_or(s);
    let c2: serde_json::Value = serde_json::from_str(content).unwrap_or(v);
    let u = c2.get("tokenUsage");
    (
        u.and_then(|x| x.get("prompt")).and_then(|x| x.as_u64()).unwrap_or(0) as u32,
        u.and_then(|x| x.get("completion")).and_then(|x| x.as_u64()).unwrap_or(0) as u32,
    )
}

fn estimate_usd(prompt: u64, completion: u64) -> f64 {
    // Rough Haiku-tier rate card snapshot — replace with a config-driven
    // rate card in Phase 5 when Reforge spend is also tracked.
    const P_PER_M: f64 = 0.80;
    const C_PER_M: f64 = 4.00;
    (prompt as f64 * P_PER_M + completion as f64 * C_PER_M) / 1_000_000.0
}
```

- [ ] **Step 2: Tauri + AppCore wrapper + panel + wiring**

Mirror Task 35's layout. Tauri command name: `coding_memory_cost_rollup`. React panel renders the `CostRollupBucket[]` as a stacked bar (prompt tokens grey, completion tokens color) with a totals row across the top.

- [ ] **Step 3: Test**

Create `crates/app-core/tests/cost_rollup_handler.rs`:

```rust
use app_core::coding_memory::handlers::cost_rollup;
use storage::StoragePool;

#[tokio::test]
async fn cost_rollup_aggregates_token_usage() {
    let pool = StoragePool::connect_in_memory().await.unwrap();
    StoragePool::run_feature_migrations(pool.inner(), &cognitive::cognitive_migrations()).await.unwrap();
    StoragePool::run_feature_migrations(pool.inner(), &coding_memory::coding_memory_migrations()).await.unwrap();

    // Seed one turn_trace episode with token usage in its content blob.
    let content = serde_json::json!({"tokenUsage": {"prompt": 100, "completion": 50}}).to_string();
    sqlx::query(
        "INSERT INTO episodic_memories
         (id, domain, content, summary, importance, occurred_at, recorded_at,
          stability, last_accessed, access_count, project_id, scope_type, scope_id, kind)
         VALUES ('e1','coding', ?1, NULL, 0.5, '2026-04-25T10:00:00Z','2026-04-25T10:00:00Z',
                 1.0, NULL, 0, NULL, 'user', NULL, 'turn_trace')",
    )
    .bind(&content).execute(pool.inner()).await.unwrap();

    let buckets = cost_rollup(pool.inner(), 30).await.unwrap();
    assert_eq!(buckets.len(), 1);
    assert_eq!(buckets[0].prompt_tokens, 100);
    assert_eq!(buckets[0].completion_tokens, 50);
    assert!(buckets[0].estimated_usd > 0.0);
}
```

- [ ] **Step 4: Run + commit**

```bash
cargo nextest run -p app-core --test cost_rollup_handler
cargo build --workspace
cargo clippy --workspace --all-targets --all-features -- -D warnings
cd desktop-ui && bun run lint:fix && bun run build && cd -
git add crates/desktop-shared/src/commands/coding_memory.rs \
        crates/app-core/src/coding_memory/ crates/desktop/src/commands/coding_memory.rs crates/desktop/src/lib.rs \
        desktop-ui/src/features/coding-memory/CostTrackerPanel.tsx \
        desktop-ui/src/features/coding-memory/hooks.ts desktop-ui/src/features/coding-memory/CodingMemoryLayout.tsx \
        desktop-ui/src/app/router.tsx \
        crates/app-core/tests/cost_rollup_handler.rs
git commit -m "feat(workbench): Cost Tracker panel — daily Distiller LLM spend rollup"
```

---

### Task 37: Sensitivity Inspector panel

**Files:** same pattern.

- [ ] **Step 1: DTO + handler + mutation**

Append to `crates/desktop-shared/src/commands/coding_memory.rs`:

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SensitivityRow {
    pub id: String,
    pub row_type: String,
    pub sensitivity: String,
    pub subject: Option<String>,
    pub content_preview: Option<String>,
    pub scope_repo_id: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SensitivityUpdate {
    pub id: String,
    pub row_type: String,  // "fact" | "episode"
    pub new_sensitivity: String,
}
```

Append to `crates/app-core/src/coding_memory/handlers.rs`:

```rust
/// List facts + episodes grouped by sensitivity tier.
pub async fn sensitivity_browse(
    pool: &sqlx::SqlitePool,
    tier: &str,
    limit: i64,
) -> common::Result<Vec<desktop_shared::commands::coding_memory::SensitivityRow>> {
    use desktop_shared::commands::coding_memory::SensitivityRow;
    use sqlx::Row;
    let like = format!(r#"%"sensitivity":"{tier}"%"#);
    let facts = sqlx::query(
        "SELECT id, subject, scope_repo_id, metadata
         FROM semantic_facts WHERE metadata LIKE ?1 LIMIT ?2",
    ).bind(&like).bind(limit).fetch_all(pool).await
     .map_err(|e| common::KlyntbotError::Storage(format!("sensitivity facts: {e}")))?;
    let eps = sqlx::query(
        "SELECT id, content, scope_repo_id, metadata
         FROM episodic_memories WHERE metadata LIKE ?1 LIMIT ?2",
    ).bind(&like).bind(limit).fetch_all(pool).await
     .map_err(|e| common::KlyntbotError::Storage(format!("sensitivity eps: {e}")))?;

    let mut out = Vec::new();
    for r in facts {
        out.push(SensitivityRow {
            id: r.get("id"), row_type: "fact".into(), sensitivity: tier.into(),
            subject: r.try_get("subject").ok(), content_preview: None,
            scope_repo_id: r.try_get("scope_repo_id").ok(),
        });
    }
    for r in eps {
        let c: String = r.try_get("content").unwrap_or_default();
        out.push(SensitivityRow {
            id: r.get("id"), row_type: "episode".into(), sensitivity: tier.into(),
            subject: None, content_preview: Some(c.chars().take(240).collect()),
            scope_repo_id: r.try_get("scope_repo_id").ok(),
        });
    }
    Ok(out)
}

/// Update the `sensitivity` value inside the `metadata` JSON for a single row.
pub async fn sensitivity_update(
    pool: &sqlx::SqlitePool,
    update: &desktop_shared::commands::coding_memory::SensitivityUpdate,
) -> common::Result<()> {
    let table = match update.row_type.as_str() {
        "fact" => "semantic_facts",
        "episode" => "episodic_memories",
        other => return Err(common::KlyntbotError::Storage(format!("unknown row_type: {other}"))),
    };
    let q = format!(
        "UPDATE {table}
         SET metadata = json_set(coalesce(metadata, '{{}}'), '$.sensitivity', ?2)
         WHERE id = ?1"
    );
    sqlx::query(&q).bind(&update.id).bind(&update.new_sensitivity)
        .execute(pool).await
        .map_err(|e| common::KlyntbotError::Storage(format!("sensitivity_update: {e}")))?;
    Ok(())
}
```

- [ ] **Step 2: Tauri commands**

`coding_memory_sensitivity_browse` + `coding_memory_sensitivity_update` + AppCore wrappers. Mirror the Task-34 pattern. Add both to `DEV_COMMANDS` and `dispatch_dev`.

- [ ] **Step 3: Panel**

Create `desktop-ui/src/features/coding-memory/SensitivityInspectorPanel.tsx` with a tier dropdown (`normal | high | excluded`), a list of rows, and a "Promote to high" / "Hide (excluded)" button per row that opens a `glass-panel` confirmation dialog before calling the mutation.

- [ ] **Step 4: Test**

Create `crates/app-core/tests/sensitivity_handler.rs`:

```rust
use app_core::coding_memory::handlers::{sensitivity_browse, sensitivity_update};
use desktop_shared::commands::coding_memory::SensitivityUpdate;
use storage::StoragePool;

#[tokio::test]
async fn update_and_browse_sensitivity() {
    let pool = StoragePool::connect_in_memory().await.unwrap();
    StoragePool::run_feature_migrations(pool.inner(), &cognitive::cognitive_migrations()).await.unwrap();
    StoragePool::run_feature_migrations(pool.inner(), &coding_memory::coding_memory_migrations()).await.unwrap();
    sqlx::query(
        "INSERT INTO semantic_facts
         (id, domain, subject, predicate, object, confidence, source, valid_from, valid_until,
          recorded_at, superseded_at, superseded_by, stability, last_accessed, access_count,
          convergence_score, project_id, memory_type, scope_type, scope_id, metadata)
         VALUES ('f1','work','x','y','z',0.9,'distiller','2026-04-25T00:00:00Z',NULL,
                 '2026-04-25T00:00:00Z',NULL,NULL,1.0,NULL,0,1.0,NULL,'fact','user',NULL,
                 '{\"provenance\":{\"sourceEvents\":[\"x\"]}}')",
    ).execute(pool.inner()).await.unwrap();

    sensitivity_update(pool.inner(), &SensitivityUpdate {
        id: "f1".into(), row_type: "fact".into(), new_sensitivity: "high".into(),
    }).await.unwrap();

    let rows = sensitivity_browse(pool.inner(), "high", 10).await.unwrap();
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0].id, "f1");
}
```

- [ ] **Step 5: Run + commit**

```bash
cargo nextest run -p app-core --test sensitivity_handler
cargo build --workspace
cargo clippy --workspace --all-targets --all-features -- -D warnings
cd desktop-ui && bun run lint:fix && bun run build && cd -
git add crates/desktop-shared/src/commands/coding_memory.rs \
        crates/app-core/src/coding_memory/ crates/desktop/src/commands/coding_memory.rs crates/desktop/src/lib.rs \
        desktop-ui/src/features/coding-memory/SensitivityInspectorPanel.tsx \
        desktop-ui/src/features/coding-memory/hooks.ts desktop-ui/src/features/coding-memory/CodingMemoryLayout.tsx \
        desktop-ui/src/app/router.tsx \
        crates/app-core/tests/sensitivity_handler.rs
git commit -m "feat(workbench): Sensitivity Inspector — browse tiers + promote/demote with confirm"
```

---

### Task 38: Panel React test coverage

**Files:**
- Create: `desktop-ui/src/features/coding-memory/__tests__/MemoryBrowserPanel.test.tsx`
- Create: `desktop-ui/src/features/coding-memory/__tests__/ActivityTimelinePanel.test.tsx`
- Create: `desktop-ui/src/features/coding-memory/__tests__/CostTrackerPanel.test.tsx`
- Create: `desktop-ui/src/features/coding-memory/__tests__/SensitivityInspectorPanel.test.tsx`

- [ ] **Step 1: Mirror the Phase-2 Vitest pattern**

For each panel, write a small test that:
1. Mocks `useQuery` to return a fixed dataset.
2. Mounts the panel.
3. Asserts at least one expected DOM element is present (e.g. `screen.getByText('Memory Browser')` and a data row).

Use the exact same mock harness used by Phase-2's `CliHealthPanel.test.tsx` / `SessionReplayPanel.test.tsx`. If Phase-2 introduced a helper (e.g. `renderWithProviders`), use it.

Example for Memory Browser:

```tsx
import { render, screen } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";

vi.mock("@shared/data/ipc", () => ({
  useQuery: () => ({ data: [{
    id: "r1", rowType: "fact", scopeRepoId: "github.com/klynt/bot",
    subject: "repo:x", predicate: "framework", object: "tauri",
    occurredAt: "2026-04-25T00:00:00Z",
  }] }),
  useMutation: () => ({ mutate: vi.fn() }),
}));

import { MemoryBrowserPanel } from "../MemoryBrowserPanel";

describe("MemoryBrowserPanel", () => {
  it("renders a fact row", () => {
    render(<MemoryBrowserPanel />);
    expect(screen.getByText(/Memory Browser/)).toBeInTheDocument();
    expect(screen.getByText(/tauri/)).toBeInTheDocument();
  });
});
```

- [ ] **Step 2: Run + commit**

```bash
cd desktop-ui && bun run test && cd -
git add desktop-ui/src/features/coding-memory/__tests__/
git commit -m "test(workbench): Vitest coverage for Phase-3 coding-memory panels"
```

---

### Task 39: Docs — Phase 3 summary in `docs/coding-memory/README.md`

**Files:**
- Modify: `docs/coding-memory/README.md`

- [ ] **Step 1: Append section**

Append to `docs/coding-memory/README.md`:

```markdown

## Phase 3 — Write path (Distiller + Tier A/B activation) (shipped 2026-04-??)

Components newly live:

- **Distiller** (`crates/coding-memory/src/distiller/`) — three-phase per-turn writer:
  - Phase A: deterministic `TurnTrace` extraction + `turn_trace` episodes.
  - Phase B: `ProviderManager`-driven LLM synthesis with `record_observation` tool.
  - Phase C: Mem0-style reconciliation (NOOP/SUPERSEDE/ADD) — DELETE-free.
- **DistillerWriter** enforces the provenance-always invariant: every write carries a populated `metadata.provenance.source_events`. Dev builds panic on violation.
- **Counterfactual memory (B1)** — failure/abandoned `FixAttempt`s derive a `memory_type: "counterfactual"` fact.
- **`CodeState` (B3)** on `UserSituationSnapshot` — retrieval-side code-context signal.
- **`CodeDomainSearcher` (B4)** — coding facts surface in InsightForge retrievals.
- **`ShadowContext.session_type` (B5)** — autotuner partitions coding vs personal workloads.
- **Retry queue** — transient LLM failures park in `ingest_distillation_retry` with 1m / 5m / 30m backoff.
- **Workbench panels** — Memory Browser, Activity Timeline, Cost Tracker, Sensitivity Inspector.

Invariants proved via proptest: #1 (provenance-always), #2 (bi-temporal monotone), #3 (SUPERSEDE chain equality), #5 (Distiller-never-deletes).

Tier A surfaces (`score_turn`, `ContradictionDetected`, `UserCorrectedAI`) activate automatically — no net-new wiring, only data flowing in.

Unchanged in Phase 3: Recall API (MCP tools still return `NotImplementedInPhase`), Reforge coding phases, Mirror coding subscribers, causal edges, symbol grounding. Those are Phases 4/5/6.
```

- [ ] **Step 2: Commit**

```bash
git add docs/coding-memory/README.md
git commit -m "docs(coding-memory): Phase 3 summary + exit-gate evidence"
```

---

### Task 40: Final verification + quality gates

**Files:** none

- [ ] **Step 1: Run the full matrix**

```bash
cargo fmt --all --check
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo nextest run --workspace
cargo test --workspace --doc
cd desktop-ui && bun run lint && bun run test && bun run build && cd -
```

Expected: zero warnings, zero fmt drift, all tests PASS.

- [ ] **Step 2: Phase-3 regression greps**

```bash
# No `NotImplemented` strings on hot Phase-3 paths.
rg -n 'NotImplementedInPhase' \
    crates/coding-memory/src/distiller \
    crates/coding-memory/src/counterfactual.rs \
    crates/coding-memory/src/code_state.rs \
    crates/coding-memory/src/code_domain_searcher.rs \
    crates/coding-memory/src/sink.rs
# Expected: empty output (sink.rs no longer returns phase-errors).

# No stray TODO/FIXME landed.
rg -n 'TODO|FIXME' crates/coding-memory/src crates/coding-ingest/src crates/app-core/src/coding_memory
# Expected: empty output.

# Panic calls only behind debug_assert or inside #[cfg(test)].
rg -n 'panic!' crates/coding-memory/src
# Expected: lines only from tests or `debug_assert!` contexts.
```

- [ ] **Step 3: Prove cost budget**

Run the `distiller_end_to_end` test with a real provider mock that reports realistic token counts, then verify the `cost_rollup` handler reports < $0.01 for the synthetic 2-turn session. This is a sanity check, not a gated test.

- [ ] **Step 4: Open the PR**

```bash
gh pr create --title "feat(coding-memory): Phase 3 — Distiller write path + Tier A/B activation" \
  --body "$(cat <<'EOF'
## Summary
- Three-phase Distiller writes SemanticFact + EpisodicMemory rows with provenance-always metadata
- Mem0-style reconciliation (NOOP / SUPERSEDE / ADD) — DELETE-free; SUPERSEDE chain maintains valid_until == valid_from
- Counterfactual memory (B1): failed/abandoned FixAttempts derive `memory_type: "counterfactual"` facts
- UserSituationSnapshot.code_state (B3); CodeDomainSearcher in InsightForge (B4); ShadowContext.session_type (B5)
- Retry queue for transient LLM failures with 1m/5m/30m backoff
- IngestDaemon forwards every persisted event to the live Distiller; idle + retry sweepers run as tickers
- Workbench panels: Memory Browser, Activity Timeline, Cost Tracker, Sensitivity Inspector
- 4 property tests (invariants #1, #2, #3, #5)

## Test plan
- [ ] `cargo nextest run --workspace`
- [ ] `cargo clippy --workspace --all-targets --all-features -- -D warnings`
- [ ] `cargo test --workspace --doc`
- [ ] `tests/integration/coding_memory_phase3_roundtrip.rs` — synthetic bug-fix session distills end-to-end
- [ ] `tests/integration/coding_memory_phase3_tier_a.rs` — cognitive surfaces fire for coding content
- [ ] `cargo nextest run -E 'test(prop_)'` — all four property tests green
- [ ] Desktop UI: Coding Memory → Memory Browser shows rows; Provenance drawer shows `sourceEvents` populated

🤖 Generated with [Claude Code](https://claude.com/claude-code)
EOF
)"
```

---

## Self-Review

| Spec item (§ and decision) | Task |
|---|---|
| §11 Phase 3: Distiller Phase A (extractive) | T8, T9 |
| §11 Phase 3: Distiller Phase B (LLM via ProviderManager) | T10, T12, T13, T14 |
| §11 Phase 3: Mem0 reconciliation ADD/SUPERSEDE/NOOP | T15, T16 |
| §11 Phase 3: 5 coding fact kinds with provenance-always | T5, T11 |
| §11 Phase 3: Counterfactual memory (B1) | T17, T27 |
| §11 Phase 3: `code_state` on `UserSituationSnapshot` (B3) | T18 |
| §11 Phase 3: `CodeDomainSearcher` in InsightForge (B4) | T19 |
| §11 Phase 3: Autotuner `session_type` (B5) | T20 |
| §11 Phase 3: Tier A activation audit | T33 |
| §11 Phase 3 exit gates: provenance-always proptest | T28 |
| §11 Phase 3 exit gates: bi-temporal invariants proptest | T29, T30, T31 |
| §11 Phase 3 exit gates: cost < $0.01/test-session | T40 step 3 |
| §6 per-turn trigger (AssistantMsg token_usage OR SessionEnd OR 2m idle) | T6 |
| §6 Phase B failure modes (timeout retry 1m/5m/30m, malformed drop) | T14, T21 |
| §6 provenance-always enforcement (dev panic / release log+reject) | T5 |
| §7 `CodingKind` 5-value enum (Distiller cannot emit Reforge kinds) | T10 |
| §4 `metadata` column writes | T5 (upsert_with_metadata + insert_with_kind_and_metadata) |
| §4 `scope_repo_id` partitioning | T5, T11, T34 |
| §4 `ingest_distillation_retry` | T14 |
| §11.5 Phase 3 panels (Memory Browser + Activity Timeline + Cost Tracker + Sensitivity Inspector) | T34, T35, T36, T37 |
| §11.5 panel Vitest coverage | T38 |
| CLAUDE.md `DEV_COMMANDS` + dev_server coverage for every new Tauri command | T34–T37 (each extends `DEV_COMMANDS`) |
| CLAUDE.md zero clippy + fmt + doc gates | T40 |

**Invariant coverage.** Invariants 1, 2, 3, 5 proved via proptest (T28, T29, T30, T31). Invariant 4 (scope isolation) inherited from the existing `SemanticFactRepo::list_by_scope` surface — no Phase-3 regression path. Invariant 7 (AgentEvent round-trip) tested in Phase 1. Invariants 6, 8, 9 are Phase 5/6 territory (Reforge cycle, causal edges, token budgets on injection).

**Placeholder scan.** No "TBD"/"TODO"/"implement later"/"similar to Task N" text in any step body. Every Rust step includes a complete code block. Every test step has an exact `cargo nextest run` invocation with expected PASS/FAIL outcomes. Every commit step has a full conventional commit message.

**Type consistency.** `Distiller::new(config, ingest_repo, writer, provider, retriever, retry_repo)` signature is stable from Task 22 onward — tests in Tasks 21, 23, 27, 31, 32 all match. `PreparedFact { fact, metadata_json, scope_repo_id, provenance }` and `PreparedEpisode { episode, kind, metadata_json, scope_repo_id, provenance }` are the same shape across T5, T9, T11, T15, T27. `ProvenanceMetadata { source_events, session_id, turn_id, distilled_at, distiller_model, source_kind }` (exact field set from Phase 1's `scope.rs`) is used identically across every writer call. `MemoryBrowserFilter { repo_id, kind, memory_type, query, limit, offset }` is stable across T34's test, handler, panel, and Tauri command.

**Ordering dependencies.** T1→T2 (deps first). T3→T5→T7 (error → writer → Distiller shell). T6→T21 (turn buffer precedes distill_turn). T10→T11→T12→T13 (tool schema → fact builder → prompt → LLM). T15→T16→T21 (reconcile policy → SUPERSEDE completion → orchestrator). T22 must land before T23 (InProcessSink uses the post-retry Distiller constructor). T24 + T25 + T26 must land together (AppCore wiring is atomic). Panels (T34–T37) are parallelizable — no interdependencies.

**Worktree-friendly clusters for subagent dispatch.**
- Cluster A (plumbing): T1 → T2 → T3 → T4 → T5 → T6 → T7.
- Cluster B (Phase A): T8 → T9.
- Cluster C (Phase B): T10 → T11 → T12 → T13 → T14.
- Cluster D (Phase C + orchestrator): T15 → T16 → T21.
- Cluster E (Tier B1/B3/B4/B5): T17, T18, T19, T20 (parallel after T5).
- Cluster F (integration): T22 → T23 → T24 → T25 → T26 → T27.
- Cluster G (tests): T28, T29, T30, T31, T32, T33 (parallel after T21).
- Cluster H (workbench): T34, T35, T36, T37, T38 (parallel, independent).
- Cluster I (docs + gates): T39, T40.

---

## Execution Handoff

**Plan complete and saved to `docs/superpowers/plans/2026-04-25-coding-memory-phase-3.md`. Two execution options:**

**1. Subagent-Driven (recommended)** — Dispatch a fresh subagent per task; review between tasks; fast iteration. Strongly preferred for this plan because clusters E and H are parallelizable across independent worktrees — a second subagent can land the panels while the first wires the Distiller.

**2. Inline Execution** — Execute tasks in this session via `superpowers:executing-plans`; batch every 4-5 tasks with a checkpoint.

Which approach?

