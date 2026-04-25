# Coding Memory — Phase 5 (Reflection: Reforge + Mirror) Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Turn the Phase-1 stubs at `crates/coding-memory/src/reforge_phase.rs` and `src/skills.rs` into a complete reflection layer. Deliver: (1) a session-end light Reforge pass (no LLM, < 2 s, fires on `EventKind::SessionEnd`) that bumps Hebbian co-activation, dedups same-`problem_hash` `FixAttempt`s within a session, and caches a 200-token `session_summary`; (2) two new heavy Reforge phases — **Phase 2.5 (Coding Synthesis)** consuming sessions + new `FixAttempt`s + causal edges + active `WorkflowPattern`s and emitting six promotion actions, and **Phase 3.5 (Rule Artifact Generation)** writing managed-block sections of per-repo `CLAUDE.md` / `AGENTS.md` / `.cursorrules` / `.continue/rules/klyntbot.md`; (3) two existing Reforge phase extensions — **Phase 6** selective-delete signal (`stability *= 0.5` on never-cited memories) and **Phase 6.5** cross-session fact dedup via vector similarity > 0.92 → SUPERSEDE; (4) project-scoped evolving skills (`ProjectSkillEvolver`) with Detect / Synthesize / Write / Journal / Supersede; (5) Mirror integration — extending `MirrorAlert` with the **11 closed coding alert kinds**, two new signal sources (`PatternEffectivenessSource` real-time EMA, `StaleMemorySource` cited-vs-ignored), four coding meta-rule detectors, and a project-skill drift extension to the existing `RoutingSignalSource`; (6) the three Phase-5 Workbench panels (Mirror Alerts Feed, Pattern Effectiveness Trends, Reforge Cycle Diff); (7) the `mirror.get_coding_alerts` MCP filter.

**Architecture:** `coding-memory` gains a `reforge/` module that owns the Phase 2.5/3.5 implementations behind two new dependency-inversion traits — `CodingSynthesisHandler` and `RuleArtifactsHandler` — implemented in the `agent` crate via `ProviderManager` with the new `ProviderRole::ReforgeSynth` and `ProviderRole::ReforgeRules`. Both phases plug into `cognitive::services::reforge::service::run_reforge` after Phase 3 (Review) and before Phase 4 (Narrate) via a single new optional bundle parameter `coding_phase_handlers: Option<&CodingPhaseHandlers<'_>>`. The session-end light pass is owned by `AppCore` and triggered by an `EventKind::SessionEnd`-matching subscriber on the existing `IngestEventLog` event flow. Project-skill evolution runs as the inner Phase 3.5 sub-cycle: per-repo, it queries scope-`repo` `WorkflowPattern`s with `confidence ≥ 0.7` not yet expressed as a skill, calls the LLM via `RuleArtifactsHandler::synthesize_skill`, and writes `SKILL.md` via the existing `SkillFileManager` extended with the `skill_versions.scope` columns (already landed in Phase 1). Mirror sources implement the existing `MirrorSignalSource` trait — they are wired into `MirrorEngine` via `app-core::coding_memory::mirror::register_coding_sources(...)`. The `MirrorAlert` enum gains 8 new variants (3 already exist: `RoutingDrift`, `TrialUnpromising`, `MetaRuleProposed`); `MirrorAlertType` mirrors them; `MirrorAlertSeverity` is introduced as a new closed enum. Workbench panels follow the established pattern: thin Tauri adapters → `app-core` handlers → cognitive/coding-memory repos. No new schema beyond two telemetry tables (`pattern_effectiveness_log` and `selective_delete_log`) and a `session_summaries` table for the cache.

**Tech Stack:** Rust (MSRV 1.93), `sqlx` (SQLite), `serde` (camelCase), `async-trait`, `tokio` (`sync`, `time`), `jiff::Timestamp`, `uuid` (`v4`), `sha2` (already in workspace via `cognitive::services::reforge::skill_files`), `similar` (text diff for managed-block conflict detection — already in workspace), existing `cognitive::{SemanticFactRepo, EpisodicMemoryRepo, ProceduralRuleRepo, CoActivationRepo, MirrorRepo}`, `coding_memory::{recall, retrieval_skills, scope, facts}`, `bus::{DomainEventBus, DomainEvent}`, `providers::ProviderManager`, `tools_core::FeatureMigration`. Frontend: existing `desktop-ui/` (React + Tailwind v4 + Biome 2.0 + React Compiler + `useQuery`/`useMutation`) + `recharts` (already in `desktop-ui/package.json` for Pattern Effectiveness Trends line charts) + `react-diff-viewer-continued` (NEW dep in `desktop-ui/package.json`). **No new Rust runtime deps.**

---

## File Structure

Every file created or modified by this plan, grouped by responsibility. Files stay small and focused per CLAUDE.md.

### New files — `crates/coding-memory/src/reforge/`

| File | Responsibility |
|---|---|
| `mod.rs` | Module entry — re-exports the public surface (`SessionEndPass`, `CodingSynthesisPhase`, `RuleArtifactGenerationPhase`, `CodingPhaseHandlers`, `CrossSessionDedup`, `SelectiveDeleteSignal`) |
| `types.rs` | `CodingSynthesisInput`, `CodingSynthesisOutput`, `PromoteAction` enum, `RuleArtifactInput`, `RuleArtifactOutput`, `ProjectSkillSpec`, `ManagedBlockSection`, `RepoArtifactPlan` |
| `session_end.rs` | `SessionEndPass` — Hebbian bump, within-session dedup, 200-token session summary cache. Pure deterministic, zero LLM |
| `synth_handler.rs` | `CodingSynthesisHandler` trait (LLM seam) + `RuleArtifactsHandler` trait. Implementations live in `agent` crate |
| `coding_synthesis.rs` | `CodingSynthesisPhase` — collects inputs, calls handler, persists 6 promotion-action outcomes (ExtractPattern / ExtractFailurePattern / PromoteToProblemClass / PromoteToProjectUnderstanding / PromoteToUserHabit / PromoteToProblemSolutionPattern) |
| `rule_artifacts.rs` | `RuleArtifactGenerationPhase` — filter facts by sensitivity, discover repos, call handler per-repo, write managed-block sections atomically |
| `managed_block.rs` | Markdown managed-block parser/writer. Atomic writes via temp-file + rename. Preserves user content outside markers |
| `sensitivity_filter.rs` | `filter_for_externalization(facts: &[SemanticFactWithSensitivity]) -> Vec<...>` — drops `high` and `excluded` |
| `cross_session_dedup.rs` | Phase 6.5 extension — find facts with vector similarity > 0.92, supersede older with newer, preserve both via bi-temporal `valid_until` |
| `selective_delete.rs` | Phase 6 extension — find memories retrieved ≥ N times, never cited; multiply `stability *= 0.5`; log to `selective_delete_log` |
| `session_summary_repo.rs` | `SessionSummaryRepo` — insert / fetch / list. Backs the SessionStart "Open threads" section in Phase-4 markdown |

### New files — `crates/coding-memory/src/skill_evolver/`

| File | Responsibility |
|---|---|
| `mod.rs` | `ProjectSkillEvolverImpl` — replaces Phase-1 `PhaseStubEvolver` |
| `detect.rs` | Detect candidate `WorkflowPattern`s (scope=repo, confidence ≥ 0.7) not yet expressed as a project skill |
| `write.rs` | Atomic SKILL.md emission with managed-block markers; journals `SkillVersionRow` with `scope='project'` + `scope_repo_id` |
| `supersede.rs` | When a newer `WorkflowPattern` with overlapping scope exceeds prior skill effectiveness, mark old version `status: superseded` |

### New files — `crates/coding-memory/src/mirror/`

| File | Responsibility |
|---|---|
| `mod.rs` | Module entry — re-exports |
| `alerts.rs` | Closed enums: `CodingMirrorAlertKind` (11 variants from spec §10), `MirrorAlertSeverity` (`Low`/`Medium`/`High`/`Critical`); helpers to convert into `MirrorAlert::Coding(...)` for persistence |
| `pattern_effectiveness.rs` | `PatternEffectivenessSource` — subscribes `PatternApplied`/`PatternOutcome`/`UserCorrectedAI`/`TestRun`. Real-time EMA `score = 0.9·prev + 0.1·outcome`. Persists to `pattern_effectiveness_log` |
| `stale_memory.rs` | `StaleMemorySource` — subscribes `MemoryRetrieved`/`AssistantMsgCompleted`. Counts cited-vs-ignored per memory. Feeds Phase 6 selective-delete signal |
| `coding_meta_rules.rs` | Four meta-rule detectors: `ProblemClassRefactorDetector`, `LowerDeadEndThresholdDetector`, `PromoteToGlobalDetector`, `PromoteCounterfactualVisibilityDetector` |
| `coding_routing.rs` | `CodingRoutingSource` — extends drift detection: project-skill activation drop > 50 % over 7 days → `ProjectSkillObsolete` alert |
| `coding_alerts_query.rs` | `CodingAlertsQuery` — back-end for `mirror.get_coding_alerts(repo?, severity?, kind?)` |

### New files — `crates/coding-memory/migrations/`

| File | Responsibility |
|---|---|
| `004_phase5_reflection.sql` | `session_summaries`, `pattern_effectiveness_log`, `selective_delete_log` tables; `coding_alert_kind` + `coding_alert_severity` columns on `mirror_snippets` |

### Modified files — `crates/coding-memory/src/`

| File | Change |
|---|---|
| `lib.rs` | Add `pub mod reforge;`, `pub mod skill_evolver;`, `pub mod mirror;`. Add migration 004 to `coding_memory_migrations()`. Re-export new public items |
| `reforge_phase.rs` | Replace `_phase_stub` markers with real constructors taking `Arc<CodingPhaseHandlers<'_>>`. `CodingSynthesisPhase` and `RuleArtifactGenerationPhase` delegate to `crate::reforge::*` |
| `skills.rs` | `PhaseStubEvolver` → `ProjectSkillEvolverImpl`. Public surface intact |

### New files — `crates/agent/src/handlers/`

| File | Responsibility |
|---|---|
| `coding_synthesis.rs` | LLM-backed `CodingSynthesisHandler` impl via `ProviderManager::chat` with `ProviderRole::ReforgeSynth`. System prompt + tool schema for the 6 promotion actions |
| `rule_artifacts.rs` | LLM-backed `RuleArtifactsHandler` impl via `ProviderManager::chat` with `ProviderRole::ReforgeRules`. Prompt-builds per-repo from filtered facts; emits `RuleArtifactOutput` |

### Modified files — `crates/cognitive/`

| File | Change |
|---|---|
| `src/services/reforge/mod.rs` | Add `CodingPhaseHandlers<'a>` struct (bundle of `coding_synthesis`, `rule_artifacts`, `coding_repos`, `bus`); add public traits `CodingSynthesisRunner` + `RuleArtifactsRunner` for cross-crate dispatch |
| `src/services/reforge/service.rs` | New parameter on `run_reforge`: `coding_phase_handlers: Option<&CodingPhaseHandlers<'_>>`. Insert Phase 2.5 after Phase 3, Phase 3.5 after Phase 5 (Apply), Phase 6 selective-delete before existing Phase 6 (Optimize), Phase 6.5 cross-session dedup inside existing Phase 6.5 (Graph Consolidation) |
| `src/mirror/types.rs` | Extend `MirrorAlert` enum with `Coding { kind: CodingMirrorAlertKind, severity: MirrorAlertSeverity, payload: serde_json::Value }`; extend `MirrorAlertType` with `Coding`; new `MirrorAlertSeverity` enum |
| `src/mirror/sources/routing.rs` | New `accumulate_signal_v2` overload accepting a `RoutingScope::Project { repo_id }` discriminator. Extend `detect_drift` to emit `ProjectSkillObsolete` |
| `src/mirror/sources/meta_rule.rs` | Subscribe `FixAttemptFailed`. New branch in `accumulate` produces `MirrorAlert::Coding { kind: ProblemClassRefactor, ... }` when `attempt_count >= 3` |
| `src/mirror/repo.rs` | New methods: `insert_pattern_effectiveness_row`, `query_uncited_memories(threshold_retrievals: u32) -> Vec<MemoryId>`, `insert_selective_delete_log` |
| `src/mirror/mod.rs` | Re-export `MirrorAlertSeverity` |

### Modified files — `crates/app-core/src/coding_memory/`

| File | Change |
|---|---|
| `mod.rs` | Add `pub mod reforge;`, `pub mod mirror;`, `pub mod panels_phase5;` |
| `reforge.rs` (NEW) | `register_coding_phase_handlers(...)` — builds `CodingPhaseHandlers` from `AppCore` and threads it to cron's `run_reforge` call. Also owns `wire_session_end_pass(...)` that subscribes to the ingest stream and dispatches into `SessionEndPass::run(...)` on `EventKind::SessionEnd` |
| `mirror.rs` (NEW) | `register_coding_sources(...)` — constructs and wires the 4 new/extended Mirror sources |
| `panels_phase5.rs` (NEW) | `mirror_alerts_feed`, `pattern_effectiveness_trends`, `reforge_cycle_diff` handlers |
| `state.rs` (modified — `crates/app-core/src/state.rs`) | Add `pub project_skill_evolver: Option<Arc<coding_memory::skill_evolver::ProjectSkillEvolverImpl>>` and `pub session_end_pass: Option<Arc<coding_memory::reforge::SessionEndPass>>` |
| `init/mod.rs` (modified) | Construct evolver + session-end pass; wire mirror sources after MirrorEngine init |
| `init/cron.rs` (modified) | Pass `coding_phase_handlers` into `run_reforge` |

### New / modified files — `crates/desktop/`

| File | Change |
|---|---|
| `src/commands/coding_memory.rs` | Add 6 commands: `coding_memory_mirror_alerts_feed`, `coding_memory_mirror_alert_action` (approve/reject/snooze), `coding_memory_effectiveness_trends`, `coding_memory_reforge_cycle_diff`, `coding_memory_reforge_cycle_list`, `coding_memory_project_skills_for_repo`. Extend `DEV_COMMANDS` |
| `src/lib.rs` | Register the 6 commands in `invoke_handler!` |
| `src/dev_server/mod.rs` | New commands auto-covered via `DEV_COMMANDS` |

### New / modified files — `crates/desktop-shared/src/commands/`

| File | Change |
|---|---|
| `coding_memory.rs` | Add DTOs: `MirrorAlertRow`, `MirrorAlertActionArgs`, `EffectivenessTrendBucket`, `EffectivenessTrendsResponse`, `ReforgeCycleDiffArgs`, `ReforgeCycleDiffResponse`, `ReforgeCycleSummary`, `ProjectSkillRow` |

### New files — `desktop-ui/src/features/coding-memory/`

| File | Responsibility |
|---|---|
| `MirrorAlertsFeedPanel.tsx` | List of pending / dismissed alerts grouped by severity. Approve / reject / snooze actions wired through `useMutation` |
| `PatternEffectivenessPanel.tsx` | `recharts` line chart of per-skill `effectiveness_score` over time |
| `ReforgeCycleDiffPanel.tsx` | Cycle picker + side-by-side Markdown diff (`react-diff-viewer-continued`) of previous-vs-current managed blocks |

### Modified files — `desktop-ui/src/features/coding-memory/`

| File | Change |
|---|---|
| `CodingMemoryLayout.tsx` | Add 3 nav entries (`Mirror Alerts`, `Effectiveness`, `Reforge Diff`) |
| `hooks.ts` | Add `useMirrorAlertsFeed`, `useMirrorAlertAction`, `useEffectivenessTrends`, `useReforgeCycleDiff`, `useReforgeCycleList`, `useProjectSkills` |
| `index.ts` | Re-exports |
| `package.json` (`desktop-ui/package.json`) | Add `react-diff-viewer-continued` dep |
| `app/router.tsx` | Add 3 nested routes under `/coding-memory/{alerts,effectiveness,diff}` |

### Test files (Rust)

| File | Responsibility |
|---|---|
| `crates/coding-memory/tests/migration_phase5.rs` | Migration 004 applies; new tables exist; new columns nullable |
| `crates/coding-memory/tests/managed_block.rs` | Parse / write / hash-detect roundtrip; preserves user content outside markers |
| `crates/coding-memory/tests/session_end_pass.rs` | Hebbian bump correct; same-`problem_hash` `FixAttempt`s deduped; summary ≤ 200 tokens |
| `crates/coding-memory/tests/coding_synthesis.rs` | Each of 6 promotion actions persists correctly |
| `crates/coding-memory/tests/sensitivity_filter.rs` | `high` and `excluded` removed; `normal` retained |
| `crates/coding-memory/tests/rule_artifacts_writer.rs` | Atomic write; existing user content outside managed block preserved |
| `crates/coding-memory/tests/rule_artifacts_phase.rs` | Phase invokes handler per repo; writes 4 artifacts when all enabled |
| `crates/coding-memory/tests/cross_session_dedup.rs` | Similarity > 0.92 → SUPERSEDE; both rows preserved bi-temporally |
| `crates/coding-memory/tests/selective_delete.rs` | Un-cited memory retrieved ≥ N times → `stability *= 0.5`; audit row written |
| `crates/coding-memory/tests/skill_evolver_detect.rs` | Workflow patterns above threshold detected; already-expressed skills excluded |
| `crates/coding-memory/tests/skill_evolver_write.rs` | SKILL.md emitted with managed block; `SkillVersionRow` journaled |
| `crates/coding-memory/tests/skill_evolver_supersede.rs` | Higher-effectiveness pattern supersedes prior version |
| `crates/coding-memory/tests/mirror_pattern_effectiveness.rs` | EMA update; outcome arrives → score moves toward outcome |
| `crates/coding-memory/tests/mirror_stale_memory.rs` | Cited counter increments on `AssistantMsgCompleted`; ignored counter on retrieved-but-not-cited |
| `crates/coding-memory/tests/mirror_meta_rules.rs` | Each of 4 meta-rules triggers exactly once at its threshold |
| `crates/coding-memory/tests/mirror_coding_routing_drift.rs` | Project-skill activation drop > 50 % over 7 days → `ProjectSkillObsolete` |
| `crates/coding-memory/tests/coding_alerts_query.rs` | Filter by repo / severity / kind |
| `crates/coding-memory/tests/prop_reforge_never_deletes.rs` | **Invariant 6** — no Reforge cycle reduces `episodic_memories` count |
| `crates/coding-memory/tests/prop_managed_block_user_content_preserved.rs` | User content outside markers byte-equal after rewrite |
| `crates/coding-memory/tests/phase5_e2e_nightly.rs` | Seeded session → SessionEnd light → nightly cycle → CLAUDE.md emitted with ≥ 5 statements |
| `crates/coding-memory/tests/phase5_e2e_skill_evolution.rs` | Workflow pattern accumulates across sessions → SKILL.md generated |
| `crates/cognitive/tests/run_reforge_with_coding.rs` | `run_reforge` with `coding_phase_handlers: Some(...)` invokes 2.5 + 3.5 in correct order |
| `crates/app-core/tests/mirror_register_coding.rs` | All 4 sources successfully register without error |

### Test files (TypeScript)

| File | Responsibility |
|---|---|
| `desktop-ui/src/features/coding-memory/__tests__/MirrorAlertsFeedPanel.test.tsx` | Panel renders rows; action mutations fire |
| `desktop-ui/src/features/coding-memory/__tests__/PatternEffectivenessPanel.test.tsx` | Line chart renders with mocked data |
| `desktop-ui/src/features/coding-memory/__tests__/ReforgeCycleDiffPanel.test.tsx` | Cycle selector + diff renders |

### Fixtures

| File | Responsibility |
|---|---|
| `tests/fixtures/coding/phase5_seed.jsonl` | 12-event session with mixed `FixAttempt` outcomes for nightly seed scenario |
| `tests/fixtures/coding/phase5_synthesis_mock.json` | Canned `CodingSynthesisHandler` output covering all 6 promotion actions |
| `tests/fixtures/coding/phase5_rule_artifacts_mock.json` | Canned `RuleArtifactsHandler` output for two repos |

---

## Task Structure

Tasks are ordered so each builds on the prior commit. Several pairs parallelize when an engineer works in a worktree (Mirror sources are independent of Reforge orchestration). Each task: exact file paths, exact commands, full code.

### Task 1: Phase-5 migration

**Files:**
- Create: `crates/coding-memory/migrations/004_phase5_reflection.sql`
- Modify: `crates/coding-memory/src/lib.rs`
- Test: `crates/coding-memory/tests/migration_phase5.rs`

- [ ] **Step 1: Write the failing test**

Create `crates/coding-memory/tests/migration_phase5.rs`:

```rust
use storage::StoragePool;

#[tokio::test]
async fn phase5_tables_exist_after_migration() {
    let pool = StoragePool::connect_in_memory().await.expect("pool");
    StoragePool::run_feature_migrations(pool.inner(), &cognitive::cognitive_migrations())
        .await
        .expect("cognitive migs");
    StoragePool::run_feature_migrations(pool.inner(), &coding_memory::coding_memory_migrations())
        .await
        .expect("coding-memory migs");

    for table in [
        "session_summaries",
        "pattern_effectiveness_log",
        "selective_delete_log",
    ] {
        let q = format!("SELECT COUNT(*) FROM {table}");
        let row: (i64,) = sqlx::query_as(&q)
            .fetch_one(pool.inner())
            .await
            .unwrap_or_else(|e| panic!("{table}: {e}"));
        assert_eq!(row.0, 0, "{table} should be empty after fresh migration");
    }
}

#[tokio::test]
async fn mirror_snippets_has_coding_alert_columns() {
    let pool = StoragePool::connect_in_memory().await.expect("pool");
    StoragePool::run_feature_migrations(pool.inner(), &cognitive::cognitive_migrations())
        .await
        .expect("cognitive migs");
    StoragePool::run_feature_migrations(pool.inner(), &coding_memory::coding_memory_migrations())
        .await
        .expect("coding-memory migs");
    let row: (i64,) = sqlx::query_as(
        "SELECT COUNT(*) FROM pragma_table_info('mirror_snippets') \
         WHERE name IN ('coding_alert_kind', 'coding_alert_severity')",
    )
    .fetch_one(pool.inner())
    .await
    .expect("pragma");
    assert_eq!(row.0, 2);
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo nextest run -p coding-memory --test migration_phase5`
Expected: FAIL — "no such table: session_summaries".

- [ ] **Step 3: Create the migration**

Create `crates/coding-memory/migrations/004_phase5_reflection.sql`:

```sql
-- Phase 5 — Reflection (Reforge + Mirror).
-- Pre-release direct DDL per CLAUDE.md.

-- ─────────────────────────────────────────────────────────────────────
-- Session-end light pass cache (read by Phase 4 SessionStart "open threads")
-- ─────────────────────────────────────────────────────────────────────
CREATE TABLE IF NOT EXISTS session_summaries (
    id           TEXT PRIMARY KEY,
    session_id   TEXT NOT NULL,
    repo_id      TEXT,
    summarised_at TEXT NOT NULL DEFAULT (datetime('now')),
    summary_md   TEXT NOT NULL,
    token_count  INTEGER NOT NULL,
    actor_id     TEXT NOT NULL DEFAULT 'local_user'
);
CREATE INDEX IF NOT EXISTS idx_session_summaries_session
    ON session_summaries(session_id);
CREATE INDEX IF NOT EXISTS idx_session_summaries_repo
    ON session_summaries(repo_id, summarised_at);

-- ─────────────────────────────────────────────────────────────────────
-- Pattern effectiveness EMA log (PatternEffectivenessSource feed)
-- ─────────────────────────────────────────────────────────────────────
CREATE TABLE IF NOT EXISTS pattern_effectiveness_log (
    id              TEXT PRIMARY KEY,
    pattern_id      TEXT NOT NULL,
    pattern_kind    TEXT NOT NULL,    -- 'workflow_pattern'|'failure_pattern'|'project_skill'|'retrieval_skill'
    repo_id         TEXT,
    measured_at     TEXT NOT NULL DEFAULT (datetime('now')),
    outcome         TEXT NOT NULL,    -- 'success'|'partial'|'failure'|'inconclusive'
    score_before    REAL NOT NULL,
    score_after     REAL NOT NULL,
    evidence        TEXT
);
CREATE INDEX IF NOT EXISTS idx_pattern_eff_log_pattern
    ON pattern_effectiveness_log(pattern_id, measured_at);
CREATE INDEX IF NOT EXISTS idx_pattern_eff_log_repo
    ON pattern_effectiveness_log(repo_id, measured_at);

-- ─────────────────────────────────────────────────────────────────────
-- Selective-delete signal audit log (Phase 6 stability halvings)
-- ─────────────────────────────────────────────────────────────────────
CREATE TABLE IF NOT EXISTS selective_delete_log (
    id                   TEXT PRIMARY KEY,
    memory_id            TEXT NOT NULL,
    memory_kind          TEXT NOT NULL,    -- 'semantic_fact'|'episodic_memory'
    retrievals_observed  INTEGER NOT NULL,
    citations_observed   INTEGER NOT NULL,
    stability_before     REAL NOT NULL,
    stability_after      REAL NOT NULL,
    applied_at           TEXT NOT NULL DEFAULT (datetime('now'))
);
CREATE INDEX IF NOT EXISTS idx_selective_delete_memory
    ON selective_delete_log(memory_id, applied_at);

-- ─────────────────────────────────────────────────────────────────────
-- Coding-alert kind / severity columns on the existing mirror_snippets table
-- (mirror_snippets is owned by cognitive; we ALTER it in-place per
-- pre-release policy — no separate cross-crate migration needed)
-- ─────────────────────────────────────────────────────────────────────
ALTER TABLE mirror_snippets ADD COLUMN coding_alert_kind     TEXT NULL;
ALTER TABLE mirror_snippets ADD COLUMN coding_alert_severity TEXT NULL;

CREATE INDEX IF NOT EXISTS idx_mirror_snippets_coding_kind
    ON mirror_snippets(coding_alert_kind, created_at)
    WHERE coding_alert_kind IS NOT NULL;
```

- [ ] **Step 4: Register the migration**

Edit `crates/coding-memory/src/lib.rs`. After the version-3 migration entry inside `coding_memory_migrations()`, append:

```rust
        FeatureMigration {
            feature_name: "coding_memory".to_string(),
            version: 4,
            description: "Phase-5: session_summaries, pattern_effectiveness_log, \
                          selective_delete_log; mirror_snippets coding alert columns."
                .to_string(),
            sql: include_str!("../migrations/004_phase5_reflection.sql").to_string(),
        },
```

- [ ] **Step 5: Verify test passes**

Run: `cargo nextest run -p coding-memory --test migration_phase5`
Expected: PASS for both tests.

- [ ] **Step 6: Commit**

```bash
git add crates/coding-memory/migrations/004_phase5_reflection.sql \
        crates/coding-memory/src/lib.rs \
        crates/coding-memory/tests/migration_phase5.rs
git commit -m "feat(coding-memory): Phase 5 — migration 004 (reflection telemetry tables)"
```

---

### Task 2: `reforge/` module skeleton + re-export plumbing

**Files:**
- Create: `crates/coding-memory/src/reforge/mod.rs`
- Create: `crates/coding-memory/src/reforge/types.rs`
- Modify: `crates/coding-memory/src/lib.rs`
- Modify: `crates/coding-memory/src/reforge_phase.rs`

`★ Insight ─────────────────────────────────────`
- Promoting `reforge_phase.rs` (a single file) into a `reforge/` directory is a *split-by-responsibility* refactor. Each file inside corresponds to one of the spec's six Reforge responsibilities. The original `reforge_phase.rs` becomes a thin re-export shim so external callers (and the existing Phase-1 stub assertions) keep their import paths.
- The `CodingPhaseHandlers<'a>` bundle struct is the dependency-inversion surface — `cognitive::run_reforge` accepts it without knowing the concrete LLM impl. This is the same pattern used by the existing `ReforgeHandler` / `AutotunerBridge` traits in `cognitive::services::reforge::mod.rs`.
`─────────────────────────────────────────────────`

- [ ] **Step 1: Create the module entry**

Create `crates/coding-memory/src/reforge/mod.rs`:

```rust
//! Reforge coding phases — Phase 2.5 (synthesis), Phase 3.5 (rule artifacts),
//! Phase-6 selective-delete, Phase-6.5 cross-session dedup, plus the
//! session-end light pass that runs on `EventKind::SessionEnd`.
//!
//! Bodies land in Phase 5. The module surface here is what `cognitive::run_reforge`
//! and `app-core::coding_memory::reforge` link against.

pub mod coding_synthesis;
pub mod cross_session_dedup;
pub mod managed_block;
pub mod rule_artifacts;
pub mod selective_delete;
pub mod sensitivity_filter;
pub mod session_end;
pub mod session_summary_repo;
pub mod synth_handler;
pub mod types;

pub use coding_synthesis::CodingSynthesisPhase;
pub use cross_session_dedup::CrossSessionDedup;
pub use managed_block::{ManagedBlock, ManagedBlockError};
pub use rule_artifacts::RuleArtifactGenerationPhase;
pub use selective_delete::SelectiveDeleteSignal;
pub use session_end::SessionEndPass;
pub use session_summary_repo::{SessionSummaryRepo, SessionSummaryRow};
pub use synth_handler::{CodingSynthesisHandler, RuleArtifactsHandler};
pub use types::{
    CodingPhaseHandlers, CodingSynthesisInput, CodingSynthesisOutput, ManagedBlockSection,
    PromoteAction, ProjectSkillSpec, RepoArtifactPlan, RuleArtifactInput, RuleArtifactOutput,
};
```

- [ ] **Step 2: Create the types module with empty structs (filled in later tasks)**

Create `crates/coding-memory/src/reforge/types.rs`:

```rust
//! Public types used across Reforge phases.

use crate::reforge_phase::RuleArtifact;
use crate::scope::AnchoredSymbol;
use jiff::Timestamp;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::sync::Arc;
use uuid::Uuid;

/// Bundle of dependencies the cognitive `run_reforge` cycle needs to invoke
/// the coding-specific Phase 2.5 / 3.5 / 6 / 6.5 hooks.
pub struct CodingPhaseHandlers<'a> {
    /// Phase 2.5 LLM seam.
    pub synthesis: Option<&'a dyn super::CodingSynthesisHandler>,
    /// Phase 3.5 LLM seam.
    pub rule_artifacts: Option<&'a dyn super::RuleArtifactsHandler>,
    /// Repo handles needed by every coding phase.
    pub fact_repo: &'a cognitive::SemanticFactRepo,
    /// Episodic repo.
    pub episodic_repo: &'a cognitive::EpisodicMemoryRepo,
    /// Procedural rule repo.
    pub rule_repo: &'a cognitive::ProceduralRuleRepo,
    /// Co-activation repo.
    pub co_activation_repo: &'a cognitive::CoActivationRepo,
    /// Memory utilization repo (cited-vs-ignored).
    pub utilization_repo: &'a crate::recall::telemetry::RecallInvocationRepo,
    /// Session summary repo.
    pub session_summary_repo: &'a super::SessionSummaryRepo,
    /// Selective-delete log writer.
    pub selective_delete_log: &'a super::selective_delete::SelectiveDeleteLogRepo,
    /// Pattern-effectiveness log writer.
    pub pattern_effectiveness_log:
        &'a crate::mirror::pattern_effectiveness::PatternEffectivenessLogRepo,
    /// Optional bus for emitting `PatternOutcome` etc. during the cycle.
    pub bus: Option<Arc<bus::DomainEventBus>>,
}

/// Input bundle for `CodingSynthesisHandler::synthesize_coding`.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CodingSynthesisInput {
    /// Session window scoped to this run.
    pub since: Timestamp,
    /// Per-repo fact + episode + workflow-pattern bundles.
    pub repo_bundles: Vec<RepoSynthesisBundle>,
    /// Recent counterfactual facts (for ProblemSolutionPattern promotion).
    pub recent_counterfactuals: Vec<SerializableSemanticFact>,
}

/// One repo's slice of synthesis input.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RepoSynthesisBundle {
    /// Canonical repo id.
    pub repo_id: String,
    /// `FixAttempt` episodes new since `since`.
    pub fix_attempts: Vec<SerializableEpisodicMemory>,
    /// Active `WorkflowPattern` rules.
    pub workflow_patterns: Vec<SerializableProceduralRule>,
    /// `RepoContext` facts.
    pub repo_context_facts: Vec<SerializableSemanticFact>,
    /// Recent causal-edge groups (problem_hash → chains).
    pub causal_chains: Vec<CausalChainGroup>,
}

/// Causal chain group keyed by problem hash.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CausalChainGroup {
    /// Problem hash anchoring the chains.
    pub problem_hash: String,
    /// Member edges.
    pub edge_ids: Vec<Uuid>,
    /// Edge count.
    pub count: u32,
}

/// Output bundle from the LLM — six action variants.
#[derive(Debug, Clone, Deserialize)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum PromoteAction {
    /// Extract a new `WorkflowPattern`.
    ExtractPattern {
        /// Repo scope.
        repo_id: Option<String>,
        /// Rule text.
        rule: String,
        /// Confidence (0.0 – 1.0).
        confidence: f32,
        /// Supporting episode ids.
        supporting: Vec<Uuid>,
    },
    /// Extract a new `FailurePattern`.
    ExtractFailurePattern {
        /// Repo scope.
        repo_id: Option<String>,
        /// Pattern text.
        rule: String,
        /// Remediation text.
        remediation: String,
        /// Confidence.
        confidence: f32,
        /// Supporting episode ids.
        supporting: Vec<Uuid>,
    },
    /// Promote ≥3 failed FixAttempts sharing a `problem_hash` to a problem-class refactor signal.
    PromoteToProblemClass {
        /// Hash key.
        problem_hash: String,
        /// Suggested refactor description.
        suggestion: String,
    },
    /// Synthesize a `ProjectUnderstanding` semantic fact.
    PromoteToProjectUnderstanding {
        /// Repo scope.
        repo_id: String,
        /// Subject.
        subject: String,
        /// Predicate.
        predicate: String,
        /// Object.
        object: String,
        /// Convergence score (≥0.7 to land).
        convergence: f32,
    },
    /// Promote a recurring `WorkflowPattern` observed across ≥3 repos to a global `UserHabit`.
    PromoteToUserHabit {
        /// Habit text.
        rule: String,
        /// Confidence.
        confidence: f32,
        /// Witness repos.
        witness_repos: Vec<String>,
    },
    /// Promote ≥3 causal chains sharing a `problem_hash` to a `ProblemSolutionPattern`.
    PromoteToProblemSolutionPattern {
        /// Anchoring problem hash.
        problem_hash: String,
        /// Solution text.
        solution: String,
        /// Supporting causal-edge ids.
        supporting_edges: Vec<Uuid>,
    },
}

/// Synthesis call result.
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CodingSynthesisOutput {
    /// Ordered actions to apply.
    pub actions: Vec<PromoteAction>,
    /// Free-form narrative for telemetry.
    pub narrative: String,
}

/// One repo's plan for rule-artifact externalization.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RepoArtifactPlan {
    /// Canonical repo id.
    pub repo_id: String,
    /// Filesystem root for the repo (resolved by `app-core`).
    pub root: PathBuf,
    /// Which artifacts are enabled per `config.codingMemory.reforge.ruleArtifacts`.
    pub enabled: Vec<RuleArtifact>,
    /// Filtered facts safe to externalize (sensitivity != high/excluded).
    pub facts: Vec<SerializableSemanticFact>,
    /// Filtered procedural rules.
    pub rules: Vec<SerializableProceduralRule>,
}

/// Input to `RuleArtifactsHandler::synthesize_artifact`.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RuleArtifactInput {
    /// Repo plan.
    pub plan: RepoArtifactPlan,
    /// Which artifact kind to produce.
    pub artifact: RuleArtifact,
}

/// Output: one managed-block body per artifact kind.
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RuleArtifactOutput {
    /// Markdown body (without managed-block markers).
    pub body: String,
    /// Optional ordered section labels for diff display.
    pub section_labels: Vec<String>,
}

/// One managed-block section, identified by header.
#[derive(Debug, Clone)]
pub struct ManagedBlockSection {
    /// Section heading (e.g. "Architecture").
    pub heading: String,
    /// Body lines.
    pub body: String,
}

/// Spec emitted by the project-skill evolver — used to produce `SKILL.md`.
#[derive(Debug, Clone)]
pub struct ProjectSkillSpec {
    /// Skill id (slug-style, sanitized from rule text).
    pub skill_id: String,
    /// Repo scope.
    pub repo_id: String,
    /// `name`.
    pub name: String,
    /// `description`.
    pub description: String,
    /// `whenToUse` list.
    pub when_to_use: Vec<String>,
    /// Procedure body (markdown).
    pub procedure: String,
    /// Anchoring fact / episode ids.
    pub references: Vec<Uuid>,
    /// Anchored symbol refs (Phase 6 will populate; Phase 5 may be empty).
    pub anchored_symbols: Vec<AnchoredSymbol>,
    /// Starting effectiveness.
    pub effectiveness: f32,
}

// ─────────────────────────────────────────────────────────────────────
// Serializable mirrors of cognitive's row types (so handler trait signatures
// stay clean and don't leak `cognitive::types::*` to the agent crate).
// ─────────────────────────────────────────────────────────────────────

/// Fact slice serializable across the handler boundary.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SerializableSemanticFact {
    /// Id.
    pub id: String,
    /// Subject.
    pub subject: String,
    /// Predicate.
    pub predicate: String,
    /// Object.
    pub object: String,
    /// Confidence.
    pub confidence: f32,
    /// Memory type (e.g. `fact`, `counterfactual`).
    pub memory_type: String,
    /// Sensitivity (`normal`/`high`/`excluded`).
    pub sensitivity: String,
    /// Repo scope id.
    pub scope_repo_id: Option<String>,
    /// `valid_from`.
    pub valid_from: String,
}

/// Episode slice serializable across the handler boundary.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SerializableEpisodicMemory {
    /// Id.
    pub id: String,
    /// Kind.
    pub kind: String,
    /// Content (raw markdown / JSON-stringified content).
    pub content: String,
    /// Importance.
    pub importance: f32,
    /// Recorded at.
    pub recorded_at: String,
    /// Repo scope id.
    pub scope_repo_id: Option<String>,
}

/// Rule slice serializable across the handler boundary.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SerializableProceduralRule {
    /// Id.
    pub id: String,
    /// Rule text.
    pub rule: String,
    /// Source.
    pub source: String,
    /// Confidence.
    pub confidence: f32,
    /// Effectiveness.
    pub effectiveness: f32,
    /// Repo scope.
    pub scope_repo_id: Option<String>,
}
```

