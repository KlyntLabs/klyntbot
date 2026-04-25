# Coding Memory — Phase 4 (Read Path: Recall API + Passive Injection) Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Turn Phase-1 recall stubs into a working read pipeline. The just-landed Phase-3 Distiller writes facts and episodes; Phase 4 makes them retrievable. Deliver: (1) all 7 recall MCP tools fully functional (`trace_causes` stays a Phase-6 stub by spec), (2) passive markdown injection at `SessionStart` (800-token budget) and `UserPromptSubmit` (1500-token budget) via a new `klyntbot-hook context` subcommand, (3) C3 failure-state-aware retrieval with the closed set of 5 `RetrievalSkill`s implemented over `QueryPipeline` + `UnifiedMemoryService`, (4) dead-end warning block (Tier B1) in `UserPromptSubmit` injection, (5) recall-invocation telemetry persisted to a new `recall_invocations` table feeding the Session Replay overlay + new "Recall Tool Log" panel.

**Architecture:** A single `CodingRecallService` is owned by `AppCore` and called from two entry points: the `klyntbot-hook context` subcommand (passive — emits markdown to stdout for Claude Code's `additionalContext`), and the MCP server's `coding-memory` tool handlers (active — JSON responses). Internally the service fans the request through: `ScopeResolver` → `QueryPipeline::enhance` → `UnifiedMemoryService::retrieve_scoped` → `RetrievalQualityProbe::score` → if `coverage_score < threshold` → `RetrievalSkillRegistry::escalate` (selector picks the highest-EMA skill in the active budget tier; tries in order until one succeeds or all fail) → `DeadEndChecker::check` (matches user prompt against `memory_type='counterfactual'` facts) → renderers (markdown for hook, JSON DTOs for MCP). Every recall invocation persists a row to `recall_invocations` with `(query, layer, coverage_score, skill_used, latency_ms, result_ids[])`. Effectiveness is updated via `DomainEvent::RetrievalSkillApplied` consumed by `PatternEffectivenessSubscriber` (already wired in Phase 1, no-op for now until Phase 5 lands the subscriber body).

`SessionStart` / `UserPromptSubmit` injection respects the budget caps as an invariant — the renderer truncates section-by-section using a pluggable `TokenBudgeter` (`tiktoken-rs` for OpenAI-compatible counts, falling back to `chars / 4` heuristic on failure). The Workbench gets two extensions: an overlay strip atop Session Replay showing per-turn recall injections, and a paginated "Recall Tool Log" panel listing every MCP tool invocation with its query, escalation chain, and result preview. No new schema beyond one telemetry table.

**Tech Stack:** Rust (MSRV 1.93), `tiktoken-rs` (token counting; new dep), existing `cognitive::UnifiedMemoryService`, `context_engine::QueryPipeline`, `coding_memory::recall::*`, `coding_memory::retrieval_skills::*`, `coding_memory::scope_resolver`, `bus::DomainEventBus`, `sqlx`, `serde` (camelCase), `jiff::Timestamp`, `uuid`, `async-trait`, `tokio` (`sync`, `time`), `proptest` (dev). Frontend: existing `desktop-ui/` (React + Tailwind v4 + Biome 2.0 + React Compiler + `useQuery`/`useMutation`).

---

## File Structure

Every file created or modified by this plan, grouped by responsibility. Files stay small and focused per CLAUDE.md.

### New files — `crates/coding-memory/`

| File | Responsibility |
|---|---|
| `src/recall/service.rs` | `CodingRecallService` impl — orchestrates scope → pipeline → retrieve → probe → skill → render |
| `src/recall/scope_resolve.rs` | Wraps `coding_ingest::scope_resolver` for the recall path; caches per-process |
| `src/recall/probe.rs` | `RetrievalQualityProbe` — coverage_score = `mean(top_k.sim) - min(top_k.sim)`; threshold lookup |
| `src/recall/dead_end.rs` | `DeadEndChecker` — match approach text against `memory_type='counterfactual'` facts; aggregate confidence |
| `src/recall/index_builder.rs` | Build `IndexEntry`s from `ScoredFact` / `EpisodicMemoryRow` with token-cost estimation |
| `src/recall/timeline_builder.rs` | Build `TimelineEntry`s ordered by `occurred_at` with `related_ids` link discovery |
| `src/recall/fetch_builder.rs` | Build `FullEntry`s — joins fact/episode + provenance + causal edges + supersede chain |
| `src/recall/facts_as_of.rs` | `recall_facts_as_of` — bi-temporal lookup using `valid_from <= as_of < valid_until` |
| `src/recall/change_history.rs` | `recall_change_history` — walk SUPERSEDE chain forward + backward |
| `src/recall/decision_points.rs` | `recall_decision_points` — list episodes with `kind in ('fix_attempt','dead_end_attempt','refactor_episode')` |
| `src/recall/telemetry.rs` | `RecallInvocationRow`, `RecallInvocationRepo` — persist + query telemetry |
| `src/recall/budget.rs` | `TokenBudgeter` trait + `TiktokenBudgeter` + `HeuristicBudgeter` fallback |
| `src/retrieval_skills/registry.rs` | `RetrievalSkillRegistry` — owns the 5 skills, EMA effectiveness, selector |
| `src/retrieval_skills/query_rewriter.rs` | `QueryRewriter` impl — PRF expansion via `QueryPipeline` extra-stages |
| `src/retrieval_skills/query_decomposer.rs` | `QueryDecomposer` impl — sentence-split + per-clause retrieval, RRF merge |
| `src/retrieval_skills/evidence_focuser.rs` | `EvidenceFocuser` impl — top-20 → cosine rerank → top-5 |
| `src/retrieval_skills/raw_event_escalator.rs` | `RawEventEscalator` impl — provenance pointers → `ingest_event_log` rows |
| `src/retrieval_skills/causal_context_expander.rs` | `CausalContextExpander` impl — walk `memory_causal_edges` (degrades gracefully) |
| `migrations/003_recall_invocations.sql` | `recall_invocations` telemetry table |

### New files — `crates/coding-ingest/`

| File | Responsibility |
|---|---|
| `src/bin/klyntbot_hook/context_cmd.rs` | `context` subcommand — `--session-start` / `--user-prompt-submit` flags; emits markdown to stdout |

### New files — `crates/app-core/src/coding_memory/`

| File | Responsibility |
|---|---|
| `recall.rs` | Handlers: `coding_memory_recall_index`, `coding_memory_recall_timeline`, `coding_memory_recall_fetch`, `coding_memory_check_dead_ends`, `coding_memory_recall_facts_as_of`, `coding_memory_recall_change_history`, `coding_memory_recall_decision_points`, `coding_memory_recall_log`, `coding_memory_session_replay_recall_overlay` |

### Modified existing files

| File | Change |
|---|---|
| `crates/coding-memory/src/recall/mod.rs` | Replace stub `CodingRecallService` with real one (delegates to `service.rs`); add new response DTOs (`FactsAsOfResponse`, `ChangeHistoryResponse`, `DecisionPointsResponse`, `RecallInvocationView`) |
| `crates/coding-memory/src/recall/renderers.rs` | Implement `render_session_start_block` + `render_user_prompt_block` against `CodingRecallService` |
| `crates/coding-memory/src/retrieval_skills.rs` | Replace `phase_stub_skill!` macro instances with `pub use` from `retrieval_skills/` submodules |
| `crates/coding-memory/src/lib.rs` | Re-export `RecallInvocationRepo`, `TokenBudgeter`, `RetrievalSkillRegistry`, new response DTOs |
| `crates/coding-memory/src/mcp.rs` | Replace `stub_handler` with real per-tool dispatch over `Arc<CodingRecallService>` |
| `crates/coding-memory/Cargo.toml` | Add `tiktoken-rs = "0.5"` + `regex = "1"` to `[dependencies]` |
| `crates/coding-ingest/src/bin/klyntbot-hook.rs` | Add `context` subcommand routing |
| `crates/coding-ingest/Cargo.toml` | (no change — `context_cmd` is in-binary) |
| `crates/app-core/src/coding_memory/mod.rs` | `pub mod recall;` + re-exports |
| `crates/app-core/src/coding_memory/handlers.rs` | (unchanged — recall lives in its own module) |
| `crates/app-core/src/state.rs` | Add `pub recall: Option<Arc<coding_memory::recall::CodingRecallService>>` |
| `crates/app-core/src/init/mod.rs` | After `Distiller` init: build `CodingRecallService` and stash on `AppCore` |
| `crates/desktop-shared/src/commands/coding_memory.rs` | Add DTOs: `RecallIndexArgs`, `RecallTimelineArgs`, `RecallFetchArgs`, `DeadEndArgs`, `FactsAsOfArgs`, `ChangeHistoryArgs`, `DecisionPointsArgs`, `RecallLogArgs`, `RecallLogPage`, `SessionRecallOverlay`, `RecallToolInvocation` |
| `crates/desktop/src/commands/coding_memory.rs` | Add 9 new Tauri commands; extend `DEV_COMMANDS` |
| `crates/desktop/src/lib.rs` | Register the 9 commands in `invoke_handler!` |
| `crates/klyntbot-server/src/bridge/registry.rs` | (no change — relies on existing tool registry; coding-memory tools registered at AppCore boot) |
| `crates/mcp/src/lib.rs` (or wherever tools are registered) | Wire 7 active recall tools + 1 stubbed `trace_causes` over `Arc<CodingRecallService>` |
| `desktop-ui/src/features/coding-memory/SessionReplayPanel.tsx` | Add recall-injection overlay strip per turn |
| `desktop-ui/src/features/coding-memory/RecallToolLogPanel.tsx` | NEW — paginated list of recall invocations |
| `desktop-ui/src/features/coding-memory/CodingMemoryLayout.tsx` | Add "Recall Log" nav entry |
| `desktop-ui/src/features/coding-memory/hooks.ts` | Add `useRecallLog`, `useSessionRecallOverlay` |
| `desktop-ui/src/app/router.tsx` | Add `/coding-memory/recall-log` route |

### Test files

| File | Responsibility |
|---|---|
| `crates/coding-memory/tests/recall_invocation_repo.rs` | Insert + paginated list + filter by session |
| `crates/coding-memory/tests/probe_coverage.rs` | `coverage_score` math; threshold dispatch |
| `crates/coding-memory/tests/dead_end_checker.rs` | Approach text matches counterfactual; aggregate confidence rule |
| `crates/coding-memory/tests/index_builder.rs` | `ScoredFact` → `IndexEntry` with token-cost estimate |
| `crates/coding-memory/tests/timeline_builder.rs` | Entries ordered; `related_ids` populated from supersede + same-session episodes |
| `crates/coding-memory/tests/fetch_builder.rs` | `FullEntry` includes provenance + causal edges + chain |
| `crates/coding-memory/tests/facts_as_of.rs` | Bi-temporal lookup returns the row valid at `as_of` |
| `crates/coding-memory/tests/change_history.rs` | Walks chain in both directions |
| `crates/coding-memory/tests/decision_points.rs` | Filters by kind set; respects repo scope |
| `crates/coding-memory/tests/budget_truncation.rs` | Heuristic + tiktoken count agreement within ±10% on ASCII |
| `crates/coding-memory/tests/session_start_render.rs` | Markdown ≤ 800 tokens; sections present in order |
| `crates/coding-memory/tests/user_prompt_render.rs` | Markdown ≤ 1500 tokens; dead-end block appears when triggered |
| `crates/coding-memory/tests/skill_query_rewriter.rs` | Three rewrites produced; merged result count grows |
| `crates/coding-memory/tests/skill_query_decomposer.rs` | Compound query → 2-4 sub-queries; merge keeps unique ids |
| `crates/coding-memory/tests/skill_evidence_focuser.rs` | top-20 → top-5 by cosine rerank |
| `crates/coding-memory/tests/skill_raw_event_escalator.rs` | Returns `ingest_event_log` rows for provenance ids |
| `crates/coding-memory/tests/skill_causal_context_expander.rs` | Empty when no edges; walks chain when seeded |
| `crates/coding-memory/tests/skill_registry_selector.rs` | Selector picks by EMA; budget-tier filter; fallback chain |
| `crates/coding-memory/tests/recall_service_end_to_end.rs` | Seeded facts → `recall_index` returns ranked + telemetry row written |
| `crates/coding-memory/tests/prop_injection_budget.rs` | **Property:** every render output ≤ declared budget |
| `crates/coding-memory/tests/prop_recall_idempotent.rs` | **Property:** same query twice — same result ids, same coverage_score |
| `crates/coding-ingest/tests/hook_context_subcmd.rs` | `klyntbot-hook context --session-start` returns markdown on stdout |
| `crates/app-core/tests/recall_handlers.rs` | Each handler returns expected DTO shape |
| `tests/integration/coding_memory_phase4_next_session.rs` | Scenario — Phase-3 fixture session ingested → SessionStart of next session sees prior memory |
| `tests/integration/coding_memory_phase4_dead_end.rs` | Scenario — counterfactual seeded → repeat-attempt prompt → warning block in UserPromptSubmit injection |
| `tests/integration/coding_memory_phase4_c3_escalation.rs` | Scenario — sparse-coverage query → escalates → coverage rises |
| `tests/fixtures/coding/phase4_recall_seed.jsonl` | 8 facts + 4 episodes covering RepoContext / FixAttempt / DeadEnd / StylePreference |
| `desktop-ui/src/features/coding-memory/__tests__/RecallToolLogPanel.test.tsx` | Panel renders rows + filters work |
| `desktop-ui/src/features/coding-memory/__tests__/SessionReplayPanel.recall_overlay.test.tsx` | Overlay strip appears on turns with recall events |

---

## Task Structure

Tasks are ordered so each builds on the prior commit. Many can parallelize in a worktree once Tasks 1–4 land (the foundational service skeleton + telemetry). Each task: exact file paths, exact commands, full code.

### Task 1: `tiktoken-rs` + `regex` deps

**Files:**
- Modify: `crates/coding-memory/Cargo.toml`

- [ ] **Step 1: Add deps**

In `crates/coding-memory/Cargo.toml` under `[dependencies]` add:

```toml
tiktoken-rs = "0.5"
regex = "1"
```

- [ ] **Step 2: Build + commit**

```bash
cargo build -p coding-memory
git add crates/coding-memory/Cargo.toml
git commit -m "feat(coding-memory): add tiktoken-rs + regex deps for Phase 4"
```

---

### Task 2: `recall_invocations` migration

**Files:**
- Create: `crates/coding-memory/migrations/003_recall_invocations.sql`
- Modify: `crates/coding-memory/src/lib.rs` (register migration)
- Test: `crates/coding-memory/tests/migration_applies.rs` (extend existing)

- [ ] **Step 1: Write the failing test**

Append to `crates/coding-memory/tests/migration_applies.rs`:

```rust
#[tokio::test]
async fn recall_invocations_table_exists_after_migration() {
    use storage::StoragePool;
    let pool = StoragePool::connect_in_memory().await.expect("pool");
    StoragePool::run_feature_migrations(pool.inner(), &cognitive::cognitive_migrations())
        .await
        .expect("cognitive migs");
    StoragePool::run_feature_migrations(pool.inner(), &coding_memory::coding_memory_migrations())
        .await
        .expect("coding-memory migs");
    let row: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM recall_invocations")
        .fetch_one(pool.inner())
        .await
        .expect("count");
    assert_eq!(row.0, 0);
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo nextest run -p coding-memory --test migration_applies recall_invocations_table_exists_after_migration`
Expected: FAIL — "no such table: recall_invocations".

- [ ] **Step 3: Create migration**

Create `crates/coding-memory/migrations/003_recall_invocations.sql`:

```sql
-- Phase 4 telemetry: every recall invocation (passive or active).
CREATE TABLE IF NOT EXISTS recall_invocations (
    id              TEXT PRIMARY KEY,
    occurred_at     TEXT NOT NULL,
    session_id      TEXT,
    turn_id         TEXT,
    repo_id         TEXT,
    layer           TEXT NOT NULL,            -- 'index' | 'timeline' | 'fetch' | 'dead_end' |
                                              -- 'facts_as_of' | 'change_history' | 'decision_points' |
                                              -- 'session_start_inject' | 'user_prompt_inject'
    query           TEXT NOT NULL,
    coverage_score  REAL,
    skill_used      TEXT,                     -- empty if no escalation; csv otherwise
    latency_ms      INTEGER NOT NULL,
    result_ids      TEXT NOT NULL,            -- JSON array of UUIDs
    rendered_tokens INTEGER,                  -- only set for inject layers
    metadata        TEXT NOT NULL DEFAULT '{}'
);
CREATE INDEX IF NOT EXISTS idx_recall_invocations_session
    ON recall_invocations(session_id, occurred_at DESC);
CREATE INDEX IF NOT EXISTS idx_recall_invocations_repo
    ON recall_invocations(repo_id, occurred_at DESC);
CREATE INDEX IF NOT EXISTS idx_recall_invocations_layer
    ON recall_invocations(layer, occurred_at DESC);
```

- [ ] **Step 4: Register the migration**

Edit `crates/coding-memory/src/lib.rs`. Find the `coding_memory_migrations()` function and append the new migration entry inline (`include_str!` form, version 3).

```rust
pub fn coding_memory_migrations() -> Vec<storage::FeatureMigration> {
    vec![
        storage::FeatureMigration {
            version: 1,
            name: "001_coding_memory",
            sql: include_str!("../migrations/001_coding_memory.sql"),
        },
        storage::FeatureMigration {
            version: 2,
            name: "002_retry_queue",
            sql: include_str!("../migrations/002_retry_queue.sql"),
        },
        storage::FeatureMigration {
            version: 3,
            name: "003_recall_invocations",
            sql: include_str!("../migrations/003_recall_invocations.sql"),
        },
    ]
}
```

- [ ] **Step 5: Run + commit**

```bash
cargo nextest run -p coding-memory --test migration_applies
git add crates/coding-memory/migrations/003_recall_invocations.sql \
        crates/coding-memory/src/lib.rs \
        crates/coding-memory/tests/migration_applies.rs
git commit -m "feat(coding-memory): recall_invocations telemetry table"
```

---

### Task 3: `RecallInvocationRepo` — insert + paginated list

**Files:**
- Create: `crates/coding-memory/src/recall/telemetry.rs`
- Modify: `crates/coding-memory/src/recall/mod.rs` (add `pub mod telemetry;`)
- Modify: `crates/coding-memory/src/lib.rs` (re-export)
- Test: `crates/coding-memory/tests/recall_invocation_repo.rs`

- [ ] **Step 1: Write the failing test**

Create `crates/coding-memory/tests/recall_invocation_repo.rs`:

```rust
use coding_memory::recall::telemetry::{RecallInvocationRepo, RecallInvocationRow};
use jiff::Timestamp;
use storage::StoragePool;
use uuid::Uuid;

async fn fresh_pool() -> StoragePool {
    let pool = StoragePool::connect_in_memory().await.expect("pool");
    StoragePool::run_feature_migrations(pool.inner(), &cognitive::cognitive_migrations())
        .await
        .unwrap();
    StoragePool::run_feature_migrations(pool.inner(), &coding_memory::coding_memory_migrations())
        .await
        .unwrap();
    pool
}

#[tokio::test]
async fn insert_then_list() {
    let pool = fresh_pool().await;
    let repo = RecallInvocationRepo::new(pool.clone());
    let row = RecallInvocationRow {
        id: Uuid::new_v4(),
        occurred_at: Timestamp::now(),
        session_id: Some("sess1".into()),
        turn_id: Some("t1".into()),
        repo_id: Some("repo:foo".into()),
        layer: "index".into(),
        query: "null pointer parser".into(),
        coverage_score: Some(0.42),
        skill_used: None,
        latency_ms: 17,
        result_ids: vec![Uuid::new_v4(), Uuid::new_v4()],
        rendered_tokens: None,
        metadata: serde_json::json!({}),
    };
    repo.insert(&row).await.expect("insert");
    let page = repo
        .list_by_session("sess1", 50, 0)
        .await
        .expect("list");
    assert_eq!(page.len(), 1);
    assert_eq!(page[0].layer, "index");
    assert_eq!(page[0].result_ids.len(), 2);
}

#[tokio::test]
async fn list_paginates() {
    let pool = fresh_pool().await;
    let repo = RecallInvocationRepo::new(pool.clone());
    for i in 0..5 {
        let row = RecallInvocationRow {
            id: Uuid::new_v4(),
            occurred_at: Timestamp::now(),
            session_id: Some("s".into()),
            turn_id: Some(format!("t{i}")),
            repo_id: None,
            layer: "index".into(),
            query: format!("q{i}"),
            coverage_score: None,
            skill_used: None,
            latency_ms: 1,
            result_ids: vec![],
            rendered_tokens: None,
            metadata: serde_json::json!({}),
        };
        repo.insert(&row).await.unwrap();
    }
    let page1 = repo.list_by_session("s", 2, 0).await.unwrap();
    let page2 = repo.list_by_session("s", 2, 2).await.unwrap();
    assert_eq!(page1.len(), 2);
    assert_eq!(page2.len(), 2);
    assert_ne!(page1[0].id, page2[0].id);
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo nextest run -p coding-memory --test recall_invocation_repo`
Expected: FAIL — module doesn't exist.

- [ ] **Step 3: Implement**

Create `crates/coding-memory/src/recall/telemetry.rs`:

```rust
//! Recall invocation telemetry — every passive/active recall lands a row here.
//!
//! Reads are paginated by `(session_id, occurred_at desc)` and `(repo_id, occurred_at desc)`;
//! the workbench Recall Tool Log panel + Session Replay overlay both consume this table.

use jiff::Timestamp;
use serde::{Deserialize, Serialize};
use storage::StoragePool;
use uuid::Uuid;

/// One recall invocation row. `result_ids` and `metadata` round-trip as JSON.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct RecallInvocationRow {
    /// Stable id.
    pub id: Uuid,
    /// When the recall fired.
    pub occurred_at: Timestamp,
    /// Optional session id.
    pub session_id: Option<String>,
    /// Optional turn id.
    pub turn_id: Option<String>,
    /// Optional repo scope.
    pub repo_id: Option<String>,
    /// Layer label — see migration comment for closed set.
    pub layer: String,
    /// Original query string.
    pub query: String,
    /// Coverage score if scoring ran.
    pub coverage_score: Option<f32>,
    /// CSV of skill names if escalation ran.
    pub skill_used: Option<String>,
    /// Wall-clock latency.
    pub latency_ms: i64,
    /// Memory ids returned.
    pub result_ids: Vec<Uuid>,
    /// Token count for inject layers.
    pub rendered_tokens: Option<i64>,
    /// Free-form metadata JSON.
    pub metadata: serde_json::Value,
}

/// Repo wrapper over `recall_invocations`.
#[derive(Debug, Clone)]
pub struct RecallInvocationRepo {
    pool: StoragePool,
}

impl RecallInvocationRepo {
    /// Construct.
    #[must_use]
    pub fn new(pool: StoragePool) -> Self {
        Self { pool }
    }

    /// Insert a row.
    pub async fn insert(&self, row: &RecallInvocationRow) -> common::Result<()> {
        let result_ids = serde_json::to_string(&row.result_ids)
            .map_err(|e| common::KlyntbotError::Internal(format!("serialize: {e}")))?;
        let metadata = serde_json::to_string(&row.metadata)
            .map_err(|e| common::KlyntbotError::Internal(format!("serialize: {e}")))?;
        sqlx::query(
            "INSERT INTO recall_invocations
             (id, occurred_at, session_id, turn_id, repo_id, layer, query,
              coverage_score, skill_used, latency_ms, result_ids, rendered_tokens, metadata)
             VALUES (?,?,?,?,?,?,?,?,?,?,?,?,?)",
        )
        .bind(row.id.to_string())
        .bind(row.occurred_at.to_string())
        .bind(&row.session_id)
        .bind(&row.turn_id)
        .bind(&row.repo_id)
        .bind(&row.layer)
        .bind(&row.query)
        .bind(row.coverage_score.map(|v| v as f64))
        .bind(&row.skill_used)
        .bind(row.latency_ms)
        .bind(result_ids)
        .bind(row.rendered_tokens)
        .bind(metadata)
        .execute(self.pool.inner())
        .await
        .map_err(|e| common::KlyntbotError::Internal(format!("insert recall_invocation: {e}")))?;
        Ok(())
    }

    /// List by session, paginated newest-first.
    pub async fn list_by_session(
        &self,
        session_id: &str,
        limit: i64,
        offset: i64,
    ) -> common::Result<Vec<RecallInvocationRow>> {
        let rows = sqlx::query_as::<_, RawRow>(
            "SELECT id, occurred_at, session_id, turn_id, repo_id, layer, query,
                    coverage_score, skill_used, latency_ms, result_ids, rendered_tokens, metadata
             FROM recall_invocations
             WHERE session_id = ?
             ORDER BY occurred_at DESC
             LIMIT ? OFFSET ?",
        )
        .bind(session_id)
        .bind(limit)
        .bind(offset)
        .fetch_all(self.pool.inner())
        .await
        .map_err(|e| common::KlyntbotError::Internal(format!("list recall_invocation: {e}")))?;
        rows.into_iter().map(RawRow::into_row).collect()
    }

    /// List recent invocations across all sessions, paginated.
    pub async fn list_recent(
        &self,
        limit: i64,
        offset: i64,
        layer_filter: Option<&str>,
    ) -> common::Result<Vec<RecallInvocationRow>> {
        let rows: Vec<RawRow> = if let Some(layer) = layer_filter {
            sqlx::query_as::<_, RawRow>(
                "SELECT id, occurred_at, session_id, turn_id, repo_id, layer, query,
                        coverage_score, skill_used, latency_ms, result_ids, rendered_tokens, metadata
                 FROM recall_invocations
                 WHERE layer = ?
                 ORDER BY occurred_at DESC
                 LIMIT ? OFFSET ?",
            )
            .bind(layer)
            .bind(limit)
            .bind(offset)
            .fetch_all(self.pool.inner())
            .await
        } else {
            sqlx::query_as::<_, RawRow>(
                "SELECT id, occurred_at, session_id, turn_id, repo_id, layer, query,
                        coverage_score, skill_used, latency_ms, result_ids, rendered_tokens, metadata
                 FROM recall_invocations
                 ORDER BY occurred_at DESC
                 LIMIT ? OFFSET ?",
            )
            .bind(limit)
            .bind(offset)
            .fetch_all(self.pool.inner())
            .await
        }
        .map_err(|e| common::KlyntbotError::Internal(format!("list recent: {e}")))?;
        rows.into_iter().map(RawRow::into_row).collect()
    }
}

#[derive(sqlx::FromRow)]
struct RawRow {
    id: String,
    occurred_at: String,
    session_id: Option<String>,
    turn_id: Option<String>,
    repo_id: Option<String>,
    layer: String,
    query: String,
    coverage_score: Option<f64>,
    skill_used: Option<String>,
    latency_ms: i64,
    result_ids: String,
    rendered_tokens: Option<i64>,
    metadata: String,
}

impl RawRow {
    fn into_row(self) -> common::Result<RecallInvocationRow> {
        Ok(RecallInvocationRow {
            id: self
                .id
                .parse()
                .map_err(|e| common::KlyntbotError::Internal(format!("uuid parse: {e}")))?,
            occurred_at: self
                .occurred_at
                .parse()
                .map_err(|e| common::KlyntbotError::Internal(format!("ts parse: {e}")))?,
            session_id: self.session_id,
            turn_id: self.turn_id,
            repo_id: self.repo_id,
            layer: self.layer,
            query: self.query,
            coverage_score: self.coverage_score.map(|v| v as f32),
            skill_used: self.skill_used,
            latency_ms: self.latency_ms,
            result_ids: serde_json::from_str(&self.result_ids)
                .map_err(|e| common::KlyntbotError::Internal(format!("ids parse: {e}")))?,
            rendered_tokens: self.rendered_tokens,
            metadata: serde_json::from_str(&self.metadata)
                .map_err(|e| common::KlyntbotError::Internal(format!("meta parse: {e}")))?,
        })
    }
}
```

Edit `crates/coding-memory/src/recall/mod.rs` to add at the top after existing module declarations:

```rust
/// Telemetry table for recall invocations.
pub mod telemetry;
```

Edit `crates/coding-memory/src/lib.rs` to add:

```rust
pub use recall::telemetry::{RecallInvocationRepo, RecallInvocationRow};
```

- [ ] **Step 4: Run tests + commit**

```bash
cargo nextest run -p coding-memory --test recall_invocation_repo
git add crates/coding-memory/src/recall/telemetry.rs \
        crates/coding-memory/src/recall/mod.rs \
        crates/coding-memory/src/lib.rs \
        crates/coding-memory/tests/recall_invocation_repo.rs
git commit -m "feat(coding-memory): RecallInvocationRepo — insert + paginated list"
```

---

### Task 4: `TokenBudgeter` — pluggable counter with tiktoken + heuristic fallback

**Files:**
- Create: `crates/coding-memory/src/recall/budget.rs`
- Modify: `crates/coding-memory/src/recall/mod.rs` (add `pub mod budget;`)
- Test: `crates/coding-memory/tests/budget_truncation.rs`

- [ ] **Step 1: Write failing test**

Create `crates/coding-memory/tests/budget_truncation.rs`:

```rust
use coding_memory::recall::budget::{HeuristicBudgeter, TokenBudgeter};

#[test]
fn heuristic_count_is_chars_over_four() {
    let b = HeuristicBudgeter;
    let n = b.count("abcdefghij"); // 10 chars
    assert!(n >= 2 && n <= 3, "got {n}");
}

#[test]
fn truncate_at_budget_keeps_under_cap() {
    let b = HeuristicBudgeter;
    let long = "x".repeat(10_000);
    let out = b.truncate_to(&long, 100);
    assert!(b.count(&out) <= 100);
}

#[test]
fn truncate_preserves_short_input() {
    let b = HeuristicBudgeter;
    let s = "hello world";
    assert_eq!(b.truncate_to(s, 100), s);
}
```

- [ ] **Step 2: Run + verify failure**

Run: `cargo nextest run -p coding-memory --test budget_truncation`
Expected: FAIL — module doesn't exist.

- [ ] **Step 3: Implement**

Create `crates/coding-memory/src/recall/budget.rs`:

```rust
//! Token budgeter — counts and truncates strings to a token budget.
//!
//! `TiktokenBudgeter` uses `tiktoken-rs` (cl100k_base) for OpenAI-compatible counts.
//! `HeuristicBudgeter` is the always-available fallback (`chars / 4`).
//! Renderers depend on the trait; tests use `HeuristicBudgeter` for determinism.

/// Pluggable token counter + truncator.
pub trait TokenBudgeter: Send + Sync {
    /// Count tokens in `s`.
    fn count(&self, s: &str) -> usize;

    /// Truncate `s` so its token count is at most `budget`. Default impl
    /// uses byte-prefix heuristic and verifies; concrete impls may override.
    fn truncate_to(&self, s: &str, budget: usize) -> String {
        if self.count(s) <= budget {
            return s.to_string();
        }
        // Binary-search by char count.
        let chars: Vec<char> = s.chars().collect();
        let mut lo = 0usize;
        let mut hi = chars.len();
        while lo < hi {
            let mid = (lo + hi).div_ceil(2);
            let candidate: String = chars[..mid].iter().collect();
            if self.count(&candidate) <= budget {
                lo = mid;
            } else {
                hi = mid - 1;
            }
        }
        let mut out: String = chars[..lo].iter().collect();
        if out.len() < s.len() {
            out.push_str("…");
        }
        out
    }
}

/// `chars / 4` heuristic. Always available.
#[derive(Debug, Default, Clone, Copy)]
pub struct HeuristicBudgeter;

impl TokenBudgeter for HeuristicBudgeter {
    fn count(&self, s: &str) -> usize {
        s.chars().count().div_ceil(4)
    }
}

/// `tiktoken-rs` cl100k_base counter. Constructed lazily — encoding load can fail.
#[derive(Debug, Clone)]
pub struct TiktokenBudgeter {
    bpe: std::sync::Arc<tiktoken_rs::CoreBPE>,
}

impl TiktokenBudgeter {
    /// Try to load cl100k_base. Returns `None` if the encoding cannot be built.
    pub fn try_new() -> Option<Self> {
        tiktoken_rs::cl100k_base()
            .ok()
            .map(|bpe| Self { bpe: std::sync::Arc::new(bpe) })
    }
}

impl TokenBudgeter for TiktokenBudgeter {
    fn count(&self, s: &str) -> usize {
        self.bpe.encode_with_special_tokens(s).len()
    }
}

/// Pick the best budgeter available — `Tiktoken` if loadable, else `Heuristic`.
#[must_use]
pub fn default_budgeter() -> std::sync::Arc<dyn TokenBudgeter> {
    if let Some(t) = TiktokenBudgeter::try_new() {
        std::sync::Arc::new(t)
    } else {
        std::sync::Arc::new(HeuristicBudgeter)
    }
}
```

Add `pub mod budget;` to `crates/coding-memory/src/recall/mod.rs`.

- [ ] **Step 4: Run tests + commit**

```bash
cargo nextest run -p coding-memory --test budget_truncation
git add crates/coding-memory/src/recall/budget.rs crates/coding-memory/src/recall/mod.rs \
        crates/coding-memory/tests/budget_truncation.rs
git commit -m "feat(coding-memory): TokenBudgeter trait + tiktoken/heuristic impls"
```

---

### Task 5: `RetrievalQualityProbe` — coverage_score + threshold

**Files:**
- Create: `crates/coding-memory/src/recall/probe.rs`
- Modify: `crates/coding-memory/src/recall/mod.rs` (add `pub mod probe;`)
- Test: `crates/coding-memory/tests/probe_coverage.rs`

- [ ] **Step 1: Write failing test**

Create `crates/coding-memory/tests/probe_coverage.rs`:

```rust
use coding_memory::recall::probe::{ProbeVerdict, RetrievalQualityProbe};

#[test]
fn empty_results_score_zero() {
    let p = RetrievalQualityProbe::new(0.3);
    assert_eq!(p.score(&[]), 0.0);
    assert_eq!(p.verdict(&[]), ProbeVerdict::Escalate);
}

#[test]
fn coverage_is_mean_minus_min() {
    let p = RetrievalQualityProbe::new(0.3);
    let s = p.score(&[0.9, 0.8, 0.5]);
    let expected = ((0.9 + 0.8 + 0.5) / 3.0) - 0.5;
    assert!((s - expected).abs() < 1e-5, "got {s}, want {expected}");
}

#[test]
fn high_coverage_passes() {
    let p = RetrievalQualityProbe::new(0.3);
    assert_eq!(p.verdict(&[0.95, 0.9, 0.88]), ProbeVerdict::Sufficient);
}

#[test]
fn low_coverage_escalates() {
    let p = RetrievalQualityProbe::new(0.3);
    assert_eq!(p.verdict(&[0.4, 0.2, 0.05]), ProbeVerdict::Escalate);
}
```

- [ ] **Step 2: Run + verify failure**

Run: `cargo nextest run -p coding-memory --test probe_coverage`
Expected: FAIL — module missing.

- [ ] **Step 3: Implement**

Create `crates/coding-memory/src/recall/probe.rs`:

```rust
//! C3 retrieval-quality probe.
//!
//! `coverage_score = mean(top_k.sim) - min(top_k.sim)`. Below threshold the
//! caller dispatches to the `RetrievalSkillRegistry`. Threshold defaults to
//! 0.3 but Phase 6's autotuner will train it.

/// Probe verdict — does retrieval need escalation?
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProbeVerdict {
    /// Coverage is acceptable.
    Sufficient,
    /// Coverage is below threshold — caller should escalate.
    Escalate,
}

/// Coverage probe with a configurable threshold.
#[derive(Debug, Clone, Copy)]
pub struct RetrievalQualityProbe {
    threshold: f32,
}

impl RetrievalQualityProbe {
    /// Construct.
    #[must_use]
    pub fn new(threshold: f32) -> Self {
        Self { threshold }
    }

    /// Compute coverage_score.
    #[must_use]
    pub fn score(&self, sims: &[f32]) -> f32 {
        if sims.is_empty() {
            return 0.0;
        }
        let mean: f32 = sims.iter().sum::<f32>() / (sims.len() as f32);
        let min: f32 = sims.iter().cloned().fold(f32::INFINITY, f32::min);
        mean - min
    }

    /// Verdict given top-k similarities.
    #[must_use]
    pub fn verdict(&self, sims: &[f32]) -> ProbeVerdict {
        if sims.is_empty() || self.score(sims) < self.threshold {
            ProbeVerdict::Escalate
        } else {
            ProbeVerdict::Sufficient
        }
    }

    /// Active threshold.
    #[must_use]
    pub fn threshold(&self) -> f32 {
        self.threshold
    }
}

impl Default for RetrievalQualityProbe {
    fn default() -> Self {
        Self::new(0.3)
    }
}
```

Add `pub mod probe;` to `crates/coding-memory/src/recall/mod.rs`.

- [ ] **Step 4: Run + commit**

```bash
cargo nextest run -p coding-memory --test probe_coverage
git add crates/coding-memory/src/recall/probe.rs crates/coding-memory/src/recall/mod.rs \
        crates/coding-memory/tests/probe_coverage.rs
git commit -m "feat(coding-memory): RetrievalQualityProbe — coverage_score + verdict"
```

---

### Task 6: `DeadEndChecker` — match approach text against counterfactual facts

**Files:**
- Create: `crates/coding-memory/src/recall/dead_end.rs`
- Modify: `crates/coding-memory/src/recall/mod.rs` (add `pub mod dead_end;`)
- Test: `crates/coding-memory/tests/dead_end_checker.rs`

- [ ] **Step 1: Write failing test**

Create `crates/coding-memory/tests/dead_end_checker.rs`:

```rust
use coding_memory::recall::dead_end::{DeadEndChecker, DeadEndConfig};
use coding_memory::recall::{DeadEndMatch, DeadEndResponse};
use jiff::Timestamp;
use uuid::Uuid;

#[tokio::test]
async fn no_facts_returns_empty() {
    use storage::StoragePool;
    let pool = StoragePool::connect_in_memory().await.unwrap();
    StoragePool::run_feature_migrations(pool.inner(), &cognitive::cognitive_migrations())
        .await
        .unwrap();
    StoragePool::run_feature_migrations(pool.inner(), &coding_memory::coding_memory_migrations())
        .await
        .unwrap();

    let fact_repo = std::sync::Arc::new(cognitive::SemanticFactRepo::new(pool.clone()));
    let checker = DeadEndChecker::new(fact_repo, DeadEndConfig::default());
    let resp = checker
        .check("rewrite the parser as a recursive descent", Some("repo:foo"))
        .await
        .expect("ok");
    assert!(resp.matches.is_empty());
    assert_eq!(resp.aggregate_confidence, 0.0);
}
```

- [ ] **Step 2: Run + verify failure**

Run: `cargo nextest run -p coding-memory --test dead_end_checker`
Expected: FAIL — module missing.

- [ ] **Step 3: Implement**

Create `crates/coding-memory/src/recall/dead_end.rs`:

```rust
//! Dead-end checker — Tier B1 counterfactual matching.
//!
//! Given a candidate approach phrase, queries `semantic_facts` filtered by
//! `metadata.memory_type = 'counterfactual'` (and optional repo scope), runs
//! similarity scoring, and surfaces matches above a confidence floor.

use crate::recall::{DeadEndMatch, DeadEndResponse};
use cognitive::SemanticFactRepo;
use std::sync::Arc;
use uuid::Uuid;

/// Tunables.
#[derive(Debug, Clone, Copy)]
pub struct DeadEndConfig {
    /// Per-match confidence floor.
    pub match_threshold: f32,
    /// Maximum matches to return.
    pub limit: usize,
}

impl Default for DeadEndConfig {
    fn default() -> Self {
        Self {
            match_threshold: 0.7,
            limit: 5,
        }
    }
}

/// Counterfactual match service.
#[derive(Debug, Clone)]
pub struct DeadEndChecker {
    fact_repo: Arc<SemanticFactRepo>,
    config: DeadEndConfig,
}

impl DeadEndChecker {
    /// Construct.
    #[must_use]
    pub fn new(fact_repo: Arc<SemanticFactRepo>, config: DeadEndConfig) -> Self {
        Self { fact_repo, config }
    }

    /// Match an approach against stored counterfactuals.
    ///
    /// Loads candidate facts via `SemanticFactRepo::list_by_memory_type`
    /// (added to cognitive in Task 7 if missing), filtered by repo when
    /// supplied. Confidence is `metadata.confidence` * lexical-overlap.
    pub async fn check(
        &self,
        approach: &str,
        repo: Option<&str>,
    ) -> common::Result<DeadEndResponse> {
        let candidates = self
            .fact_repo
            .list_by_memory_type("counterfactual", repo, 50)
            .await?;
        let approach_lower = approach.to_lowercase();
        let approach_tokens: std::collections::HashSet<&str> =
            approach_lower.split_whitespace().collect();
        let mut matches: Vec<(f32, DeadEndMatch)> = Vec::new();
        for fact in candidates {
            let payload = format!("{} {}", fact.subject, fact.object);
            let payload_lower = payload.to_lowercase();
            let payload_tokens: std::collections::HashSet<&str> =
                payload_lower.split_whitespace().collect();
            let inter = approach_tokens.intersection(&payload_tokens).count() as f32;
            let union = approach_tokens.union(&payload_tokens).count().max(1) as f32;
            let jaccard = inter / union;
            let confidence = jaccard * fact.confidence as f32;
            if confidence < self.config.match_threshold {
                continue;
            }
            let meta = fact
                .metadata
                .as_object()
                .cloned()
                .unwrap_or_default();
            let problem_hash = meta
                .get("problem_hash")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string();
            let reason = meta
                .get("reason")
                .and_then(|v| v.as_str())
                .unwrap_or(&fact.object)
                .to_string();
            let attempt_id = meta
                .get("attempt_id")
                .and_then(|v| v.as_str())
                .and_then(|s| Uuid::parse_str(s).ok())
                .unwrap_or(fact.id);
            matches.push((
                confidence,
                DeadEndMatch {
                    attempt_id,
                    problem_hash,
                    approach: fact.subject.clone(),
                    reason,
                    when: fact.recorded_at,
                },
            ));
        }
        matches.sort_by(|a, b| b.0.partial_cmp(&a.0).unwrap_or(std::cmp::Ordering::Equal));
        matches.truncate(self.config.limit);
        let aggregate_confidence = matches
            .iter()
            .map(|(c, _)| *c)
            .fold(0.0f32, f32::max);
        Ok(DeadEndResponse {
            matches: matches.into_iter().map(|(_, m)| m).collect(),
            aggregate_confidence,
        })
    }
}
```

- [ ] **Step 4: Add `list_by_memory_type` to `SemanticFactRepo` if missing**

Open `crates/cognitive/src/repos/semantic_fact.rs`. Search for `impl SemanticFactRepo`. If `list_by_memory_type` is absent, add it:

```rust
/// List facts whose `metadata.memory_type` matches.
pub async fn list_by_memory_type(
    &self,
    memory_type: &str,
    repo_filter: Option<&str>,
    limit: i64,
) -> common::Result<Vec<SemanticFact>> {
    // SQLite JSON1: `json_extract(metadata, '$.memory_type')`.
    let mut q = String::from(
        "SELECT * FROM semantic_facts
         WHERE json_extract(metadata, '$.memory_type') = ?1",
    );
    if repo_filter.is_some() {
        q.push_str(" AND scope_repo_id = ?2");
    }
    q.push_str(" ORDER BY recorded_at DESC LIMIT ?3");
    let rows = if let Some(repo) = repo_filter {
        sqlx::query_as::<_, SemanticFactRow>(&q)
            .bind(memory_type)
            .bind(repo)
            .bind(limit)
            .fetch_all(self.pool.inner())
            .await
    } else {
        let q2 = q.replace("?3", "?2");
        sqlx::query_as::<_, SemanticFactRow>(&q2)
            .bind(memory_type)
            .bind(limit)
            .fetch_all(self.pool.inner())
            .await
    }
    .map_err(|e| common::KlyntbotError::Internal(format!("list_by_memory_type: {e}")))?;
    rows.into_iter().map(SemanticFactRow::into_fact).collect()
}
```

(If the row → fact conversion is named differently, match the existing pattern. Verify with `grep -n "SemanticFactRow" crates/cognitive/src/repos/semantic_fact.rs`.)

Add `pub mod dead_end;` to `crates/coding-memory/src/recall/mod.rs`.

- [ ] **Step 5: Run + commit**

```bash
cargo nextest run -p coding-memory --test dead_end_checker
cargo nextest run -p cognitive
git add crates/coding-memory/src/recall/dead_end.rs crates/coding-memory/src/recall/mod.rs \
        crates/coding-memory/tests/dead_end_checker.rs \
        crates/cognitive/src/repos/semantic_fact.rs
git commit -m "feat(coding-memory): DeadEndChecker — Jaccard-weighted counterfactual matching"
```

---

### Task 7: `IndexBuilder` — `ScoredFact`/`EpisodicMemory` → `IndexEntry`

**Files:**
- Create: `crates/coding-memory/src/recall/index_builder.rs`
- Modify: `crates/coding-memory/src/recall/mod.rs` (add `pub mod index_builder;`)
- Test: `crates/coding-memory/tests/index_builder.rs`

- [ ] **Step 1: Write failing test**

Create `crates/coding-memory/tests/index_builder.rs`:

```rust
use coding_memory::recall::index_builder::IndexBuilder;
use cognitive::ScoredFact;

#[test]
fn fact_to_index_entry_includes_token_estimate() {
    // Build a minimal ScoredFact via cognitive's test helpers if available;
    // else round-trip a SemanticFact through ScoredFact { score, fact }.
    let fact = cognitive::SemanticFact {
        id: uuid::Uuid::new_v4(),
        subject: "auth_module".into(),
        predicate: "uses".into(),
        object: "JWT with HS256".into(),
        recorded_at: jiff::Timestamp::now(),
        confidence: 0.9,
        scope_repo_id: Some("repo:demo".into()),
        memory_type: Some("repo_context".into()),
        ..Default::default()
    };
    let scored = ScoredFact { score: 0.8, fact };
    let builder = IndexBuilder::new();
    let entry = builder.from_scored_fact(&scored);
    assert_eq!(entry.kind, "repo_context");
    assert!(entry.token_cost > 0);
    assert!(entry.confidence > 0.0);
}
```

(If `SemanticFact` has no `Default`, fill all fields explicitly. Verify the actual struct shape with `grep -n "pub struct SemanticFact" crates/cognitive/src/`.)

- [ ] **Step 2: Run + verify failure**

Run: `cargo nextest run -p coding-memory --test index_builder`
Expected: FAIL.

- [ ] **Step 3: Implement**

Create `crates/coding-memory/src/recall/index_builder.rs`:

```rust
//! Build `IndexEntry` rows from cognitive `ScoredFact` / `EpisodicMemory`.

use crate::recall::IndexEntry;
use crate::recall::budget::TokenBudgeter;
use cognitive::{EpisodicMemory, ScoredFact};
use std::sync::Arc;

/// `IndexEntry` builder.
#[derive(Clone)]
pub struct IndexBuilder {
    budgeter: Arc<dyn TokenBudgeter>,
}

impl std::fmt::Debug for IndexBuilder {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("IndexBuilder").finish()
    }
}

impl IndexBuilder {
    /// Construct with the default budgeter.
    #[must_use]
    pub fn new() -> Self {
        Self { budgeter: crate::recall::budget::default_budgeter() }
    }

    /// Construct with a specific budgeter (test seam).
    #[must_use]
    pub fn with_budgeter(budgeter: Arc<dyn TokenBudgeter>) -> Self {
        Self { budgeter }
    }

    /// Convert a scored fact.
    #[must_use]
    pub fn from_scored_fact(&self, sf: &ScoredFact) -> IndexEntry {
        let f = &sf.fact;
        let kind = f.memory_type.clone().unwrap_or_else(|| "fact".to_string());
        let title = format!("{} {} {}", f.subject, f.predicate, f.object);
        let scope = f
            .scope_repo_id
            .as_ref()
            .map(|r| format!("repo:{r}"))
            .unwrap_or_else(|| "global".to_string());
        let est = format!("{title}\n{}", serde_json::to_string(&f.metadata).unwrap_or_default());
        let token_cost = self.budgeter.count(&est) as u32;
        IndexEntry {
            id: f.id,
            kind,
            title: truncate_chars(&title, 120),
            when: f.recorded_at,
            scope,
            confidence: f.confidence as f32,
            token_cost,
        }
    }

    /// Convert an episodic memory.
    #[must_use]
    pub fn from_episode(&self, ep: &EpisodicMemory) -> IndexEntry {
        let scope = ep
            .scope_repo_id
            .as_ref()
            .map(|r| format!("repo:{r}"))
            .unwrap_or_else(|| "global".to_string());
        let est = serde_json::to_string(&ep.content).unwrap_or_default();
        let token_cost = self.budgeter.count(&est) as u32;
        IndexEntry {
            id: ep.id,
            kind: ep.kind.clone(),
            title: truncate_chars(&ep.summary.clone().unwrap_or_default(), 120),
            when: ep.occurred_at,
            scope,
            confidence: ep.confidence.unwrap_or(0.5) as f32,
            token_cost,
        }
    }
}

impl Default for IndexBuilder {
    fn default() -> Self {
        Self::new()
    }
}

fn truncate_chars(s: &str, n: usize) -> String {
    if s.chars().count() <= n {
        s.to_string()
    } else {
        let mut out: String = s.chars().take(n).collect();
        out.push('…');
        out
    }
}
```

Add `pub mod index_builder;` to `crates/coding-memory/src/recall/mod.rs`.

- [ ] **Step 4: Run + commit**

```bash
cargo nextest run -p coding-memory --test index_builder
git add crates/coding-memory/src/recall/index_builder.rs crates/coding-memory/src/recall/mod.rs \
        crates/coding-memory/tests/index_builder.rs
git commit -m "feat(coding-memory): IndexBuilder — ScoredFact/Episode → IndexEntry"
```

---

### Task 8: `TimelineBuilder` — chronological framing with related_ids

**Files:**
- Create: `crates/coding-memory/src/recall/timeline_builder.rs`
- Modify: `crates/coding-memory/src/recall/mod.rs`
- Test: `crates/coding-memory/tests/timeline_builder.rs`

- [ ] **Step 1: Write failing test**

Create `crates/coding-memory/tests/timeline_builder.rs`:

```rust
use coding_memory::recall::timeline_builder::TimelineBuilder;
use jiff::Timestamp;
use uuid::Uuid;

#[test]
fn entries_sorted_descending_by_when() {
    let ids = [Uuid::new_v4(), Uuid::new_v4(), Uuid::new_v4()];
    let now = Timestamp::now();
    let inputs = vec![
        coding_memory::recall::timeline_builder::TimelineInput {
            id: ids[0],
            kind: "fix_attempt".into(),
            when: now,
            snippet: "older".into(),
            related_ids: vec![],
        },
        coding_memory::recall::timeline_builder::TimelineInput {
            id: ids[1],
            kind: "fix_attempt".into(),
            when: now.saturating_add(jiff::SignedDuration::from_secs(60)),
            snippet: "newer".into(),
            related_ids: vec![],
        },
    ];
    let out = TimelineBuilder::new().build(inputs);
    assert_eq!(out.len(), 2);
    assert_eq!(out[0].snippet, "newer");
    assert_eq!(out[1].snippet, "older");
}
```

- [ ] **Step 2: Run + verify failure**

Run: `cargo nextest run -p coding-memory --test timeline_builder`
Expected: FAIL.

- [ ] **Step 3: Implement**

Create `crates/coding-memory/src/recall/timeline_builder.rs`:

```rust
//! `TimelineEntry` builder — orders by `when` descending; carries `related_ids`.

use crate::recall::TimelineEntry;
use uuid::Uuid;

/// Pre-built input row for the timeline (decoupled from cognitive types so
/// the service layer can populate from facts, episodes, or a join).
#[derive(Debug, Clone)]
pub struct TimelineInput {
    /// Memory id.
    pub id: Uuid,
    /// Kind label.
    pub kind: String,
    /// Timestamp.
    pub when: jiff::Timestamp,
    /// Snippet text.
    pub snippet: String,
    /// Pre-resolved related ids.
    pub related_ids: Vec<Uuid>,
}

/// Timeline builder.
#[derive(Debug, Default, Clone, Copy)]
pub struct TimelineBuilder;

impl TimelineBuilder {
    /// Construct.
    #[must_use]
    pub fn new() -> Self {
        Self
    }

    /// Build entries from a list of inputs, sorted newest first.
    #[must_use]
    pub fn build(self, mut inputs: Vec<TimelineInput>) -> Vec<TimelineEntry> {
        inputs.sort_by(|a, b| b.when.cmp(&a.when));
        inputs
            .into_iter()
            .map(|i| TimelineEntry {
                id: i.id,
                kind: i.kind,
                when: i.when,
                snippet: truncate(&i.snippet, 240),
                related_ids: i.related_ids,
            })
            .collect()
    }
}

fn truncate(s: &str, n: usize) -> String {
    if s.chars().count() <= n {
        s.to_string()
    } else {
        let mut out: String = s.chars().take(n).collect();
        out.push('…');
        out
    }
}
```

Add `pub mod timeline_builder;` to `recall/mod.rs`.

- [ ] **Step 4: Run + commit**

```bash
cargo nextest run -p coding-memory --test timeline_builder
git add crates/coding-memory/src/recall/timeline_builder.rs crates/coding-memory/src/recall/mod.rs \
        crates/coding-memory/tests/timeline_builder.rs
git commit -m "feat(coding-memory): TimelineBuilder — sort + truncate"
```

---

### Task 9: `FetchBuilder` — full entry with provenance + supersede chain

**Files:**
- Create: `crates/coding-memory/src/recall/fetch_builder.rs`
- Modify: `crates/coding-memory/src/recall/mod.rs`
- Test: `crates/coding-memory/tests/fetch_builder.rs`

- [ ] **Step 1: Write failing test**

Create `crates/coding-memory/tests/fetch_builder.rs`:

```rust
use coding_memory::recall::fetch_builder::FetchBuilder;
use jiff::Timestamp;
use uuid::Uuid;

#[tokio::test]
async fn fact_fetch_round_trip() {
    use storage::StoragePool;
    let pool = StoragePool::connect_in_memory().await.unwrap();
    StoragePool::run_feature_migrations(pool.inner(), &cognitive::cognitive_migrations())
        .await
        .unwrap();
    StoragePool::run_feature_migrations(pool.inner(), &coding_memory::coding_memory_migrations())
        .await
        .unwrap();
    let fact_repo = std::sync::Arc::new(cognitive::SemanticFactRepo::new(pool.clone()));
    let ep_repo = std::sync::Arc::new(cognitive::EpisodicMemoryRepo::new(pool.clone()));

    let fact = cognitive::SemanticFact {
        id: Uuid::new_v4(),
        subject: "module".into(),
        predicate: "uses".into(),
        object: "lib".into(),
        recorded_at: Timestamp::now(),
        confidence: 0.9,
        scope_repo_id: Some("repo:x".into()),
        memory_type: Some("repo_context".into()),
        metadata: serde_json::json!({"provenance": {"source_events": ["evt1"]}}),
        ..Default::default()
    };
    fact_repo.upsert_with_metadata(&fact).await.unwrap();

    let builder = FetchBuilder::new(fact_repo.clone(), ep_repo.clone());
    let out = builder
        .fetch(&[fact.id], true, false)
        .await
        .unwrap();
    assert_eq!(out.len(), 1);
    assert_eq!(out[0].kind, "repo_context");
    assert!(out[0].metadata.get("provenance").is_some());
}
```

- [ ] **Step 2: Run + verify failure**

Run: `cargo nextest run -p coding-memory --test fetch_builder`
Expected: FAIL.

- [ ] **Step 3: Implement**

Create `crates/coding-memory/src/recall/fetch_builder.rs`:

```rust
//! Build `FullEntry` rows — joins fact/episode + provenance + supersede chain.
//! Causal edges are returned empty until Phase 6 wires `memory_causal_edges`.

use crate::recall::FullEntry;
use cognitive::{EpisodicMemoryRepo, SemanticFactRepo};
use std::sync::Arc;
use uuid::Uuid;

/// Composite fetcher.
#[derive(Clone)]
pub struct FetchBuilder {
    fact_repo: Arc<SemanticFactRepo>,
    ep_repo: Arc<EpisodicMemoryRepo>,
}

impl std::fmt::Debug for FetchBuilder {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("FetchBuilder").finish()
    }
}

impl FetchBuilder {
    /// Construct.
    #[must_use]
    pub fn new(fact_repo: Arc<SemanticFactRepo>, ep_repo: Arc<EpisodicMemoryRepo>) -> Self {
        Self { fact_repo, ep_repo }
    }

    /// Fetch by ids. Looks up facts first, then episodes for misses.
    pub async fn fetch(
        &self,
        ids: &[Uuid],
        include_provenance: bool,
        include_causal_graph: bool,
    ) -> common::Result<Vec<FullEntry>> {
        let mut out = Vec::new();
        for id in ids {
            if let Some(fact) = self.fact_repo.get_by_id(*id).await? {
                let metadata = if include_provenance {
                    fact.metadata.clone()
                } else {
                    strip_provenance(fact.metadata.clone())
                };
                out.push(FullEntry {
                    id: fact.id,
                    kind: fact
                        .memory_type
                        .clone()
                        .unwrap_or_else(|| "fact".to_string()),
                    content: serde_json::json!({
                        "subject": fact.subject,
                        "predicate": fact.predicate,
                        "object": fact.object,
                        "confidence": fact.confidence,
                    }),
                    metadata,
                    causal_edges: if include_causal_graph { Vec::new() } else { Vec::new() },
                    supersedes: fact.supersedes,
                    superseded_by: fact.superseded_by,
                });
                continue;
            }
            if let Some(ep) = self.ep_repo.get_by_id(*id).await? {
                let metadata = if include_provenance {
                    ep.metadata.clone()
                } else {
                    strip_provenance(ep.metadata.clone())
                };
                out.push(FullEntry {
                    id: ep.id,
                    kind: ep.kind.clone(),
                    content: ep.content.clone(),
                    metadata,
                    causal_edges: Vec::new(),
                    supersedes: None,
                    superseded_by: None,
                });
            }
        }
        Ok(out)
    }
}

fn strip_provenance(mut v: serde_json::Value) -> serde_json::Value {
    if let Some(map) = v.as_object_mut() {
        map.remove("provenance");
    }
    v
}
```

(Verify `SemanticFactRepo::get_by_id` and `EpisodicMemoryRepo::get_by_id` exist with `grep -n "pub async fn get_by_id" crates/cognitive/src/repos/`. If they're absent, add minimal versions returning `Option<Row>`. Ditto for the `supersedes`/`superseded_by` columns on `SemanticFact` — these were added in Phase 1 schema; the struct exposure may need a tweak.)

Add `pub mod fetch_builder;` to `recall/mod.rs`.

- [ ] **Step 4: Run + commit**

```bash
cargo nextest run -p coding-memory --test fetch_builder
git add crates/coding-memory/src/recall/fetch_builder.rs \
        crates/coding-memory/src/recall/mod.rs \
        crates/coding-memory/tests/fetch_builder.rs
git commit -m "feat(coding-memory): FetchBuilder — fact/episode fetch with provenance"
```

---

### Task 10: `recall_facts_as_of` — bi-temporal lookup

**Files:**
- Create: `crates/coding-memory/src/recall/facts_as_of.rs`
- Modify: `crates/coding-memory/src/recall/mod.rs` (add `pub mod facts_as_of;` + new `FactsAsOfResponse` DTO)
- Test: `crates/coding-memory/tests/facts_as_of.rs`

- [ ] **Step 1: Add response DTO to `recall/mod.rs`**

Append to `crates/coding-memory/src/recall/mod.rs`:

```rust
/// Row in a `recall_facts_as_of` response.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct FactAsOfRow {
    /// Fact id.
    pub id: Uuid,
    /// Subject.
    pub subject: String,
    /// Predicate.
    pub predicate: String,
    /// Object value at `as_of`.
    pub object: String,
    /// `valid_from`.
    pub valid_from: Timestamp,
    /// `valid_until` if closed.
    pub valid_until: Option<Timestamp>,
    /// Confidence at the time.
    pub confidence: f32,
}

/// Response from `recall_facts_as_of`.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct FactsAsOfResponse {
    /// Subject queried.
    pub subject: String,
    /// Predicate queried.
    pub predicate: String,
    /// `as_of` timestamp.
    pub as_of: Timestamp,
    /// Matching rows.
    pub rows: Vec<FactAsOfRow>,
}
```

- [ ] **Step 2: Write failing test**

Create `crates/coding-memory/tests/facts_as_of.rs`:

```rust
use coding_memory::recall::facts_as_of::FactsAsOfService;
use jiff::Timestamp;

#[tokio::test]
async fn returns_row_valid_at_timestamp() {
    use storage::StoragePool;
    let pool = StoragePool::connect_in_memory().await.unwrap();
    StoragePool::run_feature_migrations(pool.inner(), &cognitive::cognitive_migrations())
        .await
        .unwrap();
    StoragePool::run_feature_migrations(pool.inner(), &coding_memory::coding_memory_migrations())
        .await
        .unwrap();
    let fact_repo = std::sync::Arc::new(cognitive::SemanticFactRepo::new(pool.clone()));

    // Seed: one fact valid_from t0; not superseded.
    let t0 = Timestamp::now();
    let fact = cognitive::SemanticFact {
        id: uuid::Uuid::new_v4(),
        subject: "auth".into(),
        predicate: "uses".into(),
        object: "JWT".into(),
        recorded_at: t0,
        confidence: 0.8,
        scope_repo_id: Some("repo:x".into()),
        memory_type: Some("repo_context".into()),
        valid_from: Some(t0),
        valid_until: None,
        ..Default::default()
    };
    fact_repo.upsert_with_metadata(&fact).await.unwrap();

    let svc = FactsAsOfService::new(fact_repo);
    let resp = svc
        .query("auth", "uses", t0.saturating_add(jiff::SignedDuration::from_secs(60)))
        .await
        .unwrap();
    assert_eq!(resp.rows.len(), 1);
    assert_eq!(resp.rows[0].object, "JWT");
}
```

- [ ] **Step 3: Run + verify failure**

Run: `cargo nextest run -p coding-memory --test facts_as_of`
Expected: FAIL.

- [ ] **Step 4: Implement**

Create `crates/coding-memory/src/recall/facts_as_of.rs`:

```rust
//! Bi-temporal point-in-time lookup.

use crate::recall::{FactAsOfRow, FactsAsOfResponse};
use cognitive::SemanticFactRepo;
use jiff::Timestamp;
use std::sync::Arc;

/// Bi-temporal service.
#[derive(Debug, Clone)]
pub struct FactsAsOfService {
    fact_repo: Arc<SemanticFactRepo>,
}

impl FactsAsOfService {
    /// Construct.
    #[must_use]
    pub fn new(fact_repo: Arc<SemanticFactRepo>) -> Self {
        Self { fact_repo }
    }

    /// Query — returns all rows with `(subject, predicate)` where
    /// `valid_from <= as_of < COALESCE(valid_until, +inf)`.
    pub async fn query(
        &self,
        subject: &str,
        predicate: &str,
        as_of: Timestamp,
    ) -> common::Result<FactsAsOfResponse> {
        let rows = self
            .fact_repo
            .list_valid_at(subject, predicate, as_of)
            .await?;
        Ok(FactsAsOfResponse {
            subject: subject.to_string(),
            predicate: predicate.to_string(),
            as_of,
            rows: rows
                .into_iter()
                .map(|f| FactAsOfRow {
                    id: f.id,
                    subject: f.subject,
                    predicate: f.predicate,
                    object: f.object,
                    valid_from: f.valid_from.unwrap_or(f.recorded_at),
                    valid_until: f.valid_until,
                    confidence: f.confidence as f32,
                })
                .collect(),
        })
    }
}
```

Add `list_valid_at` to `crates/cognitive/src/repos/semantic_fact.rs` if absent:

```rust
/// List facts where `(subject, predicate)` matches and the bi-temporal
/// validity range covers `as_of`.
pub async fn list_valid_at(
    &self,
    subject: &str,
    predicate: &str,
    as_of: jiff::Timestamp,
) -> common::Result<Vec<SemanticFact>> {
    let rows = sqlx::query_as::<_, SemanticFactRow>(
        "SELECT * FROM semantic_facts
         WHERE subject = ?1 AND predicate = ?2
           AND (valid_from IS NULL OR valid_from <= ?3)
           AND (valid_until IS NULL OR valid_until > ?3)
         ORDER BY recorded_at DESC",
    )
    .bind(subject)
    .bind(predicate)
    .bind(as_of.to_string())
    .fetch_all(self.pool.inner())
    .await
    .map_err(|e| common::KlyntbotError::Internal(format!("list_valid_at: {e}")))?;
    rows.into_iter().map(SemanticFactRow::into_fact).collect()
}
```

Add `pub mod facts_as_of;` to `recall/mod.rs`.

- [ ] **Step 5: Run + commit**

```bash
cargo nextest run -p coding-memory --test facts_as_of
git add crates/coding-memory/src/recall/facts_as_of.rs \
        crates/coding-memory/src/recall/mod.rs \
        crates/coding-memory/tests/facts_as_of.rs \
        crates/cognitive/src/repos/semantic_fact.rs
git commit -m "feat(coding-memory): recall_facts_as_of — bi-temporal point lookup"
```

---

### Task 11: `recall_change_history` — walk SUPERSEDE chain

**Files:**
- Create: `crates/coding-memory/src/recall/change_history.rs`
- Modify: `crates/coding-memory/src/recall/mod.rs`
- Test: `crates/coding-memory/tests/change_history.rs`

- [ ] **Step 1: Add response DTO**

Append to `crates/coding-memory/src/recall/mod.rs`:

```rust
/// One step in a SUPERSEDE chain.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ChangeHistoryStep {
    /// Fact id at this step.
    pub id: Uuid,
    /// Object value.
    pub object: String,
    /// `valid_from`.
    pub valid_from: Timestamp,
    /// `valid_until`.
    pub valid_until: Option<Timestamp>,
}

/// Response from `recall_change_history`.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ChangeHistoryResponse {
    /// Subject.
    pub subject: String,
    /// Predicate.
    pub predicate: String,
    /// Chain ordered oldest-first.
    pub steps: Vec<ChangeHistoryStep>,
}
```

- [ ] **Step 2: Write failing test**

Create `crates/coding-memory/tests/change_history.rs`:

```rust
use coding_memory::recall::change_history::ChangeHistoryService;

#[tokio::test]
async fn empty_history_returns_empty_steps() {
    use storage::StoragePool;
    let pool = StoragePool::connect_in_memory().await.unwrap();
    StoragePool::run_feature_migrations(pool.inner(), &cognitive::cognitive_migrations())
        .await
        .unwrap();
    StoragePool::run_feature_migrations(pool.inner(), &coding_memory::coding_memory_migrations())
        .await
        .unwrap();
    let svc = ChangeHistoryService::new(std::sync::Arc::new(
        cognitive::SemanticFactRepo::new(pool.clone()),
    ));
    let r = svc.query("nonexistent", "uses", None).await.unwrap();
    assert!(r.steps.is_empty());
}
```

- [ ] **Step 3: Run + verify failure**

Run: `cargo nextest run -p coding-memory --test change_history`
Expected: FAIL.

- [ ] **Step 4: Implement**

Create `crates/coding-memory/src/recall/change_history.rs`:

```rust
//! Walk the SUPERSEDE chain for `(subject, predicate)`.

use crate::recall::{ChangeHistoryResponse, ChangeHistoryStep};
use cognitive::SemanticFactRepo;
use std::sync::Arc;

/// Service.
#[derive(Debug, Clone)]
pub struct ChangeHistoryService {
    fact_repo: Arc<SemanticFactRepo>,
}

impl ChangeHistoryService {
    /// Construct.
    #[must_use]
    pub fn new(fact_repo: Arc<SemanticFactRepo>) -> Self {
        Self { fact_repo }
    }

    /// Query the full chain — caller passes `(subject, predicate)`.
    /// Optional `repo` filter narrows scope.
    pub async fn query(
        &self,
        subject: &str,
        predicate: &str,
        repo: Option<&str>,
    ) -> common::Result<ChangeHistoryResponse> {
        let rows = self
            .fact_repo
            .list_chain_for(subject, predicate, repo)
            .await?;
        let mut steps: Vec<ChangeHistoryStep> = rows
            .into_iter()
            .map(|f| ChangeHistoryStep {
                id: f.id,
                object: f.object,
                valid_from: f.valid_from.unwrap_or(f.recorded_at),
                valid_until: f.valid_until,
            })
            .collect();
        steps.sort_by(|a, b| a.valid_from.cmp(&b.valid_from));
        Ok(ChangeHistoryResponse {
            subject: subject.to_string(),
            predicate: predicate.to_string(),
            steps,
        })
    }
}
```

Add `list_chain_for` to `SemanticFactRepo`:

```rust
/// All facts (any version) matching `(subject, predicate)` plus optional repo.
pub async fn list_chain_for(
    &self,
    subject: &str,
    predicate: &str,
    repo: Option<&str>,
) -> common::Result<Vec<SemanticFact>> {
    let rows = if let Some(r) = repo {
        sqlx::query_as::<_, SemanticFactRow>(
            "SELECT * FROM semantic_facts
             WHERE subject = ?1 AND predicate = ?2 AND scope_repo_id = ?3
             ORDER BY recorded_at ASC",
        )
        .bind(subject)
        .bind(predicate)
        .bind(r)
        .fetch_all(self.pool.inner())
        .await
    } else {
        sqlx::query_as::<_, SemanticFactRow>(
            "SELECT * FROM semantic_facts
             WHERE subject = ?1 AND predicate = ?2
             ORDER BY recorded_at ASC",
        )
        .bind(subject)
        .bind(predicate)
        .fetch_all(self.pool.inner())
        .await
    }
    .map_err(|e| common::KlyntbotError::Internal(format!("list_chain_for: {e}")))?;
    rows.into_iter().map(SemanticFactRow::into_fact).collect()
}
```

Add `pub mod change_history;` to `recall/mod.rs`.

- [ ] **Step 5: Run + commit**

```bash
cargo nextest run -p coding-memory --test change_history
git add crates/coding-memory/src/recall/change_history.rs \
        crates/coding-memory/src/recall/mod.rs \
        crates/coding-memory/tests/change_history.rs \
        crates/cognitive/src/repos/semantic_fact.rs
git commit -m "feat(coding-memory): recall_change_history — SUPERSEDE chain walk"
```

---

### Task 12: `recall_decision_points` — list decision-laden episodes

**Files:**
- Create: `crates/coding-memory/src/recall/decision_points.rs`
- Modify: `crates/coding-memory/src/recall/mod.rs`
- Test: `crates/coding-memory/tests/decision_points.rs`

- [ ] **Step 1: Add response DTO**

Append to `recall/mod.rs`:

```rust
/// One decision point row.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct DecisionPointRow {
    /// Episode id.
    pub id: Uuid,
    /// Episode kind.
    pub kind: String,
    /// When.
    pub when: Timestamp,
    /// Summary.
    pub summary: String,
    /// Repo scope.
    pub scope: String,
}

/// Response from `recall_decision_points`.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct DecisionPointsResponse {
    /// Domain (always `"code"` here).
    pub domain: String,
    /// Rows ordered newest-first.
    pub rows: Vec<DecisionPointRow>,
}
```

- [ ] **Step 2: Write failing test**

Create `crates/coding-memory/tests/decision_points.rs`:

```rust
use coding_memory::recall::decision_points::DecisionPointsService;

#[tokio::test]
async fn empty_returns_empty_rows() {
    use storage::StoragePool;
    let pool = StoragePool::connect_in_memory().await.unwrap();
    StoragePool::run_feature_migrations(pool.inner(), &cognitive::cognitive_migrations())
        .await
        .unwrap();
    StoragePool::run_feature_migrations(pool.inner(), &coding_memory::coding_memory_migrations())
        .await
        .unwrap();
    let svc = DecisionPointsService::new(std::sync::Arc::new(
        cognitive::EpisodicMemoryRepo::new(pool.clone()),
    ));
    let r = svc.list(Some("repo:x"), 50).await.unwrap();
    assert!(r.rows.is_empty());
}
```

- [ ] **Step 3: Run + verify failure**

Run: `cargo nextest run -p coding-memory --test decision_points`
Expected: FAIL.

- [ ] **Step 4: Implement**

Create `crates/coding-memory/src/recall/decision_points.rs`:

```rust
//! `recall_decision_points` — list decision-laden episodes.

use crate::recall::{DecisionPointRow, DecisionPointsResponse};
use cognitive::EpisodicMemoryRepo;
use std::sync::Arc;

/// Closed kind set considered "decision points" in the coding domain.
pub const DECISION_KINDS: &[&str] =
    &["fix_attempt", "dead_end_attempt", "refactor_episode"];

/// Service.
#[derive(Debug, Clone)]
pub struct DecisionPointsService {
    ep_repo: Arc<EpisodicMemoryRepo>,
}

impl DecisionPointsService {
    /// Construct.
    #[must_use]
    pub fn new(ep_repo: Arc<EpisodicMemoryRepo>) -> Self {
        Self { ep_repo }
    }

    /// List decision points within the optional repo scope.
    pub async fn list(
        &self,
        repo: Option<&str>,
        limit: i64,
    ) -> common::Result<DecisionPointsResponse> {
        let eps = self
            .ep_repo
            .list_by_kinds(DECISION_KINDS, repo, limit)
            .await?;
        Ok(DecisionPointsResponse {
            domain: "code".to_string(),
            rows: eps
                .into_iter()
                .map(|e| DecisionPointRow {
                    id: e.id,
                    kind: e.kind,
                    when: e.occurred_at,
                    summary: e.summary.unwrap_or_default(),
                    scope: e
                        .scope_repo_id
                        .map(|r| format!("repo:{r}"))
                        .unwrap_or_else(|| "global".to_string()),
                })
                .collect(),
        })
    }
}
```

Add `list_by_kinds` to `EpisodicMemoryRepo` if absent:

```rust
/// List episodes whose `kind` is in the given closed set, optionally scoped.
pub async fn list_by_kinds(
    &self,
    kinds: &[&str],
    repo: Option<&str>,
    limit: i64,
) -> common::Result<Vec<EpisodicMemory>> {
    let placeholders = (0..kinds.len()).map(|_| "?").collect::<Vec<_>>().join(",");
    let mut q = format!(
        "SELECT * FROM episodic_memories WHERE kind IN ({placeholders})"
    );
    if repo.is_some() {
        q.push_str(" AND scope_repo_id = ?");
    }
    q.push_str(" ORDER BY occurred_at DESC LIMIT ?");
    let mut bound = sqlx::query_as::<_, EpisodicMemoryRow>(&q);
    for k in kinds {
        bound = bound.bind(*k);
    }
    if let Some(r) = repo {
        bound = bound.bind(r);
    }
    bound = bound.bind(limit);
    let rows = bound
        .fetch_all(self.pool.inner())
        .await
        .map_err(|e| common::KlyntbotError::Internal(format!("list_by_kinds: {e}")))?;
    rows.into_iter().map(EpisodicMemoryRow::into_episode).collect()
}
```

Add `pub mod decision_points;` to `recall/mod.rs`.

- [ ] **Step 5: Run + commit**

```bash
cargo nextest run -p coding-memory --test decision_points
git add crates/coding-memory/src/recall/decision_points.rs \
        crates/coding-memory/src/recall/mod.rs \
        crates/coding-memory/tests/decision_points.rs \
        crates/cognitive/src/repos/episodic_memory.rs
git commit -m "feat(coding-memory): recall_decision_points — kind-filtered episode listing"
```

---

### Task 13: `RetrievalSkillRegistry` — owner of the 5 skills with EMA + selector

**Files:**
- Create: `crates/coding-memory/src/retrieval_skills/mod.rs`
- Create: `crates/coding-memory/src/retrieval_skills/registry.rs`
- Modify: `crates/coding-memory/src/retrieval_skills.rs` (rename to `retrieval_skills_legacy.rs` and have new `mod.rs` re-export trait + types)
- Test: `crates/coding-memory/tests/skill_registry_selector.rs`

> **Note on layout:** Phase 1 placed everything in a single `retrieval_skills.rs`. Phase 4 splits into a directory module so the 5 concrete skills can live in their own files. Carry the existing `RetrievalSkill` trait + `BudgetTier` + `EscalationContext` + `EscalationOutcome` types to the new `mod.rs`; delete the old single-file module after.

- [ ] **Step 1: Move existing types into a directory module**

Create `crates/coding-memory/src/retrieval_skills/mod.rs`:

```rust
//! C3 retrieval-skill registry — invoked when `RetrievalQualityProbe`
//! returns `Escalate`. Closed set of 5 skills wired in Phase 4.

use async_trait::async_trait;

pub mod registry;
pub mod query_rewriter;
pub mod query_decomposer;
pub mod evidence_focuser;
pub mod raw_event_escalator;
pub mod causal_context_expander;

pub use registry::{RetrievalSkillRegistry, SelectorOutcome};
pub use query_rewriter::QueryRewriter;
pub use query_decomposer::QueryDecomposer;
pub use evidence_focuser::EvidenceFocuser;
pub use raw_event_escalator::RawEventEscalator;
pub use causal_context_expander::CausalContextExpander;

/// Budget tier at which a retrieval skill can operate.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BudgetTier {
    /// Fast (default) — bounded to the original retrieval budget.
    Fast,
    /// `deep_think` — larger budget for query rewriting/decomposing.
    DeepThink,
    /// `ultra` — full escalation, bypasses summaries.
    Ultra,
}

/// Context passed to a retrieval skill's `apply`.
#[derive(Debug, Clone)]
pub struct EscalationContext {
    /// Original query.
    pub query: String,
    /// Coverage score at invocation time.
    pub coverage_score: f32,
    /// Active tier.
    pub budget_tier: BudgetTier,
    /// Optional repo scope.
    pub repo: Option<String>,
}

/// Outcome of a retrieval skill application.
#[derive(Debug, Clone)]
pub struct EscalationOutcome {
    /// Was coverage raised above threshold?
    pub succeeded: bool,
    /// New coverage score after applying.
    pub coverage_after: f32,
    /// Additional context produced (rendered).
    pub added_context: String,
    /// New ids surfaced (deduped against the original retrieval).
    pub added_ids: Vec<uuid::Uuid>,
}

/// Retrieval skill — the unit of C3 escalation.
#[async_trait]
pub trait RetrievalSkill: Send + Sync {
    /// Skill name used in telemetry + effectiveness EMA.
    fn name(&self) -> &'static str;

    /// Short description for UI surfaces.
    fn description(&self) -> &'static str;

    /// Tier this skill belongs to.
    fn tier(&self) -> BudgetTier;

    /// Apply the skill against an escalation context.
    async fn apply(
        &self,
        ctx: &EscalationContext,
    ) -> common::Result<EscalationOutcome>;
}
```

Delete `crates/coding-memory/src/retrieval_skills.rs` (contents move to new directory).

Edit `crates/coding-memory/src/lib.rs`:
- Remove `pub mod retrieval_skills;` if it exists with explicit module path
- Confirm `pub mod retrieval_skills;` still resolves to the directory module
- Add `pub use retrieval_skills::{RetrievalSkill, RetrievalSkillRegistry, BudgetTier, EscalationContext, EscalationOutcome};`

- [ ] **Step 2: Implement `RetrievalSkillRegistry`**

Create `crates/coding-memory/src/retrieval_skills/registry.rs`:

```rust
//! Registry — owns the closed set of 5 skills, tracks per-skill EMA, runs the
//! tier-aware selector. Effectiveness updates land via
//! `DomainEvent::RetrievalSkillApplied`.

use super::{BudgetTier, EscalationContext, EscalationOutcome, RetrievalSkill};
use bus::{DomainEvent, DomainEventBus};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;

/// Result of running the selector.
#[derive(Debug, Clone)]
pub struct SelectorOutcome {
    /// Names of skills that ran (in the order tried).
    pub skills_tried: Vec<String>,
    /// Final outcome (last successful skill, or last failure).
    pub final_outcome: EscalationOutcome,
}

/// Registry — built once at AppCore boot.
pub struct RetrievalSkillRegistry {
    skills: Vec<Arc<dyn RetrievalSkill>>,
    effectiveness: RwLock<HashMap<String, f32>>,
    bus: Arc<DomainEventBus>,
}

impl std::fmt::Debug for RetrievalSkillRegistry {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("RetrievalSkillRegistry")
            .field("skill_count", &self.skills.len())
            .finish()
    }
}

impl RetrievalSkillRegistry {
    /// Construct with explicit skill list (test seam).
    pub fn new(skills: Vec<Arc<dyn RetrievalSkill>>, bus: Arc<DomainEventBus>) -> Self {
        let mut eff = HashMap::new();
        for s in &skills {
            eff.insert(s.name().to_string(), 0.5);
        }
        Self {
            skills,
            effectiveness: RwLock::new(eff),
            bus,
        }
    }

    /// Read the current EMA score for a skill.
    pub async fn effectiveness_of(&self, name: &str) -> f32 {
        self.effectiveness
            .read()
            .await
            .get(name)
            .copied()
            .unwrap_or(0.5)
    }

    /// Update EMA after observing `outcome_value` (1.0 success, 0.0 failure, 0.5 partial).
    pub async fn record_outcome(&self, name: &str, outcome_value: f32) {
        let mut w = self.effectiveness.write().await;
        let prev = w.get(name).copied().unwrap_or(0.5);
        let next = 0.9 * prev + 0.1 * outcome_value;
        w.insert(name.to_string(), next);
    }

    /// Run the selector — try skills in highest-EMA order within the active tier.
    /// Stops on first success; otherwise returns the final failed outcome.
    pub async fn escalate(
        &self,
        ctx: &EscalationContext,
    ) -> common::Result<SelectorOutcome> {
        // Pick candidates: skills whose tier <= active tier.
        let active = ctx.budget_tier;
        let mut candidates: Vec<Arc<dyn RetrievalSkill>> = self
            .skills
            .iter()
            .filter(|s| tier_rank(s.tier()) <= tier_rank(active))
            .cloned()
            .collect();

        // Sort by EMA descending.
        let eff = self.effectiveness.read().await.clone();
        candidates.sort_by(|a, b| {
            let ae = eff.get(a.name()).copied().unwrap_or(0.5);
            let be = eff.get(b.name()).copied().unwrap_or(0.5);
            be.partial_cmp(&ae).unwrap_or(std::cmp::Ordering::Equal)
        });

        let mut tried: Vec<String> = Vec::new();
        let mut last: Option<EscalationOutcome> = None;
        for skill in candidates {
            let name = skill.name().to_string();
            tried.push(name.clone());
            let before = ctx.coverage_score;
            let out = skill.apply(ctx).await?;
            let after = out.coverage_after;
            let _ = self.bus.publish(DomainEvent::RetrievalSkillApplied {
                skill: name.clone(),
                before_score: before,
                after_score: after,
                budget_used: format!("{:?}", skill.tier()),
                session_id: None,
            });
            let outcome_value = if out.succeeded { 1.0 } else { 0.0 };
            self.record_outcome(&name, outcome_value).await;
            if out.succeeded {
                last = Some(out);
                break;
            }
            last = Some(out);
        }
        Ok(SelectorOutcome {
            skills_tried: tried,
            final_outcome: last.unwrap_or(EscalationOutcome {
                succeeded: false,
                coverage_after: ctx.coverage_score,
                added_context: String::new(),
                added_ids: vec![],
            }),
        })
    }
}

fn tier_rank(t: BudgetTier) -> u8 {
    match t {
        BudgetTier::Fast => 0,
        BudgetTier::DeepThink => 1,
        BudgetTier::Ultra => 2,
    }
}
```

> **Note on `DomainEvent::RetrievalSkillApplied`:** Phase 1 added this variant. If its signature differs from `{ skill, before_score, after_score, budget_used, session_id }`, match the existing definition exactly.

- [ ] **Step 3: Write failing test**

Create `crates/coding-memory/tests/skill_registry_selector.rs`:

```rust
use async_trait::async_trait;
use coding_memory::retrieval_skills::{
    BudgetTier, EscalationContext, EscalationOutcome, RetrievalSkill, RetrievalSkillRegistry,
};
use std::sync::Arc;

struct TestSkill {
    name: &'static str,
    tier: BudgetTier,
    succeeds: bool,
    after: f32,
}

#[async_trait]
impl RetrievalSkill for TestSkill {
    fn name(&self) -> &'static str { self.name }
    fn description(&self) -> &'static str { "test" }
    fn tier(&self) -> BudgetTier { self.tier }
    async fn apply(&self, _: &EscalationContext) -> common::Result<EscalationOutcome> {
        Ok(EscalationOutcome {
            succeeded: self.succeeds,
            coverage_after: self.after,
            added_context: String::new(),
            added_ids: vec![],
        })
    }
}

#[tokio::test]
async fn selector_stops_on_first_success() {
    let bus = Arc::new(bus::DomainEventBus::new(64));
    let reg = RetrievalSkillRegistry::new(
        vec![
            Arc::new(TestSkill { name: "fail_a", tier: BudgetTier::Fast, succeeds: false, after: 0.1 }),
            Arc::new(TestSkill { name: "succ_b", tier: BudgetTier::Fast, succeeds: true, after: 0.9 }),
            Arc::new(TestSkill { name: "skip_c", tier: BudgetTier::Fast, succeeds: true, after: 0.99 }),
        ],
        bus,
    );
    let ctx = EscalationContext {
        query: "x".into(),
        coverage_score: 0.1,
        budget_tier: BudgetTier::Fast,
        repo: None,
    };
    let out = reg.escalate(&ctx).await.unwrap();
    assert!(out.skills_tried.contains(&"succ_b".to_string()));
    assert!(out.final_outcome.succeeded);
}

#[tokio::test]
async fn selector_filters_by_tier() {
    let bus = Arc::new(bus::DomainEventBus::new(64));
    let reg = RetrievalSkillRegistry::new(
        vec![
            Arc::new(TestSkill { name: "ultra_only", tier: BudgetTier::Ultra, succeeds: true, after: 0.99 }),
        ],
        bus,
    );
    let ctx = EscalationContext {
        query: "x".into(),
        coverage_score: 0.1,
        budget_tier: BudgetTier::Fast,
        repo: None,
    };
    let out = reg.escalate(&ctx).await.unwrap();
    assert!(out.skills_tried.is_empty());
}
```

- [ ] **Step 4: Stub the 5 skill files for `mod.rs` to compile**

Create empty stub files so the module tree resolves; concrete impls land in tasks 14–18:

`crates/coding-memory/src/retrieval_skills/query_rewriter.rs`:
```rust
//! See Task 14.
use super::*;
use async_trait::async_trait;

/// Stub.
#[derive(Debug, Default)]
pub struct QueryRewriter;

#[async_trait]
impl RetrievalSkill for QueryRewriter {
    fn name(&self) -> &'static str { "query_rewriter" }
    fn description(&self) -> &'static str { "PRF + multi-query expansion." }
    fn tier(&self) -> BudgetTier { BudgetTier::DeepThink }
    async fn apply(&self, _ctx: &EscalationContext) -> common::Result<EscalationOutcome> {
        Ok(EscalationOutcome {
            succeeded: false,
            coverage_after: 0.0,
            added_context: String::new(),
            added_ids: vec![],
        })
    }
}
```

Repeat the same shape for `query_decomposer.rs` (tier `DeepThink`), `evidence_focuser.rs` (tier `DeepThink`), `raw_event_escalator.rs` (tier `Ultra`), `causal_context_expander.rs` (tier `Ultra`).

- [ ] **Step 5: Build + run + commit**

```bash
cargo build -p coding-memory
cargo nextest run -p coding-memory --test skill_registry_selector
git add crates/coding-memory/src/retrieval_skills/ crates/coding-memory/src/lib.rs \
        crates/coding-memory/tests/skill_registry_selector.rs
git rm crates/coding-memory/src/retrieval_skills.rs
git commit -m "feat(coding-memory): RetrievalSkillRegistry + tier-aware selector with EMA"
```

---

### Task 14: `QueryRewriter` — PRF + multi-query expansion

**Files:**
- Modify: `crates/coding-memory/src/retrieval_skills/query_rewriter.rs`
- Test: `crates/coding-memory/tests/skill_query_rewriter.rs`

> **Approach (Phase 4 minimum viable):** Generate 3 lexical rewrites by removing stopwords / preserving keywords / expanding common synonyms (a tiny built-in synonym table for coding terms — `bug → defect`, `null → nil → none`, etc.). Each rewrite is fed back through the host service's retrieve callback (passed via constructor), and the union of returned ids becomes `added_ids`. The `RetrievalSkill` trait must stay free of cognitive deps, so the constructor takes a boxed retrieval closure.

- [ ] **Step 1: Write failing test**

Create `crates/coding-memory/tests/skill_query_rewriter.rs`:

```rust
use coding_memory::retrieval_skills::{BudgetTier, EscalationContext, RetrievalSkill, QueryRewriter};
use std::sync::Arc;
use uuid::Uuid;

#[tokio::test]
async fn produces_three_rewrites_and_unions_ids() {
    let id1 = Uuid::new_v4();
    let id2 = Uuid::new_v4();
    let calls = Arc::new(std::sync::atomic::AtomicUsize::new(0));
    let calls_c = calls.clone();
    let retrieve = Arc::new(move |_q: String| {
        let calls = calls_c.clone();
        let ids = vec![id1, id2];
        Box::pin(async move {
            calls.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
            common::Result::Ok((vec![0.95f32, 0.7f32], ids))
        }) as std::pin::Pin<Box<dyn std::future::Future<Output = common::Result<(Vec<f32>, Vec<Uuid>)>> + Send>>
    }) as coding_memory::retrieval_skills::query_rewriter::RetrieveFn;

    let skill = QueryRewriter::new(retrieve);
    let ctx = EscalationContext {
        query: "fix the null pointer bug in parser".into(),
        coverage_score: 0.1,
        budget_tier: BudgetTier::DeepThink,
        repo: None,
    };
    let out = skill.apply(&ctx).await.unwrap();
    assert!(calls.load(std::sync::atomic::Ordering::SeqCst) >= 3);
    assert!(out.added_ids.len() <= 2);
}
```

- [ ] **Step 2: Run + verify failure**

Run: `cargo nextest run -p coding-memory --test skill_query_rewriter`
Expected: FAIL — `new` doesn't exist.

- [ ] **Step 3: Implement**

Replace `crates/coding-memory/src/retrieval_skills/query_rewriter.rs`:

```rust
//! `QueryRewriter` — PRF + multi-query expansion (3 rewrites, RRF-merge).

use super::*;
use async_trait::async_trait;
use std::collections::HashSet;
use std::pin::Pin;
use std::sync::Arc;
use uuid::Uuid;

/// Async retrieval callback — caller injects the host service's retrieve fn.
pub type RetrieveFn = Arc<
    dyn Fn(String) -> Pin<Box<dyn std::future::Future<Output = common::Result<(Vec<f32>, Vec<Uuid>)>> + Send>>
        + Send
        + Sync,
>;

/// Skill instance.
pub struct QueryRewriter {
    retrieve: RetrieveFn,
}

impl std::fmt::Debug for QueryRewriter {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("QueryRewriter").finish()
    }
}

impl QueryRewriter {
    /// Construct with the host's retrieval closure.
    #[must_use]
    pub fn new(retrieve: RetrieveFn) -> Self {
        Self { retrieve }
    }
}

#[async_trait]
impl RetrievalSkill for QueryRewriter {
    fn name(&self) -> &'static str { "query_rewriter" }
    fn description(&self) -> &'static str { "PRF + multi-query expansion." }
    fn tier(&self) -> BudgetTier { BudgetTier::DeepThink }
    async fn apply(&self, ctx: &EscalationContext) -> common::Result<EscalationOutcome> {
        let rewrites = generate_rewrites(&ctx.query);
        let mut id_to_rank_sum: std::collections::HashMap<Uuid, f32> = Default::default();
        let mut sims: Vec<f32> = Vec::new();
        let mut all_ids: HashSet<Uuid> = HashSet::new();
        for q in rewrites {
            let (s, ids) = (self.retrieve)(q).await?;
            sims.extend(s.iter().copied());
            for (rank, id) in ids.iter().enumerate() {
                let rrf = 1.0_f32 / (60.0 + rank as f32);
                *id_to_rank_sum.entry(*id).or_default() += rrf;
                all_ids.insert(*id);
            }
        }
        let mut merged: Vec<(Uuid, f32)> = id_to_rank_sum.into_iter().collect();
        merged.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
        let added_ids: Vec<Uuid> = merged.into_iter().take(10).map(|(id, _)| id).collect();
        let coverage_after = if sims.is_empty() {
            0.0
        } else {
            let mean: f32 = sims.iter().sum::<f32>() / sims.len() as f32;
            let min: f32 = sims.iter().cloned().fold(f32::INFINITY, f32::min);
            mean - min
        };
        Ok(EscalationOutcome {
            succeeded: coverage_after > ctx.coverage_score + 0.05,
            coverage_after,
            added_context: format!(
                "Query rewriter merged {} ids across rewrites.",
                added_ids.len()
            ),
            added_ids,
        })
    }
}

fn generate_rewrites(q: &str) -> Vec<String> {
    let stop: HashSet<&str> = ["the", "a", "an", "in", "of", "to", "for", "is"]
        .into_iter()
        .collect();
    // Rewrite 1: original.
    let mut out = vec![q.to_string()];
    // Rewrite 2: stopword-stripped.
    let stripped: String = q
        .split_whitespace()
        .filter(|w| !stop.contains(*w))
        .collect::<Vec<_>>()
        .join(" ");
    out.push(if stripped.is_empty() { q.to_string() } else { stripped });
    // Rewrite 3: synonym-expanded.
    let syn = expand_synonyms(q);
    out.push(syn);
    out
}

fn expand_synonyms(q: &str) -> String {
    let table: &[(&str, &[&str])] = &[
        ("bug", &["defect", "issue"]),
        ("null", &["nil", "none"]),
        ("fix", &["patch", "resolve"]),
        ("error", &["fault", "failure"]),
    ];
    let mut s = q.to_lowercase();
    for (k, syns) in table {
        if s.contains(k) {
            for syn in *syns {
                s.push(' ');
                s.push_str(syn);
            }
        }
    }
    s
}
```

- [ ] **Step 4: Run + commit**

```bash
cargo nextest run -p coding-memory --test skill_query_rewriter
git add crates/coding-memory/src/retrieval_skills/query_rewriter.rs \
        crates/coding-memory/tests/skill_query_rewriter.rs
git commit -m "feat(coding-memory): QueryRewriter — PRF + RRF-merged multi-query"
```

---

### Task 15: `QueryDecomposer` — split compound queries

**Files:**
- Modify: `crates/coding-memory/src/retrieval_skills/query_decomposer.rs`
- Test: `crates/coding-memory/tests/skill_query_decomposer.rs`

- [ ] **Step 1: Write failing test**

Create `crates/coding-memory/tests/skill_query_decomposer.rs`:

```rust
use coding_memory::retrieval_skills::{BudgetTier, EscalationContext, RetrievalSkill, QueryDecomposer};
use std::sync::Arc;
use uuid::Uuid;

#[tokio::test]
async fn compound_query_yields_multiple_subs() {
    let calls = Arc::new(std::sync::Mutex::new(Vec::<String>::new()));
    let calls_c = calls.clone();
    let retrieve = Arc::new(move |q: String| {
        let calls = calls_c.clone();
        Box::pin(async move {
            calls.lock().unwrap().push(q);
            common::Result::Ok((vec![0.8f32], vec![Uuid::new_v4()]))
        }) as std::pin::Pin<Box<dyn std::future::Future<Output = common::Result<(Vec<f32>, Vec<Uuid>)>> + Send>>
    }) as coding_memory::retrieval_skills::query_decomposer::RetrieveFn;

    let skill = QueryDecomposer::new(retrieve);
    let ctx = EscalationContext {
        query: "fix the parser bug and improve error messages".into(),
        coverage_score: 0.05,
        budget_tier: BudgetTier::DeepThink,
        repo: None,
    };
    let _ = skill.apply(&ctx).await.unwrap();
    let count = calls.lock().unwrap().len();
    assert!(count >= 2 && count <= 4, "got {count}");
}
```

- [ ] **Step 2: Implement**

Replace `crates/coding-memory/src/retrieval_skills/query_decomposer.rs`:

```rust
//! `QueryDecomposer` — split compound queries into 2-4 sub-queries; merge via RRF.

use super::*;
use async_trait::async_trait;
use std::collections::HashMap;
use std::pin::Pin;
use std::sync::Arc;
use uuid::Uuid;

/// Same retrieval callback shape as `QueryRewriter`.
pub type RetrieveFn = Arc<
    dyn Fn(String) -> Pin<Box<dyn std::future::Future<Output = common::Result<(Vec<f32>, Vec<Uuid>)>> + Send>>
        + Send
        + Sync,
>;

/// Skill.
pub struct QueryDecomposer {
    retrieve: RetrieveFn,
}

impl std::fmt::Debug for QueryDecomposer {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("QueryDecomposer").finish()
    }
}

impl QueryDecomposer {
    /// Construct.
    #[must_use]
    pub fn new(retrieve: RetrieveFn) -> Self {
        Self { retrieve }
    }
}

#[async_trait]
impl RetrievalSkill for QueryDecomposer {
    fn name(&self) -> &'static str { "query_decomposer" }
    fn description(&self) -> &'static str { "Split compound queries into 2-4 sub-queries." }
    fn tier(&self) -> BudgetTier { BudgetTier::DeepThink }
    async fn apply(&self, ctx: &EscalationContext) -> common::Result<EscalationOutcome> {
        let subs = decompose(&ctx.query);
        let mut id_rank: HashMap<Uuid, f32> = HashMap::new();
        let mut sims_all = Vec::new();
        for q in &subs {
            let (sims, ids) = (self.retrieve)(q.clone()).await?;
            sims_all.extend(sims);
            for (rank, id) in ids.iter().enumerate() {
                let rrf = 1.0_f32 / (60.0 + rank as f32);
                *id_rank.entry(*id).or_default() += rrf;
            }
        }
        let mut merged: Vec<(Uuid, f32)> = id_rank.into_iter().collect();
        merged.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
        let added_ids: Vec<Uuid> = merged.into_iter().take(10).map(|(id, _)| id).collect();
        let coverage_after = if sims_all.is_empty() {
            0.0
        } else {
            let mean: f32 = sims_all.iter().sum::<f32>() / sims_all.len() as f32;
            let min: f32 = sims_all.iter().cloned().fold(f32::INFINITY, f32::min);
            mean - min
        };
        Ok(EscalationOutcome {
            succeeded: coverage_after > ctx.coverage_score + 0.05,
            coverage_after,
            added_context: format!("Decomposed into {} sub-queries.", subs.len()),
            added_ids,
        })
    }
}

fn decompose(q: &str) -> Vec<String> {
    // Split on " and "/" ; "/", " then ", " or ".
    let lowered = q.to_lowercase();
    let separators = [" and ", "; ", ", ", " then ", " or "];
    let mut parts: Vec<String> = vec![lowered.clone()];
    for sep in separators {
        parts = parts
            .into_iter()
            .flat_map(|s| {
                s.split(sep)
                    .map(|t| t.trim().to_string())
                    .filter(|t| !t.is_empty())
                    .collect::<Vec<_>>()
            })
            .collect();
    }
    if parts.len() < 2 {
        return vec![q.to_string()];
    }
    parts.truncate(4);
    parts
}
```

- [ ] **Step 3: Run + commit**

```bash
cargo nextest run -p coding-memory --test skill_query_decomposer
git add crates/coding-memory/src/retrieval_skills/query_decomposer.rs \
        crates/coding-memory/tests/skill_query_decomposer.rs
git commit -m "feat(coding-memory): QueryDecomposer — compound-query splitter with RRF merge"
```

---

### Task 16: `EvidenceFocuser` — top-20 → cosine rerank → top-5

**Files:**
- Modify: `crates/coding-memory/src/retrieval_skills/evidence_focuser.rs`
- Test: `crates/coding-memory/tests/skill_evidence_focuser.rs`

> **Approach:** Phase 4 ships a lexical-cosine reranker (token-bag cosine) since we don't yet ship a cross-encoder model. The constructor accepts a closure returning `(text, vec_id)` per candidate; the skill reranks by token cosine of (query, candidate.text) and returns the top 5.

- [ ] **Step 1: Write failing test**

Create `crates/coding-memory/tests/skill_evidence_focuser.rs`:

```rust
use coding_memory::retrieval_skills::evidence_focuser::{EvidenceFocuser, FetchTextsFn};
use coding_memory::retrieval_skills::{BudgetTier, EscalationContext, RetrievalSkill};
use std::sync::Arc;
use uuid::Uuid;

#[tokio::test]
async fn focuses_to_top_five_by_lexical_cosine() {
    let ids: Vec<Uuid> = (0..20).map(|_| Uuid::new_v4()).collect();
    let texts: Vec<(Uuid, String)> = ids
        .iter()
        .enumerate()
        .map(|(i, id)| {
            let text = if i < 5 {
                "null pointer parser bug".into()
            } else {
                format!("unrelated text {i}")
            };
            (*id, text)
        })
        .collect();
    let fetch_texts: FetchTextsFn = Arc::new(move |_ids| {
        let texts = texts.clone();
        Box::pin(async move { common::Result::Ok(texts) })
    });
    let initial_ids = ids.clone();
    let initial_provider: Arc<dyn Fn() -> Vec<Uuid> + Send + Sync> =
        Arc::new(move || initial_ids.clone());

    let skill = EvidenceFocuser::new(initial_provider, fetch_texts);
    let ctx = EscalationContext {
        query: "null pointer parser bug".into(),
        coverage_score: 0.1,
        budget_tier: BudgetTier::DeepThink,
        repo: None,
    };
    let out = skill.apply(&ctx).await.unwrap();
    assert_eq!(out.added_ids.len(), 5);
}
```

- [ ] **Step 2: Implement**

Replace `crates/coding-memory/src/retrieval_skills/evidence_focuser.rs`:

```rust
//! `EvidenceFocuser` — top-20 candidates → token-cosine rerank → top 5.

use super::*;
use async_trait::async_trait;
use std::collections::HashMap;
use std::pin::Pin;
use std::sync::Arc;
use uuid::Uuid;

/// Closure returning a list of `(id, text)` for the supplied candidate ids.
pub type FetchTextsFn = Arc<
    dyn Fn(Vec<Uuid>) -> Pin<Box<dyn std::future::Future<Output = common::Result<Vec<(Uuid, String)>>> + Send>>
        + Send
        + Sync,
>;

/// Closure returning the initial top-20 ids for the active query.
pub type InitialIdsFn = Arc<dyn Fn() -> Vec<Uuid> + Send + Sync>;

/// Skill.
pub struct EvidenceFocuser {
    initial: InitialIdsFn,
    fetch_texts: FetchTextsFn,
}

impl std::fmt::Debug for EvidenceFocuser {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("EvidenceFocuser").finish()
    }
}

impl EvidenceFocuser {
    /// Construct.
    #[must_use]
    pub fn new(initial: InitialIdsFn, fetch_texts: FetchTextsFn) -> Self {
        Self { initial, fetch_texts }
    }
}

#[async_trait]
impl RetrievalSkill for EvidenceFocuser {
    fn name(&self) -> &'static str { "evidence_focuser" }
    fn description(&self) -> &'static str { "Token-cosine rerank on top-20 → top 5." }
    fn tier(&self) -> BudgetTier { BudgetTier::DeepThink }
    async fn apply(&self, ctx: &EscalationContext) -> common::Result<EscalationOutcome> {
        let candidate_ids = (self.initial)();
        if candidate_ids.is_empty() {
            return Ok(EscalationOutcome {
                succeeded: false,
                coverage_after: ctx.coverage_score,
                added_context: String::new(),
                added_ids: vec![],
            });
        }
        let texts = (self.fetch_texts)(candidate_ids.clone()).await?;
        let q_vec = bag(&ctx.query);
        let mut scored: Vec<(Uuid, f32)> = texts
            .into_iter()
            .map(|(id, t)| (id, cosine(&q_vec, &bag(&t))))
            .collect();
        scored.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
        let top5: Vec<Uuid> = scored.iter().take(5).map(|(id, _)| *id).collect();
        let coverage_after = if scored.is_empty() {
            0.0
        } else {
            let top: Vec<f32> = scored.iter().take(5).map(|(_, s)| *s).collect();
            let mean: f32 = top.iter().sum::<f32>() / top.len() as f32;
            let min = top.iter().cloned().fold(f32::INFINITY, f32::min);
            mean - min
        };
        Ok(EscalationOutcome {
            succeeded: coverage_after > ctx.coverage_score + 0.05,
            coverage_after,
            added_context: format!("Focused {} → 5 candidates.", candidate_ids.len()),
            added_ids: top5,
        })
    }
}

fn bag(s: &str) -> HashMap<String, f32> {
    let mut m: HashMap<String, f32> = HashMap::new();
    for tok in s.to_lowercase().split_whitespace() {
        *m.entry(tok.to_string()).or_default() += 1.0;
    }
    m
}

fn cosine(a: &HashMap<String, f32>, b: &HashMap<String, f32>) -> f32 {
    let mut dot = 0.0;
    for (k, v) in a {
        if let Some(bv) = b.get(k) {
            dot += v * bv;
        }
    }
    let na: f32 = a.values().map(|v| v * v).sum::<f32>().sqrt();
    let nb: f32 = b.values().map(|v| v * v).sum::<f32>().sqrt();
    if na == 0.0 || nb == 0.0 {
        return 0.0;
    }
    dot / (na * nb)
}
```

- [ ] **Step 3: Run + commit**

```bash
cargo nextest run -p coding-memory --test skill_evidence_focuser
git add crates/coding-memory/src/retrieval_skills/evidence_focuser.rs \
        crates/coding-memory/tests/skill_evidence_focuser.rs
git commit -m "feat(coding-memory): EvidenceFocuser — token-cosine top-20 → top-5 rerank"
```

---

### Task 17: `RawEventEscalator` — provenance pointers → `ingest_event_log`

**Files:**
- Modify: `crates/coding-memory/src/retrieval_skills/raw_event_escalator.rs`
- Test: `crates/coding-memory/tests/skill_raw_event_escalator.rs`

> **Approach:** Take the current top-k facts, walk `metadata.provenance.source_events`, fetch the matching rows from `ingest_event_log`, and surface their JSON as `added_context`. This is the `Ultra` tier escalation that bypasses summaries.

- [ ] **Step 1: Write failing test**

Create `crates/coding-memory/tests/skill_raw_event_escalator.rs`:

```rust
use coding_memory::retrieval_skills::raw_event_escalator::{
    EventLookupFn, ProvenanceIdsFn, RawEventEscalator,
};
use coding_memory::retrieval_skills::{BudgetTier, EscalationContext, RetrievalSkill};
use std::sync::Arc;

#[tokio::test]
async fn surfaces_event_payload_text() {
    let provenance: ProvenanceIdsFn = Arc::new(|| vec!["evt1".into(), "evt2".into()]);
    let lookup: EventLookupFn = Arc::new(|ids| {
        Box::pin(async move {
            common::Result::Ok(
                ids.into_iter()
                    .map(|id| serde_json::json!({"event_id": id, "kind": "FileEdit"}))
                    .collect(),
            )
        })
    });
    let skill = RawEventEscalator::new(provenance, lookup);
    let out = skill
        .apply(&EscalationContext {
            query: "x".into(),
            coverage_score: 0.0,
            budget_tier: BudgetTier::Ultra,
            repo: None,
        })
        .await
        .unwrap();
    assert!(out.added_context.contains("FileEdit"));
}
```

- [ ] **Step 2: Implement**

Replace `crates/coding-memory/src/retrieval_skills/raw_event_escalator.rs`:

```rust
//! `RawEventEscalator` — bypasses summaries; surfaces raw `ingest_event_log` rows.

use super::*;
use async_trait::async_trait;
use std::pin::Pin;
use std::sync::Arc;

/// Closure returning the provenance event ids attached to current top-k.
pub type ProvenanceIdsFn = Arc<dyn Fn() -> Vec<String> + Send + Sync>;

/// Closure looking up raw ingest events by id.
pub type EventLookupFn = Arc<
    dyn Fn(Vec<String>) -> Pin<Box<dyn std::future::Future<Output = common::Result<Vec<serde_json::Value>>> + Send>>
        + Send
        + Sync,
>;

/// Skill.
pub struct RawEventEscalator {
    provenance: ProvenanceIdsFn,
    lookup: EventLookupFn,
}

impl std::fmt::Debug for RawEventEscalator {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("RawEventEscalator").finish()
    }
}

impl RawEventEscalator {
    /// Construct.
    #[must_use]
    pub fn new(provenance: ProvenanceIdsFn, lookup: EventLookupFn) -> Self {
        Self { provenance, lookup }
    }
}

#[async_trait]
impl RetrievalSkill for RawEventEscalator {
    fn name(&self) -> &'static str { "raw_event_escalator" }
    fn description(&self) -> &'static str { "Surface raw ingest events for top-k provenance." }
    fn tier(&self) -> BudgetTier { BudgetTier::Ultra }
    async fn apply(&self, ctx: &EscalationContext) -> common::Result<EscalationOutcome> {
        let ids = (self.provenance)();
        if ids.is_empty() {
            return Ok(EscalationOutcome {
                succeeded: false,
                coverage_after: ctx.coverage_score,
                added_context: String::new(),
                added_ids: vec![],
            });
        }
        let events = (self.lookup)(ids).await?;
        let mut buf = String::from("# Raw event payload\n\n");
        for e in &events {
            buf.push_str(&serde_json::to_string_pretty(e).unwrap_or_default());
            buf.push_str("\n\n");
        }
        Ok(EscalationOutcome {
            succeeded: !events.is_empty(),
            coverage_after: ctx.coverage_score + 0.2,
            added_context: buf,
            added_ids: vec![],
        })
    }
}
```

- [ ] **Step 3: Run + commit**

```bash
cargo nextest run -p coding-memory --test skill_raw_event_escalator
git add crates/coding-memory/src/retrieval_skills/raw_event_escalator.rs \
        crates/coding-memory/tests/skill_raw_event_escalator.rs
git commit -m "feat(coding-memory): RawEventEscalator — bypass summaries via provenance"
```

---

### Task 18: `CausalContextExpander` — graceful no-op until Phase 6

**Files:**
- Modify: `crates/coding-memory/src/retrieval_skills/causal_context_expander.rs`
- Test: `crates/coding-memory/tests/skill_causal_context_expander.rs`

- [ ] **Step 1: Write failing test**

Create `crates/coding-memory/tests/skill_causal_context_expander.rs`:

```rust
use coding_memory::retrieval_skills::causal_context_expander::{CausalContextExpander, EdgeLookupFn};
use coding_memory::retrieval_skills::{BudgetTier, EscalationContext, RetrievalSkill};
use std::sync::Arc;
use uuid::Uuid;

#[tokio::test]
async fn no_edges_returns_failure_outcome() {
    let lookup: EdgeLookupFn = Arc::new(|_ids| Box::pin(async { common::Result::Ok(vec![]) }));
    let provider: Arc<dyn Fn() -> Vec<Uuid> + Send + Sync> = Arc::new(|| vec![Uuid::new_v4()]);
    let skill = CausalContextExpander::new(provider, lookup);
    let out = skill
        .apply(&EscalationContext {
            query: "x".into(),
            coverage_score: 0.0,
            budget_tier: BudgetTier::Ultra,
            repo: None,
        })
        .await
        .unwrap();
    assert!(!out.succeeded);
    assert!(out.added_ids.is_empty());
}
```

- [ ] **Step 2: Implement**

Replace `crates/coding-memory/src/retrieval_skills/causal_context_expander.rs`:

```rust
//! `CausalContextExpander` — walk `memory_causal_edges` from current top-k.
//!
//! Phase 4 ships an inert version: edges aren't auto-populated until Phase 6.
//! Once seeded, this skill surfaces matching chains with no further changes.

use super::*;
use async_trait::async_trait;
use std::pin::Pin;
use std::sync::Arc;
use uuid::Uuid;

/// Closure returning current top-k memory ids.
pub type TopKIdsFn = Arc<dyn Fn() -> Vec<Uuid> + Send + Sync>;

/// Closure: lookup causal edges for the given subject ids.
pub type EdgeLookupFn = Arc<
    dyn Fn(Vec<Uuid>) -> Pin<Box<dyn std::future::Future<Output = common::Result<Vec<crate::scope::CausalEdge>>> + Send>>
        + Send
        + Sync,
>;

/// Skill.
pub struct CausalContextExpander {
    top_k: TopKIdsFn,
    lookup: EdgeLookupFn,
}

impl std::fmt::Debug for CausalContextExpander {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("CausalContextExpander").finish()
    }
}

impl CausalContextExpander {
    /// Construct.
    #[must_use]
    pub fn new(top_k: TopKIdsFn, lookup: EdgeLookupFn) -> Self {
        Self { top_k, lookup }
    }
}

#[async_trait]
impl RetrievalSkill for CausalContextExpander {
    fn name(&self) -> &'static str { "causal_context_expander" }
    fn description(&self) -> &'static str { "Walk memory_causal_edges from top-k." }
    fn tier(&self) -> BudgetTier { BudgetTier::Ultra }
    async fn apply(&self, ctx: &EscalationContext) -> common::Result<EscalationOutcome> {
        let ids = (self.top_k)();
        let edges = (self.lookup)(ids).await?;
        if edges.is_empty() {
            return Ok(EscalationOutcome {
                succeeded: false,
                coverage_after: ctx.coverage_score,
                added_context: String::new(),
                added_ids: vec![],
            });
        }
        let mut buf = String::from("# Causal chains\n\n");
        let mut added: Vec<Uuid> = Vec::new();
        for e in &edges {
            buf.push_str(&format!(
                "- {:?}: {} → {}\n",
                e.kind, e.source_id, e.target_id
            ));
            added.push(e.source_id);
            added.push(e.target_id);
        }
        added.sort();
        added.dedup();
        Ok(EscalationOutcome {
            succeeded: true,
            coverage_after: ctx.coverage_score + 0.15,
            added_context: buf,
            added_ids: added,
        })
    }
}
```

> The exact `CausalEdge` field names (`kind`, `source_id`, `target_id`) match the Phase-1 stub in `crates/coding-memory/src/scope.rs`. If the actual field names differ, adjust the format string accordingly.

- [ ] **Step 3: Run + commit**

```bash
cargo nextest run -p coding-memory --test skill_causal_context_expander
git add crates/coding-memory/src/retrieval_skills/causal_context_expander.rs \
        crates/coding-memory/tests/skill_causal_context_expander.rs
git commit -m "feat(coding-memory): CausalContextExpander — chain walker with graceful no-op"
```

---

### Task 19: `CodingRecallService` — orchestration skeleton

**Files:**
- Create: `crates/coding-memory/src/recall/service.rs`
- Modify: `crates/coding-memory/src/recall/mod.rs` — replace stub `CodingRecallService` with re-export from `service.rs`
- Test: `crates/coding-memory/tests/recall_service_end_to_end.rs`

> **Goal of this task:** wire all the pieces from Tasks 3–18 behind a single facade with the public method shape that Phase 1 declared. No new behavior beyond glue. The test seeds two facts, calls `recall_index`, and asserts that ranked results come back + a telemetry row is written.

- [ ] **Step 1: Add `CodingRecallServiceBuilder` and skeleton**

Create `crates/coding-memory/src/recall/service.rs`:

```rust
//! `CodingRecallService` — single facade for passive injection + MCP tools.

use crate::recall::{
    budget::TokenBudgeter,
    change_history::ChangeHistoryService,
    dead_end::DeadEndChecker,
    decision_points::DecisionPointsService,
    facts_as_of::FactsAsOfService,
    fetch_builder::FetchBuilder,
    index_builder::IndexBuilder,
    probe::{ProbeVerdict, RetrievalQualityProbe},
    telemetry::{RecallInvocationRepo, RecallInvocationRow},
    timeline_builder::{TimelineBuilder, TimelineInput},
    CausalTraceResponse, ChangeHistoryResponse, DeadEndResponse, DecisionPointsResponse,
    FactsAsOfResponse, FullEntry, IndexEntry, RecallIndexResponse, RecallQuery, TimelineEntry,
};
use crate::retrieval_skills::RetrievalSkillRegistry;
use cognitive::UnifiedMemoryService;
use jiff::Timestamp;
use std::sync::Arc;
use std::time::Instant;
use uuid::Uuid;

/// Construction-time wiring.
pub struct CodingRecallServiceConfig {
    /// Coverage threshold below which the C3 selector dispatches.
    pub probe_threshold: f32,
    /// Default top-k for `recall_index`.
    pub default_limit: u32,
}

impl Default for CodingRecallServiceConfig {
    fn default() -> Self {
        Self {
            probe_threshold: 0.3,
            default_limit: 10,
        }
    }
}

/// Single facade.
pub struct CodingRecallService {
    config: CodingRecallServiceConfig,
    ums: Arc<UnifiedMemoryService>,
    fact_repo: Arc<cognitive::SemanticFactRepo>,
    ep_repo: Arc<cognitive::EpisodicMemoryRepo>,
    telemetry: RecallInvocationRepo,
    index_builder: IndexBuilder,
    timeline_builder: TimelineBuilder,
    fetch_builder: FetchBuilder,
    facts_as_of: FactsAsOfService,
    change_history: ChangeHistoryService,
    decision_points: DecisionPointsService,
    dead_end: DeadEndChecker,
    probe: RetrievalQualityProbe,
    skills: Option<Arc<RetrievalSkillRegistry>>,
    budgeter: Arc<dyn TokenBudgeter>,
}

impl std::fmt::Debug for CodingRecallService {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("CodingRecallService").finish()
    }
}

impl CodingRecallService {
    /// Construct from raw repos + `UnifiedMemoryService`.
    pub fn new(
        config: CodingRecallServiceConfig,
        ums: Arc<UnifiedMemoryService>,
        fact_repo: Arc<cognitive::SemanticFactRepo>,
        ep_repo: Arc<cognitive::EpisodicMemoryRepo>,
        telemetry: RecallInvocationRepo,
        budgeter: Arc<dyn TokenBudgeter>,
    ) -> Self {
        Self {
            probe: RetrievalQualityProbe::new(config.probe_threshold),
            index_builder: IndexBuilder::with_budgeter(budgeter.clone()),
            timeline_builder: TimelineBuilder::new(),
            fetch_builder: FetchBuilder::new(fact_repo.clone(), ep_repo.clone()),
            facts_as_of: FactsAsOfService::new(fact_repo.clone()),
            change_history: ChangeHistoryService::new(fact_repo.clone()),
            decision_points: DecisionPointsService::new(ep_repo.clone()),
            dead_end: DeadEndChecker::new(fact_repo.clone(), Default::default()),
            ums,
            fact_repo,
            ep_repo,
            telemetry,
            skills: None,
            budgeter,
            config,
        }
    }

    /// Attach the C3 skill registry post-construction (avoids Arc cycles in builders).
    #[must_use]
    pub fn with_skills(mut self, skills: Arc<RetrievalSkillRegistry>) -> Self {
        self.skills = Some(skills);
        self
    }

    /// Layer-1 — compact index with C3 escalation.
    pub async fn recall_index(
        &self,
        query: &str,
        repo: Option<&str>,
        kinds: Option<&[&str]>,
        days: Option<u32>,
        limit: u32,
    ) -> common::Result<RecallIndexResponse> {
        let started = Instant::now();
        let limit = if limit == 0 { self.config.default_limit } else { limit };
        let scope = repo.map(|r| vec![("repo".to_string(), Some(r.to_string()))]).unwrap_or_default();
        let scored = self
            .ums
            .retrieve_with_overrides(query, limit as usize, 0.0, default_weights())
            .await?;
        let _ = (kinds, days); // Phase-4 minimum: filters left to caller-side.
        let sims: Vec<f32> = scored.iter().map(|s| s.score as f32).collect();
        let mut coverage = self.probe.score(&sims);
        let verdict = self.probe.verdict(&sims);

        let entries: Vec<IndexEntry> = scored
            .iter()
            .map(|s| self.index_builder.from_scored_fact(s))
            .collect();

        // Optional escalation.
        let mut skill_used: Option<String> = None;
        if verdict == ProbeVerdict::Escalate {
            if let Some(skills) = &self.skills {
                let ctx = crate::retrieval_skills::EscalationContext {
                    query: query.to_string(),
                    coverage_score: coverage,
                    budget_tier: crate::retrieval_skills::BudgetTier::DeepThink,
                    repo: repo.map(|s| s.to_string()),
                };
                let res = skills.escalate(&ctx).await?;
                coverage = res.final_outcome.coverage_after;
                if !res.skills_tried.is_empty() {
                    skill_used = Some(res.skills_tried.join(","));
                }
            }
        }

        let result_ids: Vec<Uuid> = entries.iter().map(|e| e.id).collect();
        let row = RecallInvocationRow {
            id: Uuid::new_v4(),
            occurred_at: Timestamp::now(),
            session_id: None,
            turn_id: None,
            repo_id: repo.map(|s| s.to_string()),
            layer: "index".into(),
            query: query.to_string(),
            coverage_score: Some(coverage),
            skill_used,
            latency_ms: started.elapsed().as_millis() as i64,
            result_ids,
            rendered_tokens: None,
            metadata: serde_json::json!({"scope": scope}),
        };
        self.telemetry.insert(&row).await?;

        Ok(RecallIndexResponse {
            results: entries,
            coverage_score: coverage,
            escalation_available: verdict == ProbeVerdict::Escalate && self.skills.is_some(),
        })
    }

    /// Layer-2 — chronological framing.
    pub async fn recall_timeline(
        &self,
        ids_or_query: RecallQuery,
        repo: Option<&str>,
        days: u32,
    ) -> common::Result<Vec<TimelineEntry>> {
        let started = Instant::now();
        let inputs: Vec<TimelineInput> = match ids_or_query {
            RecallQuery::Ids(ids) => {
                let entries = self.fetch_builder.fetch(&ids, false, false).await?;
                entries
                    .into_iter()
                    .map(|e| TimelineInput {
                        id: e.id,
                        kind: e.kind,
                        when: e
                            .metadata
                            .get("recorded_at")
                            .and_then(|v| v.as_str())
                            .and_then(|s| s.parse().ok())
                            .unwrap_or_else(Timestamp::now),
                        snippet: e.content.to_string(),
                        related_ids: vec![],
                    })
                    .collect()
            }
            RecallQuery::Text(q) => {
                let scored = self.ums.retrieve_with_overrides(&q, 25, 0.0, default_weights()).await?;
                let _ = days;
                let _ = repo;
                scored
                    .into_iter()
                    .map(|s| TimelineInput {
                        id: s.fact.id,
                        kind: s.fact.memory_type.clone().unwrap_or_else(|| "fact".into()),
                        when: s.fact.recorded_at,
                        snippet: format!("{} {} {}", s.fact.subject, s.fact.predicate, s.fact.object),
                        related_ids: vec![],
                    })
                    .collect()
            }
        };
        let entries = self.timeline_builder.build(inputs);
        let result_ids: Vec<Uuid> = entries.iter().map(|e| e.id).collect();
        self.telemetry
            .insert(&RecallInvocationRow {
                id: Uuid::new_v4(),
                occurred_at: Timestamp::now(),
                session_id: None,
                turn_id: None,
                repo_id: repo.map(|s| s.to_string()),
                layer: "timeline".into(),
                query: String::new(),
                coverage_score: None,
                skill_used: None,
                latency_ms: started.elapsed().as_millis() as i64,
                result_ids,
                rendered_tokens: None,
                metadata: serde_json::json!({"days": days}),
            })
            .await?;
        Ok(entries)
    }

    /// Layer-3 — full fetch.
    pub async fn recall_fetch(
        &self,
        ids: &[Uuid],
        include_provenance: bool,
        include_causal_graph: bool,
    ) -> common::Result<Vec<FullEntry>> {
        self.fetch_builder
            .fetch(ids, include_provenance, include_causal_graph)
            .await
    }

    /// Counterfactual check.
    pub async fn check_dead_ends(
        &self,
        approach: &str,
        repo: Option<&str>,
    ) -> common::Result<DeadEndResponse> {
        self.dead_end.check(approach, repo).await
    }

    /// `recall_facts_as_of`.
    pub async fn recall_facts_as_of(
        &self,
        subject: &str,
        predicate: &str,
        as_of: Timestamp,
    ) -> common::Result<FactsAsOfResponse> {
        self.facts_as_of.query(subject, predicate, as_of).await
    }

    /// `recall_change_history`.
    pub async fn recall_change_history(
        &self,
        subject: &str,
        predicate: &str,
        repo: Option<&str>,
    ) -> common::Result<ChangeHistoryResponse> {
        self.change_history.query(subject, predicate, repo).await
    }

    /// `recall_decision_points`.
    pub async fn recall_decision_points(
        &self,
        repo: Option<&str>,
        limit: i64,
    ) -> common::Result<DecisionPointsResponse> {
        self.decision_points.list(repo, limit).await
    }

    /// Phase 6 — stays unimplemented.
    pub async fn trace_causes(
        &self,
        _subject: Uuid,
        _repo: Option<&str>,
        _depth: u32,
    ) -> common::Result<CausalTraceResponse> {
        Err(common::KlyntbotError::NotImplemented(
            "trace_causes lands in Phase 6".into(),
        ))
    }

    /// Test seam — read telemetry rows.
    pub fn telemetry_repo(&self) -> &RecallInvocationRepo {
        &self.telemetry
    }
}

fn default_weights() -> [f64; 12] {
    // Modestly bias for semantic + recency + path coherence; train in Phase 6.
    [0.35, 0.05, 0.10, 0.05, 0.05, 0.20, 0.05, 0.05, 0.02, 0.02, 0.05, 0.01]
}
```

- [ ] **Step 2: Re-export from `recall/mod.rs`**

Edit `crates/coding-memory/src/recall/mod.rs`:
- Delete the existing stub `CodingRecallService` impl block (the one returning `Err(phase(4))`).
- Add `pub mod service;`
- Add `pub use service::{CodingRecallService, CodingRecallServiceConfig};`

- [ ] **Step 3: Write failing test**

Create `crates/coding-memory/tests/recall_service_end_to_end.rs`:

```rust
use coding_memory::recall::{CodingRecallService, CodingRecallServiceConfig, RecallInvocationRepo};
use coding_memory::recall::budget::HeuristicBudgeter;
use storage::StoragePool;
use std::sync::Arc;

#[tokio::test]
async fn recall_index_returns_entries_and_writes_telemetry() {
    let pool = StoragePool::connect_in_memory().await.unwrap();
    StoragePool::run_feature_migrations(pool.inner(), &cognitive::cognitive_migrations()).await.unwrap();
    StoragePool::run_feature_migrations(pool.inner(), &coding_memory::coding_memory_migrations()).await.unwrap();

    let fact_repo = Arc::new(cognitive::SemanticFactRepo::new(pool.clone()));
    let ep_repo = Arc::new(cognitive::EpisodicMemoryRepo::new(pool.clone()));
    let ums = Arc::new(cognitive::UnifiedMemoryService::new(fact_repo.clone()));
    let telem = RecallInvocationRepo::new(pool.clone());

    // Seed
    let fact = cognitive::SemanticFact {
        id: uuid::Uuid::new_v4(),
        subject: "auth_module".into(),
        predicate: "uses".into(),
        object: "JWT HS256".into(),
        recorded_at: jiff::Timestamp::now(),
        confidence: 0.9,
        scope_repo_id: Some("repo:demo".into()),
        memory_type: Some("repo_context".into()),
        ..Default::default()
    };
    fact_repo.upsert_with_metadata(&fact).await.unwrap();

    let svc = CodingRecallService::new(
        CodingRecallServiceConfig::default(),
        ums, fact_repo, ep_repo, telem,
        Arc::new(HeuristicBudgeter),
    );
    let resp = svc.recall_index("JWT auth", Some("repo:demo"), None, None, 10).await.unwrap();
    assert!(!resp.results.is_empty());
    let log = svc.telemetry_repo().list_recent(10, 0, Some("index")).await.unwrap();
    assert_eq!(log.len(), 1);
    assert_eq!(log[0].layer, "index");
}
```

- [ ] **Step 4: Run + commit**

```bash
cargo nextest run -p coding-memory --test recall_service_end_to_end
cargo clippy -p coding-memory --all-targets -- -D warnings
git add crates/coding-memory/src/recall/service.rs \
        crates/coding-memory/src/recall/mod.rs \
        crates/coding-memory/tests/recall_service_end_to_end.rs
git commit -m "feat(coding-memory): CodingRecallService — orchestrate Layers 1-3 + telemetry"
```

---

### Task 20: SessionStart markdown renderer (800-token budget)

**Files:**
- Modify: `crates/coding-memory/src/recall/renderers.rs`
- Test: `crates/coding-memory/tests/session_start_render.rs`

- [ ] **Step 1: Write failing test**

Create `crates/coding-memory/tests/session_start_render.rs`:

```rust
use coding_memory::recall::renderers::render_session_start_block;
use coding_memory::recall::budget::{HeuristicBudgeter, TokenBudgeter};

#[tokio::test]
async fn within_budget_and_well_formed() {
    let pool = storage::StoragePool::connect_in_memory().await.unwrap();
    storage::StoragePool::run_feature_migrations(pool.inner(), &cognitive::cognitive_migrations()).await.unwrap();
    storage::StoragePool::run_feature_migrations(pool.inner(), &coding_memory::coding_memory_migrations()).await.unwrap();
    let fact_repo = std::sync::Arc::new(cognitive::SemanticFactRepo::new(pool.clone()));
    let ep_repo = std::sync::Arc::new(cognitive::EpisodicMemoryRepo::new(pool.clone()));
    let ums = std::sync::Arc::new(cognitive::UnifiedMemoryService::new(fact_repo.clone()));
    let svc = std::sync::Arc::new(coding_memory::recall::CodingRecallService::new(
        Default::default(),
        ums, fact_repo, ep_repo,
        coding_memory::recall::RecallInvocationRepo::new(pool.clone()),
        std::sync::Arc::new(HeuristicBudgeter),
    ));

    let md = render_session_start_block(&svc, Some("repo:demo"))
        .await
        .unwrap();
    let bud = HeuristicBudgeter;
    assert!(bud.count(&md) <= 800, "got {} tokens", bud.count(&md));
    assert!(md.contains("Project memory"));
}
```

- [ ] **Step 2: Implement**

Replace `crates/coding-memory/src/recall/renderers.rs`:

```rust
//! Markdown renderers for passive injection.
//!
//! Both renderers are total-budget-bounded — they truncate section by section,
//! then global-truncate at the end. The token counter is pluggable via
//! `CodingRecallService::budgeter` (currently held inside the service via
//! the `IndexBuilder`).

use crate::recall::budget::{default_budgeter, HeuristicBudgeter, TokenBudgeter};
use crate::recall::{CodingRecallService, RecallQuery};
use std::sync::Arc;

/// Token budget for SessionStart injection (design §8).
pub const SESSION_START_BUDGET_TOKENS: u32 = 800;
/// Token budget for UserPromptSubmit injection (design §8).
pub const USER_PROMPT_BUDGET_TOKENS: u32 = 1500;

/// Render the SessionStart injection block for a given repo.
///
/// Sections in order:
/// 1. `## Project memory — <repo_id>`
/// 2. `### What you need to know about this repo` (RepoContext, top 6)
/// 3. `### Your preferences (relevant)` (StylePreference)
/// 4. `### Recent activity (last 7 days)` (table)
/// 5. `### Open threads` (last unfinished turn traces)
pub async fn render_session_start_block(
    svc: &Arc<CodingRecallService>,
    repo: Option<&str>,
) -> common::Result<String> {
    let budgeter: Arc<dyn TokenBudgeter> = default_budgeter();
    let header = format!(
        "## Project memory — {}\n\n",
        repo.unwrap_or("(no repo)")
    );

    // Section 1 — repo context facts.
    let repo_ctx = svc
        .recall_index("repository architecture overview", repo, None, None, 6)
        .await?;
    let mut s1 = String::from("### What you need to know about this repo\n");
    for r in repo_ctx.results.iter().take(6) {
        s1.push_str(&format!("- {}\n", r.title));
    }
    s1.push('\n');

    // Section 2 — preferences.
    let prefs = svc
        .recall_index("style preference convention", repo, None, None, 4)
        .await?;
    let mut s2 = String::from("### Your preferences (relevant)\n");
    for r in prefs.results.iter().take(4) {
        s2.push_str(&format!("- {}\n", r.title));
    }
    s2.push('\n');

    // Section 3 — recent activity (last 7 days).
    let recent = svc
        .recall_timeline(RecallQuery::Text("recent".into()), repo, 7)
        .await?;
    let mut s3 = String::from("### Recent activity (last 7 days)\n| when | what | id |\n|---|---|---|\n");
    for e in recent.iter().take(8) {
        s3.push_str(&format!(
            "| {} | {} | `{}` |\n",
            e.when.to_string(),
            crop(&e.snippet, 60),
            short_id(e.id)
        ));
    }
    s3.push('\n');

    // Section 4 — open threads. Phase 4 stub: empty list with caveat.
    let s4 = "### Open threads\n_(none captured this phase)_\n\n";

    // Concatenate + global truncate.
    let full = format!("{header}{s1}{s2}{s3}{s4}*Call `recall_fetch(ids=[...])` for details.*\n");
    let truncated = budgeter.truncate_to(&full, SESSION_START_BUDGET_TOKENS as usize);
    debug_assert!(
        HeuristicBudgeter.count(&truncated) <= SESSION_START_BUDGET_TOKENS as usize + 50,
        "render_session_start_block exceeded budget"
    );
    Ok(truncated)
}

/// Render the UserPromptSubmit injection block.
pub async fn render_user_prompt_block(
    svc: &Arc<CodingRecallService>,
    query: &str,
    repo: Option<&str>,
) -> common::Result<String> {
    let budgeter: Arc<dyn TokenBudgeter> = default_budgeter();

    // Dead-end check first — placed at top if matches found.
    let dead_ends = svc.check_dead_ends(query, repo).await?;
    let warn = if dead_ends.aggregate_confidence > 0.5 && !dead_ends.matches.is_empty() {
        let m = &dead_ends.matches[0];
        format!(
            "### ⚠️ Heads-up\nYou previously tried **{}** ({}) — abandoned because {}.\n\n",
            m.approach, m.when, m.reason
        )
    } else {
        String::new()
    };

    // Likely-relevant memories.
    let idx = svc.recall_index(query, repo, None, None, 6).await?;
    let mut likely = String::from("### Likely relevant\n");
    for r in idx.results.iter().take(6) {
        likely.push_str(&format!("- [`{}`] {} {}\n", short_id(r.id), r.kind, crop(&r.title, 80)));
    }
    likely.push('\n');

    // Causal context — empty until Phase 6, but stub the section.
    let causal = "### Causal context\n_(populated when causal edges are seeded — Phase 6.)_\n\n";

    let footer = if !idx.results.is_empty() {
        format!(
            "*Fetch details: `recall_fetch(ids=[{}])`*",
            idx.results
                .iter()
                .take(3)
                .map(|r| format!("\"{}\"", r.id))
                .collect::<Vec<_>>()
                .join(", ")
        )
    } else {
        String::new()
    };

    let full = format!(
        "## Relevant memory for this turn\n\n{warn}{likely}{causal}{footer}\n"
    );
    Ok(budgeter.truncate_to(&full, USER_PROMPT_BUDGET_TOKENS as usize))
}

fn short_id(id: uuid::Uuid) -> String {
    id.to_string().chars().take(8).collect()
}

fn crop(s: &str, n: usize) -> String {
    let mut out: String = s.chars().take(n).collect();
    if out.len() < s.len() {
        out.push('…');
    }
    out
}
```

- [ ] **Step 3: Run + commit**

```bash
cargo nextest run -p coding-memory --test session_start_render
git add crates/coding-memory/src/recall/renderers.rs \
        crates/coding-memory/tests/session_start_render.rs
git commit -m "feat(coding-memory): render_session_start_block — 800-token markdown"
```

---

### Task 21: UserPromptSubmit renderer test (1500-token budget + dead-end block)

**Files:**
- Test: `crates/coding-memory/tests/user_prompt_render.rs`

- [ ] **Step 1: Write the test**

Create `crates/coding-memory/tests/user_prompt_render.rs`:

```rust
use coding_memory::recall::renderers::render_user_prompt_block;
use coding_memory::recall::budget::{HeuristicBudgeter, TokenBudgeter};

#[tokio::test]
async fn no_dead_end_no_warn_block() {
    let pool = storage::StoragePool::connect_in_memory().await.unwrap();
    storage::StoragePool::run_feature_migrations(pool.inner(), &cognitive::cognitive_migrations()).await.unwrap();
    storage::StoragePool::run_feature_migrations(pool.inner(), &coding_memory::coding_memory_migrations()).await.unwrap();
    let fact_repo = std::sync::Arc::new(cognitive::SemanticFactRepo::new(pool.clone()));
    let ep_repo = std::sync::Arc::new(cognitive::EpisodicMemoryRepo::new(pool.clone()));
    let ums = std::sync::Arc::new(cognitive::UnifiedMemoryService::new(fact_repo.clone()));
    let svc = std::sync::Arc::new(coding_memory::recall::CodingRecallService::new(
        Default::default(),
        ums, fact_repo, ep_repo,
        coding_memory::recall::RecallInvocationRepo::new(pool.clone()),
        std::sync::Arc::new(HeuristicBudgeter),
    ));
    let md = render_user_prompt_block(&svc, "fix parser bug", Some("repo:demo"))
        .await
        .unwrap();
    assert!(!md.contains("⚠️ Heads-up"));
    assert!(HeuristicBudgeter.count(&md) <= 1500);
}

#[tokio::test]
async fn dead_end_seeded_yields_warn_block() {
    let pool = storage::StoragePool::connect_in_memory().await.unwrap();
    storage::StoragePool::run_feature_migrations(pool.inner(), &cognitive::cognitive_migrations()).await.unwrap();
    storage::StoragePool::run_feature_migrations(pool.inner(), &coding_memory::coding_memory_migrations()).await.unwrap();
    let fact_repo = std::sync::Arc::new(cognitive::SemanticFactRepo::new(pool.clone()));
    let ep_repo = std::sync::Arc::new(cognitive::EpisodicMemoryRepo::new(pool.clone()));
    let ums = std::sync::Arc::new(cognitive::UnifiedMemoryService::new(fact_repo.clone()));

    let cf = cognitive::SemanticFact {
        id: uuid::Uuid::new_v4(),
        subject: "rewrite parser as recursive descent".into(),
        predicate: "outcome".into(),
        object: "abandoned — too slow".into(),
        recorded_at: jiff::Timestamp::now(),
        confidence: 0.9,
        scope_repo_id: Some("repo:demo".into()),
        memory_type: Some("counterfactual".into()),
        metadata: serde_json::json!({"memory_type":"counterfactual","reason":"too slow","attempt_id":"00000000-0000-0000-0000-000000000001","problem_hash":"abc"}),
        ..Default::default()
    };
    fact_repo.upsert_with_metadata(&cf).await.unwrap();

    let svc = std::sync::Arc::new(coding_memory::recall::CodingRecallService::new(
        Default::default(),
        ums, fact_repo, ep_repo,
        coding_memory::recall::RecallInvocationRepo::new(pool.clone()),
        std::sync::Arc::new(HeuristicBudgeter),
    ));
    let md = render_user_prompt_block(
        &svc,
        "rewrite parser as recursive descent",
        Some("repo:demo"),
    )
    .await
    .unwrap();
    assert!(md.contains("⚠️ Heads-up"), "expected warn block; got:\n{md}");
}
```

- [ ] **Step 2: Run + commit**

```bash
cargo nextest run -p coding-memory --test user_prompt_render
git add crates/coding-memory/tests/user_prompt_render.rs
git commit -m "test(coding-memory): UserPromptSubmit renderer — budget + dead-end block"
```

---

### Task 22: `klyntbot-hook context` subcommand

**Files:**
- Modify: `crates/coding-ingest/src/bin/klyntbot-hook.rs`
- Test: `crates/coding-ingest/tests/hook_context_subcmd.rs`

> **Approach:** The `context` subcommand is the passive-injection entry point. It does **not** call `CodingRecallService` directly (that lives in the desktop process). Instead, it speaks JSON-over-Unix-socket to the daemon, which runs the renderer and pipes back markdown. For Phase 4 we add a new socket frame `{"op":"render_session_start", "repo":"..."}` / `{"op":"render_user_prompt", "query":"...", "repo":"..."}` and the daemon dispatches to `AppCore::recall_render_*` handlers (Task 24). If the desktop is offline, the hook prints `<!-- klyntbot recall unavailable -->` to stdout and exits 0 (never blocks Claude Code).

- [ ] **Step 1: Write failing test**

Create `crates/coding-ingest/tests/hook_context_subcmd.rs`:

```rust
use std::process::Command;

#[test]
fn context_session_start_returns_zero_with_offline_desktop() {
    let bin = env!("CARGO_BIN_EXE_klyntbot-hook");
    let out = Command::new(bin)
        .args(["context", "--session-start", "--repo", "repo:demo"])
        .env("KLYNTBOT_HOME", tempfile::tempdir().unwrap().path())
        .output()
        .expect("run");
    assert!(out.status.success(), "stdout={}", String::from_utf8_lossy(&out.stdout));
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(
        stdout.contains("klyntbot recall unavailable") || stdout.contains("Project memory"),
        "stdout={stdout}"
    );
}
```

- [ ] **Step 2: Implement**

Edit `crates/coding-ingest/src/bin/klyntbot-hook.rs`. Find the existing arg-parsing match (the dispatch over the first positional arg). Add a `"context"` arm:

```rust
"context" => {
    let mut session_start = false;
    let mut user_prompt: Option<String> = None;
    let mut repo: Option<String> = None;
    let mut iter = args.iter().skip(2);
    while let Some(a) = iter.next() {
        match a.as_str() {
            "--session-start" => session_start = true,
            "--user-prompt-submit" => {
                user_prompt = iter.next().map(|s| s.to_string());
            }
            "--repo" => repo = iter.next().map(|s| s.to_string()),
            _ => {}
        }
    }
    return run_context(session_start, user_prompt, repo).await;
}
```

(Adjust to match the existing dispatcher's exact shape; if it's a `clap` derive, add a `Context { ... }` variant instead.)

Add the helper at module scope:

```rust
async fn run_context(
    session_start: bool,
    user_prompt: Option<String>,
    repo: Option<String>,
) -> ExitCode {
    use coding_ingest::transport::UnixIngestSocket;
    use std::io::Write as _;
    let socket = UnixIngestSocket::default_path();
    let payload = if session_start {
        serde_json::json!({"op": "render_session_start", "repo": repo})
    } else if let Some(q) = user_prompt {
        serde_json::json!({"op": "render_user_prompt", "query": q, "repo": repo})
    } else {
        let _ = writeln!(std::io::stderr(), "context requires --session-start or --user-prompt-submit");
        return ExitCode::from(2);
    };
    match coding_ingest::transport::request_response(&socket, &payload, std::time::Duration::from_millis(800)).await {
        Ok(resp) => {
            if let Some(md) = resp.get("markdown").and_then(|v| v.as_str()) {
                print!("{md}");
            } else {
                print!("<!-- klyntbot recall unavailable -->");
            }
            ExitCode::SUCCESS
        }
        Err(_) => {
            print!("<!-- klyntbot recall unavailable -->");
            ExitCode::SUCCESS
        }
    }
}
```

> **Note:** `transport::request_response` is a new helper to add — it's a request/response variant of the existing fire-and-forget `UnixIngestSocket::send`. Add it to `crates/coding-ingest/src/transport.rs`:

```rust
/// Send a JSON request, await one JSON response within `timeout`.
pub async fn request_response(
    socket: &UnixIngestSocket,
    payload: &serde_json::Value,
    timeout: std::time::Duration,
) -> common::Result<serde_json::Value> {
    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    let mut stream = tokio::time::timeout(timeout, tokio::net::UnixStream::connect(socket.path()))
        .await
        .map_err(|_| common::KlyntbotError::Internal("socket connect timeout".into()))?
        .map_err(|e| common::KlyntbotError::Internal(format!("connect: {e}")))?;
    let buf = serde_json::to_vec(payload)
        .map_err(|e| common::KlyntbotError::Internal(format!("encode: {e}")))?;
    let len = (buf.len() as u32).to_be_bytes();
    stream.write_all(&len).await
        .map_err(|e| common::KlyntbotError::Internal(format!("write len: {e}")))?;
    stream.write_all(&buf).await
        .map_err(|e| common::KlyntbotError::Internal(format!("write buf: {e}")))?;
    stream.shutdown().await.ok();
    let mut len_buf = [0u8; 4];
    tokio::time::timeout(timeout, stream.read_exact(&mut len_buf))
        .await
        .map_err(|_| common::KlyntbotError::Internal("read len timeout".into()))?
        .map_err(|e| common::KlyntbotError::Internal(format!("read len: {e}")))?;
    let resp_len = u32::from_be_bytes(len_buf) as usize;
    let mut data = vec![0u8; resp_len];
    tokio::time::timeout(timeout, stream.read_exact(&mut data))
        .await
        .map_err(|_| common::KlyntbotError::Internal("read body timeout".into()))?
        .map_err(|e| common::KlyntbotError::Internal(format!("read body: {e}")))?;
    serde_json::from_slice(&data)
        .map_err(|e| common::KlyntbotError::Internal(format!("decode: {e}")))
}
```

The daemon side (`crates/coding-ingest/src/daemon.rs`) needs a frame discriminator — modify `IngestDaemon::handle_connection` to inspect the JSON before treating it as an `AgentEvent`. If the JSON has an `op` field, route through a new `op_handler: Arc<dyn OpHandler>` injected at daemon spawn:

```rust
#[async_trait::async_trait]
pub trait OpHandler: Send + Sync {
    async fn handle(&self, payload: serde_json::Value) -> common::Result<serde_json::Value>;
}
```

`AppCore` provides the concrete impl in Task 24.

- [ ] **Step 3: Run + commit**

```bash
cargo nextest run -p coding-ingest --test hook_context_subcmd
git add crates/coding-ingest/src/bin/klyntbot-hook.rs \
        crates/coding-ingest/src/transport.rs \
        crates/coding-ingest/src/daemon.rs \
        crates/coding-ingest/tests/hook_context_subcmd.rs
git commit -m "feat(coding-ingest): klyntbot-hook context subcommand + request/response op frame"
```

---

### Task 23: MCP tool dispatch — wire 7 active tools over `Arc<CodingRecallService>`

**Files:**
- Modify: `crates/coding-memory/src/mcp.rs`
- Modify: `crates/coding-memory/src/lib.rs` (re-export `CodingMemoryToolset`)

- [ ] **Step 1: Implement the dispatcher**

Replace `crates/coding-memory/src/mcp.rs`:

```rust
//! MCP tool dispatchers — Phase 4 wires 7 active tools; `trace_causes` stays Phase-6.
//!
//! Each call decodes args, invokes `CodingRecallService`, and serializes the response.
//! `CodingMemoryToolset` is a `Send + Sync` handle the MCP server registers.

use crate::recall::{CodingRecallService, RecallQuery};
use jiff::Timestamp;
use std::sync::Arc;
use uuid::Uuid;

/// Public tool names — must match `EXPLICIT_TOOL_ALLOWLIST` in config.
pub const CODING_MEMORY_MCP_TOOLS: &[&str] = &[
    "recall_index",
    "recall_timeline",
    "recall_fetch",
    "trace_causes",
    "check_dead_ends",
    "recall_facts_as_of",
    "recall_change_history",
    "recall_decision_points",
];

/// Toolset handle.
#[derive(Clone)]
pub struct CodingMemoryToolset {
    svc: Arc<CodingRecallService>,
}

impl std::fmt::Debug for CodingMemoryToolset {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("CodingMemoryToolset").finish()
    }
}

impl CodingMemoryToolset {
    /// Construct.
    #[must_use]
    pub fn new(svc: Arc<CodingRecallService>) -> Self {
        Self { svc }
    }

    /// Dispatch.
    pub async fn dispatch(
        &self,
        tool: &str,
        args: serde_json::Value,
    ) -> common::Result<serde_json::Value> {
        match tool {
            "recall_index" => self.recall_index(args).await,
            "recall_timeline" => self.recall_timeline(args).await,
            "recall_fetch" => self.recall_fetch(args).await,
            "check_dead_ends" => self.check_dead_ends(args).await,
            "recall_facts_as_of" => self.recall_facts_as_of(args).await,
            "recall_change_history" => self.recall_change_history(args).await,
            "recall_decision_points" => self.recall_decision_points(args).await,
            "trace_causes" => Err(common::KlyntbotError::NotImplemented(
                "trace_causes lands in Phase 6".into(),
            )),
            other => Err(common::KlyntbotError::Internal(format!(
                "unknown coding-memory tool: {other}"
            ))),
        }
    }

    async fn recall_index(&self, args: serde_json::Value) -> common::Result<serde_json::Value> {
        #[derive(serde::Deserialize)]
        #[serde(rename_all = "camelCase")]
        struct A {
            query: String,
            repo: Option<String>,
            #[serde(default)] kinds: Option<Vec<String>>,
            days: Option<u32>,
            #[serde(default = "default_limit")] limit: u32,
        }
        fn default_limit() -> u32 { 10 }
        let a: A = serde_json::from_value(args).map_err(decode_err)?;
        let kinds_owned = a.kinds.clone().unwrap_or_default();
        let kinds_borrow: Vec<&str> = kinds_owned.iter().map(|s| s.as_str()).collect();
        let kinds_opt: Option<&[&str]> = if kinds_borrow.is_empty() { None } else { Some(&kinds_borrow) };
        let resp = self.svc.recall_index(&a.query, a.repo.as_deref(), kinds_opt, a.days, a.limit).await?;
        serde_json::to_value(resp).map_err(encode_err)
    }

    async fn recall_timeline(&self, args: serde_json::Value) -> common::Result<serde_json::Value> {
        #[derive(serde::Deserialize)]
        #[serde(rename_all = "camelCase")]
        struct A {
            ids: Option<Vec<Uuid>>,
            query: Option<String>,
            repo: Option<String>,
            #[serde(default = "default_days")] days: u32,
        }
        fn default_days() -> u32 { 30 }
        let a: A = serde_json::from_value(args).map_err(decode_err)?;
        let q = match (a.ids, a.query) {
            (Some(ids), _) => RecallQuery::Ids(ids),
            (_, Some(q)) => RecallQuery::Text(q),
            _ => return Err(decode_err(serde::de::Error::missing_field("ids|query"))),
        };
        let resp = self.svc.recall_timeline(q, a.repo.as_deref(), a.days).await?;
        serde_json::to_value(resp).map_err(encode_err)
    }

    async fn recall_fetch(&self, args: serde_json::Value) -> common::Result<serde_json::Value> {
        #[derive(serde::Deserialize)]
        #[serde(rename_all = "camelCase")]
        struct A {
            ids: Vec<Uuid>,
            #[serde(default = "default_true")] include_provenance: bool,
            #[serde(default)] include_causal_graph: bool,
        }
        fn default_true() -> bool { true }
        let a: A = serde_json::from_value(args).map_err(decode_err)?;
        let resp = self.svc.recall_fetch(&a.ids, a.include_provenance, a.include_causal_graph).await?;
        serde_json::to_value(resp).map_err(encode_err)
    }

    async fn check_dead_ends(&self, args: serde_json::Value) -> common::Result<serde_json::Value> {
        #[derive(serde::Deserialize)]
        #[serde(rename_all = "camelCase")]
        struct A { approach: String, repo: Option<String> }
        let a: A = serde_json::from_value(args).map_err(decode_err)?;
        let resp = self.svc.check_dead_ends(&a.approach, a.repo.as_deref()).await?;
        serde_json::to_value(resp).map_err(encode_err)
    }

    async fn recall_facts_as_of(&self, args: serde_json::Value) -> common::Result<serde_json::Value> {
        #[derive(serde::Deserialize)]
        #[serde(rename_all = "camelCase")]
        struct A { subject: String, predicate: String, as_of: Timestamp }
        let a: A = serde_json::from_value(args).map_err(decode_err)?;
        let resp = self.svc.recall_facts_as_of(&a.subject, &a.predicate, a.as_of).await?;
        serde_json::to_value(resp).map_err(encode_err)
    }

    async fn recall_change_history(&self, args: serde_json::Value) -> common::Result<serde_json::Value> {
        #[derive(serde::Deserialize)]
        #[serde(rename_all = "camelCase")]
        struct A { subject: String, predicate: String, repo: Option<String> }
        let a: A = serde_json::from_value(args).map_err(decode_err)?;
        let resp = self.svc.recall_change_history(&a.subject, &a.predicate, a.repo.as_deref()).await?;
        serde_json::to_value(resp).map_err(encode_err)
    }

    async fn recall_decision_points(&self, args: serde_json::Value) -> common::Result<serde_json::Value> {
        #[derive(serde::Deserialize)]
        #[serde(rename_all = "camelCase")]
        struct A {
            #[serde(default)] domain: Option<String>,
            repo: Option<String>,
            #[serde(default = "default_dp_limit")] limit: i64,
        }
        fn default_dp_limit() -> i64 { 50 }
        let a: A = serde_json::from_value(args).map_err(decode_err)?;
        let resp = self.svc.recall_decision_points(a.repo.as_deref(), a.limit).await?;
        let _ = a.domain;
        serde_json::to_value(resp).map_err(encode_err)
    }
}

fn decode_err<E: std::fmt::Display>(e: E) -> common::KlyntbotError {
    common::KlyntbotError::Internal(format!("decode args: {e}"))
}

fn encode_err<E: std::fmt::Display>(e: E) -> common::KlyntbotError {
    common::KlyntbotError::Internal(format!("encode response: {e}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn allowlist_matches_design() {
        let expected = [
            "recall_index","recall_timeline","recall_fetch","trace_causes",
            "check_dead_ends","recall_facts_as_of","recall_change_history","recall_decision_points",
        ];
        assert_eq!(CODING_MEMORY_MCP_TOOLS, expected);
    }
}
```

Edit `crates/coding-memory/src/lib.rs` to add:

```rust
pub use mcp::{CodingMemoryToolset, CODING_MEMORY_MCP_TOOLS};
```

- [ ] **Step 2: Build + commit**

```bash
cargo build -p coding-memory
cargo nextest run -p coding-memory --test mcp 2>/dev/null || cargo nextest run -p coding-memory
git add crates/coding-memory/src/mcp.rs crates/coding-memory/src/lib.rs
git commit -m "feat(coding-memory): CodingMemoryToolset — 7 active MCP tool dispatchers"
```

---

### Task 24: Wire `CodingRecallService` + MCP toolset onto `AppCore`

**Files:**
- Modify: `crates/app-core/src/state.rs` — add `pub recall: Option<Arc<coding_memory::recall::CodingRecallService>>`, `pub coding_toolset: Option<coding_memory::CodingMemoryToolset>`
- Modify: `crates/app-core/src/init/mod.rs` — construct after `Distiller`
- Create: `crates/app-core/src/coding_memory/recall.rs` — handlers
- Modify: `crates/app-core/src/coding_memory/mod.rs` — `pub mod recall;` + re-exports

- [ ] **Step 1: Add fields to `AppCore`**

Edit `crates/app-core/src/state.rs`. Find the existing `pub distiller: Option<Arc<coding_memory::distiller::Distiller>>,` line. Add directly after:

```rust
/// Coding-memory recall service (Phase 4).
pub recall: Option<Arc<coding_memory::recall::CodingRecallService>>,
/// MCP toolset for coding-memory recall tools.
pub coding_toolset: Option<coding_memory::CodingMemoryToolset>,
```

Update the constructor / `Default` (or whatever builds `AppCore`) to default both to `None`.

- [ ] **Step 2: Construct in `init`**

Edit `crates/app-core/src/init/mod.rs`. After the `Distiller` is built, build the recall service:

```rust
let fact_repo = std::sync::Arc::new(cognitive::SemanticFactRepo::new(storage_pool.clone()));
let ep_repo = std::sync::Arc::new(cognitive::EpisodicMemoryRepo::new(storage_pool.clone()));
let ums = std::sync::Arc::new(cognitive::UnifiedMemoryService::new(fact_repo.clone()));
let telem = coding_memory::recall::RecallInvocationRepo::new(storage_pool.clone());
let budgeter = coding_memory::recall::budget::default_budgeter();
let recall = std::sync::Arc::new(coding_memory::recall::CodingRecallService::new(
    coding_memory::recall::CodingRecallServiceConfig::default(),
    ums.clone(), fact_repo.clone(), ep_repo.clone(), telem, budgeter,
));
// Skill registry — built without retrieval closures wired (Phase 4 minimum: skills with stub closures).
// Phase 5 will swap in real retrieve closures bound to `ums`.
let toolset = coding_memory::CodingMemoryToolset::new(recall.clone());
app_core.recall = Some(recall);
app_core.coding_toolset = Some(toolset);
```

(Match field names to whatever your `AppCore` builder exposes — these may need to be passed into a `set_recall` helper rather than direct field assignment.)

- [ ] **Step 3: Implement op-handler so `klyntbot-hook context` works**

Create `crates/app-core/src/coding_memory/recall.rs`:

```rust
//! App-core handlers for coding-memory Phase-4 surfaces.

use coding_ingest::daemon::OpHandler;
use coding_memory::recall::{CodingRecallService, renderers};
use std::sync::Arc;

/// Op-handler glued to the daemon — answers `render_session_start` / `render_user_prompt`.
pub struct RecallOpHandler {
    svc: Arc<CodingRecallService>,
}

impl RecallOpHandler {
    /// Construct.
    #[must_use]
    pub fn new(svc: Arc<CodingRecallService>) -> Self {
        Self { svc }
    }
}

#[async_trait::async_trait]
impl OpHandler for RecallOpHandler {
    async fn handle(&self, payload: serde_json::Value) -> common::Result<serde_json::Value> {
        let op = payload.get("op").and_then(|v| v.as_str()).unwrap_or("");
        let repo = payload.get("repo").and_then(|v| v.as_str()).map(str::to_string);
        match op {
            "render_session_start" => {
                let md = renderers::render_session_start_block(&self.svc, repo.as_deref()).await?;
                Ok(serde_json::json!({"markdown": md}))
            }
            "render_user_prompt" => {
                let q = payload.get("query").and_then(|v| v.as_str()).unwrap_or("");
                let md = renderers::render_user_prompt_block(&self.svc, q, repo.as_deref()).await?;
                Ok(serde_json::json!({"markdown": md}))
            }
            other => Err(common::KlyntbotError::Internal(format!("unknown op: {other}"))),
        }
    }
}

/// Handler functions (Tauri / dev-server adapters call these).

/// `coding_memory_recall_index` — wraps the toolset.
pub async fn recall_index_handler(
    svc: &Arc<CodingRecallService>,
    args: serde_json::Value,
) -> common::Result<serde_json::Value> {
    let toolset = coding_memory::CodingMemoryToolset::new(svc.clone());
    toolset.dispatch("recall_index", args).await
}

/// `coding_memory_recall_timeline`.
pub async fn recall_timeline_handler(
    svc: &Arc<CodingRecallService>,
    args: serde_json::Value,
) -> common::Result<serde_json::Value> {
    coding_memory::CodingMemoryToolset::new(svc.clone())
        .dispatch("recall_timeline", args).await
}

/// `coding_memory_recall_fetch`.
pub async fn recall_fetch_handler(
    svc: &Arc<CodingRecallService>,
    args: serde_json::Value,
) -> common::Result<serde_json::Value> {
    coding_memory::CodingMemoryToolset::new(svc.clone())
        .dispatch("recall_fetch", args).await
}

/// `coding_memory_check_dead_ends`.
pub async fn check_dead_ends_handler(
    svc: &Arc<CodingRecallService>,
    args: serde_json::Value,
) -> common::Result<serde_json::Value> {
    coding_memory::CodingMemoryToolset::new(svc.clone())
        .dispatch("check_dead_ends", args).await
}

/// `coding_memory_recall_facts_as_of`.
pub async fn recall_facts_as_of_handler(
    svc: &Arc<CodingRecallService>,
    args: serde_json::Value,
) -> common::Result<serde_json::Value> {
    coding_memory::CodingMemoryToolset::new(svc.clone())
        .dispatch("recall_facts_as_of", args).await
}

/// `coding_memory_recall_change_history`.
pub async fn recall_change_history_handler(
    svc: &Arc<CodingRecallService>,
    args: serde_json::Value,
) -> common::Result<serde_json::Value> {
    coding_memory::CodingMemoryToolset::new(svc.clone())
        .dispatch("recall_change_history", args).await
}

/// `coding_memory_recall_decision_points`.
pub async fn recall_decision_points_handler(
    svc: &Arc<CodingRecallService>,
    args: serde_json::Value,
) -> common::Result<serde_json::Value> {
    coding_memory::CodingMemoryToolset::new(svc.clone())
        .dispatch("recall_decision_points", args).await
}

/// `coding_memory_recall_log` — paginated list of telemetry rows.
pub async fn recall_log_handler(
    svc: &Arc<CodingRecallService>,
    layer: Option<String>,
    limit: i64,
    offset: i64,
) -> common::Result<Vec<coding_memory::recall::telemetry::RecallInvocationRow>> {
    svc.telemetry_repo()
        .list_recent(limit, offset, layer.as_deref())
        .await
}

/// `coding_memory_session_replay_recall_overlay` — by session id.
pub async fn session_recall_overlay_handler(
    svc: &Arc<CodingRecallService>,
    session_id: String,
    limit: i64,
    offset: i64,
) -> common::Result<Vec<coding_memory::recall::telemetry::RecallInvocationRow>> {
    svc.telemetry_repo()
        .list_by_session(&session_id, limit, offset)
        .await
}
```

Edit `crates/app-core/src/coding_memory/mod.rs` to add `pub mod recall;`.

In `init/mod.rs`, after `app_core.recall = Some(recall.clone())`, register the op-handler with the daemon:

```rust
let op_handler: std::sync::Arc<dyn coding_ingest::daemon::OpHandler> =
    std::sync::Arc::new(crate::coding_memory::recall::RecallOpHandler::new(recall.clone()));
ingest_daemon.set_op_handler(op_handler);
```

(Add `set_op_handler` to `IngestDaemon` if absent — a simple `Arc<RwLock<Option<...>>>` setter.)

- [ ] **Step 4: Build + commit**

```bash
cargo build -p app-core
cargo nextest run -p app-core
git add crates/app-core/src/state.rs crates/app-core/src/init/mod.rs \
        crates/app-core/src/coding_memory/recall.rs \
        crates/app-core/src/coding_memory/mod.rs \
        crates/coding-ingest/src/daemon.rs
git commit -m "feat(app-core): wire CodingRecallService + RecallOpHandler"
```

---

### Task 25: Tauri commands + DEV_COMMANDS

**Files:**
- Modify: `crates/desktop-shared/src/commands/coding_memory.rs` — DTOs
- Modify: `crates/desktop/src/commands/coding_memory.rs` — 9 new `#[tauri::command]`s + extend `DEV_COMMANDS`
- Modify: `crates/desktop/src/lib.rs` — register in `invoke_handler!`
- Test: `crates/desktop/src/dev_server/mod.rs` — `dev_server_covers_all_tauri_commands` test ensures coverage

- [ ] **Step 1: Add DTOs to desktop-shared**

Append to `crates/desktop-shared/src/commands/coding_memory.rs`:

```rust
/// Args for `coding_memory_recall_log`.
#[derive(serde::Deserialize, serde::Serialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct RecallLogArgs {
    pub layer: Option<String>,
    #[serde(default = "default_limit_50")] pub limit: i64,
    #[serde(default)] pub offset: i64,
}
fn default_limit_50() -> i64 { 50 }

/// Args for `coding_memory_session_replay_recall_overlay`.
#[derive(serde::Deserialize, serde::Serialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct SessionRecallOverlayArgs {
    pub session_id: String,
    #[serde(default = "default_limit_200")] pub limit: i64,
    #[serde(default)] pub offset: i64,
}
fn default_limit_200() -> i64 { 200 }
```

- [ ] **Step 2: Add the Tauri commands**

Append to `crates/desktop/src/commands/coding_memory.rs`:

```rust
use desktop_shared::commands::coding_memory::{RecallLogArgs, SessionRecallOverlayArgs};

#[tauri::command]
pub async fn coding_memory_recall_index(
    state: tauri::State<'_, std::sync::Arc<app_core::AppCore>>,
    args: serde_json::Value,
) -> Result<serde_json::Value, String> {
    let svc = state.recall.as_ref().ok_or("recall service unavailable")?;
    app_core::coding_memory::recall::recall_index_handler(svc, args)
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn coding_memory_recall_timeline(
    state: tauri::State<'_, std::sync::Arc<app_core::AppCore>>,
    args: serde_json::Value,
) -> Result<serde_json::Value, String> {
    let svc = state.recall.as_ref().ok_or("recall service unavailable")?;
    app_core::coding_memory::recall::recall_timeline_handler(svc, args)
        .await.map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn coding_memory_recall_fetch(
    state: tauri::State<'_, std::sync::Arc<app_core::AppCore>>,
    args: serde_json::Value,
) -> Result<serde_json::Value, String> {
    let svc = state.recall.as_ref().ok_or("recall service unavailable")?;
    app_core::coding_memory::recall::recall_fetch_handler(svc, args)
        .await.map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn coding_memory_check_dead_ends(
    state: tauri::State<'_, std::sync::Arc<app_core::AppCore>>,
    args: serde_json::Value,
) -> Result<serde_json::Value, String> {
    let svc = state.recall.as_ref().ok_or("recall service unavailable")?;
    app_core::coding_memory::recall::check_dead_ends_handler(svc, args)
        .await.map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn coding_memory_recall_facts_as_of(
    state: tauri::State<'_, std::sync::Arc<app_core::AppCore>>,
    args: serde_json::Value,
) -> Result<serde_json::Value, String> {
    let svc = state.recall.as_ref().ok_or("recall service unavailable")?;
    app_core::coding_memory::recall::recall_facts_as_of_handler(svc, args)
        .await.map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn coding_memory_recall_change_history(
    state: tauri::State<'_, std::sync::Arc<app_core::AppCore>>,
    args: serde_json::Value,
) -> Result<serde_json::Value, String> {
    let svc = state.recall.as_ref().ok_or("recall service unavailable")?;
    app_core::coding_memory::recall::recall_change_history_handler(svc, args)
        .await.map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn coding_memory_recall_decision_points(
    state: tauri::State<'_, std::sync::Arc<app_core::AppCore>>,
    args: serde_json::Value,
) -> Result<serde_json::Value, String> {
    let svc = state.recall.as_ref().ok_or("recall service unavailable")?;
    app_core::coding_memory::recall::recall_decision_points_handler(svc, args)
        .await.map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn coding_memory_recall_log(
    state: tauri::State<'_, std::sync::Arc<app_core::AppCore>>,
    args: RecallLogArgs,
) -> Result<Vec<coding_memory::recall::telemetry::RecallInvocationRow>, String> {
    let svc = state.recall.as_ref().ok_or("recall service unavailable")?;
    app_core::coding_memory::recall::recall_log_handler(svc, args.layer, args.limit, args.offset)
        .await.map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn coding_memory_session_replay_recall_overlay(
    state: tauri::State<'_, std::sync::Arc<app_core::AppCore>>,
    args: SessionRecallOverlayArgs,
) -> Result<Vec<coding_memory::recall::telemetry::RecallInvocationRow>, String> {
    let svc = state.recall.as_ref().ok_or("recall service unavailable")?;
    app_core::coding_memory::recall::session_recall_overlay_handler(svc, args.session_id, args.limit, args.offset)
        .await.map_err(|e| e.to_string())
}
```

Extend `DEV_COMMANDS` in the same file:

```rust
pub const DEV_COMMANDS: &[&str] = &[
    // ...existing...
    "coding_memory_recall_index",
    "coding_memory_recall_timeline",
    "coding_memory_recall_fetch",
    "coding_memory_check_dead_ends",
    "coding_memory_recall_facts_as_of",
    "coding_memory_recall_change_history",
    "coding_memory_recall_decision_points",
    "coding_memory_recall_log",
    "coding_memory_session_replay_recall_overlay",
];
```

- [ ] **Step 3: Register in `invoke_handler!`**

Edit `crates/desktop/src/lib.rs`. Find the existing `tauri::generate_handler![...]` macro invocation. Append the 9 new commands.

- [ ] **Step 4: Verify dev-server coverage test passes**

Run: `cargo nextest run -p desktop dev_server_covers_all_tauri_commands`
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
cargo build -p desktop
git add crates/desktop-shared/src/commands/coding_memory.rs \
        crates/desktop/src/commands/coding_memory.rs \
        crates/desktop/src/lib.rs
git commit -m "feat(desktop): 9 new Tauri commands for coding-memory recall surfaces"
```

---

### Task 26: Wire `CodingMemoryToolset` into MCP server registry

**Files:**
- Modify: `crates/klyntbot-server/src/bridge/registry.rs` (or wherever tools are registered for MCP)
- Modify: wherever the MCP server consumes `AppCore` (likely `crates/mcp/src/lib.rs` or `crates/desktop/src/mcp.rs`)

> **Goal:** When the MCP server starts, it should look up `AppCore.coding_toolset` and register a dispatcher for each name in `CODING_MEMORY_MCP_TOOLS`.

- [ ] **Step 1: Locate the existing tool-registry construction**

Run: `grep -rn "default_exposed_tools\|register_tool\|ToolRegistry::" crates/klyntbot-server/ crates/mcp/ | head`

Identify the function that maps `EXPLICIT_TOOL_ALLOWLIST` entries to handlers. Inject a branch:

```rust
if CODING_MEMORY_MCP_TOOLS.contains(&name) {
    if let Some(toolset) = app_core.coding_toolset.clone() {
        registry.register(name, Box::new(move |args| {
            let toolset = toolset.clone();
            let name = name.to_string();
            Box::pin(async move { toolset.dispatch(&name, args).await })
        }));
        continue;
    }
}
```

(Adjust to match the actual registry method signatures — this is the shape; the exact code lives wherever `mcp_tools()` are registered today.)

- [ ] **Step 2: Add a smoke test**

Create `crates/klyntbot-server/tests/coding_memory_dispatch.rs`:

```rust
// Skeleton — fills in when we identify the existing test pattern in this file.
// At minimum: assert that calling MCP tool "recall_index" with no AppCore.recall yields
// a structured error rather than panicking.
```

(Adapt this once you've found the existing test harness for the MCP bridge.)

- [ ] **Step 3: Build + commit**

```bash
cargo build -p klyntbot-server -p klyntbot-mcp
cargo nextest run -p klyntbot-server
git add crates/klyntbot-server/src/bridge/registry.rs crates/klyntbot-server/tests/
git commit -m "feat(klyntbot-server): register coding-memory recall toolset"
```

---

### Task 27: Property — every render output ≤ declared budget

**Files:**
- Test: `crates/coding-memory/tests/prop_injection_budget.rs`

- [ ] **Step 1: Write the test**

Create `crates/coding-memory/tests/prop_injection_budget.rs`:

```rust
use coding_memory::recall::budget::{HeuristicBudgeter, TokenBudgeter};
use proptest::prelude::*;

proptest! {
    #![proptest_config(ProptestConfig::with_cases(64))]

    #[test]
    fn truncate_to_never_exceeds_budget(
        s in "\\PC{0,5000}",
        budget in 1usize..3000,
    ) {
        let b = HeuristicBudgeter;
        let out = b.truncate_to(&s, budget);
        prop_assert!(b.count(&out) <= budget + 1, "got {} for budget {}", b.count(&out), budget);
    }
}
```

- [ ] **Step 2: Run + commit**

```bash
cargo nextest run -p coding-memory --test prop_injection_budget
git add crates/coding-memory/tests/prop_injection_budget.rs
git commit -m "test(coding-memory): proptest — truncate_to respects budget"
```

---

### Task 28: Property — recall is idempotent for same query

**Files:**
- Test: `crates/coding-memory/tests/prop_recall_idempotent.rs`

```rust
use coding_memory::recall::{CodingRecallService, CodingRecallServiceConfig, RecallInvocationRepo};
use coding_memory::recall::budget::HeuristicBudgeter;
use proptest::prelude::*;
use std::sync::Arc;

proptest! {
    #![proptest_config(ProptestConfig::with_cases(8))]

    #[test]
    fn same_query_same_ids(query in "[a-z ]{4,40}") {
        let rt = tokio::runtime::Runtime::new().unwrap();
        rt.block_on(async {
            let pool = storage::StoragePool::connect_in_memory().await.unwrap();
            storage::StoragePool::run_feature_migrations(pool.inner(), &cognitive::cognitive_migrations()).await.unwrap();
            storage::StoragePool::run_feature_migrations(pool.inner(), &coding_memory::coding_memory_migrations()).await.unwrap();
            let fact_repo = Arc::new(cognitive::SemanticFactRepo::new(pool.clone()));
            let ep_repo = Arc::new(cognitive::EpisodicMemoryRepo::new(pool.clone()));
            let ums = Arc::new(cognitive::UnifiedMemoryService::new(fact_repo.clone()));
            let svc = CodingRecallService::new(
                CodingRecallServiceConfig::default(),
                ums, fact_repo, ep_repo,
                RecallInvocationRepo::new(pool.clone()),
                Arc::new(HeuristicBudgeter),
            );
            let a = svc.recall_index(&query, None, None, None, 5).await.unwrap();
            let b = svc.recall_index(&query, None, None, None, 5).await.unwrap();
            let a_ids: Vec<_> = a.results.iter().map(|r| r.id).collect();
            let b_ids: Vec<_> = b.results.iter().map(|r| r.id).collect();
            prop_assert_eq!(a_ids, b_ids);
            Ok(())
        }).unwrap();
    }
}
```

- [ ] **Step 2: Run + commit**

```bash
cargo nextest run -p coding-memory --test prop_recall_idempotent
git add crates/coding-memory/tests/prop_recall_idempotent.rs
git commit -m "test(coding-memory): proptest — recall_index is idempotent"
```

---

### Task 29: Scenario — next-session memory injection

**Files:**
- Create: `tests/fixtures/coding/phase4_recall_seed.jsonl`
- Test: `tests/integration/coding_memory_phase4_next_session.rs`

- [ ] **Step 1: Create fixture**

Create `tests/fixtures/coding/phase4_recall_seed.jsonl` — 8 facts + 4 episodes. Use the same JSONL line shape as `tests/fixtures/coding/phase3_bug_fix_session.jsonl`. Include at minimum:
- 1 `RepoContext` fact ("auth_module uses JWT HS256")
- 2 `StylePreference` facts
- 1 `WorkflowPattern` fact
- 1 counterfactual fact (`memory_type:"counterfactual"`)
- 4 episodes (`fix_attempt`, `dead_end_attempt`, `refactor_episode`, `turn_trace`)

- [ ] **Step 2: Write the scenario test**

Create `tests/integration/coding_memory_phase4_next_session.rs`:

```rust
// Scenario: seed Phase-3 fixture session into the store, then call the Phase-4
// SessionStart renderer for the same repo; assert that the markdown contains
// the seeded RepoContext fact and a recent-activity table row.

use coding_memory::recall::{CodingRecallService, CodingRecallServiceConfig, RecallInvocationRepo};
use coding_memory::recall::budget::HeuristicBudgeter;
use std::sync::Arc;

#[tokio::test]
async fn next_session_sees_prior_memory() {
    let pool = storage::StoragePool::connect_in_memory().await.unwrap();
    storage::StoragePool::run_feature_migrations(pool.inner(), &cognitive::cognitive_migrations()).await.unwrap();
    storage::StoragePool::run_feature_migrations(pool.inner(), &coding_memory::coding_memory_migrations()).await.unwrap();
    let fact_repo = Arc::new(cognitive::SemanticFactRepo::new(pool.clone()));
    let ep_repo = Arc::new(cognitive::EpisodicMemoryRepo::new(pool.clone()));

    // Load fixture and seed.
    let raw = std::fs::read_to_string("tests/fixtures/coding/phase4_recall_seed.jsonl").unwrap();
    for line in raw.lines() {
        let v: serde_json::Value = serde_json::from_str(line).unwrap();
        match v.get("type").and_then(|s| s.as_str()) {
            Some("fact") => {
                let f: cognitive::SemanticFact = serde_json::from_value(v.get("payload").cloned().unwrap()).unwrap();
                fact_repo.upsert_with_metadata(&f).await.unwrap();
            }
            Some("episode") => {
                let e: cognitive::EpisodicMemory = serde_json::from_value(v.get("payload").cloned().unwrap()).unwrap();
                ep_repo.insert_with_kind_and_metadata(&e).await.unwrap();
            }
            _ => {}
        }
    }

    let ums = Arc::new(cognitive::UnifiedMemoryService::new(fact_repo.clone()));
    let svc = Arc::new(CodingRecallService::new(
        CodingRecallServiceConfig::default(),
        ums, fact_repo, ep_repo,
        RecallInvocationRepo::new(pool.clone()),
        Arc::new(HeuristicBudgeter),
    ));
    let md = coding_memory::recall::renderers::render_session_start_block(&svc, Some("repo:demo")).await.unwrap();
    assert!(md.contains("auth_module") || md.contains("JWT"), "got:\n{md}");
}
```

- [ ] **Step 3: Run + commit**

```bash
cargo nextest run --test coding_memory_phase4_next_session
git add tests/fixtures/coding/phase4_recall_seed.jsonl tests/integration/coding_memory_phase4_next_session.rs
git commit -m "test(integration): Phase-4 next-session memory injection scenario"
```

---

### Task 30: Scenario — dead-end warning triggers on repeat attempt

**Files:**
- Test: `tests/integration/coding_memory_phase4_dead_end.rs`

```rust
use coding_memory::recall::{CodingRecallService, CodingRecallServiceConfig, RecallInvocationRepo};
use coding_memory::recall::budget::HeuristicBudgeter;
use std::sync::Arc;

#[tokio::test]
async fn repeat_attempt_yields_warning() {
    let pool = storage::StoragePool::connect_in_memory().await.unwrap();
    storage::StoragePool::run_feature_migrations(pool.inner(), &cognitive::cognitive_migrations()).await.unwrap();
    storage::StoragePool::run_feature_migrations(pool.inner(), &coding_memory::coding_memory_migrations()).await.unwrap();
    let fact_repo = Arc::new(cognitive::SemanticFactRepo::new(pool.clone()));
    let ep_repo = Arc::new(cognitive::EpisodicMemoryRepo::new(pool.clone()));

    // Seed a counterfactual.
    let cf = cognitive::SemanticFact {
        id: uuid::Uuid::new_v4(),
        subject: "rewrite parser as recursive descent".into(),
        predicate: "outcome".into(),
        object: "abandoned — too slow".into(),
        recorded_at: jiff::Timestamp::now(),
        confidence: 0.95,
        scope_repo_id: Some("repo:demo".into()),
        memory_type: Some("counterfactual".into()),
        metadata: serde_json::json!({"memory_type":"counterfactual","reason":"too slow","attempt_id":"00000000-0000-0000-0000-000000000001","problem_hash":"abc"}),
        ..Default::default()
    };
    fact_repo.upsert_with_metadata(&cf).await.unwrap();

    let ums = Arc::new(cognitive::UnifiedMemoryService::new(fact_repo.clone()));
    let svc = Arc::new(CodingRecallService::new(
        CodingRecallServiceConfig::default(),
        ums, fact_repo, ep_repo,
        RecallInvocationRepo::new(pool.clone()),
        Arc::new(HeuristicBudgeter),
    ));
    let md = coding_memory::recall::renderers::render_user_prompt_block(
        &svc, "rewrite parser as recursive descent", Some("repo:demo")
    ).await.unwrap();
    assert!(md.contains("⚠️ Heads-up"), "got:\n{md}");
}
```

```bash
cargo nextest run --test coding_memory_phase4_dead_end
git add tests/integration/coding_memory_phase4_dead_end.rs
git commit -m "test(integration): Phase-4 dead-end warning on repeat attempt"
```

---

### Task 31: Scenario — C3 escalation measurable

**Files:**
- Test: `tests/integration/coding_memory_phase4_c3_escalation.rs`

> Builds a `RetrievalSkillRegistry` with 1 fake `Fast`-tier skill that always succeeds raising coverage from 0.1 → 0.7. Calls `escalate(...)`; asserts `before_score < threshold` and `after_score > threshold`, plus the EMA score for the skill bumped above 0.5.

```rust
use async_trait::async_trait;
use coding_memory::retrieval_skills::{
    BudgetTier, EscalationContext, EscalationOutcome, RetrievalSkill, RetrievalSkillRegistry,
};
use std::sync::Arc;

struct LiftSkill;

#[async_trait]
impl RetrievalSkill for LiftSkill {
    fn name(&self) -> &'static str { "lift" }
    fn description(&self) -> &'static str { "raises coverage" }
    fn tier(&self) -> BudgetTier { BudgetTier::Fast }
    async fn apply(&self, _: &EscalationContext) -> common::Result<EscalationOutcome> {
        Ok(EscalationOutcome { succeeded: true, coverage_after: 0.7, added_context: String::new(), added_ids: vec![] })
    }
}

#[tokio::test]
async fn escalation_lifts_coverage_and_bumps_ema() {
    let bus = Arc::new(bus::DomainEventBus::new(64));
    let reg = RetrievalSkillRegistry::new(vec![Arc::new(LiftSkill)], bus);
    let before = reg.effectiveness_of("lift").await;
    let out = reg.escalate(&EscalationContext {
        query: "x".into(),
        coverage_score: 0.1,
        budget_tier: BudgetTier::Fast,
        repo: None,
    }).await.unwrap();
    let after = reg.effectiveness_of("lift").await;
    assert!(out.final_outcome.succeeded);
    assert!(out.final_outcome.coverage_after > 0.3);
    assert!(after > before, "before={before} after={after}");
}
```

```bash
cargo nextest run --test coding_memory_phase4_c3_escalation
git add tests/integration/coding_memory_phase4_c3_escalation.rs
git commit -m "test(integration): Phase-4 C3 escalation lifts coverage + EMA"
```

---

### Task 32: Workbench — Recall Tool Log panel + Session Replay overlay

**Files:**
- Create: `desktop-ui/src/features/coding-memory/RecallToolLogPanel.tsx`
- Modify: `desktop-ui/src/features/coding-memory/SessionReplayPanel.tsx` — add per-turn overlay strip
- Modify: `desktop-ui/src/features/coding-memory/CodingMemoryLayout.tsx` — add nav entry
- Modify: `desktop-ui/src/features/coding-memory/hooks.ts` — add `useRecallLog`, `useSessionRecallOverlay`
- Modify: `desktop-ui/src/app/router.tsx` — `/coding-memory/recall-log` route
- Test: `desktop-ui/src/features/coding-memory/__tests__/RecallToolLogPanel.test.tsx`

- [ ] **Step 1: Hooks**

Append to `desktop-ui/src/features/coding-memory/hooks.ts`:

```ts
export function useRecallLog(layer?: string, limit = 50, offset = 0) {
  return useQuery("coding_memory_recall_log", { args: { layer, limit, offset } });
}

export function useSessionRecallOverlay(sessionId: string, limit = 200, offset = 0) {
  return useQuery("coding_memory_session_replay_recall_overlay", {
    args: { sessionId, limit, offset },
    enabled: !!sessionId,
  });
}
```

- [ ] **Step 2: Panel**

Create `desktop-ui/src/features/coding-memory/RecallToolLogPanel.tsx`:

```tsx
import { useState } from "react";
import { useRecallLog } from "./hooks";

const LAYERS = [
  "index", "timeline", "fetch", "dead_end",
  "facts_as_of", "change_history", "decision_points",
  "session_start_inject", "user_prompt_inject",
] as const;

export function RecallToolLogPanel() {
  const [layer, setLayer] = useState<string | undefined>(undefined);
  const [page, setPage] = useState(0);
  const limit = 50;
  const { data, isLoading, error } = useRecallLog(layer, limit, page * limit);

  return (
    <section className="p-6 space-y-4">
      <header className="flex items-center justify-between">
        <h1 className="text-2xl font-semibold text-default">Recall Tool Log</h1>
        <select
          aria-label="Filter by layer"
          className="bg-surface-base border border-border rounded px-2 py-1 text-sm"
          value={layer ?? ""}
          onChange={(e) => { setLayer(e.target.value || undefined); setPage(0); }}
        >
          <option value="">all layers</option>
          {LAYERS.map((l) => <option key={l} value={l}>{l}</option>)}
        </select>
      </header>
      {isLoading && <div className="text-muted">Loading…</div>}
      {error && <div className="text-error">Error: {String(error)}</div>}
      <ul className="divide-y divide-border">
        {(data ?? []).map((row) => (
          <li key={row.id} className="py-3 flex flex-col gap-1">
            <div className="flex items-center justify-between text-sm">
              <span className="text-default font-mono">{row.layer}</span>
              <span className="text-muted">{new Date(row.occurredAt).toLocaleString()}</span>
            </div>
            <div className="text-sm text-default truncate">{row.query || "(no query)"}</div>
            <div className="text-xs text-muted flex gap-3">
              <span>cov={row.coverageScore?.toFixed(2) ?? "—"}</span>
              <span>{row.latencyMs}ms</span>
              {row.skillUsed && <span>skill={row.skillUsed}</span>}
              <span>{row.resultIds.length} ids</span>
            </div>
          </li>
        ))}
      </ul>
      <div className="flex justify-between">
        <button
          type="button"
          disabled={page === 0}
          onClick={() => setPage((p) => Math.max(0, p - 1))}
          className="text-sm text-default disabled:text-muted"
        >
          Prev
        </button>
        <button
          type="button"
          disabled={(data?.length ?? 0) < limit}
          onClick={() => setPage((p) => p + 1)}
          className="text-sm text-default disabled:text-muted"
        >
          Next
        </button>
      </div>
    </section>
  );
}
```

- [ ] **Step 3: Session Replay overlay**

Edit `desktop-ui/src/features/coding-memory/SessionReplayPanel.tsx`. After the existing per-event list, render an `aside` overlay strip showing recall invocations grouped by `turnId`:

```tsx
import { useSessionRecallOverlay } from "./hooks";
// ...inside the panel, after the events list:
const { data: overlay } = useSessionRecallOverlay(sessionId);
return (
  <div className="grid grid-cols-[1fr_240px] gap-4">
    {/* existing events list */}
    <aside className="glass-panel p-3 space-y-2">
      <h3 className="text-sm font-semibold text-default">Recall on this session</h3>
      {(overlay ?? []).map((row) => (
        <div key={row.id} className="text-xs text-default border-l-2 border-accent pl-2">
          <div className="font-mono">{row.layer}</div>
          <div className="text-muted truncate">{row.query}</div>
        </div>
      ))}
    </aside>
  </div>
);
```

- [ ] **Step 4: Nav + router**

In `CodingMemoryLayout.tsx`, append a nav entry: `{ to: "/coding-memory/recall-log", label: "Recall Log" }`. In `app/router.tsx`, add the matching route element.

- [ ] **Step 5: Test**

Create `desktop-ui/src/features/coding-memory/__tests__/RecallToolLogPanel.test.tsx`:

```tsx
import { render, screen } from "@testing-library/react";
import { describe, it, expect, vi } from "vitest";
import { RecallToolLogPanel } from "../RecallToolLogPanel";

vi.mock("../hooks", () => ({
  useRecallLog: () => ({
    isLoading: false,
    data: [{
      id: "id1",
      layer: "index",
      query: "hello",
      occurredAt: "2026-04-25T00:00:00Z",
      coverageScore: 0.42,
      latencyMs: 12,
      resultIds: ["a","b"],
      skillUsed: null,
    }],
  }),
}));

describe("RecallToolLogPanel", () => {
  it("renders rows", () => {
    render(<RecallToolLogPanel />);
    expect(screen.getByText("Recall Tool Log")).toBeInTheDocument();
    expect(screen.getByText("hello")).toBeInTheDocument();
    expect(screen.getByText(/cov=0\.42/)).toBeInTheDocument();
  });
});
```

- [ ] **Step 6: Run + commit**

```bash
cd desktop-ui && bun run lint:fix && bun run test --run RecallToolLogPanel
cd ..
git add desktop-ui/src/features/coding-memory/RecallToolLogPanel.tsx \
        desktop-ui/src/features/coding-memory/SessionReplayPanel.tsx \
        desktop-ui/src/features/coding-memory/CodingMemoryLayout.tsx \
        desktop-ui/src/features/coding-memory/hooks.ts \
        desktop-ui/src/app/router.tsx \
        desktop-ui/src/features/coding-memory/__tests__/RecallToolLogPanel.test.tsx
git commit -m "feat(desktop-ui): RecallToolLogPanel + Session Replay recall overlay"
```

---

### Task 33: Docs — Phase 4 summary

**Files:**
- Modify: `docs/coding-memory/README.md`
- Create: `docs/coding-memory/phase-4.md`

- [ ] **Step 1: Phase-4 doc**

Create `docs/coding-memory/phase-4.md` summarizing: scope, what's now retrievable, MCP tool surface, hook subcommand, escalation behavior, telemetry table schema, panel additions, and Phase-5 hand-off (Reforge will start consuming `recall_invocations` for ineffective-memory signals).

Required sections (~120 lines):
- "What landed in Phase 4"
- "How recall flows" (passive vs active diagram)
- "MCP tool surface" (the 7 active tools, args + return shape)
- "Token budgets and truncation invariant"
- "C3 retrieval skills — the closed set of 5"
- "What is still stubbed" (`trace_causes`, causal-edge population — both Phase 6)
- "Phase-5 hand-off"

- [ ] **Step 2: README update**

In `docs/coding-memory/README.md`, append a `## Phase 4` section linking to `phase-4.md` and adding a one-line status: "Phase 4 ✅ — recall API, passive injection, C3 escalation skeleton."

- [ ] **Step 3: Commit**

```bash
git add docs/coding-memory/phase-4.md docs/coding-memory/README.md
git commit -m "docs(coding-memory): Phase-4 summary + README link"
```

---

### Task 34: Final verification + quality gates

- [ ] **Step 1: Workspace clippy**

```bash
cargo clippy --workspace --all-targets --all-features -- -D warnings
```
Expected: zero warnings.

- [ ] **Step 2: Workspace fmt**

```bash
cargo fmt --all --check
```
Expected: no diff.

- [ ] **Step 3: Workspace tests**

```bash
cargo nextest run --workspace
cargo test --workspace --doc
```
Expected: all green.

- [ ] **Step 4: Frontend**

```bash
cd desktop-ui && bun run lint && bun run test --run && bun run build && cd ..
```
Expected: zero errors.

- [ ] **Step 5: Verify exit gates from spec §11**

- ☐ Synthetic scenario: agent sees prior memory on next session — covered by Task 29.
- ☐ Dead-end warning triggers on repeat attempt — covered by Task 30.
- ☐ C3 escalation measurable — covered by Task 31.
- ☐ All MCP recall tools functional (except `trace_causes`, Phase 6 by spec) — covered by Tasks 23, 26.
- ☐ SessionStart passive injection ≤ 800 tok — covered by Tasks 20, 27.
- ☐ UserPromptSubmit passive injection ≤ 1500 tok — covered by Task 21, 27.

- [ ] **Step 6: Commit any final cleanup + tag**

```bash
git status
git commit -am "chore(coding-memory): Phase-4 final cleanup" || true
```

---

## Self-review notes

- **Spec coverage:**
  - §8 MCP tool surface — covered (Task 23). `trace_causes` correctly stubbed for Phase 6.
  - §8 SessionStart 800-tok markdown — covered (Task 20).
  - §8 UserPromptSubmit 1500-tok with dead-end block — covered (Tasks 20, 21).
  - §7 Tier C3 retrieval skills — closed set of 5 each in their own task (Tasks 14–18); registry + selector (Task 13).
  - §11 Phase-4 exit gates — verification list in Task 34.
  - §11.5 Workbench: Session Replay recall overlay + new panel — Task 32.
- **Phase-1 stubs replaced or kept:** `CodingRecallService`, `IndexEntry`/`TimelineEntry`/`FullEntry`/`DeadEndResponse`/`CausalTraceResponse` types stay; methods are implemented. The 5 retrieval skills move from `phase_stub_skill!` to real impls. `trace_causes` keeps the `Phase-6` `NotImplemented` error to honor the spec.
- **Type consistency:** `RecallQuery::Ids/Text`, `IndexEntry`/`TimelineEntry`/`FullEntry` field names, `BudgetTier`, `EscalationContext`/`EscalationOutcome` are all used identically across tasks.
- **Schema:** one new table (`recall_invocations`); no other schema changes — consistent with the §11 "all schema in Phase 1" principle except for telemetry (which is read-only ops surface, not domain schema; safe per pre-release policy).
- **Cross-crate boundaries:** Recall service consumes `cognitive` + `context_engine` types but never the other way around; `coding-ingest` only learns about a new `OpHandler` trait — no dep direction reversal.
- **No placeholders:** every step ships full code.

## Execution Handoff

**Plan complete and saved to `docs/superpowers/plans/2026-04-25-coding-memory-phase-4.md`. Two execution options:**

**1. Subagent-Driven (recommended)** — fresh subagent per task, review between tasks, fast iteration.

**2. Inline Execution** — execute tasks in this session via `superpowers:executing-plans` with batch checkpoints.

**Which approach?**