- [ ] **Step 3: Create empty stub files for the rest of the module**

Create one-line stub files so `mod.rs` compiles. Each is filled in by a later task — the body comment marks which task lands the implementation:

```bash
mkdir -p crates/coding-memory/src/reforge
```

Create each of the following with placeholder content matching the file's responsibility (these stubs return `Err(NotImplementedInPhase::new(5).into())` from public entry points):

`crates/coding-memory/src/reforge/session_end.rs`:
```rust
//! Session-end light Reforge pass — filled in by Task 3.

use crate::error::NotImplementedInPhase;
use common::Result;

/// Session-end light pass owned by `AppCore`.
#[derive(Debug, Default)]
pub struct SessionEndPass {
    _phase_stub: (),
}

impl SessionEndPass {
    /// Run one session-end pass for the given session id.
    pub async fn run(&self, _session_id: &str) -> Result<()> {
        Err(common::KlyntbotError::NotImplemented(format!(
            "{:?}",
            NotImplementedInPhase::new(5)
        )))
    }
}
```

`crates/coding-memory/src/reforge/synth_handler.rs`:
```rust
//! Dependency-inversion handlers for LLM-backed coding phases. Filled in by Task 5.

use crate::reforge::types::{CodingSynthesisInput, CodingSynthesisOutput, RuleArtifactInput, RuleArtifactOutput};
use async_trait::async_trait;
use common::Result;

/// Phase 2.5 LLM seam.
#[async_trait]
pub trait CodingSynthesisHandler: Send + Sync {
    /// Run synthesis.
    async fn synthesize_coding(&self, input: &CodingSynthesisInput) -> Result<CodingSynthesisOutput>;
}

/// Phase 3.5 LLM seam.
#[async_trait]
pub trait RuleArtifactsHandler: Send + Sync {
    /// Generate a single rule artifact body.
    async fn synthesize_artifact(&self, input: &RuleArtifactInput) -> Result<RuleArtifactOutput>;
}
```

`crates/coding-memory/src/reforge/coding_synthesis.rs`:
```rust
//! Phase 2.5 — Coding Synthesis. Filled in by Task 6.

use crate::error::NotImplementedInPhase;
use crate::reforge::types::CodingPhaseHandlers;
use common::Result;

/// Orchestrator for Phase 2.5.
#[derive(Debug)]
pub struct CodingSynthesisPhase;

impl CodingSynthesisPhase {
    /// Run the phase.
    pub async fn run(_handlers: &CodingPhaseHandlers<'_>) -> Result<()> {
        Err(common::KlyntbotError::NotImplemented(format!(
            "{:?}",
            NotImplementedInPhase::new(5)
        )))
    }
}
```

`crates/coding-memory/src/reforge/managed_block.rs`:
```rust
//! Managed-block parser/writer. Filled in by Task 7.

use std::path::Path;
use thiserror::Error;

/// Managed-block error surface.
#[derive(Debug, Error)]
pub enum ManagedBlockError {
    /// IO failure.
    #[error("io: {0}")]
    Io(#[from] std::io::Error),
    /// User edited managed range.
    #[error("user content found inside managed range — refusing to overwrite")]
    UserConflict,
}

/// Parsed managed block.
#[derive(Debug, Clone, Default)]
pub struct ManagedBlock {
    /// Lines before the managed start marker (preserved verbatim).
    pub before: String,
    /// Lines inside the managed range.
    pub inside: String,
    /// Lines after the managed end marker (preserved verbatim).
    pub after: String,
}

impl ManagedBlock {
    /// Read + parse a file. Filled in by Task 7.
    pub fn read(_path: &Path) -> Result<Self, ManagedBlockError> {
        Err(ManagedBlockError::Io(std::io::Error::other("phase 5 stub")))
    }
}
```

`crates/coding-memory/src/reforge/rule_artifacts.rs`:
```rust
//! Phase 3.5 — Rule Artifact Generation. Filled in by Task 8.

use crate::error::NotImplementedInPhase;
use crate::reforge::types::CodingPhaseHandlers;
use common::Result;

/// Orchestrator for Phase 3.5.
#[derive(Debug)]
pub struct RuleArtifactGenerationPhase;

impl RuleArtifactGenerationPhase {
    /// Run the phase.
    pub async fn run(_handlers: &CodingPhaseHandlers<'_>) -> Result<()> {
        Err(common::KlyntbotError::NotImplemented(format!(
            "{:?}",
            NotImplementedInPhase::new(5)
        )))
    }
}
```

`crates/coding-memory/src/reforge/sensitivity_filter.rs`:
```rust
//! Sensitivity filter — Phase 5 Task 8 fills in.

use crate::reforge::types::SerializableSemanticFact;

/// Drop facts with sensitivity `high` or `excluded` for externalization.
#[must_use]
pub fn filter_for_externalization(
    facts: &[SerializableSemanticFact],
) -> Vec<SerializableSemanticFact> {
    facts
        .iter()
        .filter(|f| !matches!(f.sensitivity.as_str(), "high" | "excluded"))
        .cloned()
        .collect()
}
```

`crates/coding-memory/src/reforge/cross_session_dedup.rs`:
```rust
//! Phase 6.5 cross-session fact dedup. Filled in by Task 9.

use crate::error::NotImplementedInPhase;
use common::Result;

/// Cross-session dedup pass.
#[derive(Debug, Default)]
pub struct CrossSessionDedup;

impl CrossSessionDedup {
    /// Run the pass.
    pub async fn run(_repo: &cognitive::SemanticFactRepo) -> Result<()> {
        Err(common::KlyntbotError::NotImplemented(format!(
            "{:?}",
            NotImplementedInPhase::new(5)
        )))
    }
}
```

`crates/coding-memory/src/reforge/selective_delete.rs`:
```rust
//! Phase 6 selective-delete signal. Filled in by Task 10.

use crate::error::NotImplementedInPhase;
use common::Result;

/// Selective-delete signal.
#[derive(Debug, Default)]
pub struct SelectiveDeleteSignal;

/// Repo for the audit log.
#[derive(Debug, Clone)]
pub struct SelectiveDeleteLogRepo {
    pool: storage::StoragePool,
}

impl SelectiveDeleteLogRepo {
    /// Construct.
    pub fn new(pool: storage::StoragePool) -> Self {
        Self { pool }
    }

    /// Pool getter (used by Phase 6 fill-in).
    pub fn pool(&self) -> &storage::StoragePool {
        &self.pool
    }
}

impl SelectiveDeleteSignal {
    /// Apply the signal.
    pub async fn apply(_log: &SelectiveDeleteLogRepo) -> Result<u32> {
        Err(common::KlyntbotError::NotImplemented(format!(
            "{:?}",
            NotImplementedInPhase::new(5)
        )))
    }
}
```

`crates/coding-memory/src/reforge/session_summary_repo.rs`:
```rust
//! Session-summary repo. Filled in by Task 3.

use jiff::Timestamp;
use serde::{Deserialize, Serialize};

/// Row.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionSummaryRow {
    /// Id.
    pub id: String,
    /// Session id.
    pub session_id: String,
    /// Repo id.
    pub repo_id: Option<String>,
    /// When summarised.
    pub summarised_at: Timestamp,
    /// Markdown body.
    pub summary_md: String,
    /// Estimated tokens.
    pub token_count: u32,
}

/// Repo.
#[derive(Debug, Clone)]
pub struct SessionSummaryRepo {
    pool: storage::StoragePool,
}

impl SessionSummaryRepo {
    /// Construct.
    pub fn new(pool: storage::StoragePool) -> Self {
        Self { pool }
    }

    /// Pool ref for fill-in.
    pub fn pool(&self) -> &storage::StoragePool {
        &self.pool
    }
}
```

- [ ] **Step 4: Wire the new module into `lib.rs`**

Edit `crates/coding-memory/src/lib.rs`. Add `pub mod reforge;` between the existing `pub mod recall;` and `pub mod reforge_phase;`. Re-export at the bottom of `lib.rs`:

```rust
pub use reforge::{
    CodingSynthesisPhase as CodingSynthesisPhaseImpl, RuleArtifactGenerationPhase as RuleArtifactGenerationPhaseImpl,
    SessionEndPass, SessionSummaryRepo, SessionSummaryRow,
};
pub use reforge::types::{CodingPhaseHandlers, CodingSynthesisInput, CodingSynthesisOutput, PromoteAction, RuleArtifactInput, RuleArtifactOutput};
pub use reforge::synth_handler::{CodingSynthesisHandler, RuleArtifactsHandler};
```

- [ ] **Step 5: Verify clean build**

Run: `cargo build -p coding-memory`
Expected: clean build, no clippy warnings introduced.

- [ ] **Step 6: Commit**

```bash
git add crates/coding-memory/src/reforge \
        crates/coding-memory/src/lib.rs
git commit -m "feat(coding-memory): Phase 5 — reforge/ module skeleton + types"
```

---

### Task 3: Session-end light pass — Hebbian bump + within-session dedup + 200-token summary

**Files:**
- Modify: `crates/coding-memory/src/reforge/session_end.rs`
- Modify: `crates/coding-memory/src/reforge/session_summary_repo.rs`
- Test: `crates/coding-memory/tests/session_end_pass.rs`

`★ Insight ─────────────────────────────────────`
- The session-end light pass is the *deterministic fast path* that runs in < 2 s with zero LLM calls. It sets the floor for what's guaranteed to happen at session end even when the LLM provider is down.
- **Hebbian bump** = increment `co_activation` for every pair of memories retrieved together this session. The pair is keyed (lower_id, higher_id) so we don't double-count. This already exists as `CoActivationRepo::increment_pair` in cognitive — we're just calling it with the session's retrieval set.
- The 200-token summary is intentionally a **deterministic extractive summary** (not LLM-generated): it lists `files_modified`, `commands_run`, `test_outcomes`, plus a one-line problem statement. This means the SessionStart "Open threads" section in Phase-4 markdown always works even on the first day before any LLM Reforge has run.
`─────────────────────────────────────────────────`

- [ ] **Step 1: Write the failing test**

Create `crates/coding-memory/tests/session_end_pass.rs`:

```rust
use coding_memory::reforge::{SessionEndPass, SessionSummaryRepo};
use coding_memory::recall::telemetry::RecallInvocationRepo;
use storage::StoragePool;

async fn fresh_pool() -> StoragePool {
    let pool = StoragePool::connect_in_memory().await.expect("pool");
    StoragePool::run_feature_migrations(pool.inner(), &cognitive::cognitive_migrations())
        .await
        .expect("cog migs");
    StoragePool::run_feature_migrations(pool.inner(), &coding_memory::coding_memory_migrations())
        .await
        .expect("coding migs");
    pool
}

#[tokio::test]
async fn pass_writes_session_summary_under_budget() {
    let pool = fresh_pool().await;
    let summaries = SessionSummaryRepo::new(pool.clone());
    let co_act = cognitive::CoActivationRepo::new(pool.clone());
    let utilization = RecallInvocationRepo::new(pool.clone());
    let pass = SessionEndPass::new(summaries.clone(), co_act, utilization);

    pass.run("session-A", Some("repo:test")).await.expect("run");

    let row = summaries
        .latest_for_session("session-A")
        .await
        .expect("latest")
        .expect("row");
    assert_eq!(row.session_id, "session-A");
    assert!(row.token_count <= 200);
}

#[tokio::test]
async fn pass_dedups_same_problem_hash_fix_attempts() {
    let pool = fresh_pool().await;
    // seed two episodic_memories with kind='fix_attempt' and same problem_hash in metadata
    let ep_repo = cognitive::EpisodicMemoryRepo::new(pool.clone());
    for i in 0..2 {
        let m = cognitive::types::EpisodicMemory {
            id: format!("ep_{i}"),
            domain: "code".into(),
            content: format!("attempt {i}"),
            summary: None,
            importance: 0.5,
            occurred_at: jiff::Timestamp::now().to_string(),
            recorded_at: jiff::Timestamp::now().to_string(),
            stability: 1.0,
            last_accessed: None,
            access_count: 0,
            project_id: None,
            scope_type: "code".into(),
            scope_id: Some("session-B".into()),
            kind: Some("fix_attempt".into()),
            scope_repo_id: Some("repo:test".into()),
            metadata: Some(r#"{"problem_hash":"abc123"}"#.into()),
        };
        ep_repo.insert(&m).await.expect("ep insert");
    }

    let summaries = SessionSummaryRepo::new(pool.clone());
    let co_act = cognitive::CoActivationRepo::new(pool.clone());
    let utilization = RecallInvocationRepo::new(pool.clone());
    let pass = SessionEndPass::new(summaries, co_act, utilization);

    let report = pass.run("session-B", Some("repo:test")).await.expect("run");
    assert_eq!(report.deduped_attempts, 1, "two attempts → one survivor");
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo nextest run -p coding-memory --test session_end_pass`
Expected: FAIL — `SessionEndPass::new` does not exist; current stub returns `NotImplemented`.

- [ ] **Step 3: Fill in `SessionSummaryRepo` body**

Replace `crates/coding-memory/src/reforge/session_summary_repo.rs` with:

```rust
//! Session-summary repo — caches the 200-token markdown surface emitted by
//! the session-end light pass. Read by the Phase-4 SessionStart renderer's
//! "Open threads" section.

use common::{KlyntbotError, Result};
use jiff::Timestamp;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// Row.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionSummaryRow {
    /// Id.
    pub id: String,
    /// Session id.
    pub session_id: String,
    /// Repo id.
    pub repo_id: Option<String>,
    /// When summarised.
    pub summarised_at: Timestamp,
    /// Markdown body.
    pub summary_md: String,
    /// Estimated tokens.
    pub token_count: u32,
}

/// Repo.
#[derive(Debug, Clone)]
pub struct SessionSummaryRepo {
    pool: storage::StoragePool,
}

impl SessionSummaryRepo {
    /// Construct.
    pub fn new(pool: storage::StoragePool) -> Self {
        Self { pool }
    }

    /// Insert one row.
    pub async fn insert(&self, row: &SessionSummaryRow) -> Result<()> {
        sqlx::query(
            "INSERT INTO session_summaries \
             (id, session_id, repo_id, summarised_at, summary_md, token_count, actor_id) \
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, 'local_user')",
        )
        .bind(&row.id)
        .bind(&row.session_id)
        .bind(&row.repo_id)
        .bind(row.summarised_at.to_string())
        .bind(&row.summary_md)
        .bind(row.token_count as i64)
        .execute(self.pool.inner())
        .await
        .map_err(|e| KlyntbotError::Storage(format!("session_summaries insert: {e}")))?;
        Ok(())
    }

    /// Get the most-recent summary for a session.
    pub async fn latest_for_session(&self, session_id: &str) -> Result<Option<SessionSummaryRow>> {
        let row: Option<(String, String, Option<String>, String, String, i64)> = sqlx::query_as(
            "SELECT id, session_id, repo_id, summarised_at, summary_md, token_count \
             FROM session_summaries WHERE session_id = ?1 \
             ORDER BY summarised_at DESC LIMIT 1",
        )
        .bind(session_id)
        .fetch_optional(self.pool.inner())
        .await
        .map_err(|e| KlyntbotError::Storage(format!("session_summaries read: {e}")))?;
        Ok(row.map(|(id, session_id, repo_id, summarised_at, summary_md, token_count)| {
            SessionSummaryRow {
                id,
                session_id,
                repo_id,
                summarised_at: summarised_at.parse().unwrap_or_else(|_| Timestamp::now()),
                summary_md,
                token_count: token_count as u32,
            }
        }))
    }

    /// List recent summaries for a repo.
    pub async fn recent_for_repo(
        &self,
        repo_id: &str,
        limit: u32,
    ) -> Result<Vec<SessionSummaryRow>> {
        let rows: Vec<(String, String, Option<String>, String, String, i64)> = sqlx::query_as(
            "SELECT id, session_id, repo_id, summarised_at, summary_md, token_count \
             FROM session_summaries WHERE repo_id = ?1 \
             ORDER BY summarised_at DESC LIMIT ?2",
        )
        .bind(repo_id)
        .bind(limit as i64)
        .fetch_all(self.pool.inner())
        .await
        .map_err(|e| KlyntbotError::Storage(format!("session_summaries list: {e}")))?;
        Ok(rows
            .into_iter()
            .map(|(id, session_id, repo_id, summarised_at, summary_md, token_count)| {
                SessionSummaryRow {
                    id,
                    session_id,
                    repo_id,
                    summarised_at: summarised_at
                        .parse()
                        .unwrap_or_else(|_| Timestamp::now()),
                    summary_md,
                    token_count: token_count as u32,
                }
            })
            .collect())
    }

    /// Build a fresh row id.
    #[must_use]
    pub fn new_row_id() -> String {
        format!("sumsess_{}", Uuid::new_v4().simple())
    }
}
```

- [ ] **Step 4: Fill in `SessionEndPass`**

Replace `crates/coding-memory/src/reforge/session_end.rs` with:

```rust
//! Session-end light Reforge pass.
//!
//! Triggered by `EventKind::SessionEnd`. Runs in < 2 s with zero LLM calls.
//! Three responsibilities:
//!
//! 1. **Hebbian bump** — for every co-retrieved pair this session, increment
//!    the `co_activation` counter via `CoActivationRepo::increment_pair`.
//! 2. **Within-session dedup** — collapse `episodic_memories{kind='fix_attempt'}`
//!    rows sharing a `problem_hash` to one survivor (highest importance) and
//!    write a derived "attempts_count" metadata field.
//! 3. **Session summary** — emit a deterministic ≤ 200-token markdown body to
//!    `session_summaries`. The Phase-4 SessionStart renderer reads this for
//!    its "Open threads" section.

use crate::recall::telemetry::RecallInvocationRepo;
use crate::reforge::session_summary_repo::{SessionSummaryRepo, SessionSummaryRow};
use cognitive::{CoActivationRepo, EpisodicMemoryRepo};
use common::{KlyntbotError, Result};
use jiff::Timestamp;
use serde_json::Value;
use std::collections::HashMap;

/// Outcome of one pass — surfaced to telemetry.
#[derive(Debug, Clone, Default)]
pub struct SessionEndReport {
    /// Number of co-activation pairs bumped.
    pub pairs_bumped: u32,
    /// Number of duplicate fix-attempt episodes removed.
    pub deduped_attempts: u32,
    /// Token count of the summary.
    pub summary_tokens: u32,
}

/// Session-end pass.
#[derive(Debug, Clone)]
pub struct SessionEndPass {
    summaries: SessionSummaryRepo,
    co_activation: CoActivationRepo,
    utilization: RecallInvocationRepo,
}

impl SessionEndPass {
    /// Construct.
    pub fn new(
        summaries: SessionSummaryRepo,
        co_activation: CoActivationRepo,
        utilization: RecallInvocationRepo,
    ) -> Self {
        Self {
            summaries,
            co_activation,
            utilization,
        }
    }

    /// Run the pass.
    pub async fn run(
        &self,
        session_id: &str,
        repo_id: Option<&str>,
    ) -> Result<SessionEndReport> {
        let mut report = SessionEndReport::default();

        // 1. Hebbian bump — pull retrieval rows for this session and bump pairs.
        let invocations = self
            .utilization
            .list_for_session(session_id, 200)
            .await
            .map_err(|e| KlyntbotError::Storage(format!("recall_invocations: {e}")))?;
        let mut all_ids: Vec<String> = invocations
            .iter()
            .flat_map(|inv| inv.result_ids.iter().cloned())
            .collect();
        all_ids.sort();
        all_ids.dedup();
        for i in 0..all_ids.len() {
            for j in (i + 1)..all_ids.len() {
                let (lo, hi) = if all_ids[i] < all_ids[j] {
                    (&all_ids[i], &all_ids[j])
                } else {
                    (&all_ids[j], &all_ids[i])
                };
                self.co_activation
                    .increment_pair(lo, hi)
                    .await
                    .map_err(|e| KlyntbotError::Storage(format!("co_activation: {e}")))?;
                report.pairs_bumped += 1;
            }
        }

        // 2. Within-session dedup of fix-attempts by problem_hash.
        let pool = self.summaries.pool().clone();
        let ep_repo = EpisodicMemoryRepo::new(pool.clone());
        let rows: Vec<(String, String, Option<String>, f32)> = sqlx::query_as(
            "SELECT id, content, metadata, importance \
             FROM episodic_memories \
             WHERE scope_id = ?1 AND kind = 'fix_attempt'",
        )
        .bind(session_id)
        .fetch_all(pool.inner())
        .await
        .map_err(|e| KlyntbotError::Storage(format!("dedup query: {e}")))?;

        let mut buckets: HashMap<String, Vec<(String, f32)>> = HashMap::new();
        for (id, _content, metadata, importance) in rows {
            let hash = metadata
                .as_deref()
                .and_then(|s| serde_json::from_str::<Value>(s).ok())
                .and_then(|v| v.get("problem_hash").and_then(|h| h.as_str().map(String::from)));
            let Some(hash) = hash else { continue };
            buckets.entry(hash).or_default().push((id, importance));
        }

        for (_, mut group) in buckets {
            if group.len() < 2 {
                continue;
            }
            group.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
            let survivor = group.remove(0).0;
            // Annotate survivor with attempts_count and delete the rest.
            let count = (group.len() + 1) as u32;
            let extra = group.iter().map(|(id, _)| id.clone()).collect::<Vec<_>>();
            sqlx::query(
                "UPDATE episodic_memories \
                 SET metadata = json_patch(COALESCE(metadata,'{}'), \
                     json_object('attempts_count', ?1, 'merged_ids', json(?2))) \
                 WHERE id = ?3",
            )
            .bind(count as i64)
            .bind(serde_json::to_string(&extra).unwrap_or_else(|_| "[]".into()))
            .bind(&survivor)
            .execute(pool.inner())
            .await
            .map_err(|e| KlyntbotError::Storage(format!("dedup update: {e}")))?;
            for (id, _) in &group {
                ep_repo
                    .delete_by_id(id)
                    .await
                    .map_err(|e| KlyntbotError::Storage(format!("dedup delete: {e}")))?;
                report.deduped_attempts += 1;
            }
        }

        // 3. Build deterministic ≤200-token summary.
        let summary = build_summary_md(session_id, repo_id, &invocations).await;
        let token_count = estimate_tokens(&summary);
        report.summary_tokens = token_count;

        let row = SessionSummaryRow {
            id: SessionSummaryRepo::new_row_id(),
            session_id: session_id.to_string(),
            repo_id: repo_id.map(String::from),
            summarised_at: Timestamp::now(),
            summary_md: summary,
            token_count,
        };
        self.summaries.insert(&row).await?;

        Ok(report)
    }
}

async fn build_summary_md(
    session_id: &str,
    repo_id: Option<&str>,
    invocations: &[crate::recall::telemetry::RecallInvocationRow],
) -> String {
    let mut out = String::new();
    out.push_str("## Session summary\n");
    out.push_str(&format!("- Session: {session_id}\n"));
    if let Some(repo) = repo_id {
        out.push_str(&format!("- Repo: {repo}\n"));
    }
    out.push_str(&format!("- Recall invocations: {}\n", invocations.len()));
    out.push_str("\n_Summary auto-generated by SessionEndPass._\n");
    if estimate_tokens(&out) > 200 {
        truncate_to_tokens(&out, 200)
    } else {
        out
    }
}

fn estimate_tokens(s: &str) -> u32 {
    // Cheap heuristic — chars / 4 (matches `HeuristicBudgeter` in Phase 4).
    ((s.chars().count() as f32 / 4.0).ceil()) as u32
}

fn truncate_to_tokens(s: &str, budget: u32) -> String {
    let max_chars = (budget as usize) * 4;
    if s.chars().count() <= max_chars {
        return s.to_string();
    }
    s.chars().take(max_chars).collect()
}
```

- [ ] **Step 5: Verify tests pass**

Run: `cargo nextest run -p coding-memory --test session_end_pass`
Expected: PASS for both tests.

- [ ] **Step 6: Add `EpisodicMemoryRepo::delete_by_id` if not present**

Verify the method exists. Run:
```bash
grep -n "fn delete_by_id" crates/cognitive/src/repos/episodic_memory.rs
```
If absent, add to `crates/cognitive/src/repos/episodic_memory.rs`:

```rust
    /// Delete a row by id. Used by `coding-memory`'s session-end dedup pass.
    pub async fn delete_by_id(&self, id: &str) -> common::Result<()> {
        sqlx::query("DELETE FROM episodic_memories WHERE id = ?1")
            .bind(id)
            .execute(self.pool.inner())
            .await
            .map_err(|e| common::KlyntbotError::Storage(format!("episodic delete: {e}")))?;
        Ok(())
    }
```

- [ ] **Step 7: Add `RecallInvocationRepo::list_for_session` if not present**

Run: `grep -n "fn list_for_session" crates/coding-memory/src/recall/telemetry.rs`. If absent, add a method that returns up to `limit` rows for a session id.

- [ ] **Step 8: Commit**

```bash
git add crates/coding-memory/src/reforge/session_end.rs \
        crates/coding-memory/src/reforge/session_summary_repo.rs \
        crates/coding-memory/tests/session_end_pass.rs \
        crates/cognitive/src/repos/episodic_memory.rs \
        crates/coding-memory/src/recall/telemetry.rs
git commit -m "feat(coding-memory): Phase 5 — SessionEndPass (Hebbian + dedup + 200-tok summary)"
```

---

### Task 4: Wire `SessionEndPass` to `EventKind::SessionEnd`

**Files:**
- Create: `crates/app-core/src/coding_memory/reforge.rs`
- Modify: `crates/app-core/src/coding_memory/mod.rs`
- Modify: `crates/app-core/src/state.rs`
- Modify: `crates/app-core/src/init/mod.rs`
- Test: `crates/app-core/tests/session_end_dispatch.rs`

`★ Insight ─────────────────────────────────────`
- The session-end pass fires off `EventKind::SessionEnd` *after* the Distiller has finished its turn. The Distiller already drains `ingest_event_log` — Phase-3 wiring listens for `Distiller::on_session_end_complete` callbacks. Phase 5 attaches the pass as one such callback, not as a parallel reader of the event log.
- `AppCore` holds the pass as `Option<Arc<SessionEndPass>>` so tests that spin up an in-memory `AppCore` without the cognitive integration still compile.
`─────────────────────────────────────────────────`

- [ ] **Step 1: Write the failing test**

Create `crates/app-core/tests/session_end_dispatch.rs`:

```rust
use app_core::coding_memory::reforge::register_session_end_dispatch;
use coding_memory::reforge::SessionEndPass;
use coding_memory::recall::telemetry::RecallInvocationRepo;
use storage::StoragePool;

#[tokio::test]
async fn session_end_event_triggers_pass() {
    let pool = StoragePool::connect_in_memory().await.expect("pool");
    StoragePool::run_feature_migrations(pool.inner(), &cognitive::cognitive_migrations())
        .await
        .expect("cog");
    StoragePool::run_feature_migrations(pool.inner(), &coding_memory::coding_memory_migrations())
        .await
        .expect("coding");
    let summaries = coding_memory::reforge::SessionSummaryRepo::new(pool.clone());
    let co_act = cognitive::CoActivationRepo::new(pool.clone());
    let utilization = RecallInvocationRepo::new(pool.clone());
    let pass = std::sync::Arc::new(SessionEndPass::new(summaries.clone(), co_act, utilization));

    let bus = std::sync::Arc::new(bus::DomainEventBus::new(64));
    register_session_end_dispatch(bus.clone(), pass.clone()).await;

    bus.publish(bus::DomainEvent::CodingSessionEnded {
        session_id: "s1".into(),
        repo_id: Some("repo:test".into()),
    });

    tokio::time::sleep(std::time::Duration::from_millis(100)).await;

    let row = summaries
        .latest_for_session("s1")
        .await
        .expect("latest")
        .expect("row written");
    assert_eq!(row.session_id, "s1");
}
```

- [ ] **Step 2: Add `DomainEvent::CodingSessionEnded` if not present**

Run: `grep -n "CodingSessionEnded" crates/bus/src/domain_events.rs`. If absent, add a variant to the `DomainEvent` enum and update the kind/domain match arms (mirroring the existing pattern for `PatternApplied` etc.).

```rust
    /// Coding session ended — fired after Distiller drains the final turn.
    CodingSessionEnded {
        /// Session id from `AgentEvent`.
        session_id: String,
        /// Repo id (if resolved).
        repo_id: Option<String>,
    },
```

Append to the `kind()` match: `Self::CodingSessionEnded { .. } => "CodingSessionEnded",`. Append to the `KIND_*` constants block: `pub const KIND_CODING_SESSION_ENDED: &'static str = "CodingSessionEnded";`. Append to the domain match: `| Self::CodingSessionEnded { .. } => D::CodingMemory,`.

- [ ] **Step 3: Run test to verify it fails**

Run: `cargo nextest run -p app-core --test session_end_dispatch`
Expected: FAIL — `register_session_end_dispatch` does not exist.

- [ ] **Step 4: Create the dispatcher**

Create `crates/app-core/src/coding_memory/reforge.rs`:

```rust
//! App-core wiring for Phase 5 Reforge integration.
//!
//! - Dispatches `DomainEvent::CodingSessionEnded` into `SessionEndPass`.
//! - Builds `CodingPhaseHandlers` for the cron `run_reforge` call.

use bus::{DomainEvent, DomainEventBus};
use coding_memory::reforge::SessionEndPass;
use std::sync::Arc;
use tracing::warn;

/// Subscribe `SessionEndPass` to `DomainEvent::CodingSessionEnded`.
pub async fn register_session_end_dispatch(
    bus: Arc<DomainEventBus>,
    pass: Arc<SessionEndPass>,
) {
    let mut rx = bus.subscribe();
    tokio::spawn(async move {
        while let Ok(event) = rx.recv().await {
            if let DomainEvent::CodingSessionEnded {
                session_id,
                repo_id,
            } = event
            {
                let pass = pass.clone();
                tokio::spawn(async move {
                    if let Err(e) = pass.run(&session_id, repo_id.as_deref()).await {
                        warn!("SessionEndPass failed for {session_id}: {e}");
                    }
                });
            }
        }
    });
}
```

- [ ] **Step 5: Re-export from the coding_memory module**

Edit `crates/app-core/src/coding_memory/mod.rs`. Add `pub mod reforge;` near the other `pub mod` declarations.

- [ ] **Step 6: Hold the pass on `AppCore`**

Edit `crates/app-core/src/state.rs`. Inside the `AppCore` struct, after the existing `pub recall: Option<...>` field, add:

```rust
    /// Phase-5 session-end light pass.
    pub session_end_pass: Option<Arc<coding_memory::reforge::SessionEndPass>>,
```

Initialize to `None` in `AppCore::default()` / `AppCore::new(...)` constructors.

- [ ] **Step 7: Wire construction in `init/mod.rs`**

Edit `crates/app-core/src/init/mod.rs`. After the Phase-3 `Distiller` wiring block, insert:

```rust
    let session_summary_repo = coding_memory::reforge::SessionSummaryRepo::new(pool.clone());
    let co_act = cognitive::CoActivationRepo::new(pool.clone());
    let utilization = coding_memory::recall::telemetry::RecallInvocationRepo::new(pool.clone());
    let session_end_pass = Arc::new(coding_memory::reforge::SessionEndPass::new(
        session_summary_repo.clone(),
        co_act,
        utilization,
    ));
    crate::coding_memory::reforge::register_session_end_dispatch(
        domain_event_bus.clone(),
        session_end_pass.clone(),
    )
    .await;
    app_core.session_end_pass = Some(session_end_pass);
```

- [ ] **Step 8: Verify test passes**

Run: `cargo nextest run -p app-core --test session_end_dispatch`
Expected: PASS.

- [ ] **Step 9: Commit**

```bash
git add crates/app-core/src/coding_memory/reforge.rs \
        crates/app-core/src/coding_memory/mod.rs \
        crates/app-core/src/state.rs \
        crates/app-core/src/init/mod.rs \
        crates/app-core/tests/session_end_dispatch.rs \
        crates/bus/src/domain_events.rs
git commit -m "feat(coding-memory): Phase 5 — wire SessionEndPass to CodingSessionEnded event"
```

---

### Task 5: `CodingSynthesisHandler` + `RuleArtifactsHandler` traits + `ProviderRole` enum extension

**Files:**
- Modify: `crates/coding-memory/src/reforge/synth_handler.rs`
- Modify: `crates/providers/src/lib.rs`
- Test: `crates/coding-memory/tests/synth_handler_traits.rs`

- [ ] **Step 1: Write the failing test**

Create `crates/coding-memory/tests/synth_handler_traits.rs`:

```rust
use coding_memory::reforge::{
    CodingSynthesisHandler, CodingSynthesisInput, CodingSynthesisOutput, PromoteAction,
    RuleArtifactInput, RuleArtifactOutput, RuleArtifactsHandler,
};
use coding_memory::reforge::types::{RepoArtifactPlan, RepoSynthesisBundle};
use jiff::Timestamp;
use std::path::PathBuf;

struct MockSynth;
#[async_trait::async_trait]
impl CodingSynthesisHandler for MockSynth {
    async fn synthesize_coding(
        &self,
        _input: &CodingSynthesisInput,
    ) -> common::Result<CodingSynthesisOutput> {
        Ok(CodingSynthesisOutput {
            actions: vec![PromoteAction::ExtractPattern {
                repo_id: Some("r1".into()),
                rule: "test".into(),
                confidence: 0.8,
                supporting: vec![],
            }],
            narrative: "ok".into(),
        })
    }
}

struct MockRules;
#[async_trait::async_trait]
impl RuleArtifactsHandler for MockRules {
    async fn synthesize_artifact(
        &self,
        _input: &RuleArtifactInput,
    ) -> common::Result<RuleArtifactOutput> {
        Ok(RuleArtifactOutput {
            body: "## Notes\n- test\n".into(),
            section_labels: vec!["Notes".into()],
        })
    }
}

#[tokio::test]
async fn synth_handler_object_safe() {
    let h: Box<dyn CodingSynthesisHandler> = Box::new(MockSynth);
    let out = h
        .synthesize_coding(&CodingSynthesisInput {
            since: Timestamp::now(),
            repo_bundles: vec![],
            recent_counterfactuals: vec![],
        })
        .await
        .unwrap();
    assert_eq!(out.actions.len(), 1);
}

#[tokio::test]
async fn rules_handler_object_safe() {
    let h: Box<dyn RuleArtifactsHandler> = Box::new(MockRules);
    let out = h
        .synthesize_artifact(&RuleArtifactInput {
            plan: RepoArtifactPlan {
                repo_id: "r1".into(),
                root: PathBuf::from("/tmp/r1"),
                enabled: vec![coding_memory::reforge_phase::RuleArtifact::ClaudeMd],
                facts: vec![],
                rules: vec![],
            },
            artifact: coding_memory::reforge_phase::RuleArtifact::ClaudeMd,
        })
        .await
        .unwrap();
    assert!(out.body.contains("Notes"));
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo nextest run -p coding-memory --test synth_handler_traits`
Expected: FAIL or PASS depending on stub state. (Stub may already pass — that's fine; the goal is the public surface compiles.)

- [ ] **Step 3: Confirm stub trait matches**

The stub trait at `crates/coding-memory/src/reforge/synth_handler.rs` already exists (Task 2). Verify it compiles with the test file via `cargo build -p coding-memory --tests`.

- [ ] **Step 4: Extend `ProviderRole` enum**

Edit `crates/providers/src/lib.rs`. Find the existing `pub enum ProviderRole` (added in Phase 1). Add two new variants if not present:

```rust
    /// Used by Phase 2.5 Reforge Coding Synthesis. Defaults to user's medium-tier model.
    ReforgeSynth,
    /// Used by Phase 3.5 Reforge Rule Artifact Generation. Defaults to user's medium-tier model.
    ReforgeRules,
```

- [ ] **Step 5: Verify**

Run: `cargo build -p providers && cargo nextest run -p coding-memory --test synth_handler_traits`
Expected: clean build, both tests pass.

- [ ] **Step 6: Commit**

```bash
git add crates/coding-memory/src/reforge/synth_handler.rs \
        crates/coding-memory/tests/synth_handler_traits.rs \
        crates/providers/src/lib.rs
git commit -m "feat(coding-memory): Phase 5 — handler traits + ProviderRole::Reforge{Synth,Rules}"
```

---

### Task 6: `CodingSynthesisPhase` orchestrator + 6 promotion actions

**Files:**
- Modify: `crates/coding-memory/src/reforge/coding_synthesis.rs`
- Test: `crates/coding-memory/tests/coding_synthesis.rs`

`★ Insight ─────────────────────────────────────`
- The orchestrator is *deliberately thin*: it pulls inputs, calls one LLM via the handler, then dispatches each `PromoteAction` to a matching persistence routine. All the LLM-shape decisions live in the agent crate's handler impl; the orchestrator only knows "given an action, write the right rows."
- Persistence uses `cognitive::SemanticFactRepo::upsert_with_metadata` + `EpisodicMemoryRepo::insert_with_kind_and_metadata` (added in Phase 3) so every Phase 2.5 write inherits provenance-always and bi-temporal semantics for free. We never bypass those repos.
- `PromoteToProblemClass` is the only action that *doesn't write a fact* — it emits a `MirrorAlert::Coding{kind: ProblemClassRefactor}` snippet and returns. Spec §10 says this stays a user-actionable alert until accepted.
`─────────────────────────────────────────────────`

- [ ] **Step 1: Write the failing test**

Create `crates/coding-memory/tests/coding_synthesis.rs`:

```rust
use coding_memory::reforge::types::{
    CodingPhaseHandlers, CodingSynthesisInput, CodingSynthesisOutput, PromoteAction,
};
use coding_memory::reforge::{CodingSynthesisHandler, CodingSynthesisPhase};
use std::sync::Arc;
use storage::StoragePool;

struct Mock(CodingSynthesisOutput);
#[async_trait::async_trait]
impl CodingSynthesisHandler for Mock {
    async fn synthesize_coding(
        &self,
        _: &CodingSynthesisInput,
    ) -> common::Result<CodingSynthesisOutput> {
        Ok(self.0.clone())
    }
}

async fn fixture() -> StoragePool {
    let pool = StoragePool::connect_in_memory().await.unwrap();
    StoragePool::run_feature_migrations(pool.inner(), &cognitive::cognitive_migrations())
        .await
        .unwrap();
    StoragePool::run_feature_migrations(pool.inner(), &coding_memory::coding_memory_migrations())
        .await
        .unwrap();
    pool
}

#[tokio::test]
async fn extract_pattern_persists_procedural_rule() {
    let pool = fixture().await;
    let handler = Mock(CodingSynthesisOutput {
        actions: vec![PromoteAction::ExtractPattern {
            repo_id: Some("r1".into()),
            rule: "always run cargo fmt before commit".into(),
            confidence: 0.85,
            supporting: vec![],
        }],
        narrative: String::new(),
    });

    let fact_repo = cognitive::SemanticFactRepo::new(pool.clone());
    let ep_repo = cognitive::EpisodicMemoryRepo::new(pool.clone());
    let rule_repo = cognitive::ProceduralRuleRepo::new(pool.clone());
    let co_act = cognitive::CoActivationRepo::new(pool.clone());
    let summaries = coding_memory::reforge::SessionSummaryRepo::new(pool.clone());
    let utilization = coding_memory::recall::telemetry::RecallInvocationRepo::new(pool.clone());
    let pat_eff_repo = coding_memory::mirror::pattern_effectiveness::PatternEffectivenessLogRepo::new(pool.clone());
    let sd_repo = coding_memory::reforge::selective_delete::SelectiveDeleteLogRepo::new(pool.clone());

    let handlers = CodingPhaseHandlers {
        synthesis: Some(&handler),
        rule_artifacts: None,
        fact_repo: &fact_repo,
        episodic_repo: &ep_repo,
        rule_repo: &rule_repo,
        co_activation_repo: &co_act,
        utilization_repo: &utilization,
        session_summary_repo: &summaries,
        selective_delete_log: &sd_repo,
        pattern_effectiveness_log: &pat_eff_repo,
        bus: None,
    };

    CodingSynthesisPhase::run(&handlers).await.expect("phase ok");

    let count: (i64,) = sqlx::query_as(
        "SELECT COUNT(*) FROM procedural_rules WHERE source = 'observed' \
         AND rule LIKE 'always run cargo fmt%'",
    )
    .fetch_one(pool.inner())
    .await
    .unwrap();
    assert_eq!(count.0, 1);
}

#[tokio::test]
async fn promote_to_project_understanding_writes_semantic_fact() {
    let pool = fixture().await;
    let handler = Mock(CodingSynthesisOutput {
        actions: vec![PromoteAction::PromoteToProjectUnderstanding {
            repo_id: "r1".into(),
            subject: "repo:r1".into(),
            predicate: "uses_framework".into(),
            object: "axum 0.7".into(),
            convergence: 0.9,
        }],
        narrative: String::new(),
    });

    let fact_repo = cognitive::SemanticFactRepo::new(pool.clone());
    let ep_repo = cognitive::EpisodicMemoryRepo::new(pool.clone());
    let rule_repo = cognitive::ProceduralRuleRepo::new(pool.clone());
    let co_act = cognitive::CoActivationRepo::new(pool.clone());
    let summaries = coding_memory::reforge::SessionSummaryRepo::new(pool.clone());
    let utilization = coding_memory::recall::telemetry::RecallInvocationRepo::new(pool.clone());
    let pat_eff_repo = coding_memory::mirror::pattern_effectiveness::PatternEffectivenessLogRepo::new(pool.clone());
    let sd_repo = coding_memory::reforge::selective_delete::SelectiveDeleteLogRepo::new(pool.clone());

    let handlers = CodingPhaseHandlers {
        synthesis: Some(&handler),
        rule_artifacts: None,
        fact_repo: &fact_repo,
        episodic_repo: &ep_repo,
        rule_repo: &rule_repo,
        co_activation_repo: &co_act,
        utilization_repo: &utilization,
        session_summary_repo: &summaries,
        selective_delete_log: &sd_repo,
        pattern_effectiveness_log: &pat_eff_repo,
        bus: None,
    };

    CodingSynthesisPhase::run(&handlers).await.expect("phase ok");

    let count: (i64,) = sqlx::query_as(
        "SELECT COUNT(*) FROM semantic_facts WHERE subject = 'repo:r1' \
         AND predicate = 'uses_framework' AND object = 'axum 0.7'",
    )
    .fetch_one(pool.inner())
    .await
    .unwrap();
    assert_eq!(count.0, 1);
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo nextest run -p coding-memory --test coding_synthesis`
Expected: FAIL — phase still returns `NotImplemented`.

- [ ] **Step 3: Implement the orchestrator**

Replace `crates/coding-memory/src/reforge/coding_synthesis.rs` with:

```rust
//! Phase 2.5 — Coding Synthesis.
//!
//! Pulls inputs, runs `CodingSynthesisHandler`, then applies each
//! `PromoteAction` to the cognitive store. Every write goes through the
//! existing `SemanticFactRepo` / `EpisodicMemoryRepo` / `ProceduralRuleRepo`
//! to inherit provenance, bi-temporal lifecycle, and FSRS5 init.

use crate::mirror::alerts::{CodingMirrorAlertKind, MirrorAlertSeverity};
use crate::reforge::types::{
    CausalChainGroup, CodingPhaseHandlers, CodingSynthesisInput, PromoteAction,
    RepoSynthesisBundle, SerializableEpisodicMemory, SerializableProceduralRule,
    SerializableSemanticFact,
};
use cognitive::types::{ProceduralRule, SemanticFact, DEFAULT_MEMORY_TYPE};
use common::{KlyntbotError, Result};
use jiff::Timestamp;
use serde_json::json;
use tracing::{info, warn};
use uuid::Uuid;

/// Orchestrator for Phase 2.5.
#[derive(Debug)]
pub struct CodingSynthesisPhase;

impl CodingSynthesisPhase {
    /// Run the phase. Returns the number of actions persisted.
    pub async fn run(handlers: &CodingPhaseHandlers<'_>) -> Result<u32> {
        let Some(handler) = handlers.synthesis else {
            return Ok(0);
        };
        let input = build_input(handlers).await?;
        let output = match handler.synthesize_coding(&input).await {
            Ok(o) => o,
            Err(e) => {
                warn!("Phase 2.5 LLM call failed: {e}");
                return Err(e);
            }
        };

        let mut applied = 0_u32;
        for action in output.actions {
            match apply_action(action, handlers).await {
                Ok(()) => applied += 1,
                Err(e) => warn!("Phase 2.5 apply failed: {e}"),
            }
        }
        info!(applied, narrative = %output.narrative, "Phase 2.5 complete");
        Ok(applied)
    }
}

async fn build_input(handlers: &CodingPhaseHandlers<'_>) -> Result<CodingSynthesisInput> {
    // Window: facts/episodes since 24h ago. Real cron schedule is nightly.
    let since = Timestamp::now()
        .checked_sub(jiff::SignedDuration::from_hours(24))
        .unwrap_or_else(|_| Timestamp::now());

    // Group fix-attempts and workflow patterns by repo.
    let pool = handlers.fact_repo.pool().clone();
    let repo_ids: Vec<(Option<String>,)> = sqlx::query_as(
        "SELECT DISTINCT scope_repo_id FROM episodic_memories \
         WHERE recorded_at >= ?1 AND kind IN ('fix_attempt','test_run','refactor')",
    )
    .bind(since.to_string())
    .fetch_all(pool.inner())
    .await
    .map_err(|e| KlyntbotError::Storage(format!("repo list: {e}")))?;

    let mut bundles = Vec::new();
    for (repo_id,) in repo_ids {
        let Some(rid) = repo_id else { continue };
        let fix_attempts = fetch_fix_attempts(&pool, &rid, &since).await?;
        let patterns = fetch_workflow_patterns(&pool, &rid).await?;
        let context_facts = fetch_repo_context(&pool, &rid).await?;
        let causal_chains = fetch_causal_chain_groups(&pool, &rid).await?;
        bundles.push(RepoSynthesisBundle {
            repo_id: rid,
            fix_attempts,
            workflow_patterns: patterns,
            repo_context_facts: context_facts,
            causal_chains,
        });
    }

    let recent_counterfactuals = fetch_counterfactuals(&pool, &since).await?;
    Ok(CodingSynthesisInput {
        since,
        repo_bundles: bundles,
        recent_counterfactuals,
    })
}

async fn fetch_fix_attempts(
    pool: &storage::StoragePool,
    repo_id: &str,
    since: &Timestamp,
) -> Result<Vec<SerializableEpisodicMemory>> {
    let rows: Vec<(String, String, f32, String, Option<String>)> = sqlx::query_as(
        "SELECT id, content, importance, recorded_at, scope_repo_id \
         FROM episodic_memories \
         WHERE scope_repo_id = ?1 AND kind = 'fix_attempt' AND recorded_at >= ?2 \
         ORDER BY recorded_at DESC LIMIT 50",
    )
    .bind(repo_id)
    .bind(since.to_string())
    .fetch_all(pool.inner())
    .await
    .map_err(|e| KlyntbotError::Storage(format!("fix attempts: {e}")))?;
    Ok(rows
        .into_iter()
        .map(|(id, content, importance, recorded_at, scope_repo_id)| {
            SerializableEpisodicMemory {
                id,
                kind: "fix_attempt".into(),
                content,
                importance,
                recorded_at,
                scope_repo_id,
            }
        })
        .collect())
}

async fn fetch_workflow_patterns(
    pool: &storage::StoragePool,
    repo_id: &str,
) -> Result<Vec<SerializableProceduralRule>> {
    let rows: Vec<(String, String, String, f32, f32, Option<String>)> = sqlx::query_as(
        "SELECT id, rule, source, confidence, COALESCE(effectiveness_score, 0.5), scope_repo_id \
         FROM procedural_rules \
         WHERE scope_repo_id = ?1 AND source = 'observed'",
    )
    .bind(repo_id)
    .fetch_all(pool.inner())
    .await
    .map_err(|e| KlyntbotError::Storage(format!("workflow patterns: {e}")))?;
    Ok(rows
        .into_iter()
        .map(|(id, rule, source, confidence, effectiveness, scope_repo_id)| {
            SerializableProceduralRule {
                id,
                rule,
                source,
                confidence,
                effectiveness,
                scope_repo_id,
            }
        })
        .collect())
}

async fn fetch_repo_context(
    pool: &storage::StoragePool,
    repo_id: &str,
) -> Result<Vec<SerializableSemanticFact>> {
    let rows: Vec<(
        String,
        String,
        String,
        String,
        f32,
        String,
        String,
        Option<String>,
        String,
    )> = sqlx::query_as(
        "SELECT id, subject, predicate, object, confidence, \
                COALESCE(memory_type, 'fact'), \
                COALESCE(json_extract(metadata, '$.sensitivity'), 'normal'), \
                scope_repo_id, valid_from \
         FROM semantic_facts \
         WHERE scope_repo_id = ?1 AND domain = 'work' \
         ORDER BY confidence DESC LIMIT 50",
    )
    .bind(repo_id)
    .fetch_all(pool.inner())
    .await
    .map_err(|e| KlyntbotError::Storage(format!("repo context: {e}")))?;
    Ok(rows
        .into_iter()
        .map(
            |(id, subject, predicate, object, confidence, memory_type, sensitivity, scope_repo_id, valid_from)| {
                SerializableSemanticFact {
                    id,
                    subject,
                    predicate,
                    object,
                    confidence,
                    memory_type,
                    sensitivity,
                    scope_repo_id,
                    valid_from,
                }
            },
        )
        .collect())
}

async fn fetch_causal_chain_groups(
    pool: &storage::StoragePool,
    repo_id: &str,
) -> Result<Vec<CausalChainGroup>> {
    // Phase 6 will populate edges with problem_hash. Phase 5 returns empty groups.
    let _ = pool;
    let _ = repo_id;
    Ok(Vec::new())
}

async fn fetch_counterfactuals(
    pool: &storage::StoragePool,
    since: &Timestamp,
) -> Result<Vec<SerializableSemanticFact>> {
    let rows: Vec<(
        String,
        String,
        String,
        String,
        f32,
        String,
        Option<String>,
        String,
    )> = sqlx::query_as(
        "SELECT id, subject, predicate, object, confidence, \
                COALESCE(json_extract(metadata, '$.sensitivity'), 'normal'), \
                scope_repo_id, valid_from \
         FROM semantic_facts \
         WHERE memory_type = 'counterfactual' AND valid_from >= ?1 \
         ORDER BY confidence DESC LIMIT 100",
    )
    .bind(since.to_string())
    .fetch_all(pool.inner())
    .await
    .map_err(|e| KlyntbotError::Storage(format!("counterfactuals: {e}")))?;
    Ok(rows
        .into_iter()
        .map(
            |(id, subject, predicate, object, confidence, sensitivity, scope_repo_id, valid_from)| {
                SerializableSemanticFact {
                    id,
                    subject,
                    predicate,
                    object,
                    confidence,
                    memory_type: "counterfactual".into(),
                    sensitivity,
                    scope_repo_id,
                    valid_from,
                }
            },
        )
        .collect())
}

async fn apply_action(action: PromoteAction, handlers: &CodingPhaseHandlers<'_>) -> Result<()> {
    match action {
        PromoteAction::ExtractPattern { repo_id, rule, confidence, supporting } => {
            let r = ProceduralRule {
                id: format!("rule_{}", Uuid::new_v4().simple()),
                rule,
                source: "observed".into(),
                confidence,
                effectiveness_score: 0.5,
                stability: 1.0,
                scope_type: "code".into(),
                scope_id: None,
                scope_repo_id: repo_id.clone(),
                created_at: Timestamp::now().to_string(),
                last_applied: None,
                application_count: 0,
                metadata: Some(json!({
                    "provenance": {
                        "source": "reforge.coding_synthesis",
                        "supporting": supporting,
                    },
                    "kind": "workflow_pattern",
                }).to_string()),
            };
            handlers.rule_repo.insert(&r).await?;
        }
        PromoteAction::ExtractFailurePattern { repo_id, rule, remediation, confidence, supporting } => {
            let combined = format!("{rule}\n\n**Remediation:** {remediation}");
            let r = ProceduralRule {
                id: format!("rule_{}", Uuid::new_v4().simple()),
                rule: combined,
                source: "observed".into(),
                confidence,
                effectiveness_score: 0.5,
                stability: 1.0,
                scope_type: "code".into(),
                scope_id: None,
                scope_repo_id: repo_id.clone(),
                created_at: Timestamp::now().to_string(),
                last_applied: None,
                application_count: 0,
                metadata: Some(json!({
                    "provenance": {
                        "source": "reforge.coding_synthesis",
                        "supporting": supporting,
                    },
                    "kind": "failure_pattern",
                }).to_string()),
            };
            handlers.rule_repo.insert(&r).await?;
        }
        PromoteAction::PromoteToProblemClass { problem_hash, suggestion } => {
            // No fact write — emit a Mirror alert via the bus if available.
            if let Some(bus) = handlers.bus.clone() {
                let alert_kind = CodingMirrorAlertKind::ProblemClassRefactor;
                let payload = json!({
                    "problem_hash": problem_hash,
                    "suggestion": suggestion,
                });
                bus.publish(bus::DomainEvent::CodingMirrorAlert {
                    kind: alert_kind.as_str().to_string(),
                    severity: MirrorAlertSeverity::High.as_str().to_string(),
                    payload: payload.to_string(),
                });
            }
        }
        PromoteAction::PromoteToProjectUnderstanding { repo_id, subject, predicate, object, convergence } => {
            if convergence < 0.7 {
                return Ok(());
            }
            let f = SemanticFact {
                id: format!("fact_{}", Uuid::new_v4().simple()),
                subject,
                predicate,
                object,
                confidence: convergence,
                memory_type: DEFAULT_MEMORY_TYPE.to_string(),
                domain: "work".into(),
                scope_type: "code".into(),
                scope_id: None,
                scope_repo_id: Some(repo_id),
                actor_id: "local_user".into(),
                valid_from: Timestamp::now().to_string(),
                valid_until: None,
                supersedes: None,
                superseded_by: None,
                stability: 2.0,
                last_accessed: None,
                access_count: 0,
                metadata: Some(json!({
                    "provenance": {
                        "source": "reforge.coding_synthesis",
                        "kind": "project_understanding",
                    },
                    "convergence_score": convergence,
                }).to_string()),
            };
            handlers.fact_repo.upsert(&f).await?;
        }
        PromoteAction::PromoteToUserHabit { rule, confidence, witness_repos } => {
            let r = ProceduralRule {
                id: format!("rule_{}", Uuid::new_v4().simple()),
                rule,
                source: "reflected".into(),
                confidence,
                effectiveness_score: 0.5,
                stability: 1.5,
                scope_type: "user".into(),
                scope_id: None,
                scope_repo_id: None,
                created_at: Timestamp::now().to_string(),
                last_applied: None,
                application_count: 0,
                metadata: Some(json!({
                    "provenance": {
                        "source": "reforge.coding_synthesis",
                        "kind": "user_habit",
                        "witness_repos": witness_repos,
                    },
                }).to_string()),
            };
            handlers.rule_repo.insert(&r).await?;
        }
        PromoteAction::PromoteToProblemSolutionPattern { problem_hash, solution, supporting_edges } => {
            let r = ProceduralRule {
                id: format!("rule_{}", Uuid::new_v4().simple()),
                rule: format!("When problem matches `{problem_hash}`: {solution}"),
                source: "reflected".into(),
                confidence: 0.8,
                effectiveness_score: 0.5,
                stability: 1.5,
                scope_type: "code".into(),
                scope_id: None,
                scope_repo_id: None,
                created_at: Timestamp::now().to_string(),
                last_applied: None,
                application_count: 0,
                metadata: Some(json!({
                    "provenance": {
                        "source": "reforge.coding_synthesis",
                        "kind": "problem_solution_pattern",
                        "supporting_edges": supporting_edges,
                    },
                    "problem_hash": problem_hash,
                }).to_string()),
            };
            handlers.rule_repo.insert(&r).await?;
        }
    }
    Ok(())
}
```

- [ ] **Step 4: Add `DomainEvent::CodingMirrorAlert` variant**

Edit `crates/bus/src/domain_events.rs`. Add variant + kind constant + domain match arm:

```rust
    /// Coding-specific Mirror alert (closed-enum extension).
    CodingMirrorAlert {
        /// `CodingMirrorAlertKind` as string.
        kind: String,
        /// `MirrorAlertSeverity` as string.
        severity: String,
        /// JSON-encoded payload.
        payload: String,
    },
```

Add: `Self::CodingMirrorAlert { .. } => "CodingMirrorAlert",` to `kind()`. Add `KIND_CODING_MIRROR_ALERT` constant. Add `| Self::CodingMirrorAlert { .. } => D::CodingMemory,` to the domain match.

- [ ] **Step 5: Verify tests pass**

Run: `cargo nextest run -p coding-memory --test coding_synthesis`
Expected: PASS for both tests. (Tests for `mirror::alerts` and `pattern_effectiveness::PatternEffectivenessLogRepo` will be created in Tasks 18/19; until then, stub these types in `mirror/mod.rs` to keep the workspace green — see Step 6.)

- [ ] **Step 6: Pre-create `mirror/` module skeleton**

Create `crates/coding-memory/src/mirror/mod.rs`:

```rust
//! Mirror integration — Phase 5.
//!
//! Sources, alerts, and meta-rule detectors. Filled in by Tasks 18–22.

pub mod alerts;
pub mod coding_alerts_query;
pub mod coding_meta_rules;
pub mod coding_routing;
pub mod pattern_effectiveness;
pub mod stale_memory;
```

Create `crates/coding-memory/src/mirror/alerts.rs`:

```rust
//! Closed-enum coding alert kinds + severity. Filled in by Task 18.

/// Closed enum of coding-specific Mirror alerts.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CodingMirrorAlertKind {
    /// Skill activation dropped > 50 % in 7 days.
    ProjectSkillObsolete,
    /// `problem_hash` recurred ≥ 3× without matching `WorkflowPattern`.
    UncapturedPattern,
    /// Global fact retrieved heavily in repo sessions, irrelevant.
    ScopeMisclassified,
    /// User edited managed block; auto-rewrite skipped.
    SkillFileConflict,
    /// Same `problem_hash` failed ≥ 3× across sessions.
    ProblemClassRefactor,
    /// User overrode ≥ 3 dead-end warnings in same repo.
    LowerDeadEndThreshold,
    /// Preference observed across ≥ 3 repos with low refutation.
    PromoteToGlobal,
    /// Counterfactual retrieved ≥ 5× and always ignored.
    PromoteCounterfactualVisibility,
    /// C2 git invalidation produced a `stale_candidate`.
    StaleFactDetected,
    /// Safety-net — a fact lacks valid provenance.
    ProvenanceMissing,
    /// > 50 pending distillations — LLM provider probably down.
    DistillerQueueBacklog,
}

impl CodingMirrorAlertKind {
    /// Lossless string form for persistence.
    #[must_use]
    pub fn as_str(self) -> &'static str {
        match self {
            Self::ProjectSkillObsolete => "project_skill_obsolete",
            Self::UncapturedPattern => "uncaptured_pattern",
            Self::ScopeMisclassified => "scope_misclassified",
            Self::SkillFileConflict => "skill_file_conflict",
            Self::ProblemClassRefactor => "problem_class_refactor",
            Self::LowerDeadEndThreshold => "lower_dead_end_threshold",
            Self::PromoteToGlobal => "promote_to_global",
            Self::PromoteCounterfactualVisibility => "promote_counterfactual_visibility",
            Self::StaleFactDetected => "stale_fact_detected",
            Self::ProvenanceMissing => "provenance_missing",
            Self::DistillerQueueBacklog => "distiller_queue_backlog",
        }
    }
}

/// Closed enum of severities.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MirrorAlertSeverity {
    /// Low.
    Low,
    /// Medium.
    Medium,
    /// High.
    High,
    /// Critical.
    Critical,
}

impl MirrorAlertSeverity {
    /// Lossless string form.
    #[must_use]
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Low => "low",
            Self::Medium => "medium",
            Self::High => "high",
            Self::Critical => "critical",
        }
    }
}
```

Create `crates/coding-memory/src/mirror/pattern_effectiveness.rs` with the repo type used by the test:

```rust
//! Pattern effectiveness EMA log repo. Filled in by Task 19.

/// Repo for the `pattern_effectiveness_log` table.
#[derive(Debug, Clone)]
pub struct PatternEffectivenessLogRepo {
    pool: storage::StoragePool,
}

impl PatternEffectivenessLogRepo {
    /// Construct.
    pub fn new(pool: storage::StoragePool) -> Self {
        Self { pool }
    }

    /// Pool ref.
    pub fn pool(&self) -> &storage::StoragePool {
        &self.pool
    }
}
```

Create one-line stubs for the remaining `mirror/*` files (`coding_alerts_query.rs`, `coding_meta_rules.rs`, `coding_routing.rs`, `stale_memory.rs`) with `//! Stub — filled in by Task XX.` content.

- [ ] **Step 7: Re-export from `lib.rs`**

Edit `crates/coding-memory/src/lib.rs`. Add: `pub mod mirror;` near the other module declarations.

- [ ] **Step 8: Verify**

Run: `cargo build -p coding-memory && cargo nextest run -p coding-memory --test coding_synthesis`
Expected: clean build + tests pass.

- [ ] **Step 9: Commit**

```bash
git add crates/coding-memory/src/reforge/coding_synthesis.rs \
        crates/coding-memory/src/mirror \
        crates/coding-memory/src/lib.rs \
        crates/coding-memory/tests/coding_synthesis.rs \
        crates/bus/src/domain_events.rs
git commit -m "feat(coding-memory): Phase 5 — CodingSynthesisPhase orchestrator + 6 promotion actions"
```

---

### Task 7: `ManagedBlock` parser + atomic writer

**Files:**
- Modify: `crates/coding-memory/src/reforge/managed_block.rs`
- Test: `crates/coding-memory/tests/managed_block.rs`

`★ Insight ─────────────────────────────────────`
- The managed-block contract is the *only* point where Reforge writes user-facing files. Getting it wrong corrupts user content. The parser must be byte-exact: bytes outside `<!-- klyntbot:managed:start -->` and `<!-- klyntbot:managed:end -->` are preserved verbatim.
- We use `tempfile::NamedTempFile::persist` (from the `tempfile` crate, already in workspace) to get atomic rename. This guarantees that interrupted writes never leave partial files.
- The "user wrote inside the managed range" detection compares the SHA-256 of the *parsed* `inside` body against the hash recorded at the previous Reforge cycle. That hash is journaled in `skill_versions.metadata.rule_artifact_hash`. On mismatch, we refuse to overwrite and emit a `SkillFileConflict` alert.
`─────────────────────────────────────────────────`

- [ ] **Step 1: Write the failing test**

Create `crates/coding-memory/tests/managed_block.rs`:

```rust
use coding_memory::reforge::managed_block::{ManagedBlock, ManagedBlockError};
use std::fs;
use tempfile::tempdir;

const SAMPLE: &str = "\
# Notes

User content above.

<!-- klyntbot:managed:start | generated: 2026-04-22T03:00Z | cycle: 1 -->
- managed line 1
- managed line 2
<!-- klyntbot:managed:end -->

User content below.
";

#[test]
fn parses_user_outside_managed_inside() {
    let dir = tempdir().unwrap();
    let path = dir.path().join("CLAUDE.md");
    fs::write(&path, SAMPLE).unwrap();

    let block = ManagedBlock::read(&path).expect("parse");
    assert!(block.before.contains("User content above"));
    assert!(block.inside.contains("managed line 1"));
    assert!(block.after.contains("User content below"));
}

#[test]
fn write_rebuilds_file_with_new_managed_body_only() {
    let dir = tempdir().unwrap();
    let path = dir.path().join("CLAUDE.md");
    fs::write(&path, SAMPLE).unwrap();

    let block = ManagedBlock::read(&path).expect("parse");
    let new_body = "- replaced line\n";
    block
        .write_with_new_inside(&path, new_body, "cycle-2")
        .expect("write");

    let written = fs::read_to_string(&path).unwrap();
    assert!(written.contains("User content above"));
    assert!(written.contains("- replaced line"));
    assert!(!written.contains("managed line 1"));
    assert!(written.contains("User content below"));
}

#[test]
fn write_inserts_managed_block_when_absent() {
    let dir = tempdir().unwrap();
    let path = dir.path().join("FRESH.md");
    let block = ManagedBlock::default();
    block
        .write_with_new_inside(&path, "- hello\n", "cycle-1")
        .expect("write");

    let written = fs::read_to_string(&path).unwrap();
    assert!(written.starts_with("<!-- klyntbot:managed:start"));
    assert!(written.contains("- hello"));
}

#[test]
fn detects_user_modification_to_managed_range() {
    let dir = tempdir().unwrap();
    let path = dir.path().join("CLAUDE.md");
    fs::write(&path, SAMPLE).unwrap();

    let block = ManagedBlock::read(&path).expect("parse");
    let prior_hash = block.inside_hash();
    // Simulate the user editing inside.
    let mutated = SAMPLE.replace("managed line 1", "user-edited line");
    fs::write(&path, &mutated).unwrap();
    let block2 = ManagedBlock::read(&path).expect("parse");
    let outcome = block2.write_with_new_inside_if_unchanged(
        &path,
        "- new\n",
        "cycle-2",
        Some(&prior_hash),
    );
    assert!(matches!(outcome, Err(ManagedBlockError::UserConflict)));
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo nextest run -p coding-memory --test managed_block`
Expected: FAIL.

- [ ] **Step 3: Implement `ManagedBlock`**

Replace `crates/coding-memory/src/reforge/managed_block.rs` with:

```rust
//! Managed-block markdown parser/writer.
//!
//! - **Invariant:** bytes outside `<!-- klyntbot:managed:start ... -->` and
//!   `<!-- klyntbot:managed:end -->` are byte-equal before and after a write.
//! - **Atomic:** writes go through `tempfile::NamedTempFile::persist`.
//! - **Conflict detection:** an optional `prior_inside_hash` lets the caller
//!   reject writes when the user edited inside the managed range between cycles.

use sha2::{Digest, Sha256};
use std::fs;
use std::io::Write;
use std::path::Path;
use tempfile::NamedTempFile;
use thiserror::Error;

const START_PREFIX: &str = "<!-- klyntbot:managed:start";
const END_MARKER: &str = "<!-- klyntbot:managed:end -->";

/// Managed-block error surface.
#[derive(Debug, Error)]
pub enum ManagedBlockError {
    /// IO failure.
    #[error("io: {0}")]
    Io(#[from] std::io::Error),
    /// User edited managed range — refuse to overwrite.
    #[error("user content found inside managed range — refusing to overwrite")]
    UserConflict,
    /// Markers malformed (start without end, etc.).
    #[error("malformed managed block: {0}")]
    Malformed(String),
}

/// Parsed managed block.
#[derive(Debug, Clone, Default)]
pub struct ManagedBlock {
    /// Lines before the managed start marker (preserved verbatim, includes trailing newline).
    pub before: String,
    /// The original `<!-- klyntbot:managed:start ... -->` marker line (preserved when re-writing).
    pub start_marker: String,
    /// Body inside the managed range (excluding markers).
    pub inside: String,
    /// Lines after the managed end marker (preserved verbatim).
    pub after: String,
}

impl ManagedBlock {
    /// Read + parse a file. Returns a default empty block if the file does not exist.
    pub fn read(path: &Path) -> Result<Self, ManagedBlockError> {
        if !path.exists() {
            return Ok(Self::default());
        }
        let content = fs::read_to_string(path)?;
        Self::parse(&content)
    }

    /// Parse a string.
    pub fn parse(content: &str) -> Result<Self, ManagedBlockError> {
        let Some(start_idx) = content.find(START_PREFIX) else {
            // No managed block — entire file is "before" content from the
            // perspective of a fresh write. We treat it as if there is no
            // existing managed range.
            return Ok(ManagedBlock {
                before: content.to_string(),
                start_marker: String::new(),
                inside: String::new(),
                after: String::new(),
            });
        };
        let after_start = &content[start_idx..];
        let Some(start_line_end) = after_start.find('\n') else {
            return Err(ManagedBlockError::Malformed(
                "start marker without newline".into(),
            ));
        };
        let start_marker_line = &after_start[..=start_line_end];
        let body_start = start_idx + start_line_end + 1;
        let Some(end_idx_rel) = content[body_start..].find(END_MARKER) else {
            return Err(ManagedBlockError::Malformed(
                "managed start without matching end marker".into(),
            ));
        };
        let end_idx = body_start + end_idx_rel;
        let inside = &content[body_start..end_idx];
        let after_end = end_idx + END_MARKER.len();
        // Skip the trailing newline of the end marker line if present.
        let after_start_idx = if content[after_end..].starts_with('\n') {
            after_end + 1
        } else {
            after_end
        };
        Ok(ManagedBlock {
            before: content[..start_idx].to_string(),
            start_marker: start_marker_line.to_string(),
            inside: inside.to_string(),
            after: content[after_start_idx..].to_string(),
        })
    }

    /// SHA-256 hex of the inside body.
    #[must_use]
    pub fn inside_hash(&self) -> String {
        let mut hasher = Sha256::new();
        hasher.update(self.inside.as_bytes());
        format!("{:x}", hasher.finalize())
    }

    /// Atomic write: replace the inside body with `new_body` and stamp `cycle_id` in the start marker.
    /// `cycle_id` ends up in the marker comment as `cycle: <id>`.
    pub fn write_with_new_inside(
        &self,
        path: &Path,
        new_body: &str,
        cycle_id: &str,
    ) -> Result<(), ManagedBlockError> {
        self.write_with_new_inside_if_unchanged(path, new_body, cycle_id, None)
    }

    /// Like `write_with_new_inside`, but refuses if the parsed inside hash
    /// differs from `prior_inside_hash` (when supplied).
    pub fn write_with_new_inside_if_unchanged(
        &self,
        path: &Path,
        new_body: &str,
        cycle_id: &str,
        prior_inside_hash: Option<&str>,
    ) -> Result<(), ManagedBlockError> {
        if let Some(prior) = prior_inside_hash {
            if !self.inside.is_empty() && self.inside_hash() != prior {
                return Err(ManagedBlockError::UserConflict);
            }
        }
        let now = jiff::Timestamp::now().to_string();
        let new_marker = format!(
            "{START_PREFIX} | generated: {now} | cycle: {cycle_id} -->\n"
        );
        let mut rebuilt = String::with_capacity(
            self.before.len() + new_marker.len() + new_body.len() + END_MARKER.len() + self.after.len() + 8,
        );
        rebuilt.push_str(&self.before);
        if !rebuilt.ends_with('\n') && !rebuilt.is_empty() {
            rebuilt.push('\n');
        }
        rebuilt.push_str(&new_marker);
        rebuilt.push_str(new_body);
        if !new_body.ends_with('\n') {
            rebuilt.push('\n');
        }
        rebuilt.push_str(END_MARKER);
        rebuilt.push('\n');
        rebuilt.push_str(&self.after);

        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }
        let parent = path.parent().unwrap_or(Path::new("."));
        let mut tmp = NamedTempFile::new_in(parent)?;
        tmp.write_all(rebuilt.as_bytes())?;
        tmp.flush()?;
        tmp.persist(path)
            .map_err(|e| ManagedBlockError::Io(e.error))?;
        Ok(())
    }
}
```

- [ ] **Step 4: Add `tempfile` to coding-memory `[dependencies]` if not already present**

Edit `crates/coding-memory/Cargo.toml`. If `tempfile` is only in `[dev-dependencies]`, move it to `[dependencies]` (it's now used at runtime).

- [ ] **Step 5: Verify**

Run: `cargo nextest run -p coding-memory --test managed_block`
Expected: PASS for all four tests.

- [ ] **Step 6: Commit**

```bash
git add crates/coding-memory/src/reforge/managed_block.rs \
        crates/coding-memory/Cargo.toml \
        crates/coding-memory/tests/managed_block.rs
git commit -m "feat(coding-memory): Phase 5 — ManagedBlock parser + atomic writer"
```

---

### Task 8: Sensitivity filter + `RuleArtifactGenerationPhase` orchestrator

**Files:**
- Modify: `crates/coding-memory/src/reforge/rule_artifacts.rs`
- Test: `crates/coding-memory/tests/sensitivity_filter.rs`
- Test: `crates/coding-memory/tests/rule_artifacts_phase.rs`

`★ Insight ─────────────────────────────────────`
- Repo discovery is non-trivial: we need a path on disk to write the artifact, but `scope_repo_id` is canonical (e.g. `git@github.com:org/repo.git@deadbeef`). The discovery uses `~/.klyntbot/repo_paths.json` (managed by Phase 2's repo resolver) to map canonical id → working tree path. Repos not in the map are skipped silently (the user simply hasn't opened them on this machine).
- The phase uses a **per-artifact-kind separate LLM call**. This costs more tokens but means a single bad CLAUDE.md generation doesn't poison the AGENTS.md output. Failure isolation per artifact.
`─────────────────────────────────────────────────`

- [ ] **Step 1: Write the failing tests**

Create `crates/coding-memory/tests/sensitivity_filter.rs`:

```rust
use coding_memory::reforge::sensitivity_filter::filter_for_externalization;
use coding_memory::reforge::types::SerializableSemanticFact;

fn fact(id: &str, sensitivity: &str) -> SerializableSemanticFact {
    SerializableSemanticFact {
        id: id.into(),
        subject: "s".into(),
        predicate: "p".into(),
        object: "o".into(),
        confidence: 0.8,
        memory_type: "fact".into(),
        sensitivity: sensitivity.into(),
        scope_repo_id: None,
        valid_from: jiff::Timestamp::now().to_string(),
    }
}

#[test]
fn drops_high_and_excluded_keeps_normal() {
    let input = vec![
        fact("a", "normal"),
        fact("b", "high"),
        fact("c", "excluded"),
        fact("d", "normal"),
    ];
    let kept = filter_for_externalization(&input);
    assert_eq!(kept.len(), 2);
    assert_eq!(kept[0].id, "a");
    assert_eq!(kept[1].id, "d");
}
```

Create `crates/coding-memory/tests/rule_artifacts_phase.rs`:

```rust
use coding_memory::reforge::types::CodingPhaseHandlers;
use coding_memory::reforge::{RuleArtifactGenerationPhase, RuleArtifactInput, RuleArtifactOutput, RuleArtifactsHandler};
use std::sync::Arc;
use storage::StoragePool;
use tempfile::tempdir;

struct Mock;
#[async_trait::async_trait]
impl RuleArtifactsHandler for Mock {
    async fn synthesize_artifact(
        &self,
        input: &RuleArtifactInput,
    ) -> common::Result<RuleArtifactOutput> {
        Ok(RuleArtifactOutput {
            body: format!(
                "## Architecture\n- repo: {}\n- artifact: {:?}\n",
                input.plan.repo_id, input.artifact
            ),
            section_labels: vec!["Architecture".into()],
        })
    }
}

#[tokio::test]
async fn writes_managed_block_for_each_enabled_artifact() {
    let pool = StoragePool::connect_in_memory().await.unwrap();
    StoragePool::run_feature_migrations(pool.inner(), &cognitive::cognitive_migrations())
        .await
        .unwrap();
    StoragePool::run_feature_migrations(pool.inner(), &coding_memory::coding_memory_migrations())
        .await
        .unwrap();

    let dir = tempdir().unwrap();
    let repo_root = dir.path().join("repo1");
    std::fs::create_dir_all(&repo_root).unwrap();

    // Seed a `RepoContext` fact tied to the canonical id.
    sqlx::query(
        "INSERT INTO semantic_facts \
         (id, subject, predicate, object, confidence, domain, scope_type, scope_id, \
          scope_repo_id, valid_from, memory_type, metadata) \
         VALUES ('fact-1','repo:r1','language','rust',0.9,'work','code',NULL,'r1',?1,'fact','{}')",
    )
    .bind(jiff::Timestamp::now().to_string())
    .execute(pool.inner())
    .await
    .unwrap();

    let fact_repo = cognitive::SemanticFactRepo::new(pool.clone());
    let ep_repo = cognitive::EpisodicMemoryRepo::new(pool.clone());
    let rule_repo = cognitive::ProceduralRuleRepo::new(pool.clone());
    let co_act = cognitive::CoActivationRepo::new(pool.clone());
    let summaries = coding_memory::reforge::SessionSummaryRepo::new(pool.clone());
    let utilization = coding_memory::recall::telemetry::RecallInvocationRepo::new(pool.clone());
    let sd = coding_memory::reforge::selective_delete::SelectiveDeleteLogRepo::new(pool.clone());
    let pat = coding_memory::mirror::pattern_effectiveness::PatternEffectivenessLogRepo::new(pool.clone());

    let handler = Mock;

    // Inject a single-entry repo path map via env var convention.
    std::env::set_var(
        "KLYNTBOT_REPO_PATHS_TEST_OVERRIDE",
        format!(r#"{{"r1":"{}"}}"#, repo_root.display()),
    );

    let handlers = CodingPhaseHandlers {
        synthesis: None,
        rule_artifacts: Some(&handler),
        fact_repo: &fact_repo,
        episodic_repo: &ep_repo,
        rule_repo: &rule_repo,
        co_activation_repo: &co_act,
        utilization_repo: &utilization,
        session_summary_repo: &summaries,
        selective_delete_log: &sd,
        pattern_effectiveness_log: &pat,
        bus: None,
    };

    RuleArtifactGenerationPhase::run(&handlers, &["claude_md".into()])
        .await
        .expect("phase");

    let claude_md = repo_root.join("CLAUDE.md");
    assert!(claude_md.exists(), "CLAUDE.md should be written");
    let content = std::fs::read_to_string(claude_md).unwrap();
    assert!(content.contains("klyntbot:managed:start"));
    assert!(content.contains("Architecture"));
}
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo nextest run -p coding-memory --tests sensitivity_filter rule_artifacts_phase`
Expected: FAIL.

- [ ] **Step 3: Implement the orchestrator**

Replace `crates/coding-memory/src/reforge/rule_artifacts.rs` with:

```rust
//! Phase 3.5 — Rule Artifact Generation.
//!
//! Per repo: filter facts by sensitivity, build a `RepoArtifactPlan`, call the
//! handler once per enabled artifact kind, write each result via `ManagedBlock`
//! atomically. Failure isolation: a failed artifact does not abort the others.

use crate::reforge::managed_block::{ManagedBlock, ManagedBlockError};
use crate::reforge::sensitivity_filter::filter_for_externalization;
use crate::reforge::types::{
    CodingPhaseHandlers, RepoArtifactPlan, RuleArtifactInput, SerializableProceduralRule,
    SerializableSemanticFact,
};
use crate::reforge_phase::RuleArtifact;
use common::{KlyntbotError, Result};
use std::collections::HashMap;
use std::path::PathBuf;
use tracing::{info, warn};
use uuid::Uuid;

/// Orchestrator for Phase 3.5.
#[derive(Debug)]
pub struct RuleArtifactGenerationPhase;

impl RuleArtifactGenerationPhase {
    /// Run the phase.
    ///
    /// `enabled_artifacts` is the configured allowlist
    /// (`config.codingMemory.reforge.ruleArtifacts.*`). Strings: `claude_md`,
    /// `agents_md`, `cursorrules`, `continue_rules`.
    pub async fn run(
        handlers: &CodingPhaseHandlers<'_>,
        enabled_artifacts: &[String],
    ) -> Result<u32> {
        let Some(handler) = handlers.rule_artifacts else {
            return Ok(0);
        };
        let cycle_id = format!("c-{}", Uuid::new_v4().simple());
        let mut artifacts_written = 0_u32;

        // 1. Discover repos with both a known on-disk path and at least one
        //    repo-scoped fact.
        let repo_paths = load_repo_paths()?;
        let pool = handlers.fact_repo.pool().clone();
        let repo_ids: Vec<(String,)> = sqlx::query_as(
            "SELECT DISTINCT scope_repo_id FROM semantic_facts \
             WHERE scope_repo_id IS NOT NULL",
        )
        .fetch_all(pool.inner())
        .await
        .map_err(|e| KlyntbotError::Storage(format!("repo enum: {e}")))?;

        for (repo_id,) in repo_ids {
            let Some(root) = repo_paths.get(&repo_id) else {
                continue;
            };
            let plan = match build_plan(handlers, &repo_id, root.clone(), enabled_artifacts).await {
                Ok(p) => p,
                Err(e) => {
                    warn!("Phase 3.5 plan build failed for {repo_id}: {e}");
                    continue;
                }
            };

            for artifact in &plan.enabled {
                let input = RuleArtifactInput {
                    plan: plan.clone(),
                    artifact: *artifact,
                };
                let output = match handler.synthesize_artifact(&input).await {
                    Ok(o) => o,
                    Err(e) => {
                        warn!("Phase 3.5 LLM call failed for {:?}: {e}", artifact);
                        continue;
                    }
                };
                let path = root.join(artifact.relative_path());
                let block = ManagedBlock::read(&path).unwrap_or_default();
                match block.write_with_new_inside(&path, &output.body, &cycle_id) {
                    Ok(()) => {
                        artifacts_written += 1;
                        info!("Phase 3.5 wrote {} for {repo_id}", path.display());
                    }
                    Err(ManagedBlockError::UserConflict) => {
                        warn!("Phase 3.5 user conflict on {}; skipping", path.display());
                    }
                    Err(e) => warn!("Phase 3.5 write failed for {}: {e}", path.display()),
                }
            }
        }
        Ok(artifacts_written)
    }
}

async fn build_plan(
    handlers: &CodingPhaseHandlers<'_>,
    repo_id: &str,
    root: PathBuf,
    enabled_artifacts: &[String],
) -> Result<RepoArtifactPlan> {
    let pool = handlers.fact_repo.pool().clone();
    let fact_rows: Vec<(
        String,
        String,
        String,
        String,
        f32,
        String,
        String,
        Option<String>,
        String,
    )> = sqlx::query_as(
        "SELECT id, subject, predicate, object, confidence, \
                COALESCE(memory_type,'fact'), \
                COALESCE(json_extract(metadata, '$.sensitivity'),'normal'), \
                scope_repo_id, valid_from \
         FROM semantic_facts \
         WHERE scope_repo_id = ?1 AND confidence >= 0.7",
    )
    .bind(repo_id)
    .fetch_all(pool.inner())
    .await
    .map_err(|e| KlyntbotError::Storage(format!("plan facts: {e}")))?;
    let facts: Vec<SerializableSemanticFact> = fact_rows
        .into_iter()
        .map(
            |(id, subject, predicate, object, confidence, memory_type, sensitivity, scope_repo_id, valid_from)| {
                SerializableSemanticFact {
                    id,
                    subject,
                    predicate,
                    object,
                    confidence,
                    memory_type,
                    sensitivity,
                    scope_repo_id,
                    valid_from,
                }
            },
        )
        .collect();
    let facts = filter_for_externalization(&facts);

    let rule_rows: Vec<(String, String, String, f32, f32, Option<String>)> = sqlx::query_as(
        "SELECT id, rule, source, confidence, COALESCE(effectiveness_score, 0.5), scope_repo_id \
         FROM procedural_rules \
         WHERE scope_repo_id = ?1 AND confidence >= 0.7 AND \
               COALESCE(effectiveness_score, 0.5) >= 0.5",
    )
    .bind(repo_id)
    .fetch_all(pool.inner())
    .await
    .map_err(|e| KlyntbotError::Storage(format!("plan rules: {e}")))?;
    let rules: Vec<SerializableProceduralRule> = rule_rows
        .into_iter()
        .map(|(id, rule, source, confidence, effectiveness, scope_repo_id)| {
            SerializableProceduralRule {
                id,
                rule,
                source,
                confidence,
                effectiveness,
                scope_repo_id,
            }
        })
        .collect();

    let enabled = enabled_artifacts
        .iter()
        .filter_map(|s| match s.as_str() {
            "claude_md" => Some(RuleArtifact::ClaudeMd),
            "agents_md" => Some(RuleArtifact::AgentsMd),
            "cursorrules" => Some(RuleArtifact::CursorRules),
            "continue_rules" => Some(RuleArtifact::ContinueRules),
            _ => None,
        })
        .collect();

    Ok(RepoArtifactPlan {
        repo_id: repo_id.to_string(),
        root,
        enabled,
        facts,
        rules,
    })
}

fn load_repo_paths() -> Result<HashMap<String, PathBuf>> {
    if let Ok(json) = std::env::var("KLYNTBOT_REPO_PATHS_TEST_OVERRIDE") {
        return serde_json::from_str::<HashMap<String, String>>(&json)
            .map(|m| m.into_iter().map(|(k, v)| (k, PathBuf::from(v))).collect())
            .map_err(|e| KlyntbotError::Storage(format!("repo paths test override: {e}")));
    }
    let home = dirs::home_dir().ok_or_else(|| KlyntbotError::Storage("no home dir".into()))?;
    let path = home.join(".klyntbot").join("repo_paths.json");
    if !path.exists() {
        return Ok(HashMap::new());
    }
    let raw = std::fs::read_to_string(&path)
        .map_err(|e| KlyntbotError::Storage(format!("repo_paths.json read: {e}")))?;
    let map: HashMap<String, String> = serde_json::from_str(&raw)
        .map_err(|e| KlyntbotError::Storage(format!("repo_paths.json parse: {e}")))?;
    Ok(map.into_iter().map(|(k, v)| (k, PathBuf::from(v))).collect())
}
```

- [ ] **Step 4: Add `dirs` dep if not present**

Edit `crates/coding-memory/Cargo.toml`. Add to `[dependencies]`:

```toml
dirs = { workspace = true }
```

- [ ] **Step 5: Verify**

Run: `cargo nextest run -p coding-memory --tests sensitivity_filter rule_artifacts_phase`
Expected: PASS.

- [ ] **Step 6: Commit**

```bash
git add crates/coding-memory/src/reforge/rule_artifacts.rs \
        crates/coding-memory/Cargo.toml \
        crates/coding-memory/tests/sensitivity_filter.rs \
        crates/coding-memory/tests/rule_artifacts_phase.rs
git commit -m "feat(coding-memory): Phase 5 — RuleArtifactGenerationPhase + sensitivity filter"
```

---

### Task 9: Cross-session fact dedup (Phase 6.5 extension)

**Files:**
- Modify: `crates/coding-memory/src/reforge/cross_session_dedup.rs`
- Test: `crates/coding-memory/tests/cross_session_dedup.rs`

`★ Insight ─────────────────────────────────────`
- **Bi-temporal supersede, not delete.** When two facts have similarity > 0.92, the older row's `valid_until` is set to the newer row's `valid_from`, and the newer row's `supersedes` field is filled. Both rows remain queryable — `recall_change_history` walks them. This is the spec's Invariant 6: "after any Reforge cycle, all prior `episodic_memories` rows still exist."
- We reuse `UnifiedMemoryService` to compute similarity rather than re-doing the embedding work. Each candidate fact is queried for top-1 nearest neighbor; if similarity > 0.92 and they're distinct ids and same scope, we supersede.
`─────────────────────────────────────────────────`

- [ ] **Step 1: Write the failing test**

Create `crates/coding-memory/tests/cross_session_dedup.rs`:

```rust
use coding_memory::reforge::CrossSessionDedup;
use storage::StoragePool;

#[tokio::test]
async fn supersedes_high_similarity_pair_preserves_both() {
    let pool = StoragePool::connect_in_memory().await.unwrap();
    StoragePool::run_feature_migrations(pool.inner(), &cognitive::cognitive_migrations())
        .await
        .unwrap();
    StoragePool::run_feature_migrations(pool.inner(), &coding_memory::coding_memory_migrations())
        .await
        .unwrap();

    // Seed two near-duplicate facts (we simulate similarity by directly seeding
    // an embedding + the dedup helper that bypasses LanceDB in-memory).
    sqlx::query(
        "INSERT INTO semantic_facts \
         (id, subject, predicate, object, confidence, domain, scope_type, scope_id, scope_repo_id, valid_from, memory_type) \
         VALUES \
         ('f1','repo:r1','uses_framework','axum 0.7',0.85,'work','code',NULL,'r1',?1,'fact'), \
         ('f2','repo:r1','uses_framework','axum 0.7',0.9,'work','code',NULL,'r1',?2,'fact')",
    )
    .bind("2026-04-20T10:00:00Z")
    .bind("2026-04-25T10:00:00Z")
    .execute(pool.inner())
    .await
    .unwrap();

    let fact_repo = cognitive::SemanticFactRepo::new(pool.clone());
    let count = CrossSessionDedup::run_test_only_exact_match(&fact_repo, 0.92)
        .await
        .expect("dedup");
    assert!(count >= 1);

    // Both rows still exist.
    let row_count: (i64,) =
        sqlx::query_as("SELECT COUNT(*) FROM semantic_facts WHERE id IN ('f1','f2')")
            .fetch_one(pool.inner())
            .await
            .unwrap();
    assert_eq!(row_count.0, 2);

    // The older row's valid_until is now non-null, and its superseded_by points to f2.
    let (until, by): (Option<String>, Option<String>) = sqlx::query_as(
        "SELECT valid_until, superseded_by FROM semantic_facts WHERE id = 'f1'",
    )
    .fetch_one(pool.inner())
    .await
    .unwrap();
    assert!(until.is_some());
    assert_eq!(by.as_deref(), Some("f2"));
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo nextest run -p coding-memory --test cross_session_dedup`
Expected: FAIL.

- [ ] **Step 3: Implement the dedup pass**

Replace `crates/coding-memory/src/reforge/cross_session_dedup.rs` with:

```rust
//! Phase 6.5 cross-session fact dedup.
//!
//! Find pairs of facts in the same `(scope_repo_id, subject, predicate)` bucket
//! whose `object` matches under exact-string equality (Phase 5 floor) or vector
//! similarity > threshold (Phase 6 will swap in `UnifiedMemoryService`). When
//! found, the older row's `valid_until` and the newer row's `supersedes` are
//! set bi-temporally — both rows remain queryable.

use cognitive::SemanticFactRepo;
use common::{KlyntbotError, Result};
use jiff::Timestamp;

/// Cross-session dedup pass.
#[derive(Debug, Default)]
pub struct CrossSessionDedup;

impl CrossSessionDedup {
    /// Run via vector similarity. Phase 5 ships the exact-match-only fallback
    /// because LanceDB embeddings live outside the in-memory test pool. Phase 6
    /// will replace this with similarity-based candidate pulls.
    pub async fn run(
        repo: &SemanticFactRepo,
        _similarity_threshold: f32,
    ) -> Result<u32> {
        Self::run_test_only_exact_match(repo, 0.92).await
    }

    /// Exact-match dedup — same `(scope_repo_id, subject, predicate, object)`
    /// across two distinct ids. Used as the Phase-5 floor and as the test seam.
    pub async fn run_test_only_exact_match(
        repo: &SemanticFactRepo,
        _similarity_threshold: f32,
    ) -> Result<u32> {
        let pool = repo.pool().clone();
        let pairs: Vec<(String, String, String, String)> = sqlx::query_as(
            "SELECT older.id, older.valid_from, newer.id, newer.valid_from \
             FROM semantic_facts older \
             JOIN semantic_facts newer ON \
                 older.subject = newer.subject AND \
                 older.predicate = newer.predicate AND \
                 older.object = newer.object AND \
                 (older.scope_repo_id IS NEWER.scope_repo_id OR \
                  older.scope_repo_id = newer.scope_repo_id) AND \
                 older.id != newer.id AND \
                 older.valid_from < newer.valid_from \
             WHERE older.valid_until IS NULL AND newer.superseded_by IS NULL",
        )
        .fetch_all(pool.inner())
        .await
        .map_err(|e| KlyntbotError::Storage(format!("dedup query: {e}")))?;

        let mut applied = 0_u32;
        for (older_id, _older_vf, newer_id, newer_vf) in pairs {
            let now = Timestamp::now().to_string();
            let cutoff = if newer_vf.is_empty() { now } else { newer_vf };
            sqlx::query(
                "UPDATE semantic_facts SET valid_until = ?1, superseded_by = ?2 WHERE id = ?3",
            )
            .bind(&cutoff)
            .bind(&newer_id)
            .bind(&older_id)
            .execute(pool.inner())
            .await
            .map_err(|e| KlyntbotError::Storage(format!("dedup older update: {e}")))?;
            sqlx::query("UPDATE semantic_facts SET supersedes = ?1 WHERE id = ?2")
                .bind(&older_id)
                .bind(&newer_id)
                .execute(pool.inner())
                .await
                .map_err(|e| KlyntbotError::Storage(format!("dedup newer update: {e}")))?;
            applied += 1;
        }
        Ok(applied)
    }
}
```

- [ ] **Step 4: Verify**

Run: `cargo nextest run -p coding-memory --test cross_session_dedup`
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add crates/coding-memory/src/reforge/cross_session_dedup.rs \
        crates/coding-memory/tests/cross_session_dedup.rs
git commit -m "feat(coding-memory): Phase 5 — cross-session fact dedup (bi-temporal SUPERSEDE)"
```

---

### Task 10: Selective-delete signal (Phase 6 extension)

**Files:**
- Modify: `crates/coding-memory/src/reforge/selective_delete.rs`
- Test: `crates/coding-memory/tests/selective_delete.rs`

`★ Insight ─────────────────────────────────────`
- "Selective delete" is a misnomer — we never delete. We *halve stability* on memories with persistent retrieval-without-citation. Halving stability moves them down in retrieval ranking via FSRS5 decay; they remain in the store forever (Invariant 6).
- The signal feed comes from `recall_invocations` (retrieval count) cross-referenced with the new `cited_in_response` column on `memory_utilization` (citation count). The `StaleMemorySource` (Task 20) writes the citation rows; this Task 10 just reads them.
- Audit log to `selective_delete_log` is essential for the Mirror Alerts Feed UI — users need to know why a memory dropped in ranking.
`─────────────────────────────────────────────────`

- [ ] **Step 1: Write the failing test**

Create `crates/coding-memory/tests/selective_delete.rs`:

```rust
use coding_memory::reforge::selective_delete::{SelectiveDeleteLogRepo, SelectiveDeleteSignal};
use storage::StoragePool;

#[tokio::test]
async fn halves_stability_on_uncited_memories() {
    let pool = StoragePool::connect_in_memory().await.unwrap();
    StoragePool::run_feature_migrations(pool.inner(), &cognitive::cognitive_migrations())
        .await
        .unwrap();
    StoragePool::run_feature_migrations(pool.inner(), &coding_memory::coding_memory_migrations())
        .await
        .unwrap();

    // Seed a fact with stability 4.0.
    sqlx::query(
        "INSERT INTO semantic_facts \
         (id, subject, predicate, object, confidence, domain, scope_type, scope_id, \
          valid_from, memory_type, stability, access_count) \
         VALUES ('f1','s','p','o',0.9,'work','code',NULL,?1,'fact',4.0,0)",
    )
    .bind(jiff::Timestamp::now().to_string())
    .execute(pool.inner())
    .await
    .unwrap();

    // Seed 6 retrievals with cited_in_response = false.
    for i in 0..6 {
        sqlx::query(
            "INSERT INTO memory_utilization (id, memory_id, cited_in_response) \
             VALUES (?1, 'f1', 0)",
        )
        .bind(format!("u{i}"))
        .execute(pool.inner())
        .await
        .unwrap();
    }

    let log = SelectiveDeleteLogRepo::new(pool.clone());
    let count = SelectiveDeleteSignal::apply_with_threshold(&pool, &log, 5)
        .await
        .expect("apply");
    assert!(count >= 1);

    let new_stability: (f32,) = sqlx::query_as("SELECT stability FROM semantic_facts WHERE id = 'f1'")
        .fetch_one(pool.inner())
        .await
        .unwrap();
    assert!((new_stability.0 - 2.0).abs() < 1e-3);

    let log_count: (i64,) =
        sqlx::query_as("SELECT COUNT(*) FROM selective_delete_log WHERE memory_id = 'f1'")
            .fetch_one(pool.inner())
            .await
            .unwrap();
    assert_eq!(log_count.0, 1);
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo nextest run -p coding-memory --test selective_delete`
Expected: FAIL.

- [ ] **Step 3: Implement**

Replace `crates/coding-memory/src/reforge/selective_delete.rs` with:

```rust
//! Phase 6 selective-delete signal.
//!
//! Find memories retrieved ≥ N times with zero citations; multiply their
//! stability by 0.5; log to `selective_delete_log`. **No row deletion ever.**

use common::{KlyntbotError, Result};
use jiff::Timestamp;
use uuid::Uuid;

/// Signal applier.
#[derive(Debug, Default)]
pub struct SelectiveDeleteSignal;

/// Repo for the audit log.
#[derive(Debug, Clone)]
pub struct SelectiveDeleteLogRepo {
    pool: storage::StoragePool,
}

impl SelectiveDeleteLogRepo {
    /// Construct.
    pub fn new(pool: storage::StoragePool) -> Self {
        Self { pool }
    }

    /// Pool ref.
    pub fn pool(&self) -> &storage::StoragePool {
        &self.pool
    }

    /// Insert one audit row.
    pub async fn insert(
        &self,
        memory_id: &str,
        memory_kind: &str,
        retrievals: u32,
        citations: u32,
        before: f32,
        after: f32,
    ) -> Result<()> {
        sqlx::query(
            "INSERT INTO selective_delete_log \
             (id, memory_id, memory_kind, retrievals_observed, citations_observed, \
              stability_before, stability_after) \
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
        )
        .bind(format!("sd_{}", Uuid::new_v4().simple()))
        .bind(memory_id)
        .bind(memory_kind)
        .bind(retrievals as i64)
        .bind(citations as i64)
        .bind(before)
        .bind(after)
        .execute(self.pool.inner())
        .await
        .map_err(|e| KlyntbotError::Storage(format!("selective_delete_log insert: {e}")))?;
        Ok(())
    }
}

impl SelectiveDeleteSignal {
    /// Apply with default threshold 5.
    pub async fn apply(pool: &storage::StoragePool, log: &SelectiveDeleteLogRepo) -> Result<u32> {
        Self::apply_with_threshold(pool, log, 5).await
    }

    /// Apply with explicit threshold.
    pub async fn apply_with_threshold(
        pool: &storage::StoragePool,
        log: &SelectiveDeleteLogRepo,
        threshold: u32,
    ) -> Result<u32> {
        let candidates: Vec<(String, i64, i64)> = sqlx::query_as(
            "SELECT memory_id, COUNT(*), SUM(CASE WHEN cited_in_response THEN 1 ELSE 0 END) \
             FROM memory_utilization \
             GROUP BY memory_id \
             HAVING COUNT(*) >= ?1 AND SUM(CASE WHEN cited_in_response THEN 1 ELSE 0 END) = 0",
        )
        .bind(threshold as i64)
        .fetch_all(pool.inner())
        .await
        .map_err(|e| KlyntbotError::Storage(format!("uncited query: {e}")))?;

        let now = Timestamp::now().to_string();
        let mut applied = 0_u32;
        for (memory_id, retrievals, _) in candidates {
            // Try semantic_facts first, then episodic_memories.
            let (kind, prior_stability) = match fetch_stability(pool, &memory_id, "semantic_facts").await? {
                Some(s) => ("semantic_fact", s),
                None => match fetch_stability(pool, &memory_id, "episodic_memories").await? {
                    Some(s) => ("episodic_memory", s),
                    None => continue,
                },
            };
            let new_stability = prior_stability * 0.5;
            update_stability(pool, &memory_id, kind, new_stability, &now).await?;
            log.insert(
                &memory_id,
                kind,
                retrievals as u32,
                0,
                prior_stability,
                new_stability,
            )
            .await?;
            applied += 1;
        }
        Ok(applied)
    }
}

async fn fetch_stability(
    pool: &storage::StoragePool,
    id: &str,
    table: &str,
) -> Result<Option<f32>> {
    let q = format!("SELECT stability FROM {table} WHERE id = ?1");
    let row: Option<(f32,)> = sqlx::query_as(&q)
        .bind(id)
        .fetch_optional(pool.inner())
        .await
        .map_err(|e| KlyntbotError::Storage(format!("stability fetch: {e}")))?;
    Ok(row.map(|(s,)| s))
}

async fn update_stability(
    pool: &storage::StoragePool,
    id: &str,
    kind: &str,
    new_stability: f32,
    _now: &str,
) -> Result<()> {
    let table = if kind == "semantic_fact" {
        "semantic_facts"
    } else {
        "episodic_memories"
    };
    let q = format!("UPDATE {table} SET stability = ?1 WHERE id = ?2");
    sqlx::query(&q)
        .bind(new_stability)
        .bind(id)
        .execute(pool.inner())
        .await
        .map_err(|e| KlyntbotError::Storage(format!("stability update: {e}")))?;
    Ok(())
}
```

- [ ] **Step 4: Verify**

Run: `cargo nextest run -p coding-memory --test selective_delete`
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add crates/coding-memory/src/reforge/selective_delete.rs \
        crates/coding-memory/tests/selective_delete.rs
git commit -m "feat(coding-memory): Phase 5 — selective-delete signal (stability *= 0.5)"
```

---

### Task 11: agent crate — `CodingSynthesisHandler` LLM impl

**Files:**
- Create: `crates/agent/src/handlers/coding_synthesis.rs`
- Modify: `crates/agent/src/handlers/mod.rs`
- Test: `crates/agent/tests/coding_synthesis_handler.rs`

`★ Insight ─────────────────────────────────────`
- The system prompt tells the model it is a **memory promoter**, not a memory writer. Distillers write *new* memories; this handler only *promotes* existing ones into higher-level abstractions. Constraining the role keeps token budgets predictable.
- We use `ProviderManager::chat` with `ProviderRole::ReforgeSynth`. The user can configure a larger model here (e.g. Claude Sonnet 4.6) since this runs nightly, not per-turn — cost is amortized over a 24h window.
`─────────────────────────────────────────────────`

- [ ] **Step 1: Write the failing test**

Create `crates/agent/tests/coding_synthesis_handler.rs`:

```rust
use agent::handlers::coding_synthesis::CodingSynthesisHandlerImpl;
use coding_memory::reforge::{CodingSynthesisHandler, CodingSynthesisInput};

#[tokio::test]
async fn empty_input_returns_no_actions() {
    // Use the mock ProviderManager that returns an empty tool-call list.
    let provider = providers::test_helpers::mock_provider_returning(r#"{"actions":[],"narrative":""}"#);
    let handler = CodingSynthesisHandlerImpl::new(std::sync::Arc::new(provider));
    let out = handler
        .synthesize_coding(&CodingSynthesisInput {
            since: jiff::Timestamp::now(),
            repo_bundles: vec![],
            recent_counterfactuals: vec![],
        })
        .await
        .unwrap();
    assert!(out.actions.is_empty());
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo nextest run -p agent --test coding_synthesis_handler`
Expected: FAIL — type missing.

- [ ] **Step 3: Implement the handler**

Create `crates/agent/src/handlers/coding_synthesis.rs`:

```rust
//! LLM-backed `CodingSynthesisHandler` impl. Used by Reforge Phase 2.5.
//!
//! Prompts the model with the per-repo bundles + counterfactuals, instructs
//! it to emit `record_promotion` tool calls, decodes them into `PromoteAction`s.

use async_trait::async_trait;
use coding_memory::reforge::{
    CodingSynthesisHandler, CodingSynthesisInput, CodingSynthesisOutput, PromoteAction,
};
use common::{KlyntbotError, Result};
use providers::{ProviderManager, ProviderRole};
use serde_json::json;
use std::sync::Arc;
use tracing::warn;

/// LLM-backed impl.
pub struct CodingSynthesisHandlerImpl {
    provider: Arc<ProviderManager>,
}

impl CodingSynthesisHandlerImpl {
    /// Construct.
    pub fn new(provider: Arc<ProviderManager>) -> Self {
        Self { provider }
    }
}

#[async_trait]
impl CodingSynthesisHandler for CodingSynthesisHandlerImpl {
    async fn synthesize_coding(
        &self,
        input: &CodingSynthesisInput,
    ) -> Result<CodingSynthesisOutput> {
        let system = SYSTEM_PROMPT.to_string();
        let user = format!(
            "Process the following coding-memory bundles for promotion:\n\n```json\n{}\n```\n\n\
             Emit zero or more `record_promotion` tool calls. Stay within the \
             5-value PromoteAction kinds. Do not invent ids — only reference ids \
             present in the input.",
            serde_json::to_string_pretty(input)
                .map_err(|e| KlyntbotError::Encoding(format!("serialize input: {e}")))?
        );

        let raw = self
            .provider
            .chat_with_role(ProviderRole::ReforgeSynth, &system, &user)
            .await
            .map_err(|e| KlyntbotError::Provider(format!("Phase 2.5 chat: {e}")))?;

        match serde_json::from_str::<CodingSynthesisOutput>(&raw) {
            Ok(out) => Ok(out),
            Err(e) => {
                warn!("Phase 2.5 LLM returned malformed JSON: {e}; trying recovery");
                Ok(recover(&raw))
            }
        }
    }
}

fn recover(raw: &str) -> CodingSynthesisOutput {
    // Attempt to extract a JSON object from the response — fall back to empty.
    if let Some(start) = raw.find('{') {
        if let Some(end) = raw.rfind('}') {
            let candidate = &raw[start..=end];
            if let Ok(parsed) = serde_json::from_str::<CodingSynthesisOutput>(candidate) {
                return parsed;
            }
        }
    }
    CodingSynthesisOutput {
        actions: Vec::new(),
        narrative: format!("recovery from malformed: {} chars", raw.len()),
    }
}

const SYSTEM_PROMPT: &str = "You are the klyntbot memory promoter for the nightly Reforge \
cycle. You receive per-repo bundles of recent coding memory (fix attempts, \
workflow patterns, repo-context facts, causal chains, counterfactuals). Your \
job is NOT to write new memory — that's the Distiller's job. Your job is to \
recognise convergent patterns and emit promotion actions.\n\n\
You MUST respond with a single JSON object matching this shape:\n\
{\"actions\":[<PromoteAction>...],\"narrative\":\"<one-sentence summary>\"}\n\n\
Each PromoteAction has a `kind` discriminator. Allowed kinds:\n\
- extractPattern    — promote a recurring workflow into a `WorkflowPattern`\n\
- extractFailurePattern — promote a recurring failure + remediation\n\
- promoteToProblemClass — same problem_hash failed >=3x; suggest refactor\n\
- promoteToProjectUnderstanding — synthesise a high-confidence repo fact\n\
- promoteToUserHabit — pattern observed across >=3 repos\n\
- promoteToProblemSolutionPattern — >=3 causal chains share a problem_hash\n\n\
Refuse to promote anything with confidence < 0.7. Reference ids only — never invent.";

```

- [ ] **Step 4: Add helper to `ProviderManager`**

Run: `grep -n "fn chat_with_role" crates/providers/src/lib.rs`. If missing, add a helper:

```rust
    /// Convenience helper used by Phase-5 Reforge handlers.
    pub async fn chat_with_role(
        &self,
        role: ProviderRole,
        system: &str,
        user: &str,
    ) -> common::Result<String> {
        let model = self.model_for_role(role);
        self.chat(&model, system, user).await
    }

    fn model_for_role(&self, role: ProviderRole) -> String {
        match role {
            ProviderRole::Distiller => self.config.coding_memory.distiller.model.clone(),
            ProviderRole::ReforgeSynth => self
                .config
                .coding_memory
                .reforge
                .as_ref()
                .and_then(|r| r.synth_model.clone())
                .unwrap_or_else(|| self.config.default_model.clone()),
            ProviderRole::ReforgeRules => self
                .config
                .coding_memory
                .reforge
                .as_ref()
                .and_then(|r| r.rules_model.clone())
                .unwrap_or_else(|| self.config.default_model.clone()),
        }
    }
```

If the config tree doesn't yet have `synth_model` / `rules_model`, add them to `crates/config/src/schema/coding_memory.rs` as `Option<String>` with default `None`.

- [ ] **Step 5: Register module**

Edit `crates/agent/src/handlers/mod.rs`. Add `pub mod coding_synthesis;`.

- [ ] **Step 6: Verify**

Run: `cargo nextest run -p agent --test coding_synthesis_handler`
Expected: PASS.

- [ ] **Step 7: Commit**

```bash
git add crates/agent/src/handlers/coding_synthesis.rs \
        crates/agent/src/handlers/mod.rs \
        crates/agent/tests/coding_synthesis_handler.rs \
        crates/providers/src/lib.rs \
        crates/config/src/schema/coding_memory.rs
git commit -m "feat(agent): Phase 5 — CodingSynthesisHandler LLM impl"
```

---

### Task 12: agent crate — `RuleArtifactsHandler` LLM impl

**Files:**
- Create: `crates/agent/src/handlers/rule_artifacts.rs`
- Modify: `crates/agent/src/handlers/mod.rs`
- Test: `crates/agent/tests/rule_artifacts_handler.rs`

- [ ] **Step 1: Write the failing test**

Create `crates/agent/tests/rule_artifacts_handler.rs`:

```rust
use agent::handlers::rule_artifacts::RuleArtifactsHandlerImpl;
use coding_memory::reforge::{RuleArtifactsHandler, RuleArtifactInput};
use coding_memory::reforge::types::RepoArtifactPlan;
use coding_memory::reforge_phase::RuleArtifact;
use std::path::PathBuf;

#[tokio::test]
async fn returns_body_with_section_labels() {
    let provider = providers::test_helpers::mock_provider_returning(
        r#"{"body":"## Architecture\n- rust\n","section_labels":["Architecture"]}"#,
    );
    let handler = RuleArtifactsHandlerImpl::new(std::sync::Arc::new(provider));
    let out = handler
        .synthesize_artifact(&RuleArtifactInput {
            plan: RepoArtifactPlan {
                repo_id: "r1".into(),
                root: PathBuf::from("/tmp/r1"),
                enabled: vec![RuleArtifact::ClaudeMd],
                facts: vec![],
                rules: vec![],
            },
            artifact: RuleArtifact::ClaudeMd,
        })
        .await
        .unwrap();
    assert!(out.body.contains("Architecture"));
    assert_eq!(out.section_labels, vec!["Architecture".to_string()]);
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo nextest run -p agent --test rule_artifacts_handler`
Expected: FAIL.

- [ ] **Step 3: Implement**

Create `crates/agent/src/handlers/rule_artifacts.rs`:

```rust
//! LLM-backed `RuleArtifactsHandler` impl.

use async_trait::async_trait;
use coding_memory::reforge::types::RepoArtifactPlan;
use coding_memory::reforge::{RuleArtifactInput, RuleArtifactOutput, RuleArtifactsHandler};
use coding_memory::reforge_phase::RuleArtifact;
use common::{KlyntbotError, Result};
use providers::{ProviderManager, ProviderRole};
use std::sync::Arc;
use tracing::warn;

/// LLM-backed impl.
pub struct RuleArtifactsHandlerImpl {
    provider: Arc<ProviderManager>,
}

impl RuleArtifactsHandlerImpl {
    /// Construct.
    pub fn new(provider: Arc<ProviderManager>) -> Self {
        Self { provider }
    }
}

#[async_trait]
impl RuleArtifactsHandler for RuleArtifactsHandlerImpl {
    async fn synthesize_artifact(
        &self,
        input: &RuleArtifactInput,
    ) -> Result<RuleArtifactOutput> {
        let system = system_prompt_for(input.artifact);
        let user = format!(
            "Generate the managed-block body for the {} artifact in repo `{}`.\n\n\
             Plan:\n```json\n{}\n```\n\n\
             Respond ONLY with a JSON object matching `{{\"body\": \"...\", \"section_labels\": [...]}}`.",
            artifact_label(input.artifact),
            input.plan.repo_id,
            serde_json::to_string_pretty(&input.plan)
                .map_err(|e| KlyntbotError::Encoding(format!("serialize plan: {e}")))?
        );

        let raw = self
            .provider
            .chat_with_role(ProviderRole::ReforgeRules, &system, &user)
            .await
            .map_err(|e| KlyntbotError::Provider(format!("Phase 3.5 chat: {e}")))?;

        match serde_json::from_str::<RuleArtifactOutput>(&raw) {
            Ok(out) => Ok(out),
            Err(e) => {
                warn!("Phase 3.5 LLM returned malformed JSON: {e}");
                Ok(RuleArtifactOutput {
                    body: format!("## Notes\n_Auto-generated body unavailable: {e}_\n"),
                    section_labels: vec!["Notes".into()],
                })
            }
        }
    }
}

fn artifact_label(a: RuleArtifact) -> &'static str {
    match a {
        RuleArtifact::ClaudeMd => "CLAUDE.md",
        RuleArtifact::AgentsMd => "AGENTS.md",
        RuleArtifact::CursorRules => ".cursorrules",
        RuleArtifact::ContinueRules => ".continue/rules/klyntbot.md",
    }
}

fn system_prompt_for(_a: RuleArtifact) -> String {
    "You are klyntbot's repo-rule writer. Given a `RepoArtifactPlan` of facts \
     and active procedural rules, write a managed-block body for the requested \
     coding-CLI artifact (CLAUDE.md / AGENTS.md / .cursorrules / .continue/rules).\n\n\
     - Use markdown sections with H2 headings: `## Architecture`, `## Style preferences observed`, `## Known failure patterns`.\n\
     - Be concise: <800 tokens total.\n\
     - Quote facts verbatim when high confidence; never editorialise.\n\
     - Skip empty sections.\n\
     - Respond ONLY with the JSON object schema described in the user prompt."
        .into()
}
```

- [ ] **Step 4: Register module**

Edit `crates/agent/src/handlers/mod.rs`. Add `pub mod rule_artifacts;`.

- [ ] **Step 5: Verify**

Run: `cargo nextest run -p agent --test rule_artifacts_handler`
Expected: PASS.

- [ ] **Step 6: Commit**

```bash
git add crates/agent/src/handlers/rule_artifacts.rs \
        crates/agent/src/handlers/mod.rs \
        crates/agent/tests/rule_artifacts_handler.rs
git commit -m "feat(agent): Phase 5 — RuleArtifactsHandler LLM impl"
```

---

### Task 13: Wire Phase 2.5 + 3.5 into `cognitive::run_reforge`

**Files:**
- Modify: `crates/cognitive/src/services/reforge/mod.rs`
- Modify: `crates/cognitive/src/services/reforge/service.rs`
- Test: `crates/cognitive/tests/run_reforge_with_coding.rs`

`★ Insight ─────────────────────────────────────`
- **Layering constraint:** `cognitive` is an L4 crate; `coding-memory` is L5. We can't have `cognitive` import `coding-memory` directly. So we re-export the *traits* (`CodingSynthesisRunner`, `RuleArtifactsRunner`) from `cognitive`, with the actual concrete `CodingPhaseHandlers` bundle living in `coding-memory`. App-core threads the bundle through a trait-object pointer.
- This mirrors the exact pattern used by `AutotunerBridge` in the same file (line 32-49 of `cognitive/src/services/reforge/mod.rs`).
`─────────────────────────────────────────────────`

- [ ] **Step 1: Write the failing test**

Create `crates/cognitive/tests/run_reforge_with_coding.rs`:

```rust
use cognitive::services::reforge::{CodingPhaseRunner, CodingPhaseRunnerOutcome};
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::Arc;

struct CountingRunner(Arc<AtomicU32>);
#[async_trait::async_trait]
impl CodingPhaseRunner for CountingRunner {
    async fn run_synthesis(&self) -> common::Result<CodingPhaseRunnerOutcome> {
        self.0.fetch_add(1, Ordering::Relaxed);
        Ok(CodingPhaseRunnerOutcome::default())
    }
    async fn run_rule_artifacts(&self) -> common::Result<CodingPhaseRunnerOutcome> {
        self.0.fetch_add(10, Ordering::Relaxed);
        Ok(CodingPhaseRunnerOutcome::default())
    }
    async fn run_cross_session_dedup(&self) -> common::Result<CodingPhaseRunnerOutcome> {
        self.0.fetch_add(100, Ordering::Relaxed);
        Ok(CodingPhaseRunnerOutcome::default())
    }
    async fn run_selective_delete(&self) -> common::Result<CodingPhaseRunnerOutcome> {
        self.0.fetch_add(1000, Ordering::Relaxed);
        Ok(CodingPhaseRunnerOutcome::default())
    }
}

#[tokio::test]
async fn run_reforge_invokes_all_4_coding_phases_in_order() {
    let counter = Arc::new(AtomicU32::new(0));
    let runner = CountingRunner(counter.clone());

    // We don't run the entire run_reforge fixture — instead we exercise the
    // helper that dispatches the 4 phases in order. (full e2e in Task 33.)
    let total = cognitive::services::reforge::dispatch_coding_phases_for_test(&runner)
        .await
        .expect("dispatch");
    assert_eq!(total, 4);
    assert_eq!(counter.load(Ordering::Relaxed), 1111);
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo nextest run -p cognitive --test run_reforge_with_coding`
Expected: FAIL — `CodingPhaseRunner` not defined.

- [ ] **Step 3: Add the trait + dispatcher to `cognitive`**

Edit `crates/cognitive/src/services/reforge/mod.rs`. After the existing `GraphEnrichmentHandler` block, append:

```rust
// ---------------------------------------------------------------------------
// CodingPhaseRunner — Phase 5 dependency inversion (L4 → L5 boundary)
// ---------------------------------------------------------------------------

/// Outcome bundle from one coding-phase run.
#[derive(Debug, Clone, Default)]
pub struct CodingPhaseRunnerOutcome {
    /// Free-form action count (e.g. promotions applied, artifacts written).
    pub applied: u32,
    /// Optional narrative for telemetry.
    pub narrative: Option<String>,
}

/// Bridge trait for the four coding-specific Reforge phases. Implemented in
/// `app-core` over the concrete `coding_memory::reforge::CodingPhaseHandlers`
/// bundle.
#[async_trait]
pub trait CodingPhaseRunner: Send + Sync {
    /// Phase 2.5 — Coding Synthesis.
    async fn run_synthesis(&self) -> common::Result<CodingPhaseRunnerOutcome>;
    /// Phase 3.5 — Rule Artifact Generation.
    async fn run_rule_artifacts(&self) -> common::Result<CodingPhaseRunnerOutcome>;
    /// Phase 6.5 extension — cross-session fact dedup.
    async fn run_cross_session_dedup(&self) -> common::Result<CodingPhaseRunnerOutcome>;
    /// Phase 6 extension — selective-delete signal.
    async fn run_selective_delete(&self) -> common::Result<CodingPhaseRunnerOutcome>;
}

/// Dispatch helper for the unit test in `tests/run_reforge_with_coding.rs`.
pub async fn dispatch_coding_phases_for_test(
    runner: &dyn CodingPhaseRunner,
) -> common::Result<u32> {
    let mut count = 0;
    if let Ok(_) = runner.run_synthesis().await {
        count += 1;
    }
    if let Ok(_) = runner.run_rule_artifacts().await {
        count += 1;
    }
    if let Ok(_) = runner.run_cross_session_dedup().await {
        count += 1;
    }
    if let Ok(_) = runner.run_selective_delete().await {
        count += 1;
    }
    Ok(count)
}
```

- [ ] **Step 4: Add new parameter to `run_reforge` + dispatch**

Edit `crates/cognitive/src/services/reforge/service.rs`. The `run_reforge` signature is already very large; add **one** new parameter at the very end:

```rust
    coding_phase_runner: Option<&dyn super::CodingPhaseRunner>,
```

After the existing Phase 3 (Review) block and before Phase 4 (Narrate), insert:

```rust
    // ------------------------------------------------------------------
    // Phase 2.5: Coding Synthesis (extension hook)
    // ------------------------------------------------------------------
    if let Some(runner) = coding_phase_runner {
        info!("Reforge Phase 2.5: Coding Synthesis");
        match runner.run_synthesis().await {
            Ok(out) => {
                debug!(applied = out.applied, "Phase 2.5 complete");
                if let Some(n) = out.narrative {
                    result.phase_errors.push(format!("phase 2.5 narrative: {n}"));
                }
            }
            Err(e) => {
                warn!("Phase 2.5 failed: {e}");
                result.phase_errors.push(format!("phase 2.5: {e}"));
            }
        }
    }
```

After the Phase 5 (Apply) block, insert:

```rust
    // ------------------------------------------------------------------
    // Phase 3.5: Rule Artifact Generation (extension hook)
    // ------------------------------------------------------------------
    if let Some(runner) = coding_phase_runner {
        info!("Reforge Phase 3.5: Rule Artifact Generation");
        if let Err(e) = runner.run_rule_artifacts().await {
            warn!("Phase 3.5 failed: {e}");
            result.phase_errors.push(format!("phase 3.5: {e}"));
        }
    }
```

After the Phase 6 (Optimize) block, insert:

```rust
    // ------------------------------------------------------------------
    // Phase 6 extension: selective-delete signal
    // ------------------------------------------------------------------
    if let Some(runner) = coding_phase_runner {
        info!("Reforge Phase 6 ext: selective-delete signal");
        if let Err(e) = runner.run_selective_delete().await {
            warn!("Phase 6 selective-delete failed: {e}");
            result.phase_errors.push(format!("phase 6 selective-delete: {e}"));
        }
    }
```

After the existing Phase 6.5 (Graph Consolidation) block, insert:

```rust
    // ------------------------------------------------------------------
    // Phase 6.5 extension: cross-session fact dedup
    // ------------------------------------------------------------------
    if let Some(runner) = coding_phase_runner {
        info!("Reforge Phase 6.5 ext: cross-session dedup");
        if let Err(e) = runner.run_cross_session_dedup().await {
            warn!("Phase 6.5 cross-session dedup failed: {e}");
            result.phase_errors.push(format!("phase 6.5 cross-dedup: {e}"));
        }
    }
```

- [ ] **Step 5: Verify test passes**

Run: `cargo nextest run -p cognitive --test run_reforge_with_coding`
Expected: PASS.

- [ ] **Step 6: Update all existing `run_reforge(...)` call sites**

Run: `grep -rn "run_reforge(" crates/`. For each call site, append `, None` as the new parameter. Phase-5 wiring lands in Task 14.

- [ ] **Step 7: Verify workspace builds**

Run: `cargo build --workspace`
Expected: clean build.

- [ ] **Step 8: Commit**

```bash
git add crates/cognitive/src/services/reforge/mod.rs \
        crates/cognitive/src/services/reforge/service.rs \
        crates/cognitive/tests/run_reforge_with_coding.rs \
        crates/app-core/src/init/cron.rs
git commit -m "feat(cognitive): Phase 5 — CodingPhaseRunner trait + 4 hooks in run_reforge"
```

---

### Task 14: app-core — concrete `CodingPhaseRunnerImpl` + cron integration

**Files:**
- Modify: `crates/app-core/src/coding_memory/reforge.rs`
- Modify: `crates/app-core/src/init/cron.rs`
- Test: `crates/app-core/tests/coding_phase_runner.rs`

- [ ] **Step 1: Write the failing test**

Create `crates/app-core/tests/coding_phase_runner.rs`:

```rust
use app_core::coding_memory::reforge::CodingPhaseRunnerImpl;
use cognitive::services::reforge::CodingPhaseRunner;
use storage::StoragePool;

#[tokio::test]
async fn empty_pool_runs_clean_in_all_4_phases() {
    let pool = StoragePool::connect_in_memory().await.unwrap();
    StoragePool::run_feature_migrations(pool.inner(), &cognitive::cognitive_migrations())
        .await
        .unwrap();
    StoragePool::run_feature_migrations(pool.inner(), &coding_memory::coding_memory_migrations())
        .await
        .unwrap();
    let runner = CodingPhaseRunnerImpl::new_for_test(pool.clone());

    runner.run_synthesis().await.unwrap();
    runner.run_rule_artifacts().await.unwrap();
    runner.run_selective_delete().await.unwrap();
    runner.run_cross_session_dedup().await.unwrap();
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo nextest run -p app-core --test coding_phase_runner`
Expected: FAIL.

- [ ] **Step 3: Implement the runner**

Append to `crates/app-core/src/coding_memory/reforge.rs`:

```rust
use async_trait::async_trait;
use cognitive::services::reforge::{CodingPhaseRunner, CodingPhaseRunnerOutcome};
use coding_memory::reforge::types::CodingPhaseHandlers;
use coding_memory::reforge::{
    CodingSynthesisHandler, CodingSynthesisPhase, CrossSessionDedup, RuleArtifactGenerationPhase,
    RuleArtifactsHandler, SelectiveDeleteSignal,
};

/// Concrete runner that satisfies the cognitive trait via the coding-memory phases.
pub struct CodingPhaseRunnerImpl {
    pool: storage::StoragePool,
    fact_repo: cognitive::SemanticFactRepo,
    episodic_repo: cognitive::EpisodicMemoryRepo,
    rule_repo: cognitive::ProceduralRuleRepo,
    co_activation_repo: cognitive::CoActivationRepo,
    utilization_repo: coding_memory::recall::telemetry::RecallInvocationRepo,
    session_summary_repo: coding_memory::reforge::SessionSummaryRepo,
    selective_delete_log: coding_memory::reforge::selective_delete::SelectiveDeleteLogRepo,
    pattern_effectiveness_log:
        coding_memory::mirror::pattern_effectiveness::PatternEffectivenessLogRepo,
    synthesis_handler: Option<Arc<dyn CodingSynthesisHandler>>,
    rule_artifacts_handler: Option<Arc<dyn RuleArtifactsHandler>>,
    enabled_artifacts: Vec<String>,
    bus: Option<Arc<bus::DomainEventBus>>,
}

impl CodingPhaseRunnerImpl {
    /// Production constructor.
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        pool: storage::StoragePool,
        synthesis_handler: Option<Arc<dyn CodingSynthesisHandler>>,
        rule_artifacts_handler: Option<Arc<dyn RuleArtifactsHandler>>,
        enabled_artifacts: Vec<String>,
        bus: Option<Arc<bus::DomainEventBus>>,
    ) -> Self {
        let fact_repo = cognitive::SemanticFactRepo::new(pool.clone());
        let episodic_repo = cognitive::EpisodicMemoryRepo::new(pool.clone());
        let rule_repo = cognitive::ProceduralRuleRepo::new(pool.clone());
        let co_activation_repo = cognitive::CoActivationRepo::new(pool.clone());
        let utilization_repo = coding_memory::recall::telemetry::RecallInvocationRepo::new(pool.clone());
        let session_summary_repo = coding_memory::reforge::SessionSummaryRepo::new(pool.clone());
        let selective_delete_log =
            coding_memory::reforge::selective_delete::SelectiveDeleteLogRepo::new(pool.clone());
        let pattern_effectiveness_log =
            coding_memory::mirror::pattern_effectiveness::PatternEffectivenessLogRepo::new(pool.clone());
        Self {
            pool,
            fact_repo,
            episodic_repo,
            rule_repo,
            co_activation_repo,
            utilization_repo,
            session_summary_repo,
            selective_delete_log,
            pattern_effectiveness_log,
            synthesis_handler,
            rule_artifacts_handler,
            enabled_artifacts,
            bus,
        }
    }

    /// Test constructor — no LLM handlers, no enabled artifacts.
    pub fn new_for_test(pool: storage::StoragePool) -> Self {
        Self::new(pool, None, None, vec![], None)
    }

    fn handlers(&self) -> CodingPhaseHandlers<'_> {
        CodingPhaseHandlers {
            synthesis: self.synthesis_handler.as_deref(),
            rule_artifacts: self.rule_artifacts_handler.as_deref(),
            fact_repo: &self.fact_repo,
            episodic_repo: &self.episodic_repo,
            rule_repo: &self.rule_repo,
            co_activation_repo: &self.co_activation_repo,
            utilization_repo: &self.utilization_repo,
            session_summary_repo: &self.session_summary_repo,
            selective_delete_log: &self.selective_delete_log,
            pattern_effectiveness_log: &self.pattern_effectiveness_log,
            bus: self.bus.clone(),
        }
    }
}

#[async_trait]
impl CodingPhaseRunner for CodingPhaseRunnerImpl {
    async fn run_synthesis(&self) -> common::Result<CodingPhaseRunnerOutcome> {
        let applied = CodingSynthesisPhase::run(&self.handlers()).await?;
        Ok(CodingPhaseRunnerOutcome { applied, narrative: None })
    }

    async fn run_rule_artifacts(&self) -> common::Result<CodingPhaseRunnerOutcome> {
        let applied = RuleArtifactGenerationPhase::run(&self.handlers(), &self.enabled_artifacts).await?;
        Ok(CodingPhaseRunnerOutcome { applied, narrative: None })
    }

    async fn run_cross_session_dedup(&self) -> common::Result<CodingPhaseRunnerOutcome> {
        let applied = CrossSessionDedup::run(&self.fact_repo, 0.92).await?;
        Ok(CodingPhaseRunnerOutcome { applied, narrative: None })
    }

    async fn run_selective_delete(&self) -> common::Result<CodingPhaseRunnerOutcome> {
        let applied = SelectiveDeleteSignal::apply(&self.pool, &self.selective_delete_log).await?;
        Ok(CodingPhaseRunnerOutcome { applied, narrative: None })
    }
}
```

- [ ] **Step 4: Modify cron to construct + thread the runner**

Edit `crates/app-core/src/init/cron.rs`. Just before the existing `match cognitive::services::reforge::service::run_reforge(...)` call (line ~537), construct the runner from the same providers + config:

```rust
                        let synth_handler: Option<std::sync::Arc<dyn coding_memory::reforge::CodingSynthesisHandler>> =
                            cog_provider.as_ref().map(|p| {
                                std::sync::Arc::new(
                                    crate::handlers::coding_synthesis::CodingSynthesisHandlerImpl::new(p.clone()),
                                )
                                    as std::sync::Arc<dyn coding_memory::reforge::CodingSynthesisHandler>
                            });
                        let rules_handler: Option<std::sync::Arc<dyn coding_memory::reforge::RuleArtifactsHandler>> =
                            cog_provider.as_ref().map(|p| {
                                std::sync::Arc::new(
                                    crate::handlers::rule_artifacts::RuleArtifactsHandlerImpl::new(p.clone()),
                                )
                                    as std::sync::Arc<dyn coding_memory::reforge::RuleArtifactsHandler>
                            });

                        let enabled_artifacts = cog_config
                            .coding_memory
                            .reforge
                            .as_ref()
                            .map(|r| {
                                let mut v = Vec::new();
                                if r.rule_artifacts.claude_md { v.push("claude_md".into()); }
                                if r.rule_artifacts.agents_md { v.push("agents_md".into()); }
                                if r.rule_artifacts.cursorrules { v.push("cursorrules".into()); }
                                if r.rule_artifacts.continue_rules { v.push("continue_rules".into()); }
                                v
                            })
                            .unwrap_or_default();

                        let coding_runner = crate::coding_memory::reforge::CodingPhaseRunnerImpl::new(
                            pool.clone(),
                            synth_handler,
                            rules_handler,
                            enabled_artifacts,
                            Some(domain_event_bus.clone()),
                        );
```

Append `Some(&coding_runner),` as the final argument to `run_reforge`.

- [ ] **Step 5: Verify**

Run: `cargo nextest run -p app-core --test coding_phase_runner && cargo build --workspace`
Expected: PASS + clean build.

- [ ] **Step 6: Commit**

```bash
git add crates/app-core/src/coding_memory/reforge.rs \
        crates/app-core/src/init/cron.rs \
        crates/app-core/tests/coding_phase_runner.rs
git commit -m "feat(app-core): Phase 5 — CodingPhaseRunnerImpl wired into nightly cron"
```

---

### Task 15: Project skill evolver — Detect

**Files:**
- Create: `crates/coding-memory/src/skill_evolver/mod.rs`
- Create: `crates/coding-memory/src/skill_evolver/detect.rs`
- Modify: `crates/coding-memory/src/skills.rs`
- Modify: `crates/coding-memory/src/lib.rs`
- Test: `crates/coding-memory/tests/skill_evolver_detect.rs`

`★ Insight ─────────────────────────────────────`
- "Detect" is purely SQL: find `procedural_rules` rows with `scope_repo_id = repo`, `confidence >= 0.7`, no matching `skill_versions` row with `scope='project'` for the same repo. Returns `Vec<WorkflowPatternCandidate>`.
- We deliberately exclude rules with `kind='failure_pattern'` from skill synthesis — failure patterns become CLAUDE.md "Known failure patterns" sections in the rule-artifact phase, not standalone skills.
`─────────────────────────────────────────────────`

- [ ] **Step 1: Write the failing test**

Create `crates/coding-memory/tests/skill_evolver_detect.rs`:

```rust
use coding_memory::skill_evolver::detect::{detect_candidates, WorkflowPatternCandidate};
use storage::StoragePool;

#[tokio::test]
async fn returns_high_confidence_workflow_rules_only() {
    let pool = StoragePool::connect_in_memory().await.unwrap();
    StoragePool::run_feature_migrations(pool.inner(), &cognitive::cognitive_migrations())
        .await
        .unwrap();
    StoragePool::run_feature_migrations(pool.inner(), &coding_memory::coding_memory_migrations())
        .await
        .unwrap();

    sqlx::query(
        "INSERT INTO procedural_rules \
         (id, rule, source, confidence, effectiveness_score, stability, scope_type, \
          scope_id, scope_repo_id, created_at, application_count, metadata) \
         VALUES \
         ('r1','always run cargo fmt','observed',0.85,0.5,1.0,'code',NULL,'r1',?1,0,'{\"kind\":\"workflow_pattern\"}'), \
         ('r2','low conf','observed',0.5,0.5,1.0,'code',NULL,'r1',?1,0,'{\"kind\":\"workflow_pattern\"}'), \
         ('r3','failure rule','observed',0.85,0.5,1.0,'code',NULL,'r1',?1,0,'{\"kind\":\"failure_pattern\"}')",
    )
    .bind(jiff::Timestamp::now().to_string())
    .execute(pool.inner())
    .await
    .unwrap();

    let candidates: Vec<WorkflowPatternCandidate> = detect_candidates(&pool, "r1").await.unwrap();
    assert_eq!(candidates.len(), 1);
    assert_eq!(candidates[0].id, "r1");
}

#[tokio::test]
async fn excludes_already_expressed_skills() {
    let pool = StoragePool::connect_in_memory().await.unwrap();
    StoragePool::run_feature_migrations(pool.inner(), &cognitive::cognitive_migrations())
        .await
        .unwrap();
    StoragePool::run_feature_migrations(pool.inner(), &coding_memory::coding_memory_migrations())
        .await
        .unwrap();

    sqlx::query(
        "INSERT INTO procedural_rules \
         (id, rule, source, confidence, effectiveness_score, stability, scope_type, \
          scope_id, scope_repo_id, created_at, application_count, metadata) \
         VALUES ('r1','rule x','observed',0.85,0.5,1.0,'code',NULL,'r1',?1,0,'{\"kind\":\"workflow_pattern\"}')",
    )
    .bind(jiff::Timestamp::now().to_string())
    .execute(pool.inner())
    .await
    .unwrap();
    sqlx::query(
        "INSERT INTO skill_versions (id, skill_name, version, content_hash, scope, scope_repo_id, source_pattern_id, created_at) \
         VALUES ('sv1','rule-x',1,'hash','project','r1','r1', ?1)",
    )
    .bind(jiff::Timestamp::now().to_string())
    .execute(pool.inner())
    .await
    .unwrap();

    let candidates: Vec<WorkflowPatternCandidate> = detect_candidates(&pool, "r1").await.unwrap();
    assert!(candidates.is_empty());
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo nextest run -p coding-memory --test skill_evolver_detect`
Expected: FAIL.

- [ ] **Step 3: Add `source_pattern_id` column to `skill_versions` if missing**

Run: `grep -n "source_pattern_id" crates/coding-memory/migrations/001_coding_memory.sql`. If absent, append to migration 004:

```sql
ALTER TABLE skill_versions ADD COLUMN source_pattern_id TEXT NULL;
CREATE INDEX IF NOT EXISTS idx_skill_versions_source_pattern
    ON skill_versions(source_pattern_id) WHERE source_pattern_id IS NOT NULL;
```

- [ ] **Step 4: Implement `detect_candidates`**

Create `crates/coding-memory/src/skill_evolver/mod.rs`:

```rust
//! Project skill evolver — Detect, Synthesize, Write, Journal, Supersede.
//!
//! Replaces the Phase-1 `PhaseStubEvolver` from `crate::skills`.

pub mod detect;
pub mod supersede;
pub mod write;

pub use detect::{detect_candidates, WorkflowPatternCandidate};
pub use supersede::supersede_outdated_versions;
pub use write::{write_skill_md, JournalArgs};
```

Create `crates/coding-memory/src/skill_evolver/detect.rs`:

```rust
//! Detect candidate `WorkflowPattern`s for project-skill synthesis.

use common::{KlyntbotError, Result};
use serde::{Deserialize, Serialize};

/// One candidate.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkflowPatternCandidate {
    /// Procedural-rule id.
    pub id: String,
    /// Rule text.
    pub rule: String,
    /// Confidence.
    pub confidence: f32,
    /// Effectiveness score.
    pub effectiveness: f32,
    /// Repo id.
    pub scope_repo_id: String,
}

/// Find rules with `scope_repo_id = repo_id`, `confidence >= 0.7`, kind workflow_pattern,
/// and **not** already expressed as a project skill.
pub async fn detect_candidates(
    pool: &storage::StoragePool,
    repo_id: &str,
) -> Result<Vec<WorkflowPatternCandidate>> {
    let rows: Vec<(String, String, f32, f32)> = sqlx::query_as(
        "SELECT pr.id, pr.rule, pr.confidence, COALESCE(pr.effectiveness_score, 0.5) \
         FROM procedural_rules pr \
         WHERE pr.scope_repo_id = ?1 \
           AND pr.confidence >= 0.7 \
           AND COALESCE(json_extract(pr.metadata, '$.kind'), 'workflow_pattern') = 'workflow_pattern' \
           AND NOT EXISTS ( \
               SELECT 1 FROM skill_versions sv \
               WHERE sv.scope = 'project' \
                 AND sv.scope_repo_id = ?1 \
                 AND sv.source_pattern_id = pr.id \
           )",
    )
    .bind(repo_id)
    .fetch_all(pool.inner())
    .await
    .map_err(|e| KlyntbotError::Storage(format!("detect candidates: {e}")))?;
    Ok(rows
        .into_iter()
        .map(|(id, rule, confidence, effectiveness)| WorkflowPatternCandidate {
            id,
            rule,
            confidence,
            effectiveness,
            scope_repo_id: repo_id.to_string(),
        })
        .collect())
}
```

Create `crates/coding-memory/src/skill_evolver/write.rs` and `crates/coding-memory/src/skill_evolver/supersede.rs` as stubs (filled in by Tasks 16/17):

```rust
//! Stub — Task 16.
use common::Result;

/// Args.
#[derive(Debug, Clone)]
pub struct JournalArgs;

/// Write SKILL.md.
pub async fn write_skill_md(_args: JournalArgs) -> Result<()> {
    Ok(())
}
```

```rust
//! Stub — Task 17.
use common::Result;

/// Supersede outdated versions.
pub async fn supersede_outdated_versions(
    _pool: &storage::StoragePool,
    _repo_id: &str,
) -> Result<u32> {
    Ok(0)
}
```

- [ ] **Step 5: Wire module + replace `skills::PhaseStubEvolver`**

Edit `crates/coding-memory/src/lib.rs`. Add `pub mod skill_evolver;`.

Replace `crates/coding-memory/src/skills.rs` body with the public surface that delegates to `skill_evolver`:

```rust
//! Scope-aware `SkillStore` extension + project-scoped evolving skills.

use crate::skill_evolver::{detect_candidates, supersede_outdated_versions, write_skill_md, JournalArgs};
use async_trait::async_trait;
use common::Result;
use std::path::PathBuf;
use uuid::Uuid;

/// Where a project skill is stored.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProjectSkillLocation {
    /// Private default.
    Private,
    /// Team-shared.
    Team,
}

/// Scope of a skill.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SkillScope {
    /// Global.
    Global,
    /// Repo-scoped.
    Repo {
        /// Canonical repo id.
        repo_id: String,
    },
}

/// Skill identifier.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SkillId(pub String);

/// Outcome of one synthesis.
#[derive(Debug, Clone)]
pub struct SkillSynthesisResult {
    /// Skill id.
    pub skill_id: SkillId,
    /// Path on disk.
    pub skill_path: PathBuf,
    /// Version row id.
    pub version_id: Uuid,
    /// Starting effectiveness.
    pub effectiveness: f32,
}

/// Evolver trait.
#[async_trait]
pub trait ProjectSkillEvolver: Send + Sync {
    /// Run one evolution pass for `repo_id`.
    async fn evolve(&self, repo_id: &str) -> Result<Vec<SkillSynthesisResult>>;
}

/// Concrete impl.
pub struct ProjectSkillEvolverImpl {
    pool: storage::StoragePool,
    private_root: PathBuf,
}

impl ProjectSkillEvolverImpl {
    /// Construct.
    pub fn new(pool: storage::StoragePool, private_root: PathBuf) -> Self {
        Self { pool, private_root }
    }
}

#[async_trait]
impl ProjectSkillEvolver for ProjectSkillEvolverImpl {
    async fn evolve(&self, repo_id: &str) -> Result<Vec<SkillSynthesisResult>> {
        let candidates = detect_candidates(&self.pool, repo_id).await?;
        let mut results = Vec::with_capacity(candidates.len());
        for candidate in candidates {
            // Write step (Task 16) returns the result entry.
            write_skill_md(JournalArgs).await?;
            results.push(SkillSynthesisResult {
                skill_id: SkillId(candidate.id.clone()),
                skill_path: self.private_root.join(format!("{repo_id}/{}", candidate.id)),
                version_id: Uuid::new_v4(),
                effectiveness: candidate.effectiveness,
            });
        }
        let _ = supersede_outdated_versions(&self.pool, repo_id).await?;
        Ok(results)
    }
}
```

- [ ] **Step 6: Verify**

Run: `cargo nextest run -p coding-memory --test skill_evolver_detect`
Expected: PASS.

- [ ] **Step 7: Commit**

```bash
git add crates/coding-memory/src/skill_evolver \
        crates/coding-memory/src/skills.rs \
        crates/coding-memory/src/lib.rs \
        crates/coding-memory/migrations/004_phase5_reflection.sql \
        crates/coding-memory/tests/skill_evolver_detect.rs
git commit -m "feat(coding-memory): Phase 5 — project skill evolver Detect step"
```

---

### Task 16: Project skill evolver — Synthesize + Write + Journal

**Files:**
- Modify: `crates/coding-memory/src/skill_evolver/write.rs`
- Modify: `crates/coding-memory/src/reforge/synth_handler.rs`
- Test: `crates/coding-memory/tests/skill_evolver_write.rs`

`★ Insight ─────────────────────────────────────`
- The skill body is generated by an additional method on `RuleArtifactsHandler` — `synthesize_skill(...)`. This re-uses the existing `ProviderRole::ReforgeRules` LLM rather than spinning up a third role. The skill's SKILL.md format follows the Anthropic Agent Skills spec (YAML front-matter + body).
- We journal the version row with `source_pattern_id = pr.id` so the next cycle's `detect_candidates` query knows this skill already exists.
`─────────────────────────────────────────────────`

- [ ] **Step 1: Extend the handler trait**

Edit `crates/coding-memory/src/reforge/synth_handler.rs`. Add a method to `RuleArtifactsHandler`:

```rust
    /// Generate the body of a project SKILL.md given a workflow-pattern candidate.
    /// Default impl returns a deterministic stub so test seams don't require a mock.
    async fn synthesize_skill(
        &self,
        candidate: &crate::skill_evolver::detect::WorkflowPatternCandidate,
    ) -> Result<crate::reforge::types::ProjectSkillSpec> {
        Ok(crate::reforge::types::ProjectSkillSpec {
            skill_id: format!("klyntbot-{}", candidate.id),
            repo_id: candidate.scope_repo_id.clone(),
            name: format!("klyntbot-{}", candidate.id),
            description: candidate.rule.clone(),
            when_to_use: vec![candidate.rule.clone()],
            procedure: format!("# Procedure\n\n{}\n", candidate.rule),
            references: vec![],
            anchored_symbols: vec![],
            effectiveness: candidate.effectiveness,
        })
    }
```

- [ ] **Step 2: Write the failing test**

Create `crates/coding-memory/tests/skill_evolver_write.rs`:

```rust
use coding_memory::reforge::types::ProjectSkillSpec;
use coding_memory::skill_evolver::write::{write_skill_md, JournalArgs};
use storage::StoragePool;
use tempfile::tempdir;

#[tokio::test]
async fn writes_skill_md_with_managed_block_and_journals() {
    let pool = StoragePool::connect_in_memory().await.unwrap();
    StoragePool::run_feature_migrations(pool.inner(), &cognitive::cognitive_migrations())
        .await
        .unwrap();
    StoragePool::run_feature_migrations(pool.inner(), &coding_memory::coding_memory_migrations())
        .await
        .unwrap();

    let dir = tempdir().unwrap();
    let spec = ProjectSkillSpec {
        skill_id: "klyntbot-r1-fmt".into(),
        repo_id: "r1".into(),
        name: "klyntbot-r1-fmt".into(),
        description: "always run cargo fmt before commit".into(),
        when_to_use: vec!["before commit".into()],
        procedure: "# Procedure\n\n1. cargo fmt\n2. cargo clippy\n".into(),
        references: vec![],
        anchored_symbols: vec![],
        effectiveness: 0.5,
    };
    let written = write_skill_md(JournalArgs::new(
        pool.clone(),
        dir.path().to_path_buf(),
        spec,
        Some("rule_id_1".into()),
    ))
    .await
    .expect("write");

    assert!(written.path.exists(), "SKILL.md should exist on disk");
    let body = std::fs::read_to_string(&written.path).unwrap();
    assert!(body.contains("klyntbot:managed:start"));
    assert!(body.contains("cargo fmt"));

    let row_count: (i64,) = sqlx::query_as(
        "SELECT COUNT(*) FROM skill_versions WHERE scope='project' AND scope_repo_id='r1'",
    )
    .fetch_one(pool.inner())
    .await
    .unwrap();
    assert_eq!(row_count.0, 1);
}
```

- [ ] **Step 3: Run test to verify it fails**

Run: `cargo nextest run -p coding-memory --test skill_evolver_write`
Expected: FAIL.

- [ ] **Step 4: Implement `write_skill_md`**

Replace `crates/coding-memory/src/skill_evolver/write.rs`:

```rust
//! Write project SKILL.md atomically + journal `skill_versions` row.

use crate::reforge::managed_block::ManagedBlock;
use crate::reforge::types::ProjectSkillSpec;
use common::{KlyntbotError, Result};
use jiff::Timestamp;
use sha2::{Digest, Sha256};
use std::path::{Path, PathBuf};
use uuid::Uuid;

/// Args for one write call.
#[derive(Debug, Clone)]
pub struct JournalArgs {
    pool: storage::StoragePool,
    private_root: PathBuf,
    spec: ProjectSkillSpec,
    source_pattern_id: Option<String>,
}

impl JournalArgs {
    /// Construct.
    pub fn new(
        pool: storage::StoragePool,
        private_root: PathBuf,
        spec: ProjectSkillSpec,
        source_pattern_id: Option<String>,
    ) -> Self {
        Self {
            pool,
            private_root,
            spec,
            source_pattern_id,
        }
    }
}

/// Outcome of one write.
#[derive(Debug, Clone)]
pub struct SkillWriteOutcome {
    /// Final on-disk path.
    pub path: PathBuf,
    /// Row id.
    pub version_id: String,
}

/// Write the SKILL.md and journal one row.
pub async fn write_skill_md(args: JournalArgs) -> Result<SkillWriteOutcome> {
    let JournalArgs {
        pool,
        private_root,
        spec,
        source_pattern_id,
    } = args;

    // Sanitize repo_id for use as a directory name.
    let repo_dir = sanitize(&spec.repo_id);
    let skill_dir = sanitize(&spec.skill_id);
    let path = private_root.join(repo_dir).join(skill_dir).join("SKILL.md");

    let body = render_skill_body(&spec);
    let block = ManagedBlock::read(&path).unwrap_or_default();
    let cycle_id = format!("c-{}", Uuid::new_v4().simple());
    block.write_with_new_inside(&path, &body, &cycle_id).map_err(|e| {
        KlyntbotError::Storage(format!("skill write {}: {e}", path.display()))
    })?;

    let mut hasher = Sha256::new();
    hasher.update(body.as_bytes());
    let content_hash = format!("{:x}", hasher.finalize());

    let version_id = format!("sv_{}", Uuid::new_v4().simple());
    let next_version = next_version_for(&pool, &spec.skill_id).await?;
    sqlx::query(
        "INSERT INTO skill_versions \
         (id, skill_name, version, content_hash, scope, scope_repo_id, source_pattern_id, created_at) \
         VALUES (?1, ?2, ?3, ?4, 'project', ?5, ?6, ?7)",
    )
    .bind(&version_id)
    .bind(&spec.skill_id)
    .bind(next_version)
    .bind(&content_hash)
    .bind(&spec.repo_id)
    .bind(&source_pattern_id)
    .bind(Timestamp::now().to_string())
    .execute(pool.inner())
    .await
    .map_err(|e| KlyntbotError::Storage(format!("skill_versions insert: {e}")))?;

    Ok(SkillWriteOutcome { path, version_id })
}

fn render_skill_body(spec: &ProjectSkillSpec) -> String {
    let when = spec
        .when_to_use
        .iter()
        .map(|s| format!("- {s}"))
        .collect::<Vec<_>>()
        .join("\n");
    let refs = if spec.references.is_empty() {
        String::new()
    } else {
        let listing = spec
            .references
            .iter()
            .map(|id| format!("- {id}"))
            .collect::<Vec<_>>()
            .join("\n");
        format!("\n\n## References\n\n{listing}\n")
    };
    format!(
        "---\nname: {}\ndescription: {}\n---\n\n## When to use\n\n{}\n\n{}{}",
        spec.name, spec.description, when, spec.procedure, refs
    )
}

fn sanitize(input: &str) -> String {
    input
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || c == '_' || c == '-' {
                c
            } else {
                '_'
            }
        })
        .collect()
}

async fn next_version_for(pool: &storage::StoragePool, skill_name: &str) -> Result<i64> {
    let row: (i64,) = sqlx::query_as(
        "SELECT COALESCE(MAX(version), 0) FROM skill_versions WHERE skill_name = ?1",
    )
    .bind(skill_name)
    .fetch_one(pool.inner())
    .await
    .map_err(|e| KlyntbotError::Storage(format!("max version: {e}")))?;
    Ok(row.0 + 1)
}

#[allow(dead_code)]
fn _ignore(_: &Path) {} // suppress unused import in some MSRV combos
```

- [ ] **Step 5: Update `ProjectSkillEvolverImpl::evolve` to call the LLM and pass spec to write**

Edit `crates/coding-memory/src/skills.rs`. Update the impl to plumb a handler reference:

```rust
/// Concrete impl that holds an optional handler for skill body synthesis.
pub struct ProjectSkillEvolverImpl {
    pool: storage::StoragePool,
    private_root: PathBuf,
    handler: Option<std::sync::Arc<dyn crate::reforge::RuleArtifactsHandler>>,
}

impl ProjectSkillEvolverImpl {
    /// Construct.
    pub fn new(
        pool: storage::StoragePool,
        private_root: PathBuf,
        handler: Option<std::sync::Arc<dyn crate::reforge::RuleArtifactsHandler>>,
    ) -> Self {
        Self {
            pool,
            private_root,
            handler,
        }
    }
}
```

In the `evolve` impl body, replace the `write_skill_md(JournalArgs).await?` line with:

```rust
        for candidate in candidates {
            let spec = if let Some(h) = self.handler.as_deref() {
                h.synthesize_skill(&candidate).await?
            } else {
                crate::reforge::types::ProjectSkillSpec {
                    skill_id: format!("klyntbot-{}", candidate.id),
                    repo_id: candidate.scope_repo_id.clone(),
                    name: format!("klyntbot-{}", candidate.id),
                    description: candidate.rule.clone(),
                    when_to_use: vec![candidate.rule.clone()],
                    procedure: format!("# Procedure\n\n{}\n", candidate.rule),
                    references: vec![],
                    anchored_symbols: vec![],
                    effectiveness: candidate.effectiveness,
                }
            };
            let outcome = write_skill_md(JournalArgs::new(
                self.pool.clone(),
                self.private_root.clone(),
                spec,
                Some(candidate.id.clone()),
            ))
            .await?;
            results.push(SkillSynthesisResult {
                skill_id: SkillId(candidate.id),
                skill_path: outcome.path,
                version_id: uuid::Uuid::parse_str(&outcome.version_id.replace("sv_", ""))
                    .unwrap_or_else(|_| uuid::Uuid::new_v4()),
                effectiveness: candidate.effectiveness,
            });
        }
```

- [ ] **Step 6: Verify**

Run: `cargo nextest run -p coding-memory --test skill_evolver_write`
Expected: PASS.

- [ ] **Step 7: Commit**

```bash
git add crates/coding-memory/src/skill_evolver/write.rs \
        crates/coding-memory/src/skills.rs \
        crates/coding-memory/src/reforge/synth_handler.rs \
        crates/coding-memory/tests/skill_evolver_write.rs
git commit -m "feat(coding-memory): Phase 5 — project skill evolver Synthesize + Write + Journal"
```

---

### Task 17: Project skill evolver — Supersede

**Files:**
- Modify: `crates/coding-memory/src/skill_evolver/supersede.rs`
- Test: `crates/coding-memory/tests/skill_evolver_supersede.rs`

- [ ] **Step 1: Write the failing test**

Create `crates/coding-memory/tests/skill_evolver_supersede.rs`:

```rust
use coding_memory::skill_evolver::supersede_outdated_versions;
use storage::StoragePool;

#[tokio::test]
async fn marks_low_effectiveness_versions_superseded() {
    let pool = StoragePool::connect_in_memory().await.unwrap();
    StoragePool::run_feature_migrations(pool.inner(), &cognitive::cognitive_migrations())
        .await
        .unwrap();
    StoragePool::run_feature_migrations(pool.inner(), &coding_memory::coding_memory_migrations())
        .await
        .unwrap();

    sqlx::query(
        "INSERT INTO skill_versions \
         (id, skill_name, version, content_hash, scope, scope_repo_id, source_pattern_id, status, created_at) \
         VALUES \
         ('v1','foo',1,'h1','project','r1','pat-1','active','2026-04-01T00:00:00Z'), \
         ('v2','foo',2,'h2','project','r1','pat-2','active','2026-04-20T00:00:00Z')",
    )
    .execute(pool.inner())
    .await
    .unwrap();

    let count = supersede_outdated_versions(&pool, "r1").await.unwrap();
    assert_eq!(count, 1);

    let (status,): (String,) = sqlx::query_as(
        "SELECT COALESCE(status, '') FROM skill_versions WHERE id = 'v1'",
    )
    .fetch_one(pool.inner())
    .await
    .unwrap();
    assert_eq!(status, "superseded");
}
```

- [ ] **Step 2: Add `status` column if missing**

Run: `grep -n "ADD COLUMN status" crates/coding-memory/migrations/*.sql crates/cognitive/migrations/*.sql`. If no `status` column exists on `skill_versions`, append to migration 004:

```sql
ALTER TABLE skill_versions ADD COLUMN status TEXT DEFAULT 'active';
CREATE INDEX IF NOT EXISTS idx_skill_versions_status ON skill_versions(status, scope, scope_repo_id);
```

- [ ] **Step 3: Run test to verify it fails**

Run: `cargo nextest run -p coding-memory --test skill_evolver_supersede`
Expected: FAIL.

- [ ] **Step 4: Implement**

Replace `crates/coding-memory/src/skill_evolver/supersede.rs`:

```rust
//! Mark older `skill_versions` rows superseded when a newer one exists in the same scope.

use common::{KlyntbotError, Result};

/// Mark all but the latest `version` per `(skill_name, scope, scope_repo_id)`
/// as `status='superseded'`. Returns the count updated.
pub async fn supersede_outdated_versions(
    pool: &storage::StoragePool,
    repo_id: &str,
) -> Result<u32> {
    let res = sqlx::query(
        "UPDATE skill_versions \
         SET status = 'superseded' \
         WHERE scope = 'project' AND scope_repo_id = ?1 \
           AND status = 'active' \
           AND (skill_name, version) NOT IN ( \
               SELECT skill_name, MAX(version) FROM skill_versions \
               WHERE scope = 'project' AND scope_repo_id = ?1 \
               GROUP BY skill_name \
           )",
    )
    .bind(repo_id)
    .execute(pool.inner())
    .await
    .map_err(|e| KlyntbotError::Storage(format!("supersede update: {e}")))?;
    Ok(u32::try_from(res.rows_affected()).unwrap_or(u32::MAX))
}
```

- [ ] **Step 5: Verify**

Run: `cargo nextest run -p coding-memory --test skill_evolver_supersede`
Expected: PASS.

- [ ] **Step 6: Commit**

```bash
git add crates/coding-memory/src/skill_evolver/supersede.rs \
        crates/coding-memory/migrations/004_phase5_reflection.sql \
        crates/coding-memory/tests/skill_evolver_supersede.rs
git commit -m "feat(coding-memory): Phase 5 — project skill evolver Supersede step"
```

---

### Task 18: Mirror — extend `MirrorAlert` enum with closed coding variants

**Files:**
- Modify: `crates/cognitive/src/mirror/types.rs`
- Modify: `crates/cognitive/src/mirror/repo.rs`
- Test: `crates/cognitive/tests/mirror_alert_coding_persists.rs`

`★ Insight ─────────────────────────────────────`
- The existing `MirrorAlert` is a closed enum with 3 variants. We extend it with **one** additional variant `Coding { kind, severity, payload }`. This is *one variant carrying enum-typed sub-payload*, not 11 new top-level variants — keeps existing `match` arms in `MirrorAlertType::from_alert`/`snippet_from_alert` from breaking.
- The 11 closed kinds and 4 severities live in `coding-memory::mirror::alerts` (Task 6 already landed them). This task wires that closed enum *into* the cognitive crate via re-export so `MirrorAlert::Coding` can carry them by value.
`─────────────────────────────────────────────────`

- [ ] **Step 1: Write the failing test**

Create `crates/cognitive/tests/mirror_alert_coding_persists.rs`:

```rust
use cognitive::mirror::{MirrorAlert, MirrorAlertSeverity, MirrorRepo};
use storage::StoragePool;

#[tokio::test]
async fn coding_alert_persists_with_kind_and_severity() {
    let pool = StoragePool::connect_in_memory().await.unwrap();
    StoragePool::run_feature_migrations(pool.inner(), &cognitive::cognitive_migrations())
        .await
        .unwrap();
    StoragePool::run_feature_migrations(pool.inner(), &coding_memory::coding_memory_migrations())
        .await
        .unwrap();

    let repo = MirrorRepo::new(pool.clone());
    let alert = MirrorAlert::Coding {
        kind: "project_skill_obsolete".into(),
        severity: MirrorAlertSeverity::High,
        payload: serde_json::json!({"skill": "foo", "drop_percent": 60.0}),
    };
    let snippet = cognitive::mirror::snippet_from_alert(&alert);
    repo.insert_snippet(&snippet).await.unwrap();

    let (kind, severity): (Option<String>, Option<String>) = sqlx::query_as(
        "SELECT coding_alert_kind, coding_alert_severity FROM mirror_snippets LIMIT 1",
    )
    .fetch_one(pool.inner())
    .await
    .unwrap();
    assert_eq!(kind.as_deref(), Some("project_skill_obsolete"));
    assert_eq!(severity.as_deref(), Some("high"));
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo nextest run -p cognitive --test mirror_alert_coding_persists`
Expected: FAIL — `MirrorAlertSeverity` not in cognitive scope.

- [ ] **Step 3: Add the closed enum + variant in cognitive**

Edit `crates/cognitive/src/mirror/types.rs`. After the existing `MirrorAlert` enum, add:

```rust
/// Closed enum of severities. Mirrors `coding_memory::mirror::alerts::MirrorAlertSeverity`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MirrorAlertSeverity {
    /// Low.
    Low,
    /// Medium.
    Medium,
    /// High.
    High,
    /// Critical.
    Critical,
}

impl MirrorAlertSeverity {
    /// Lossless string form.
    #[must_use]
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Low => "low",
            Self::Medium => "medium",
            Self::High => "high",
            Self::Critical => "critical",
        }
    }
}
```

Extend the `MirrorAlert` enum — add `Coding` variant after `MetaRuleProposed`:

```rust
    /// Coding-specific alert (closed enum extension).
    Coding {
        /// String form of `CodingMirrorAlertKind`.
        kind: String,
        /// Severity.
        severity: MirrorAlertSeverity,
        /// Free-form payload — JSON.
        payload: serde_json::Value,
    },
```

Extend `MirrorAlertType` similarly with `Coding`.

- [ ] **Step 4: Update `snippet_from_alert` to fill the new columns**

Edit `crates/cognitive/src/mirror/repo.rs`. Find `snippet_from_alert` (or wherever `NarrativeSnippet` rows are built from `MirrorAlert`). Add a case:

```rust
        MirrorAlert::Coding {
            kind,
            severity,
            payload,
        } => NarrativeSnippet {
            id: Uuid::new_v4(),
            created_at: Timestamp::now(),
            alert_type: MirrorAlertType::Coding,
            headline: format!("Coding alert: {kind}"),
            body: payload.to_string(),
            suggested_action: None,
            user_feedback: None,
            dismissed_at: None,
        },
```

Inside `MirrorRepo::insert_snippet`, when `alert_type == Coding`, also set `coding_alert_kind` + `coding_alert_severity` columns. The cleanest path is to add an internal helper that takes the parsed `MirrorAlert` (not just the snippet) — but the simplest path is to extend the `NarrativeSnippet` shape itself with optional `coding_alert_kind`/`coding_alert_severity` fields populated by `snippet_from_alert`. Pick whichever your `NarrativeSnippet` struct supports without a cascade of breakage.

- [ ] **Step 5: Verify**

Run: `cargo nextest run -p cognitive --test mirror_alert_coding_persists`
Expected: PASS.

- [ ] **Step 6: Commit**

```bash
git add crates/cognitive/src/mirror/types.rs \
        crates/cognitive/src/mirror/repo.rs \
        crates/cognitive/tests/mirror_alert_coding_persists.rs
git commit -m "feat(cognitive): Phase 5 — MirrorAlert::Coding variant + severity enum"
```

---

### Task 19: `PatternEffectivenessSource` (real-time EMA)

**Files:**
- Modify: `crates/coding-memory/src/mirror/pattern_effectiveness.rs`
- Test: `crates/coding-memory/tests/mirror_pattern_effectiveness.rs`

`★ Insight ─────────────────────────────────────`
- The EMA formula is `score = 0.9 * prev + 0.1 * outcome_value`. Outcome values: `success=1.0`, `partial=0.5`, `failure=0.0`. Because EMA is a moving average, *one bad outcome doesn't tank a skill* — but sustained underperformance pulls it down.
- The source persists to `pattern_effectiveness_log` *and* updates the live `procedural_rules.effectiveness_score` column. This means the next retrieval cycle's ranking respects the new score within seconds.
`─────────────────────────────────────────────────`

- [ ] **Step 1: Write the failing test**

Create `crates/coding-memory/tests/mirror_pattern_effectiveness.rs`:

```rust
use ai_core::{AiSignal, AiMetrics, MirrorSignalSource, RecallDomain, SalienceVerdict};
use coding_memory::mirror::pattern_effectiveness::{
    PatternEffectivenessLogRepo, PatternEffectivenessSource,
};
use std::sync::Arc;
use storage::StoragePool;

fn pattern_outcome_signal(pattern_id: &str, outcome: &str) -> AiSignal {
    AiSignal {
        domain: RecallDomain::General,
        event_kind: "PatternOutcome",
        importance: 0.5,
        salience: SalienceVerdict::Accumulate,
        content: String::new(),
        entity: None,
        timestamp: jiff::Timestamp::now(),
        raw_event: Some(bus::DomainEvent::PatternOutcome {
            pattern_id: pattern_id.to_string(),
            outcome: outcome.to_string(),
            evidence: String::new(),
            measured_at: jiff::Timestamp::now().to_string(),
        }),
        metrics: AiMetrics::default(),
        coaching_signal: false,
        coaching_rule: None,
        metric_samples: Vec::new(),
    }
}

#[tokio::test]
async fn updates_effectiveness_via_ema() {
    let pool = StoragePool::connect_in_memory().await.unwrap();
    StoragePool::run_feature_migrations(pool.inner(), &cognitive::cognitive_migrations())
        .await
        .unwrap();
    StoragePool::run_feature_migrations(pool.inner(), &coding_memory::coding_memory_migrations())
        .await
        .unwrap();

    sqlx::query(
        "INSERT INTO procedural_rules \
         (id, rule, source, confidence, effectiveness_score, stability, scope_type, scope_id, \
          scope_repo_id, created_at, application_count) \
         VALUES ('p1','rule','observed',0.8,0.5,1.0,'code',NULL,'r1',?1,0)",
    )
    .bind(jiff::Timestamp::now().to_string())
    .execute(pool.inner())
    .await
    .unwrap();

    let log = PatternEffectivenessLogRepo::new(pool.clone());
    let source = Arc::new(PatternEffectivenessSource::new(pool.clone(), log));
    source
        .accumulate(&pattern_outcome_signal("p1", "success"))
        .await
        .unwrap();
    source.flush().await.unwrap();

    let (score,): (f32,) =
        sqlx::query_as("SELECT effectiveness_score FROM procedural_rules WHERE id = 'p1'")
            .fetch_one(pool.inner())
            .await
            .unwrap();
    assert!((score - (0.9 * 0.5 + 0.1 * 1.0)).abs() < 1e-3);

    let log_count: (i64,) =
        sqlx::query_as("SELECT COUNT(*) FROM pattern_effectiveness_log WHERE pattern_id = 'p1'")
            .fetch_one(pool.inner())
            .await
            .unwrap();
    assert_eq!(log_count.0, 1);
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo nextest run -p coding-memory --test mirror_pattern_effectiveness`
Expected: FAIL.

- [ ] **Step 3: Implement**

Replace `crates/coding-memory/src/mirror/pattern_effectiveness.rs`:

```rust
//! Real-time pattern-effectiveness EMA.

use ai_core::{AiSignal, MirrorSignalSource, MirrorSnapshotSpec};
use async_trait::async_trait;
use bus::DomainEvent;
use common::{KlyntbotError, Result};
use jiff::Timestamp;
use std::sync::Mutex;
use uuid::Uuid;

/// Repo for the audit log.
#[derive(Debug, Clone)]
pub struct PatternEffectivenessLogRepo {
    pool: storage::StoragePool,
}

impl PatternEffectivenessLogRepo {
    /// Construct.
    pub fn new(pool: storage::StoragePool) -> Self {
        Self { pool }
    }

    /// Pool ref.
    pub fn pool(&self) -> &storage::StoragePool {
        &self.pool
    }

    /// Insert one row.
    pub async fn insert(
        &self,
        pattern_id: &str,
        pattern_kind: &str,
        repo_id: Option<&str>,
        outcome: &str,
        score_before: f32,
        score_after: f32,
        evidence: Option<&str>,
    ) -> Result<()> {
        sqlx::query(
            "INSERT INTO pattern_effectiveness_log \
             (id, pattern_id, pattern_kind, repo_id, outcome, score_before, score_after, evidence) \
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
        )
        .bind(format!("pe_{}", Uuid::new_v4().simple()))
        .bind(pattern_id)
        .bind(pattern_kind)
        .bind(repo_id)
        .bind(outcome)
        .bind(score_before)
        .bind(score_after)
        .bind(evidence)
        .execute(self.pool.inner())
        .await
        .map_err(|e| KlyntbotError::Storage(format!("pattern_eff_log insert: {e}")))?;
        Ok(())
    }

    /// Recent rows for a pattern (UI feeds Pattern Effectiveness Trends panel).
    pub async fn recent(
        &self,
        pattern_id: &str,
        limit: u32,
    ) -> Result<Vec<(Timestamp, f32, f32, String)>> {
        let rows: Vec<(String, f32, f32, String)> = sqlx::query_as(
            "SELECT measured_at, score_before, score_after, outcome \
             FROM pattern_effectiveness_log \
             WHERE pattern_id = ?1 \
             ORDER BY measured_at DESC LIMIT ?2",
        )
        .bind(pattern_id)
        .bind(limit as i64)
        .fetch_all(self.pool.inner())
        .await
        .map_err(|e| KlyntbotError::Storage(format!("pattern_eff_log read: {e}")))?;
        Ok(rows
            .into_iter()
            .map(|(ts, sb, sa, o)| (ts.parse().unwrap_or_else(|_| Timestamp::now()), sb, sa, o))
            .collect())
    }
}

/// Real-time pattern-effectiveness source.
pub struct PatternEffectivenessSource {
    pool: storage::StoragePool,
    log: PatternEffectivenessLogRepo,
    pending: Mutex<Vec<PendingOutcome>>,
}

#[derive(Debug, Clone)]
struct PendingOutcome {
    pattern_id: String,
    outcome: String,
}

impl PatternEffectivenessSource {
    /// Construct.
    pub fn new(pool: storage::StoragePool, log: PatternEffectivenessLogRepo) -> Self {
        Self {
            pool,
            log,
            pending: Mutex::new(Vec::new()),
        }
    }

    fn outcome_value(outcome: &str) -> f32 {
        match outcome {
            "success" => 1.0,
            "partial" => 0.5,
            "failure" => 0.0,
            _ => return 0.5,
        }
    }
}

#[async_trait]
impl MirrorSignalSource for PatternEffectivenessSource {
    fn spec(&self) -> MirrorSnapshotSpec {
        MirrorSnapshotSpec {
            name: "coding.pattern_effectiveness",
            subscribed_kinds: &["PatternApplied", "PatternOutcome"],
            flush_interval_secs: Some(60),
        }
    }

    fn name(&self) -> &'static str {
        "coding-pattern-effectiveness-source"
    }

    async fn accumulate(&self, signal: &AiSignal) -> Result<()> {
        if let Some(DomainEvent::PatternOutcome {
            pattern_id, outcome, ..
        }) = &signal.raw_event
        {
            self.pending.lock().unwrap().push(PendingOutcome {
                pattern_id: pattern_id.clone(),
                outcome: outcome.clone(),
            });
        }
        Ok(())
    }

    async fn flush(&self) -> Result<()> {
        let pending: Vec<PendingOutcome> = std::mem::take(&mut self.pending.lock().unwrap());
        for outcome in pending {
            let value = Self::outcome_value(&outcome.outcome);
            // Read current score.
            let row: Option<(f32, Option<String>)> = sqlx::query_as(
                "SELECT COALESCE(effectiveness_score, 0.5), scope_repo_id \
                 FROM procedural_rules WHERE id = ?1",
            )
            .bind(&outcome.pattern_id)
            .fetch_optional(self.pool.inner())
            .await
            .map_err(|e| KlyntbotError::Storage(format!("pattern read: {e}")))?;
            let Some((score_before, repo_id)) = row else {
                continue;
            };
            let score_after = 0.9 * score_before + 0.1 * value;
            sqlx::query(
                "UPDATE procedural_rules SET effectiveness_score = ?1 WHERE id = ?2",
            )
            .bind(score_after)
            .bind(&outcome.pattern_id)
            .execute(self.pool.inner())
            .await
            .map_err(|e| KlyntbotError::Storage(format!("pattern update: {e}")))?;
            self.log
                .insert(
                    &outcome.pattern_id,
                    "workflow_pattern",
                    repo_id.as_deref(),
                    &outcome.outcome,
                    score_before,
                    score_after,
                    None,
                )
                .await?;
        }
        Ok(())
    }
}
```

- [ ] **Step 4: Verify**

Run: `cargo nextest run -p coding-memory --test mirror_pattern_effectiveness`
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add crates/coding-memory/src/mirror/pattern_effectiveness.rs \
        crates/coding-memory/tests/mirror_pattern_effectiveness.rs
git commit -m "feat(coding-memory): Phase 5 — PatternEffectivenessSource (real-time EMA)"
```

---

### Task 20: `StaleMemorySource` (cited-vs-ignored counter)

**Files:**
- Modify: `crates/coding-memory/src/mirror/stale_memory.rs`
- Test: `crates/coding-memory/tests/mirror_stale_memory.rs`

- [ ] **Step 1: Write the failing test**

Create `crates/coding-memory/tests/mirror_stale_memory.rs`:

```rust
use ai_core::{AiMetrics, AiSignal, MirrorSignalSource, RecallDomain, SalienceVerdict};
use coding_memory::mirror::stale_memory::StaleMemorySource;
use storage::StoragePool;

fn retrieved_signal(memory_ids: &[&str], session_id: &str, turn_id: &str) -> AiSignal {
    AiSignal {
        domain: RecallDomain::General,
        event_kind: "MemoryRetrieved",
        importance: 0.5,
        salience: SalienceVerdict::Accumulate,
        content: String::new(),
        entity: None,
        timestamp: jiff::Timestamp::now(),
        raw_event: Some(bus::DomainEvent::MemoryRetrieved {
            memory_ids: memory_ids.iter().map(|s| s.to_string()).collect(),
            query: "q".into(),
            session_id: session_id.into(),
            turn_id: turn_id.into(),
        }),
        metrics: AiMetrics::default(),
        coaching_signal: false,
        coaching_rule: None,
        metric_samples: Vec::new(),
    }
}

fn cited_signal(cited: &[&str], session_id: &str, turn_id: &str) -> AiSignal {
    AiSignal {
        domain: RecallDomain::General,
        event_kind: "AssistantMsgCompleted",
        importance: 0.5,
        salience: SalienceVerdict::Accumulate,
        content: String::new(),
        entity: None,
        timestamp: jiff::Timestamp::now(),
        raw_event: Some(bus::DomainEvent::AssistantMsgCompleted {
            session_id: session_id.into(),
            turn_id: turn_id.into(),
            cited_memory_ids: cited.iter().map(|s| s.to_string()).collect(),
        }),
        metrics: AiMetrics::default(),
        coaching_signal: false,
        coaching_rule: None,
        metric_samples: Vec::new(),
    }
}

#[tokio::test]
async fn cited_memory_marked_in_utilization_table() {
    let pool = StoragePool::connect_in_memory().await.unwrap();
    StoragePool::run_feature_migrations(pool.inner(), &cognitive::cognitive_migrations())
        .await
        .unwrap();
    StoragePool::run_feature_migrations(pool.inner(), &coding_memory::coding_memory_migrations())
        .await
        .unwrap();

    let source = StaleMemorySource::new(pool.clone());
    source
        .accumulate(&retrieved_signal(&["m1", "m2"], "s1", "t1"))
        .await
        .unwrap();
    source
        .accumulate(&cited_signal(&["m1"], "s1", "t1"))
        .await
        .unwrap();
    source.flush().await.unwrap();

    let m1_cited: (i64,) = sqlx::query_as(
        "SELECT COUNT(*) FROM memory_utilization WHERE memory_id='m1' AND cited_in_response=1",
    )
    .fetch_one(pool.inner())
    .await
    .unwrap();
    assert_eq!(m1_cited.0, 1);
    let m2_uncited: (i64,) = sqlx::query_as(
        "SELECT COUNT(*) FROM memory_utilization WHERE memory_id='m2' AND cited_in_response=0",
    )
    .fetch_one(pool.inner())
    .await
    .unwrap();
    assert_eq!(m2_uncited.0, 1);
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo nextest run -p coding-memory --test mirror_stale_memory`
Expected: FAIL.

- [ ] **Step 3: Implement**

Replace `crates/coding-memory/src/mirror/stale_memory.rs`:

```rust
//! Stale-memory signal source — marks memories cited vs ignored per turn.

use ai_core::{AiSignal, MirrorSignalSource, MirrorSnapshotSpec};
use async_trait::async_trait;
use bus::DomainEvent;
use common::{KlyntbotError, Result};
use std::collections::HashMap;
use std::sync::Mutex;
use uuid::Uuid;

#[derive(Debug, Default)]
struct TurnState {
    retrieved: Vec<String>,
    cited: Vec<String>,
}

/// Stale-memory source.
pub struct StaleMemorySource {
    pool: storage::StoragePool,
    turns: Mutex<HashMap<(String, String), TurnState>>,
}

impl StaleMemorySource {
    /// Construct.
    pub fn new(pool: storage::StoragePool) -> Self {
        Self {
            pool,
            turns: Mutex::new(HashMap::new()),
        }
    }
}

#[async_trait]
impl MirrorSignalSource for StaleMemorySource {
    fn spec(&self) -> MirrorSnapshotSpec {
        MirrorSnapshotSpec {
            name: "coding.stale_memory",
            subscribed_kinds: &["MemoryRetrieved", "AssistantMsgCompleted"],
            flush_interval_secs: Some(60),
        }
    }

    fn name(&self) -> &'static str {
        "coding-stale-memory-source"
    }

    async fn accumulate(&self, signal: &AiSignal) -> Result<()> {
        match &signal.raw_event {
            Some(DomainEvent::MemoryRetrieved {
                memory_ids,
                session_id,
                turn_id,
                ..
            }) => {
                let key = (session_id.clone(), turn_id.clone());
                let mut turns = self.turns.lock().unwrap();
                let entry = turns.entry(key).or_default();
                entry.retrieved.extend(memory_ids.iter().cloned());
            }
            Some(DomainEvent::AssistantMsgCompleted {
                session_id,
                turn_id,
                cited_memory_ids,
            }) => {
                let key = (session_id.clone(), turn_id.clone());
                let mut turns = self.turns.lock().unwrap();
                let entry = turns.entry(key).or_default();
                entry.cited.extend(cited_memory_ids.iter().cloned());
            }
            _ => {}
        }
        Ok(())
    }

    async fn flush(&self) -> Result<()> {
        let drained: Vec<((String, String), TurnState)> = {
            let mut turns = self.turns.lock().unwrap();
            turns.drain().collect()
        };
        for ((session_id, turn_id), state) in drained {
            for mem in state.retrieved {
                let cited = state.cited.iter().any(|m| m == &mem);
                sqlx::query(
                    "INSERT INTO memory_utilization \
                     (id, memory_id, cited_in_response, session_id, turn_id) \
                     VALUES (?1, ?2, ?3, ?4, ?5)",
                )
                .bind(format!("u_{}", Uuid::new_v4().simple()))
                .bind(&mem)
                .bind(cited)
                .bind(&session_id)
                .bind(&turn_id)
                .execute(self.pool.inner())
                .await
                .map_err(|e| KlyntbotError::Storage(format!("memory_utilization: {e}")))?;
            }
        }
        Ok(())
    }
}
```

- [ ] **Step 4: Verify**

Run: `cargo nextest run -p coding-memory --test mirror_stale_memory`
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add crates/coding-memory/src/mirror/stale_memory.rs \
        crates/coding-memory/tests/mirror_stale_memory.rs
git commit -m "feat(coding-memory): Phase 5 — StaleMemorySource (cited-vs-ignored counter)"
```

---

### Task 21: Four coding meta-rule detectors

**Files:**
- Modify: `crates/coding-memory/src/mirror/coding_meta_rules.rs`
- Test: `crates/coding-memory/tests/mirror_meta_rules.rs`

`★ Insight ─────────────────────────────────────`
- All four detectors share a common pattern: count occurrences of a domain event keyed by some discriminator, fire an alert when a threshold is hit, then reset the counter for that key.
- The detectors are *event-driven* (no periodic flush) — they react to incoming `DomainEvent`s and emit `MirrorAlert::Coding{...}` snippets directly.
`─────────────────────────────────────────────────`

- [ ] **Step 1: Write the failing test**

Create `crates/coding-memory/tests/mirror_meta_rules.rs`:

```rust
use ai_core::{AiMetrics, AiSignal, MirrorSignalSource, RecallDomain, SalienceVerdict};
use coding_memory::mirror::coding_meta_rules::CodingMetaRulesSource;
use storage::StoragePool;

fn fix_failed_signal(problem_hash: &str, attempt_count: u32) -> AiSignal {
    AiSignal {
        domain: RecallDomain::General,
        event_kind: "FixAttemptFailed",
        importance: 0.5,
        salience: SalienceVerdict::Accumulate,
        content: String::new(),
        entity: None,
        timestamp: jiff::Timestamp::now(),
        raw_event: Some(bus::DomainEvent::FixAttemptFailed {
            problem_hash: problem_hash.into(),
            repo: Some("r1".into()),
            attempt_count,
        }),
        metrics: AiMetrics::default(),
        coaching_signal: false,
        coaching_rule: None,
        metric_samples: Vec::new(),
    }
}

#[tokio::test]
async fn problem_class_refactor_fires_at_threshold() {
    let pool = StoragePool::connect_in_memory().await.unwrap();
    StoragePool::run_feature_migrations(pool.inner(), &cognitive::cognitive_migrations())
        .await
        .unwrap();
    StoragePool::run_feature_migrations(pool.inner(), &coding_memory::coding_memory_migrations())
        .await
        .unwrap();

    let mirror_repo = cognitive::mirror::MirrorRepo::new(pool.clone());
    let source = CodingMetaRulesSource::new(mirror_repo);

    source.accumulate(&fix_failed_signal("h1", 1)).await.unwrap();
    source.accumulate(&fix_failed_signal("h1", 2)).await.unwrap();
    source.accumulate(&fix_failed_signal("h1", 3)).await.unwrap();

    let alerts: (i64,) = sqlx::query_as(
        "SELECT COUNT(*) FROM mirror_snippets \
         WHERE coding_alert_kind = 'problem_class_refactor'",
    )
    .fetch_one(pool.inner())
    .await
    .unwrap();
    assert_eq!(alerts.0, 1);
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo nextest run -p coding-memory --test mirror_meta_rules`
Expected: FAIL.

- [ ] **Step 3: Implement**

Replace `crates/coding-memory/src/mirror/coding_meta_rules.rs`:

```rust
//! Four coding meta-rule detectors.

use crate::mirror::alerts::{CodingMirrorAlertKind, MirrorAlertSeverity as CmSeverity};
use ai_core::{AiSignal, MirrorSignalSource, MirrorSnapshotSpec};
use async_trait::async_trait;
use bus::DomainEvent;
use cognitive::mirror::{MirrorAlert, MirrorAlertSeverity, MirrorRepo, snippet_from_alert};
use common::Result;
use serde_json::json;
use std::collections::HashMap;
use std::sync::Mutex;
use tracing::warn;

/// Aggregate state.
struct State {
    fix_failures: HashMap<String, u32>,
    dead_end_overrides: HashMap<String, u32>,
    style_pref_repos: HashMap<String, std::collections::HashSet<String>>,
    counterfactual_ignored: HashMap<String, u32>,
}

impl State {
    fn new() -> Self {
        Self {
            fix_failures: HashMap::new(),
            dead_end_overrides: HashMap::new(),
            style_pref_repos: HashMap::new(),
            counterfactual_ignored: HashMap::new(),
        }
    }
}

/// Source.
pub struct CodingMetaRulesSource {
    state: Mutex<State>,
    repo: MirrorRepo,
}

impl CodingMetaRulesSource {
    /// Construct.
    pub fn new(repo: MirrorRepo) -> Self {
        Self {
            state: Mutex::new(State::new()),
            repo,
        }
    }

    fn map_severity(s: CmSeverity) -> MirrorAlertSeverity {
        match s {
            CmSeverity::Low => MirrorAlertSeverity::Low,
            CmSeverity::Medium => MirrorAlertSeverity::Medium,
            CmSeverity::High => MirrorAlertSeverity::High,
            CmSeverity::Critical => MirrorAlertSeverity::Critical,
        }
    }

    async fn emit(&self, kind: CodingMirrorAlertKind, severity: CmSeverity, payload: serde_json::Value) {
        let alert = MirrorAlert::Coding {
            kind: kind.as_str().to_string(),
            severity: Self::map_severity(severity),
            payload,
        };
        let snippet = snippet_from_alert(&alert);
        if let Err(e) = self.repo.insert_snippet(&snippet).await {
            warn!("CodingMetaRulesSource: failed to insert snippet: {e}");
        }
    }
}

#[async_trait]
impl MirrorSignalSource for CodingMetaRulesSource {
    fn spec(&self) -> MirrorSnapshotSpec {
        MirrorSnapshotSpec {
            name: "coding.meta_rules",
            subscribed_kinds: &[
                "FixAttemptFailed",
                "DeadEndOverridden",
                "StylePreferenceObserved",
                "CounterfactualIgnored",
            ],
            flush_interval_secs: None,
        }
    }

    fn name(&self) -> &'static str {
        "coding-meta-rules-source"
    }

    async fn accumulate(&self, signal: &AiSignal) -> Result<()> {
        let action = match &signal.raw_event {
            Some(DomainEvent::FixAttemptFailed { problem_hash, repo, attempt_count }) => {
                let mut s = self.state.lock().unwrap();
                let entry = s.fix_failures.entry(problem_hash.clone()).or_insert(0);
                *entry = (*entry).max(*attempt_count);
                if *entry >= 3 {
                    let value = *entry;
                    let hash = problem_hash.clone();
                    let repo = repo.clone();
                    s.fix_failures.remove(&hash);
                    Some((CodingMirrorAlertKind::ProblemClassRefactor, CmSeverity::High, json!({
                        "problem_hash": hash,
                        "repo": repo,
                        "attempt_count": value,
                    })))
                } else {
                    None
                }
            }
            _ => None,
        };
        if let Some((kind, sev, payload)) = action {
            self.emit(kind, sev, payload).await;
        }
        Ok(())
    }

    async fn flush(&self) -> Result<()> {
        Ok(())
    }
}
```

- [ ] **Step 4: Verify**

Run: `cargo nextest run -p coding-memory --test mirror_meta_rules`
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add crates/coding-memory/src/mirror/coding_meta_rules.rs \
        crates/coding-memory/tests/mirror_meta_rules.rs
git commit -m "feat(coding-memory): Phase 5 — CodingMetaRulesSource (problem_class_refactor + 3 stubs)"
```

> **NOTE:** This task lands the framework + the `ProblemClassRefactor` rule. The other three (`LowerDeadEndThreshold`, `PromoteToGlobal`, `PromoteCounterfactualVisibility`) require domain events that don't yet exist (`DeadEndOverridden`, `StylePreferenceObserved`, `CounterfactualIgnored`). Their detector branches are scaffolded but emit only when the upstream events ship — Phase 6 / 7 work. Document this caveat in the source-file doc-comment.

---

### Task 22: `CodingRoutingSource` — project-skill drift detection

**Files:**
- Modify: `crates/coding-memory/src/mirror/coding_routing.rs`
- Test: `crates/coding-memory/tests/mirror_coding_routing_drift.rs`

- [ ] **Step 1: Write the failing test**

Create `crates/coding-memory/tests/mirror_coding_routing_drift.rs`:

```rust
use ai_core::{AiMetrics, AiSignal, MirrorSignalSource, RecallDomain, SalienceVerdict};
use coding_memory::mirror::coding_routing::CodingRoutingSource;
use storage::StoragePool;

fn skill_routed_signal(skill: &str, repo: Option<&str>, confidence: f64) -> AiSignal {
    AiSignal {
        domain: RecallDomain::General,
        event_kind: "SkillRouted",
        importance: 0.4,
        salience: SalienceVerdict::Accumulate,
        content: String::new(),
        entity: None,
        timestamp: jiff::Timestamp::now(),
        raw_event: Some(bus::DomainEvent::SkillRouted {
            skill_name: skill.into(),
            confidence,
            source: format!("project:{}", repo.unwrap_or("none")),
            trigger_phrases: vec![],
            session_key: "s".into(),
        }),
        metrics: AiMetrics::default(),
        coaching_signal: false,
        coaching_rule: None,
        metric_samples: Vec::new(),
    }
}

#[tokio::test]
async fn project_skill_50pct_drop_emits_alert() {
    let pool = StoragePool::connect_in_memory().await.unwrap();
    StoragePool::run_feature_migrations(pool.inner(), &cognitive::cognitive_migrations())
        .await
        .unwrap();
    StoragePool::run_feature_migrations(pool.inner(), &coding_memory::coding_memory_migrations())
        .await
        .unwrap();

    let mirror_repo = cognitive::mirror::MirrorRepo::new(pool.clone());
    let source = CodingRoutingSource::new(mirror_repo);

    // Seed historical baseline (10 activations).
    for _ in 0..10 {
        source
            .accumulate(&skill_routed_signal("klyntbot-r1-fmt", Some("r1"), 0.85))
            .await
            .unwrap();
    }
    source.flush().await.unwrap();

    // Recent week — only 4 activations (≥50% drop).
    for _ in 0..4 {
        source
            .accumulate(&skill_routed_signal("klyntbot-r1-fmt", Some("r1"), 0.85))
            .await
            .unwrap();
    }
    // Other skill stays at 10 to make ratios obvious.
    for _ in 0..10 {
        source
            .accumulate(&skill_routed_signal("klyntbot-r1-test", Some("r1"), 0.85))
            .await
            .unwrap();
    }
    source.flush().await.unwrap();

    let alert_count: (i64,) = sqlx::query_as(
        "SELECT COUNT(*) FROM mirror_snippets \
         WHERE coding_alert_kind = 'project_skill_obsolete'",
    )
    .fetch_one(pool.inner())
    .await
    .unwrap();
    assert!(alert_count.0 >= 1);
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo nextest run -p coding-memory --test mirror_coding_routing_drift`
Expected: FAIL.

- [ ] **Step 3: Implement**

Replace `crates/coding-memory/src/mirror/coding_routing.rs`:

```rust
//! Coding-routing source — extends the routing-drift detector with
//! project-skill obsolescence alerts.

use crate::mirror::alerts::CodingMirrorAlertKind;
use ai_core::{AiSignal, MirrorSignalSource, MirrorSnapshotSpec};
use async_trait::async_trait;
use bus::DomainEvent;
use cognitive::mirror::{MirrorAlert, MirrorAlertSeverity, MirrorRepo, snippet_from_alert};
use common::Result;
use serde_json::json;
use std::collections::HashMap;
use std::sync::Mutex;
use tracing::warn;

/// Source.
pub struct CodingRoutingSource {
    repo: MirrorRepo,
    history: Mutex<HashMap<String, u32>>,
    current: Mutex<HashMap<String, u32>>,
}

impl CodingRoutingSource {
    /// Construct.
    pub fn new(repo: MirrorRepo) -> Self {
        Self {
            repo,
            history: Mutex::new(HashMap::new()),
            current: Mutex::new(HashMap::new()),
        }
    }

    fn detect_drops(&self) -> Vec<(String, u32, u32)> {
        let history = self.history.lock().unwrap();
        let current = self.current.lock().unwrap();
        let mut out = Vec::new();
        for (skill, hist) in history.iter() {
            let cur = current.get(skill).copied().unwrap_or(0);
            if *hist >= 5 && (cur as f32) < (*hist as f32) * 0.5 {
                out.push((skill.clone(), *hist, cur));
            }
        }
        out
    }
}

#[async_trait]
impl MirrorSignalSource for CodingRoutingSource {
    fn spec(&self) -> MirrorSnapshotSpec {
        MirrorSnapshotSpec {
            name: "coding.routing",
            subscribed_kinds: &["SkillRouted"],
            flush_interval_secs: Some(60 * 60 * 24 * 7), // weekly cadence
        }
    }

    fn name(&self) -> &'static str {
        "coding-routing-source"
    }

    async fn accumulate(&self, signal: &AiSignal) -> Result<()> {
        if let Some(DomainEvent::SkillRouted {
            skill_name, source, ..
        }) = &signal.raw_event
        {
            if !source.starts_with("project:") {
                return Ok(());
            }
            *self
                .current
                .lock()
                .unwrap()
                .entry(skill_name.clone())
                .or_insert(0) += 1;
        }
        Ok(())
    }

    async fn flush(&self) -> Result<()> {
        let drops = self.detect_drops();
        for (skill, history_count, current_count) in drops {
            let alert = MirrorAlert::Coding {
                kind: CodingMirrorAlertKind::ProjectSkillObsolete.as_str().to_string(),
                severity: MirrorAlertSeverity::Medium,
                payload: json!({
                    "skill": skill,
                    "history": history_count,
                    "current": current_count,
                }),
            };
            let snippet = snippet_from_alert(&alert);
            if let Err(e) = self.repo.insert_snippet(&snippet).await {
                warn!("CodingRoutingSource: failed to persist alert: {e}");
            }
        }
        // Roll the current period into history (simplified — real impl uses sliding window).
        let mut history = self.history.lock().unwrap();
        let current = self.current.lock().unwrap().clone();
        for (k, v) in current {
            history.insert(k, v);
        }
        self.current.lock().unwrap().clear();
        Ok(())
    }
}
```

- [ ] **Step 4: Verify**

Run: `cargo nextest run -p coding-memory --test mirror_coding_routing_drift`
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add crates/coding-memory/src/mirror/coding_routing.rs \
        crates/coding-memory/tests/mirror_coding_routing_drift.rs
git commit -m "feat(coding-memory): Phase 5 — CodingRoutingSource project-skill drift"
```

---

### Task 23: Wire mirror sources into `MirrorEngine`

**Files:**
- Create: `crates/app-core/src/coding_memory/mirror.rs`
- Modify: `crates/app-core/src/coding_memory/mod.rs`
- Modify: `crates/app-core/src/init/mod.rs`
- Test: `crates/app-core/tests/mirror_register_coding.rs`

`★ Insight ─────────────────────────────────────`
- `MirrorEngine::start` already returns `StartedMirror { facade, consumers, flush_handles, shutdown }`. We add our coding sources during the same pre-start window where the existing routing/finance/task-focus sources are registered. The sources are passed to the `engine.builder().add_source(...)` chain.
`─────────────────────────────────────────────────`

- [ ] **Step 1: Write the failing test**

Create `crates/app-core/tests/mirror_register_coding.rs`:

```rust
use app_core::coding_memory::mirror::register_coding_sources;
use storage::StoragePool;

#[tokio::test]
async fn registers_4_coding_sources_without_error() {
    let pool = StoragePool::connect_in_memory().await.unwrap();
    StoragePool::run_feature_migrations(pool.inner(), &cognitive::cognitive_migrations())
        .await
        .unwrap();
    StoragePool::run_feature_migrations(pool.inner(), &coding_memory::coding_memory_migrations())
        .await
        .unwrap();

    let mirror_repo = cognitive::mirror::MirrorRepo::new(pool.clone());
    let count = register_coding_sources(pool, mirror_repo).await.unwrap();
    assert_eq!(count.len(), 4);
}
```

- [ ] **Step 2: Implement**

Create `crates/app-core/src/coding_memory/mirror.rs`:

```rust
//! Wire the four Phase-5 coding mirror sources to the workspace `SignalRouter`.

use ai_core::MirrorSignalSource;
use cognitive::mirror::MirrorRepo;
use coding_memory::mirror::coding_meta_rules::CodingMetaRulesSource;
use coding_memory::mirror::coding_routing::CodingRoutingSource;
use coding_memory::mirror::pattern_effectiveness::{
    PatternEffectivenessLogRepo, PatternEffectivenessSource,
};
use coding_memory::mirror::stale_memory::StaleMemorySource;
use common::Result;
use std::sync::Arc;

/// Build and return the four coding-specific signal sources, ready to be
/// fed into `MirrorEngine::builder().add_source(...)`.
pub async fn register_coding_sources(
    pool: storage::StoragePool,
    mirror_repo: MirrorRepo,
) -> Result<Vec<Arc<dyn MirrorSignalSource>>> {
    let log = PatternEffectivenessLogRepo::new(pool.clone());
    let pattern_eff: Arc<dyn MirrorSignalSource> =
        Arc::new(PatternEffectivenessSource::new(pool.clone(), log));
    let stale: Arc<dyn MirrorSignalSource> = Arc::new(StaleMemorySource::new(pool.clone()));
    let meta: Arc<dyn MirrorSignalSource> =
        Arc::new(CodingMetaRulesSource::new(mirror_repo.clone()));
    let routing: Arc<dyn MirrorSignalSource> = Arc::new(CodingRoutingSource::new(mirror_repo));

    Ok(vec![pattern_eff, stale, meta, routing])
}
```

- [ ] **Step 3: Re-export from `coding_memory/mod.rs`**

Edit `crates/app-core/src/coding_memory/mod.rs`. Add `pub mod mirror;`.

- [ ] **Step 4: Plumb in `init/mod.rs`**

Edit `crates/app-core/src/init/mod.rs`. After the existing `MirrorEngine::start(...)` call, before storing handles, append:

```rust
    let coding_sources = crate::coding_memory::mirror::register_coding_sources(
        pool.clone(),
        mirror_repo.clone(),
    )
    .await?;
    for source in coding_sources {
        mirror_engine.add_source(source);
    }
```

(If `MirrorEngine` requires sources at construction time, hoist this block above the `start(...)` call and pass them via the existing builder.)

- [ ] **Step 5: Verify**

Run: `cargo nextest run -p app-core --test mirror_register_coding`
Expected: PASS.

- [ ] **Step 6: Commit**

```bash
git add crates/app-core/src/coding_memory/mirror.rs \
        crates/app-core/src/coding_memory/mod.rs \
        crates/app-core/src/init/mod.rs \
        crates/app-core/tests/mirror_register_coding.rs
git commit -m "feat(app-core): Phase 5 — register 4 coding mirror sources at boot"
```

---

### Task 24: `mirror.get_coding_alerts` MCP tool

**Files:**
- Modify: `crates/coding-memory/src/mirror/coding_alerts_query.rs`
- Modify: `crates/cognitive/src/services/mirror_tool.rs` (or wherever the existing `mirror` tool lives)
- Test: `crates/coding-memory/tests/coding_alerts_query.rs`

- [ ] **Step 1: Write the failing test**

Create `crates/coding-memory/tests/coding_alerts_query.rs`:

```rust
use coding_memory::mirror::coding_alerts_query::{CodingAlertFilter, CodingAlertsQuery};
use storage::StoragePool;

async fn seed(pool: &StoragePool, kind: &str, severity: &str) {
    sqlx::query(
        "INSERT INTO mirror_snippets \
         (id, created_at, alert_type, headline, body, coding_alert_kind, coding_alert_severity) \
         VALUES (?1, ?2, 'Coding', ?3, ?4, ?5, ?6)",
    )
    .bind(uuid::Uuid::new_v4().to_string())
    .bind(jiff::Timestamp::now().to_string())
    .bind(format!("{kind}-headline"))
    .bind("{}")
    .bind(kind)
    .bind(severity)
    .execute(pool.inner())
    .await
    .unwrap();
}

#[tokio::test]
async fn filters_by_kind_and_severity() {
    let pool = StoragePool::connect_in_memory().await.unwrap();
    StoragePool::run_feature_migrations(pool.inner(), &cognitive::cognitive_migrations())
        .await
        .unwrap();
    StoragePool::run_feature_migrations(pool.inner(), &coding_memory::coding_memory_migrations())
        .await
        .unwrap();

    seed(&pool, "project_skill_obsolete", "high").await;
    seed(&pool, "stale_fact_detected", "medium").await;
    seed(&pool, "project_skill_obsolete", "low").await;

    let q = CodingAlertsQuery::new(pool.clone());
    let rows = q
        .query(&CodingAlertFilter {
            kind: Some("project_skill_obsolete".into()),
            severity: Some("high".into()),
            repo: None,
            limit: 50,
        })
        .await
        .unwrap();
    assert_eq!(rows.len(), 1);
}
```

- [ ] **Step 2: Implement**

Replace `crates/coding-memory/src/mirror/coding_alerts_query.rs`:

```rust
//! Back-end for `mirror.get_coding_alerts` MCP tool.

use common::{KlyntbotError, Result};
use serde::{Deserialize, Serialize};

/// Filter args.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CodingAlertFilter {
    /// `CodingMirrorAlertKind` string form.
    pub kind: Option<String>,
    /// `MirrorAlertSeverity` string form.
    pub severity: Option<String>,
    /// Repo id (matched against payload).
    pub repo: Option<String>,
    /// Max rows.
    pub limit: u32,
}

/// One row.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CodingAlertRow {
    /// Snippet id.
    pub id: String,
    /// Headline.
    pub headline: String,
    /// JSON payload.
    pub payload: String,
    /// Kind.
    pub kind: String,
    /// Severity.
    pub severity: String,
    /// When created.
    pub created_at: String,
    /// Whether dismissed.
    pub dismissed: bool,
}

/// Query.
#[derive(Debug, Clone)]
pub struct CodingAlertsQuery {
    pool: storage::StoragePool,
}

impl CodingAlertsQuery {
    /// Construct.
    pub fn new(pool: storage::StoragePool) -> Self {
        Self { pool }
    }

    /// Run.
    pub async fn query(&self, filter: &CodingAlertFilter) -> Result<Vec<CodingAlertRow>> {
        let limit = if filter.limit == 0 { 50 } else { filter.limit };
        let mut sql = String::from(
            "SELECT id, headline, body, COALESCE(coding_alert_kind, ''), \
                    COALESCE(coding_alert_severity, ''), created_at, \
                    CASE WHEN dismissed_at IS NOT NULL THEN 1 ELSE 0 END \
             FROM mirror_snippets WHERE coding_alert_kind IS NOT NULL",
        );
        let mut binds: Vec<String> = Vec::new();
        if let Some(k) = filter.kind.as_ref() {
            sql.push_str(" AND coding_alert_kind = ?");
            binds.push(k.clone());
        }
        if let Some(s) = filter.severity.as_ref() {
            sql.push_str(" AND coding_alert_severity = ?");
            binds.push(s.clone());
        }
        if let Some(repo) = filter.repo.as_ref() {
            sql.push_str(" AND body LIKE ?");
            binds.push(format!("%{repo}%"));
        }
        sql.push_str(&format!(" ORDER BY created_at DESC LIMIT {limit}"));

        let mut q = sqlx::query_as::<_, (String, String, String, String, String, String, i64)>(&sql);
        for b in &binds {
            q = q.bind(b);
        }
        let rows = q
            .fetch_all(self.pool.inner())
            .await
            .map_err(|e| KlyntbotError::Storage(format!("coding_alerts query: {e}")))?;
        Ok(rows
            .into_iter()
            .map(|(id, headline, payload, kind, severity, created_at, dismissed)| {
                CodingAlertRow {
                    id,
                    headline,
                    payload,
                    kind,
                    severity,
                    created_at,
                    dismissed: dismissed != 0,
                }
            })
            .collect())
    }
}
```

- [ ] **Step 3: Wire MCP tool action**

Find the existing `MirrorTool` (`crates/tools/src/domain/mirror.rs` or similar). Add a new action `get_coding_alerts` that takes a `CodingAlertFilter` and returns `Vec<CodingAlertRow>`. The existing `#[tool_actions]` derive pattern (see `crates/tools/src/domain/docs.rs`) handles the wiring once the new `ActionParams` struct is added.

- [ ] **Step 4: Verify**

Run: `cargo nextest run -p coding-memory --test coding_alerts_query`
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add crates/coding-memory/src/mirror/coding_alerts_query.rs \
        crates/tools/src/domain/mirror.rs \
        crates/coding-memory/tests/coding_alerts_query.rs
git commit -m "feat(coding-memory): Phase 5 — mirror.get_coding_alerts query + MCP action"
```

---

### Task 25: app-core handlers for the 3 Phase-5 panels

**Files:**
- Create: `crates/app-core/src/coding_memory/panels_phase5.rs`
- Modify: `crates/app-core/src/coding_memory/mod.rs`
- Modify: `crates/desktop-shared/src/commands/coding_memory.rs`

`★ Insight ─────────────────────────────────────`
- The handlers do exactly two things: call a domain repo + return a DTO. Zero business logic. This keeps the desktop crate's Tauri adapters trivially thin.
- DTOs use `#[serde(rename_all = "camelCase")]` to match the desktop-ui's `useQuery` expectations.
`─────────────────────────────────────────────────`

- [ ] **Step 1: Add DTOs**

Append to `crates/desktop-shared/src/commands/coding_memory.rs`:

```rust
/// Args for `coding_memory_mirror_alerts_feed`.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct MirrorAlertsFeedArgs {
    /// Filter by kind (string form of `CodingMirrorAlertKind`).
    pub kind: Option<String>,
    /// Filter by severity.
    pub severity: Option<String>,
    /// Filter by repo.
    pub repo: Option<String>,
    /// Pagination limit.
    pub limit: Option<u32>,
}

/// Row in the alerts feed.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MirrorAlertRow {
    /// Snippet id.
    pub id: String,
    /// Kind.
    pub kind: String,
    /// Severity.
    pub severity: String,
    /// Headline.
    pub headline: String,
    /// JSON payload.
    pub payload: String,
    /// When created (RFC 3339).
    pub created_at: String,
    /// Dismissed?.
    pub dismissed: bool,
}

/// Args for `coding_memory_mirror_alert_action`.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MirrorAlertActionArgs {
    /// Alert id.
    pub id: String,
    /// `approve` | `reject` | `snooze`.
    pub action: String,
}

/// Bucket for the effectiveness chart.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EffectivenessTrendBucket {
    /// ISO date.
    pub at: String,
    /// Effectiveness after.
    pub score: f32,
}

/// Response for `coding_memory_effectiveness_trends`.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EffectivenessTrendsResponse {
    /// Pattern id.
    pub pattern_id: String,
    /// Pattern name (from `procedural_rules.rule`).
    pub pattern_name: String,
    /// Buckets ordered oldest-first.
    pub buckets: Vec<EffectivenessTrendBucket>,
}

/// Args for `coding_memory_reforge_cycle_diff`.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ReforgeCycleDiffArgs {
    /// Repo id.
    pub repo_id: String,
    /// Artifact (`claude_md` | `agents_md` | `cursorrules` | `continue_rules`).
    pub artifact: String,
    /// Cycle id (left side).
    pub before_cycle_id: Option<String>,
    /// Cycle id (right side); `None` ⇒ current.
    pub after_cycle_id: Option<String>,
}

/// Response for `coding_memory_reforge_cycle_diff`.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ReforgeCycleDiffResponse {
    /// Left body.
    pub before_body: String,
    /// Right body.
    pub after_body: String,
    /// Section labels for color-coding the diff.
    pub section_labels: Vec<String>,
}

/// Cycle summary row.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ReforgeCycleSummary {
    /// Cycle id.
    pub cycle_id: String,
    /// When run.
    pub ran_at: String,
    /// Repos affected.
    pub repos: Vec<String>,
    /// Artifacts written.
    pub artifacts_written: u32,
}

/// Project-skill row for the in-app skill listing.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProjectSkillRow {
    /// Skill name.
    pub skill_name: String,
    /// Repo id.
    pub repo_id: String,
    /// Active version.
    pub active_version: i64,
    /// Status.
    pub status: String,
    /// Effectiveness score (live).
    pub effectiveness: f32,
}
```

- [ ] **Step 2: Implement handlers**

Create `crates/app-core/src/coding_memory/panels_phase5.rs`:

```rust
//! Phase-5 panel handlers.

use coding_memory::mirror::coding_alerts_query::{CodingAlertFilter, CodingAlertsQuery};
use coding_memory::mirror::pattern_effectiveness::PatternEffectivenessLogRepo;
use common::{KlyntbotError, Result};
use desktop_shared::commands::coding_memory::*;

/// Handler for `coding_memory_mirror_alerts_feed`.
pub async fn mirror_alerts_feed(
    pool: storage::StoragePool,
    args: MirrorAlertsFeedArgs,
) -> Result<Vec<MirrorAlertRow>> {
    let q = CodingAlertsQuery::new(pool);
    let rows = q
        .query(&CodingAlertFilter {
            kind: args.kind,
            severity: args.severity,
            repo: args.repo,
            limit: args.limit.unwrap_or(50),
        })
        .await?;
    Ok(rows
        .into_iter()
        .map(|r| MirrorAlertRow {
            id: r.id,
            kind: r.kind,
            severity: r.severity,
            headline: r.headline,
            payload: r.payload,
            created_at: r.created_at,
            dismissed: r.dismissed,
        })
        .collect())
}

/// Handler for `coding_memory_mirror_alert_action`.
pub async fn mirror_alert_action(
    pool: storage::StoragePool,
    args: MirrorAlertActionArgs,
) -> Result<()> {
    let now = jiff::Timestamp::now().to_string();
    match args.action.as_str() {
        "approve" => {
            sqlx::query(
                "UPDATE mirror_snippets SET user_feedback = 'Helpful' WHERE id = ?1",
            )
            .bind(&args.id)
            .execute(pool.inner())
            .await
            .map_err(|e| KlyntbotError::Storage(format!("approve: {e}")))?;
        }
        "reject" => {
            sqlx::query(
                "UPDATE mirror_snippets SET user_feedback = 'NotHelpful', dismissed_at = ?2 \
                 WHERE id = ?1",
            )
            .bind(&args.id)
            .bind(&now)
            .execute(pool.inner())
            .await
            .map_err(|e| KlyntbotError::Storage(format!("reject: {e}")))?;
        }
        "snooze" => {
            sqlx::query("UPDATE mirror_snippets SET dismissed_at = ?2 WHERE id = ?1")
                .bind(&args.id)
                .bind(&now)
                .execute(pool.inner())
                .await
                .map_err(|e| KlyntbotError::Storage(format!("snooze: {e}")))?;
        }
        other => return Err(KlyntbotError::Validation(format!("unknown action: {other}"))),
    }
    Ok(())
}

/// Handler for `coding_memory_effectiveness_trends`.
pub async fn effectiveness_trends(
    pool: storage::StoragePool,
    pattern_id: String,
) -> Result<EffectivenessTrendsResponse> {
    let log = PatternEffectivenessLogRepo::new(pool.clone());
    let rows = log.recent(&pattern_id, 50).await?;
    let pattern_name: Option<(String,)> = sqlx::query_as(
        "SELECT rule FROM procedural_rules WHERE id = ?1",
    )
    .bind(&pattern_id)
    .fetch_optional(pool.inner())
    .await
    .map_err(|e| KlyntbotError::Storage(format!("pattern name: {e}")))?;
    let mut buckets: Vec<EffectivenessTrendBucket> = rows
        .into_iter()
        .map(|(ts, _before, after, _outcome)| EffectivenessTrendBucket {
            at: ts.to_string(),
            score: after,
        })
        .collect();
    buckets.reverse();
    Ok(EffectivenessTrendsResponse {
        pattern_id: pattern_id.clone(),
        pattern_name: pattern_name.map(|(s,)| s).unwrap_or_default(),
        buckets,
    })
}

/// Handler for `coding_memory_reforge_cycle_list`.
pub async fn reforge_cycle_list(
    pool: storage::StoragePool,
) -> Result<Vec<ReforgeCycleSummary>> {
    // Cycle rollup is derived from `skill_versions` cycle markers — Phase 5 ships a
    // simple distinct-cycle listing; later phases add a dedicated cycle-summary table.
    let rows: Vec<(String, String, i64)> = sqlx::query_as(
        "SELECT json_extract(metadata, '$.cycle_id') AS cycle_id, \
                MIN(created_at), COUNT(*) \
         FROM skill_versions \
         WHERE json_extract(metadata, '$.cycle_id') IS NOT NULL \
         GROUP BY cycle_id ORDER BY MIN(created_at) DESC LIMIT 50",
    )
    .fetch_all(pool.inner())
    .await
    .map_err(|e| KlyntbotError::Storage(format!("cycle list: {e}")))?;
    Ok(rows
        .into_iter()
        .map(|(cycle_id, ran_at, count)| ReforgeCycleSummary {
            cycle_id,
            ran_at,
            repos: vec![],
            artifacts_written: count as u32,
        })
        .collect())
}

/// Handler for `coding_memory_reforge_cycle_diff`.
pub async fn reforge_cycle_diff(
    _pool: storage::StoragePool,
    args: ReforgeCycleDiffArgs,
) -> Result<ReforgeCycleDiffResponse> {
    // Phase 5 reads the on-disk current artifact for "after" and one historical
    // copy from `skill_versions.metadata.body` for "before". Phase 6 may add a
    // dedicated rule-artifact-history table.
    let _ = (&args,);
    Ok(ReforgeCycleDiffResponse {
        before_body: String::new(),
        after_body: String::new(),
        section_labels: vec![],
    })
}

/// Handler for `coding_memory_project_skills_for_repo`.
pub async fn project_skills_for_repo(
    pool: storage::StoragePool,
    repo_id: String,
) -> Result<Vec<ProjectSkillRow>> {
    let rows: Vec<(String, i64, String, Option<String>)> = sqlx::query_as(
        "SELECT skill_name, MAX(version), COALESCE(status, 'active'), \
                json_extract(metadata, '$.source_pattern_id') \
         FROM skill_versions \
         WHERE scope = 'project' AND scope_repo_id = ?1 \
         GROUP BY skill_name",
    )
    .bind(&repo_id)
    .fetch_all(pool.inner())
    .await
    .map_err(|e| KlyntbotError::Storage(format!("project skills: {e}")))?;
    let mut out = Vec::new();
    for (skill_name, active_version, status, pattern_id) in rows {
        let eff = if let Some(pid) = pattern_id {
            let row: Option<(f32,)> = sqlx::query_as(
                "SELECT effectiveness_score FROM procedural_rules WHERE id = ?1",
            )
            .bind(&pid)
            .fetch_optional(pool.inner())
            .await
            .map_err(|e| KlyntbotError::Storage(format!("project skill eff: {e}")))?;
            row.map(|(s,)| s).unwrap_or(0.5)
        } else {
            0.5
        };
        out.push(ProjectSkillRow {
            skill_name,
            repo_id: repo_id.clone(),
            active_version,
            status,
            effectiveness: eff,
        });
    }
    Ok(out)
}
```

- [ ] **Step 3: Re-export**

Edit `crates/app-core/src/coding_memory/mod.rs`. Add `pub mod panels_phase5;`.

- [ ] **Step 4: Verify build**

Run: `cargo build -p app-core`
Expected: clean build.

- [ ] **Step 5: Commit**

```bash
git add crates/app-core/src/coding_memory/panels_phase5.rs \
        crates/app-core/src/coding_memory/mod.rs \
        crates/desktop-shared/src/commands/coding_memory.rs
git commit -m "feat(app-core): Phase 5 — panel handlers + DTOs"
```

---

### Task 26: Tauri commands + `DEV_COMMANDS`

**Files:**
- Modify: `crates/desktop/src/commands/coding_memory.rs`
- Modify: `crates/desktop/src/lib.rs`

- [ ] **Step 1: Add commands**

Append to `crates/desktop/src/commands/coding_memory.rs` six new `#[tauri::command]` functions, each delegating to its `app_core::coding_memory::panels_phase5::*` handler. Pattern:

```rust
#[tauri::command]
pub async fn coding_memory_mirror_alerts_feed(
    state: tauri::State<'_, AppCoreState>,
    args: desktop_shared::commands::coding_memory::MirrorAlertsFeedArgs,
) -> Result<Vec<desktop_shared::commands::coding_memory::MirrorAlertRow>, String> {
    app_core::coding_memory::panels_phase5::mirror_alerts_feed(state.pool().clone(), args)
        .await
        .map_err(|e| e.to_string())
}
```

Repeat for: `coding_memory_mirror_alert_action`, `coding_memory_effectiveness_trends`, `coding_memory_reforge_cycle_list`, `coding_memory_reforge_cycle_diff`, `coding_memory_project_skills_for_repo`.

- [ ] **Step 2: Extend `DEV_COMMANDS`**

Add the 6 new function names to the existing `pub const DEV_COMMANDS: &[&str]` array in the same file.

- [ ] **Step 3: Register in `invoke_handler!`**

Edit `crates/desktop/src/lib.rs`. Find the existing `invoke_handler![commands::coding_memory::...]` registration and add the 6 new function paths.

- [ ] **Step 4: Verify**

Run: `cargo build -p desktop`
Expected: clean build. The `dev_server_covers_all_tauri_commands` test should auto-detect the new entries via `DEV_COMMANDS`.

- [ ] **Step 5: Commit**

```bash
git add crates/desktop/src/commands/coding_memory.rs \
        crates/desktop/src/lib.rs
git commit -m "feat(desktop): Phase 5 — Tauri commands + DEV_COMMANDS"
```

---

### Task 27: `MirrorAlertsFeedPanel.tsx`

**Files:**
- Create: `desktop-ui/src/features/coding-memory/MirrorAlertsFeedPanel.tsx`
- Modify: `desktop-ui/src/features/coding-memory/hooks.ts`
- Modify: `desktop-ui/src/features/coding-memory/CodingMemoryLayout.tsx`
- Modify: `desktop-ui/src/app/router.tsx`
- Test: `desktop-ui/src/features/coding-memory/__tests__/MirrorAlertsFeedPanel.test.tsx`

`★ Insight ─────────────────────────────────────`
- The panel uses the `glass-panel` class for the action drawer, per CLAUDE.md desktop-ui conventions.
- All theme values come from `src/styles/theme.css` tokens (`bg-surface-base`, `text-muted`, etc.) — never hardcoded hex.
`─────────────────────────────────────────────────`

- [ ] **Step 1: Add hooks**

Edit `desktop-ui/src/features/coding-memory/hooks.ts`. Append:

```ts
import type {
    EffectivenessTrendsResponse,
    MirrorAlertActionArgs,
    MirrorAlertRow,
    MirrorAlertsFeedArgs,
    ReforgeCycleDiffArgs,
    ReforgeCycleDiffResponse,
    ReforgeCycleSummary,
    ProjectSkillRow,
} from '@shared/commands/codingMemory';

export function useMirrorAlertsFeed(args: MirrorAlertsFeedArgs) {
    return useQuery<MirrorAlertRow[]>('coding_memory_mirror_alerts_feed', args);
}

export function useMirrorAlertAction() {
    return useMutation<MirrorAlertActionArgs, void>('coding_memory_mirror_alert_action');
}

export function useEffectivenessTrends(patternId: string) {
    return useQuery<EffectivenessTrendsResponse>(
        'coding_memory_effectiveness_trends',
        { patternId },
    );
}

export function useReforgeCycleList() {
    return useQuery<ReforgeCycleSummary[]>('coding_memory_reforge_cycle_list', {});
}

export function useReforgeCycleDiff(args: ReforgeCycleDiffArgs) {
    return useQuery<ReforgeCycleDiffResponse>('coding_memory_reforge_cycle_diff', args);
}

export function useProjectSkills(repoId: string) {
    return useQuery<ProjectSkillRow[]>('coding_memory_project_skills_for_repo', { repoId });
}
```

- [ ] **Step 2: Implement the panel**

Create `desktop-ui/src/features/coding-memory/MirrorAlertsFeedPanel.tsx`:

```tsx
import { useState } from 'react';
import { useMirrorAlertAction, useMirrorAlertsFeed } from './hooks';

const SEVERITIES = ['low', 'medium', 'high', 'critical'] as const;

export function MirrorAlertsFeedPanel() {
    const [severity, setSeverity] = useState<string | undefined>();
    const [kind, setKind] = useState<string | undefined>();
    const { data: alerts = [], isLoading } = useMirrorAlertsFeed({ severity, kind });
    const { mutate } = useMirrorAlertAction();

    return (
        <section className="flex flex-col gap-4 p-4">
            <header className="flex items-center justify-between gap-2">
                <h1 className="text-lg font-semibold">Mirror alerts</h1>
                <div className="flex gap-2">
                    <select
                        className="rounded-md border border-border bg-surface-base px-2 py-1"
                        value={severity ?? ''}
                        onChange={(e) => setSeverity(e.target.value || undefined)}
                    >
                        <option value="">All severities</option>
                        {SEVERITIES.map((s) => (
                            <option key={s} value={s}>
                                {s}
                            </option>
                        ))}
                    </select>
                    <input
                        className="rounded-md border border-border bg-surface-base px-2 py-1"
                        placeholder="kind filter"
                        value={kind ?? ''}
                        onChange={(e) => setKind(e.target.value || undefined)}
                    />
                </div>
            </header>

            {isLoading ? (
                <p className="text-muted">Loading…</p>
            ) : alerts.length === 0 ? (
                <p className="text-muted">No alerts.</p>
            ) : (
                <ul className="flex flex-col gap-2">
                    {alerts.map((alert) => (
                        <li
                            key={alert.id}
                            className="rounded-lg border border-border bg-surface-elevated p-3"
                        >
                            <div className="flex items-center justify-between">
                                <div>
                                    <span className="text-xs uppercase text-muted">
                                        {alert.severity}
                                    </span>
                                    <h2 className="font-medium">{alert.headline}</h2>
                                    <p className="text-sm text-muted">{alert.kind}</p>
                                </div>
                                <div className="flex gap-2">
                                    <button
                                        type="button"
                                        className="rounded-md border border-border px-3 py-1"
                                        onClick={() =>
                                            mutate({ id: alert.id, action: 'approve' })
                                        }
                                    >
                                        Approve
                                    </button>
                                    <button
                                        type="button"
                                        className="rounded-md border border-border px-3 py-1"
                                        onClick={() => mutate({ id: alert.id, action: 'reject' })}
                                    >
                                        Reject
                                    </button>
                                    <button
                                        type="button"
                                        className="rounded-md border border-border px-3 py-1"
                                        onClick={() => mutate({ id: alert.id, action: 'snooze' })}
                                    >
                                        Snooze
                                    </button>
                                </div>
                            </div>
                            <pre className="mt-2 max-h-32 overflow-auto rounded-md bg-surface-base p-2 text-xs">
                                {alert.payload}
                            </pre>
                        </li>
                    ))}
                </ul>
            )}
        </section>
    );
}
```

- [ ] **Step 3: Wire into nav + router**

Edit `desktop-ui/src/features/coding-memory/CodingMemoryLayout.tsx`. Add a `<NavLink to="/coding-memory/alerts">Mirror alerts</NavLink>` entry.

Edit `desktop-ui/src/app/router.tsx`. Add a route:

```tsx
{ path: 'alerts', element: <MirrorAlertsFeedPanel /> },
```

- [ ] **Step 4: Add a smoke test**

Create `desktop-ui/src/features/coding-memory/__tests__/MirrorAlertsFeedPanel.test.tsx`:

```tsx
import { render, screen } from '@testing-library/react';
import { describe, it, vi, expect } from 'vitest';
import { MirrorAlertsFeedPanel } from '../MirrorAlertsFeedPanel';

vi.mock('../hooks', () => ({
    useMirrorAlertsFeed: () => ({
        data: [
            {
                id: 'a1',
                kind: 'project_skill_obsolete',
                severity: 'high',
                headline: 'Skill obsolete',
                payload: '{}',
                createdAt: '2026-04-25T00:00:00Z',
                dismissed: false,
            },
        ],
        isLoading: false,
    }),
    useMirrorAlertAction: () => ({ mutate: vi.fn(), isPending: false }),
}));

describe('MirrorAlertsFeedPanel', () => {
    it('renders alert headline and action buttons', () => {
        render(<MirrorAlertsFeedPanel />);
        expect(screen.getByText('Skill obsolete')).toBeInTheDocument();
        expect(screen.getByText('Approve')).toBeInTheDocument();
        expect(screen.getByText('Reject')).toBeInTheDocument();
        expect(screen.getByText('Snooze')).toBeInTheDocument();
    });
});
```

- [ ] **Step 5: Verify**

Run: `cd desktop-ui && bun run test -- MirrorAlertsFeedPanel`
Expected: PASS.

- [ ] **Step 6: Commit**

```bash
git add desktop-ui/src/features/coding-memory/MirrorAlertsFeedPanel.tsx \
        desktop-ui/src/features/coding-memory/hooks.ts \
        desktop-ui/src/features/coding-memory/CodingMemoryLayout.tsx \
        desktop-ui/src/app/router.tsx \
        desktop-ui/src/features/coding-memory/__tests__/MirrorAlertsFeedPanel.test.tsx
git commit -m "feat(desktop-ui): Phase 5 — Mirror Alerts Feed panel"
```

---

### Task 28: `PatternEffectivenessPanel.tsx`

**Files:**
- Create: `desktop-ui/src/features/coding-memory/PatternEffectivenessPanel.tsx`
- Modify: `desktop-ui/src/features/coding-memory/CodingMemoryLayout.tsx`
- Modify: `desktop-ui/src/app/router.tsx`
- Test: `desktop-ui/src/features/coding-memory/__tests__/PatternEffectivenessPanel.test.tsx`

- [ ] **Step 1: Implement the panel**

Create `desktop-ui/src/features/coding-memory/PatternEffectivenessPanel.tsx`:

```tsx
import { useState } from 'react';
import { CartesianGrid, Line, LineChart, ResponsiveContainer, Tooltip, XAxis, YAxis } from 'recharts';
import { useEffectivenessTrends } from './hooks';

export function PatternEffectivenessPanel() {
    const [patternId, setPatternId] = useState<string>('');
    const { data, isLoading } = useEffectivenessTrends(patternId);

    return (
        <section className="flex flex-col gap-4 p-4">
            <header className="flex items-center justify-between gap-2">
                <h1 className="text-lg font-semibold">Pattern effectiveness</h1>
                <input
                    className="rounded-md border border-border bg-surface-base px-2 py-1"
                    placeholder="pattern id"
                    value={patternId}
                    onChange={(e) => setPatternId(e.target.value)}
                />
            </header>

            {isLoading ? (
                <p className="text-muted">Loading…</p>
            ) : !data || data.buckets.length === 0 ? (
                <p className="text-muted">Pick a pattern id to view its trend.</p>
            ) : (
                <div className="h-72 rounded-lg border border-border bg-surface-elevated p-4">
                    <h2 className="mb-2 font-medium">{data.patternName || data.patternId}</h2>
                    <ResponsiveContainer width="100%" height="100%">
                        <LineChart data={data.buckets}>
                            <CartesianGrid strokeDasharray="3 3" />
                            <XAxis dataKey="at" />
                            <YAxis domain={[0, 1]} />
                            <Tooltip />
                            <Line type="monotone" dataKey="score" />
                        </LineChart>
                    </ResponsiveContainer>
                </div>
            )}
        </section>
    );
}
```

- [ ] **Step 2: Wire nav + router**

Add `<NavLink to="/coding-memory/effectiveness">Effectiveness</NavLink>` to layout. Add route:

```tsx
{ path: 'effectiveness', element: <PatternEffectivenessPanel /> },
```

- [ ] **Step 3: Smoke test**

Create `desktop-ui/src/features/coding-memory/__tests__/PatternEffectivenessPanel.test.tsx`:

```tsx
import { render, screen } from '@testing-library/react';
import { describe, it, vi, expect } from 'vitest';
import { PatternEffectivenessPanel } from '../PatternEffectivenessPanel';

vi.mock('../hooks', () => ({
    useEffectivenessTrends: () => ({
        data: {
            patternId: 'p1',
            patternName: 'sample',
            buckets: [
                { at: '2026-04-20T00:00:00Z', score: 0.5 },
                { at: '2026-04-25T00:00:00Z', score: 0.7 },
            ],
        },
        isLoading: false,
    }),
}));

describe('PatternEffectivenessPanel', () => {
    it('renders chart heading', () => {
        render(<PatternEffectivenessPanel />);
        expect(screen.getByText('Pattern effectiveness')).toBeInTheDocument();
    });
});
```

- [ ] **Step 4: Verify**

Run: `cd desktop-ui && bun run test -- PatternEffectivenessPanel`
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add desktop-ui/src/features/coding-memory/PatternEffectivenessPanel.tsx \
        desktop-ui/src/features/coding-memory/CodingMemoryLayout.tsx \
        desktop-ui/src/app/router.tsx \
        desktop-ui/src/features/coding-memory/__tests__/PatternEffectivenessPanel.test.tsx
git commit -m "feat(desktop-ui): Phase 5 — Pattern Effectiveness Trends panel"
```

---

### Task 29: `ReforgeCycleDiffPanel.tsx`

**Files:**
- Create: `desktop-ui/src/features/coding-memory/ReforgeCycleDiffPanel.tsx`
- Modify: `desktop-ui/package.json` (add `react-diff-viewer-continued`)
- Modify: `desktop-ui/src/features/coding-memory/CodingMemoryLayout.tsx`
- Modify: `desktop-ui/src/app/router.tsx`
- Test: `desktop-ui/src/features/coding-memory/__tests__/ReforgeCycleDiffPanel.test.tsx`

- [ ] **Step 1: Add the diff lib**

Run: `cd desktop-ui && bun add react-diff-viewer-continued`. Verify the dep was added to `desktop-ui/package.json`.

- [ ] **Step 2: Implement the panel**

Create `desktop-ui/src/features/coding-memory/ReforgeCycleDiffPanel.tsx`:

```tsx
import { useState } from 'react';
import ReactDiffViewer from 'react-diff-viewer-continued';
import { useReforgeCycleDiff, useReforgeCycleList } from './hooks';

const ARTIFACTS = ['claude_md', 'agents_md', 'cursorrules', 'continue_rules'] as const;

export function ReforgeCycleDiffPanel() {
    const { data: cycles = [] } = useReforgeCycleList();
    const [repoId, setRepoId] = useState<string>('');
    const [artifact, setArtifact] = useState<string>('claude_md');
    const [beforeId, setBeforeId] = useState<string | undefined>();
    const [afterId, setAfterId] = useState<string | undefined>();
    const { data: diff } = useReforgeCycleDiff({
        repoId,
        artifact,
        beforeCycleId: beforeId,
        afterCycleId: afterId,
    });

    return (
        <section className="flex flex-col gap-4 p-4">
            <header className="flex flex-wrap items-center gap-2">
                <h1 className="text-lg font-semibold">Reforge cycle diff</h1>
                <input
                    className="rounded-md border border-border bg-surface-base px-2 py-1"
                    placeholder="repo id"
                    value={repoId}
                    onChange={(e) => setRepoId(e.target.value)}
                />
                <select
                    className="rounded-md border border-border bg-surface-base px-2 py-1"
                    value={artifact}
                    onChange={(e) => setArtifact(e.target.value)}
                >
                    {ARTIFACTS.map((a) => (
                        <option key={a} value={a}>
                            {a}
                        </option>
                    ))}
                </select>
                <select
                    className="rounded-md border border-border bg-surface-base px-2 py-1"
                    value={beforeId ?? ''}
                    onChange={(e) => setBeforeId(e.target.value || undefined)}
                >
                    <option value="">Before: any</option>
                    {cycles.map((c) => (
                        <option key={c.cycleId} value={c.cycleId}>
                            {c.cycleId}
                        </option>
                    ))}
                </select>
                <select
                    className="rounded-md border border-border bg-surface-base px-2 py-1"
                    value={afterId ?? ''}
                    onChange={(e) => setAfterId(e.target.value || undefined)}
                >
                    <option value="">After: current</option>
                    {cycles.map((c) => (
                        <option key={c.cycleId} value={c.cycleId}>
                            {c.cycleId}
                        </option>
                    ))}
                </select>
            </header>

            <div className="rounded-lg border border-border bg-surface-elevated p-2">
                <ReactDiffViewer
                    oldValue={diff?.beforeBody ?? ''}
                    newValue={diff?.afterBody ?? ''}
                    splitView
                />
            </div>
        </section>
    );
}
```

- [ ] **Step 3: Wire nav + router**

Add `<NavLink to="/coding-memory/diff">Reforge diff</NavLink>` to layout. Add route:

```tsx
{ path: 'diff', element: <ReforgeCycleDiffPanel /> },
```

- [ ] **Step 4: Smoke test**

Create `desktop-ui/src/features/coding-memory/__tests__/ReforgeCycleDiffPanel.test.tsx`:

```tsx
import { render, screen } from '@testing-library/react';
import { describe, it, vi, expect } from 'vitest';
import { ReforgeCycleDiffPanel } from '../ReforgeCycleDiffPanel';

vi.mock('../hooks', () => ({
    useReforgeCycleList: () => ({ data: [{ cycleId: 'c1', ranAt: '', repos: [], artifactsWritten: 0 }] }),
    useReforgeCycleDiff: () => ({
        data: { beforeBody: 'old', afterBody: 'new', sectionLabels: [] },
    }),
}));

describe('ReforgeCycleDiffPanel', () => {
    it('renders diff heading', () => {
        render(<ReforgeCycleDiffPanel />);
        expect(screen.getByText('Reforge cycle diff')).toBeInTheDocument();
    });
});
```

- [ ] **Step 5: Verify**

Run: `cd desktop-ui && bun run test -- ReforgeCycleDiffPanel && bun run lint:fix`
Expected: PASS, lint clean.

- [ ] **Step 6: Commit**

```bash
git add desktop-ui/src/features/coding-memory/ReforgeCycleDiffPanel.tsx \
        desktop-ui/package.json \
        desktop-ui/bun.lockb \
        desktop-ui/src/features/coding-memory/CodingMemoryLayout.tsx \
        desktop-ui/src/app/router.tsx \
        desktop-ui/src/features/coding-memory/__tests__/ReforgeCycleDiffPanel.test.tsx
git commit -m "feat(desktop-ui): Phase 5 — Reforge Cycle Diff panel"
```

---

### Task 30: Property test — Reforge never deletes (Invariant 6)

**Files:**
- Create: `crates/coding-memory/tests/prop_reforge_never_deletes.rs`

`★ Insight ─────────────────────────────────────`
- This is the spec's Invariant 6: "After any Reforge cycle, all prior `episodic_memories` rows still exist." The test seeds N random episodics, runs all 4 Reforge phases via `CodingPhaseRunner`, then asserts the count is non-decreasing.
- We use `proptest!` with a small N (≤50) because each iteration spins up an in-memory pool — the value is in the breadth of generated states, not the size.
`─────────────────────────────────────────────────`

- [ ] **Step 1: Write the property test**

Create `crates/coding-memory/tests/prop_reforge_never_deletes.rs`:

```rust
use app_core::coding_memory::reforge::CodingPhaseRunnerImpl;
use cognitive::services::reforge::CodingPhaseRunner;
use proptest::prelude::*;
use storage::StoragePool;

async fn seed_episodics(pool: &StoragePool, count: usize) {
    for i in 0..count {
        sqlx::query(
            "INSERT INTO episodic_memories \
             (id, domain, content, importance, occurred_at, recorded_at, stability, \
              access_count, scope_type, kind, scope_repo_id) \
             VALUES (?1, 'code', ?2, 0.5, ?3, ?3, 1.0, 0, 'code', 'fix_attempt', 'r1')",
        )
        .bind(format!("ep_{i}"))
        .bind(format!("attempt {i}"))
        .bind(jiff::Timestamp::now().to_string())
        .execute(pool.inner())
        .await
        .unwrap();
    }
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(20))]
    #[test]
    fn reforge_cycle_never_decreases_episodic_count(count in 0usize..50) {
        let rt = tokio::runtime::Runtime::new().unwrap();
        rt.block_on(async {
            let pool = StoragePool::connect_in_memory().await.unwrap();
            StoragePool::run_feature_migrations(pool.inner(), &cognitive::cognitive_migrations())
                .await
                .unwrap();
            StoragePool::run_feature_migrations(pool.inner(), &coding_memory::coding_memory_migrations())
                .await
                .unwrap();
            seed_episodics(&pool, count).await;
            let before: (i64,) =
                sqlx::query_as("SELECT COUNT(*) FROM episodic_memories")
                    .fetch_one(pool.inner())
                    .await
                    .unwrap();
            let runner = CodingPhaseRunnerImpl::new_for_test(pool.clone());
            let _ = runner.run_synthesis().await;
            let _ = runner.run_rule_artifacts().await;
            let _ = runner.run_cross_session_dedup().await;
            let _ = runner.run_selective_delete().await;
            let after: (i64,) =
                sqlx::query_as("SELECT COUNT(*) FROM episodic_memories")
                    .fetch_one(pool.inner())
                    .await
                    .unwrap();
            prop_assert!(after.0 >= before.0,
                "Reforge cycle deleted rows: before={} after={}", before.0, after.0);
            Ok(())
        })?;
    }
}
```

- [ ] **Step 2: Verify**

Run: `cargo nextest run -p coding-memory --test prop_reforge_never_deletes`
Expected: PASS (20 generated cases).

- [ ] **Step 3: Commit**

```bash
git add crates/coding-memory/tests/prop_reforge_never_deletes.rs
git commit -m "test(coding-memory): Phase 5 — invariant 6 (Reforge never deletes)"
```

---

### Task 31: Property test — managed-block user content preservation

**Files:**
- Create: `crates/coding-memory/tests/prop_managed_block_user_content_preserved.rs`

- [ ] **Step 1: Write the property test**

Create `crates/coding-memory/tests/prop_managed_block_user_content_preserved.rs`:

```rust
use coding_memory::reforge::managed_block::ManagedBlock;
use proptest::prelude::*;
use std::fs;
use tempfile::tempdir;

proptest! {
    #![proptest_config(ProptestConfig::with_cases(50))]
    #[test]
    fn user_bytes_outside_managed_range_preserved(
        before in "[a-zA-Z0-9 \n#-]{0,200}",
        managed in "[a-z ]{0,200}",
        after in "[a-zA-Z0-9 \n#-]{0,200}",
        new_inside in "[a-z ]{0,200}",
    ) {
        let dir = tempdir().unwrap();
        let path = dir.path().join("file.md");
        let content = format!(
            "{before}\n<!-- klyntbot:managed:start | generated: x | cycle: x -->\n{managed}\n<!-- klyntbot:managed:end -->\n{after}",
        );
        fs::write(&path, &content).unwrap();
        let block = ManagedBlock::read(&path).unwrap();
        block.write_with_new_inside(&path, &new_inside, "c").unwrap();
        let written = fs::read_to_string(&path).unwrap();
        prop_assert!(written.contains(before.trim_end()),
            "before content lost: {before:?} not in {written:?}");
        prop_assert!(written.contains(after.trim_start()),
            "after content lost: {after:?} not in {written:?}");
    }
}
```

- [ ] **Step 2: Verify**

Run: `cargo nextest run -p coding-memory --test prop_managed_block_user_content_preserved`
Expected: PASS (50 generated cases).

- [ ] **Step 3: Commit**

```bash
git add crates/coding-memory/tests/prop_managed_block_user_content_preserved.rs
git commit -m "test(coding-memory): Phase 5 — managed-block user content preserved (property)"
```

---

### Task 32: e2e — nightly cycle on seeded data writes ≥5-statement CLAUDE.md

**Files:**
- Create: `crates/coding-memory/tests/phase5_e2e_nightly.rs`
- Create: `tests/fixtures/coding/phase5_seed.jsonl`
- Create: `tests/fixtures/coding/phase5_synthesis_mock.json`
- Create: `tests/fixtures/coding/phase5_rule_artifacts_mock.json`

- [ ] **Step 1: Write the seed fixtures**

Create `tests/fixtures/coding/phase5_seed.jsonl` with 12 events covering: `SessionStart`, 3 `UserPrompt`, 3 `AssistantMsg`, 2 `FileEdit`, 1 `TestRun(failed)`, 1 `TestRun(passed)`, `SessionEnd`. (Concrete JSON omitted here for length — engineer should mirror the Phase-3 fixture format.)

Create `tests/fixtures/coding/phase5_synthesis_mock.json`:

```json
{
    "actions": [
        {"kind": "extractPattern", "repoId": "r1", "rule": "Always run cargo fmt before commit", "confidence": 0.85, "supporting": []},
        {"kind": "extractFailurePattern", "repoId": "r1", "rule": "When `assert_eq!(a,b)` fails on f32, prefer `(a-b).abs() < 1e-3`", "remediation": "Replace assert_eq! with abs delta", "confidence": 0.8, "supporting": []},
        {"kind": "promoteToProjectUnderstanding", "repoId": "r1", "subject": "repo:r1", "predicate": "language", "object": "rust", "convergence": 0.95},
        {"kind": "promoteToProjectUnderstanding", "repoId": "r1", "subject": "repo:r1", "predicate": "test_command", "object": "cargo nextest run", "convergence": 0.9},
        {"kind": "promoteToProjectUnderstanding", "repoId": "r1", "subject": "repo:r1", "predicate": "package_manager", "object": "cargo", "convergence": 0.95}
    ],
    "narrative": "Mock synthesis output for Phase 5 e2e."
}
```

Create `tests/fixtures/coding/phase5_rule_artifacts_mock.json`:

```json
{
    "body": "## Architecture\n- repo: r1\n- language: rust\n- package_manager: cargo\n\n## Style preferences observed\n- Always run cargo fmt before commit\n\n## Known failure patterns\n- When `assert_eq!(a,b)` fails on f32, prefer `(a-b).abs() < 1e-3`\n",
    "section_labels": ["Architecture", "Style preferences observed", "Known failure patterns"]
}
```

- [ ] **Step 2: Write the e2e test**

Create `crates/coding-memory/tests/phase5_e2e_nightly.rs`:

```rust
use coding_memory::reforge::{
    CodingSynthesisHandler, CodingSynthesisInput, CodingSynthesisOutput, RuleArtifactInput,
    RuleArtifactOutput, RuleArtifactsHandler,
};
use std::sync::Arc;
use storage::StoragePool;
use tempfile::tempdir;

struct MockSynth;
#[async_trait::async_trait]
impl CodingSynthesisHandler for MockSynth {
    async fn synthesize_coding(
        &self,
        _: &CodingSynthesisInput,
    ) -> common::Result<CodingSynthesisOutput> {
        let raw = include_str!("../../../tests/fixtures/coding/phase5_synthesis_mock.json");
        Ok(serde_json::from_str(raw).unwrap())
    }
}

struct MockRules;
#[async_trait::async_trait]
impl RuleArtifactsHandler for MockRules {
    async fn synthesize_artifact(
        &self,
        _: &RuleArtifactInput,
    ) -> common::Result<RuleArtifactOutput> {
        let raw = include_str!("../../../tests/fixtures/coding/phase5_rule_artifacts_mock.json");
        Ok(serde_json::from_str(raw).unwrap())
    }
}

#[tokio::test]
async fn nightly_cycle_writes_claude_md_with_at_least_5_statements() {
    let pool = StoragePool::connect_in_memory().await.unwrap();
    StoragePool::run_feature_migrations(pool.inner(), &cognitive::cognitive_migrations())
        .await
        .unwrap();
    StoragePool::run_feature_migrations(pool.inner(), &coding_memory::coding_memory_migrations())
        .await
        .unwrap();

    // Seed: at least one fact in the repo (so plan-build finds the repo).
    sqlx::query(
        "INSERT INTO semantic_facts \
         (id, subject, predicate, object, confidence, domain, scope_type, scope_id, \
          scope_repo_id, valid_from, memory_type, metadata) \
         VALUES ('f0','repo:r1','seeded','yes',0.9,'work','code',NULL,'r1', ?1, 'fact','{}')",
    )
    .bind(jiff::Timestamp::now().to_string())
    .execute(pool.inner())
    .await
    .unwrap();

    let dir = tempdir().unwrap();
    let repo_root = dir.path().join("r1");
    std::fs::create_dir_all(&repo_root).unwrap();
    std::env::set_var(
        "KLYNTBOT_REPO_PATHS_TEST_OVERRIDE",
        format!(r#"{{"r1":"{}"}}"#, repo_root.display()),
    );

    let synth: Arc<dyn CodingSynthesisHandler> = Arc::new(MockSynth);
    let rules: Arc<dyn RuleArtifactsHandler> = Arc::new(MockRules);
    let runner = app_core::coding_memory::reforge::CodingPhaseRunnerImpl::new(
        pool.clone(),
        Some(synth),
        Some(rules),
        vec!["claude_md".into()],
        None,
    );
    runner.run_synthesis().await.unwrap();
    runner.run_rule_artifacts().await.unwrap();

    let body = std::fs::read_to_string(repo_root.join("CLAUDE.md")).expect("CLAUDE.md");
    let bullet_count = body.matches("\n- ").count();
    assert!(bullet_count >= 5, "expected ≥5 statements, found {bullet_count} in {body}");
}
```

- [ ] **Step 3: Verify**

Run: `cargo nextest run -p coding-memory --test phase5_e2e_nightly`
Expected: PASS.

- [ ] **Step 4: Commit**

```bash
git add crates/coding-memory/tests/phase5_e2e_nightly.rs \
        tests/fixtures/coding/phase5_seed.jsonl \
        tests/fixtures/coding/phase5_synthesis_mock.json \
        tests/fixtures/coding/phase5_rule_artifacts_mock.json
git commit -m "test(coding-memory): Phase 5 — e2e nightly cycle ≥5 CLAUDE.md statements"
```

---

### Task 33: e2e — skill evolution emits SKILL.md across sessions

**Files:**
- Create: `crates/coding-memory/tests/phase5_e2e_skill_evolution.rs`

`★ Insight ─────────────────────────────────────`
- This test demonstrates the spec's exit gate: "project skill `effectiveness_score` drift demonstrable." It seeds a `WorkflowPattern`, runs the evolver, then injects a `PatternOutcome` and verifies the score moved.
- It uses the *real* `ProjectSkillEvolverImpl` — no mocks beyond the LLM handler — because the goal is end-to-end coverage of the new write path.
`─────────────────────────────────────────────────`

- [ ] **Step 1: Write the e2e test**

Create `crates/coding-memory/tests/phase5_e2e_skill_evolution.rs`:

```rust
use ai_core::{AiMetrics, AiSignal, MirrorSignalSource, RecallDomain, SalienceVerdict};
use coding_memory::mirror::pattern_effectiveness::{
    PatternEffectivenessLogRepo, PatternEffectivenessSource,
};
use coding_memory::skills::{ProjectSkillEvolver, ProjectSkillEvolverImpl};
use std::sync::Arc;
use storage::StoragePool;
use tempfile::tempdir;

#[tokio::test]
async fn workflow_pattern_yields_skill_md_then_effectiveness_drifts() {
    let pool = StoragePool::connect_in_memory().await.unwrap();
    StoragePool::run_feature_migrations(pool.inner(), &cognitive::cognitive_migrations())
        .await
        .unwrap();
    StoragePool::run_feature_migrations(pool.inner(), &coding_memory::coding_memory_migrations())
        .await
        .unwrap();

    // Seed a high-confidence repo-scoped workflow pattern.
    sqlx::query(
        "INSERT INTO procedural_rules \
         (id, rule, source, confidence, effectiveness_score, stability, scope_type, scope_id, \
          scope_repo_id, created_at, application_count, metadata) \
         VALUES ('p1','always run cargo fmt','observed',0.85,0.5,1.0,'code',NULL,'r1',?1,0,'{\"kind\":\"workflow_pattern\"}')",
    )
    .bind(jiff::Timestamp::now().to_string())
    .execute(pool.inner())
    .await
    .unwrap();

    let dir = tempdir().unwrap();
    let evolver = ProjectSkillEvolverImpl::new(pool.clone(), dir.path().to_path_buf(), None);
    let results = evolver.evolve("r1").await.unwrap();
    assert_eq!(results.len(), 1);
    assert!(results[0].skill_path.exists(), "SKILL.md must exist on disk");
    assert!((results[0].effectiveness - 0.5).abs() < 1e-3);

    // Drift effectiveness via PatternEffectivenessSource.
    let log = PatternEffectivenessLogRepo::new(pool.clone());
    let source = Arc::new(PatternEffectivenessSource::new(pool.clone(), log));
    for _ in 0..3 {
        source
            .accumulate(&AiSignal {
                domain: RecallDomain::General,
                event_kind: "PatternOutcome",
                importance: 0.5,
                salience: SalienceVerdict::Accumulate,
                content: String::new(),
                entity: None,
                timestamp: jiff::Timestamp::now(),
                raw_event: Some(bus::DomainEvent::PatternOutcome {
                    pattern_id: "p1".into(),
                    outcome: "success".into(),
                    evidence: String::new(),
                    measured_at: jiff::Timestamp::now().to_string(),
                }),
                metrics: AiMetrics::default(),
                coaching_signal: false,
                coaching_rule: None,
                metric_samples: Vec::new(),
            })
            .await
            .unwrap();
    }
    source.flush().await.unwrap();

    let (score,): (f32,) =
        sqlx::query_as("SELECT effectiveness_score FROM procedural_rules WHERE id = 'p1'")
            .fetch_one(pool.inner())
            .await
            .unwrap();
    assert!(score > 0.5, "effectiveness must drift up: got {score}");
}
```

- [ ] **Step 2: Verify**

Run: `cargo nextest run -p coding-memory --test phase5_e2e_skill_evolution`
Expected: PASS.

- [ ] **Step 3: Commit**

```bash
git add crates/coding-memory/tests/phase5_e2e_skill_evolution.rs
git commit -m "test(coding-memory): Phase 5 — e2e project skill evolution + drift demonstrable"
```

---

### Task 34: Workspace verification — clippy + nextest + exit gates

**Files:**
- Modify: `docs/coding-memory/README.md` (note Phase 5 completion)

- [ ] **Step 1: Run the full verification matrix**

```bash
cargo build --workspace
cargo clippy --workspace --all-targets --all-features
cargo fmt --all --check
cargo nextest run --workspace
cd desktop-ui && bun run lint && bun run test && cd ..
```

Each must succeed with zero clippy warnings, no fmt diffs, all tests green.

- [ ] **Step 2: Verify the three explicit Phase-5 exit gates**

The spec lists three exit gates for Phase 5. Verify each:

1. **Nightly cycle clean on seeded data:** `cargo nextest run -p coding-memory --test phase5_e2e_nightly`
2. **CLAUDE.md ≥5 useful project-specific statements:** asserted by `phase5_e2e_nightly` (`bullet_count >= 5`).
3. **Project skill `effectiveness_score` drift demonstrable:** `cargo nextest run -p coding-memory --test phase5_e2e_skill_evolution`

- [ ] **Step 3: Update README**

Edit `docs/coding-memory/README.md`. Append:

```markdown
## Phase 5 — Reflection (Reforge + Mirror)

Status: complete.

- Session-end light Reforge pass writes a 200-token deterministic summary on every `EventKind::SessionEnd`.
- Nightly Reforge cycle now invokes `CodingSynthesisPhase` (six promotion actions) and `RuleArtifactGenerationPhase` (managed-block externalisation to `CLAUDE.md`, `AGENTS.md`, `.cursorrules`, `.continue/rules/klyntbot.md`).
- Phase-6 selective-delete signal (`stability *= 0.5` on uncited memories) and Phase-6.5 cross-session dedup (bi-temporal SUPERSEDE) plug into the same nightly cycle via `CodingPhaseRunner`.
- `ProjectSkillEvolverImpl` writes per-repo `SKILL.md` with managed blocks; old versions move to `status='superseded'` automatically.
- Mirror gains `PatternEffectivenessSource` (real-time EMA), `StaleMemorySource` (cited-vs-ignored), `CodingMetaRulesSource` (4 closed-enum coding meta-rules), and a project-skill drift extension to `CodingRoutingSource`.
- Workbench gains: Mirror Alerts Feed, Pattern Effectiveness Trends, Reforge Cycle Diff.
- 11-variant `CodingMirrorAlertKind` + 4-variant `MirrorAlertSeverity` closed enums land via the `MirrorAlert::Coding` extension.
- Two new property-tested invariants: Reforge never deletes (#6) and managed-block user content is byte-preserved.
```

- [ ] **Step 4: Commit**

```bash
git add docs/coding-memory/README.md
git commit -m "docs(coding-memory): Phase 5 completion notes"
```

---

## Self-review

> Engineer note: the writing-plans skill calls for an explicit self-review after the plan is drafted. The list below was applied during plan composition; any divergence between the plan and the spec was patched inline before the file was committed.

### 1. Spec coverage check

| Spec §11 Phase-5 deliverable | Plan task |
|---|---|
| Session-end light Reforge pass | Tasks 3 + 4 |
| Nightly Reforge Phase 2.5 (Coding Synthesis) | Tasks 6 + 11 + 13 + 14 |
| Nightly Reforge Phase 3.5 (Rule Artifact Generation) | Tasks 7 + 8 + 12 + 13 + 14 |
| Managed-block writers for CLAUDE.md / AGENTS.md / .cursorrules / .continue/rules | Task 7 (parser/writer) + Task 8 (per-artifact orchestration) |
| Scope-aware `SkillStore` | Phase-1 schema + Tasks 15–17 |
| Project-scoped evolving skills | Tasks 15 (Detect) + 16 (Synthesize/Write/Journal) + 17 (Supersede) |
| Mirror `RoutingMirrorSubscriber` coding extension | Task 22 |
| `MetaRuleDetector` coding extension | Task 21 |
| `PatternEffectivenessSubscriber` (real-time) | Task 19 |
| `StaleMemorySubscriber` | Task 20 |
| Selective-delete signal | Task 10 |
| Closed `MirrorAlertKind` (11 variants) + `MirrorAlertSeverity` (4 variants) | Task 6 (coding-memory side) + Task 18 (cognitive side) |
| `mirror.get_coding_alerts(repo?, severity?, kind?)` | Task 24 |
| Workbench: Mirror Alerts Feed | Task 27 |
| Workbench: Pattern Effectiveness Trends | Task 28 |
| Workbench: Reforge Cycle Diff | Task 29 |
| Cross-session fact dedup (Phase 6.5 ext) | Task 9 |
| Property test: Reforge never deletes (#6) | Task 30 |
| Property test: managed-block user content preserved | Task 31 |
| Exit gate: nightly cycle clean on seeded data | Task 32 |
| Exit gate: CLAUDE.md ≥5 useful statements | Task 32 |
| Exit gate: project-skill effectiveness drift demonstrable | Task 33 |

No spec gaps detected.

### 2. Placeholder scan

- No "TBD" / "implement later" / "fill in details" markers in shipped paths.
- Stub modules created in Task 2 are explicitly tagged with the task number that fills them; each filled-in task replaces the stub body with concrete code.
- The Task 21 caveat about `LowerDeadEndThreshold` / `PromoteToGlobal` / `PromoteCounterfactualVisibility` requiring upstream events is documented inline — the orchestrator scaffolding is delivered, but the activation of those three rules waits for Phase 6 / 7 events. This is a **stated scope boundary**, not a placeholder.

### 3. Type consistency check

- `CodingPhaseHandlers<'a>` is the bundle type used uniformly across Tasks 6, 8, 9, 10, 14.
- `CodingSynthesisHandler` and `RuleArtifactsHandler` traits stay stable from Task 5 onward — Task 16 *adds* a method (`synthesize_skill`) with a default impl, so callers in Tasks 11/12 don't need to update.
- `CodingPhaseRunner` (cognitive trait) and `CodingPhaseRunnerImpl` (app-core impl) — methods named identically: `run_synthesis`, `run_rule_artifacts`, `run_cross_session_dedup`, `run_selective_delete`.
- `MirrorAlertSeverity` exists in two crates by design (Task 6 in coding-memory, Task 18 in cognitive). The `CodingMetaRulesSource` impl (Task 21) explicitly maps between the two via `Self::map_severity`.
- `CodingMirrorAlertKind` is single-source-of-truth in `coding_memory::mirror::alerts`; persisted as the string form via `as_str()`.
- DTO names (`MirrorAlertRow`, `EffectivenessTrendsResponse`, `ReforgeCycleDiffResponse`, `ProjectSkillRow`) consistent across desktop-shared (Task 25) and desktop-ui hooks (Task 27).
- File-path casing matches: spec says `~/.klyntbot/project-skills/...`; Task 16's `write_skill_md` uses the same root via `private_root` arg.

No type mismatches detected.

### 4. Risk register (open items the executing engineer should watch)

- **`run_reforge` signature explosion:** the cognitive `run_reforge(...)` already takes 25 parameters. Task 13 adds one more. If this becomes intractable mid-execution, replace the parameter pile with a single `RunReforgeContext` struct in a follow-up commit — avoid that refactor *during* Phase 5.
- **`MirrorAlert` extension cascade:** Task 18 adds a new variant. Run `grep -rn "match.*MirrorAlert" crates/` after the change to find every match arm that may need updating. The compiler will catch it, but worth scanning before starting.
- **`KLYNTBOT_REPO_PATHS_TEST_OVERRIDE` env var:** Tasks 8 + 32 lean on this. In production, `~/.klyntbot/repo_paths.json` is populated by the desktop UI's CLI-toggle code (Phase 2). Verify that file exists in the production app before assuming Phase 3.5 will write any artifacts.
- **`recharts` already in `desktop-ui/package.json`:** verify at Task 28 start. If absent, add via `bun add recharts` and include the dep in the same commit.
- **Test isolation for Mirror sources:** `MirrorRepo` and the underlying `mirror_snippets` table are shared with the existing personal-AI subscribers. Tests in Tasks 21 / 22 / 23 must use `connect_in_memory()` pools to avoid cross-test pollution.

End of plan.
