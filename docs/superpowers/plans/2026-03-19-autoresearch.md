# Autoresearch: Self-Optimizing Agent — Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Build a self-optimizing experiment loop that shadow-scores routing parameter variants on live traffic, evaluates them against multi-metric constraints, and promotes winners — all guided by LLM reasoning.

**Architecture:** New `autotuner` crate at L4 holds pure evaluation/generation logic. Thin orchestrator in `agent/autotuner/` (L5) wires shadow classification and metric collection to the runtime. `TrialParams` lives in `common` (L0) for cross-layer access. Storage in SQLite via `TrialRepo`. Nightly experiment cycle via `CronService`.

**Tech Stack:** Rust, SQLite (via `storage::SqlitePool`), `serde_json`, `async-trait`, `chrono`, `uuid`, React/TypeScript (desktop-ui), Tauri 2 commands.

**Spec:** `docs/superpowers/specs/2026-03-19-autoresearch-design.md`

---

## File Structure

### New files

```
crates/common/src/autotuner.rs                    — TrialParams (pure value object)
crates/config/src/schema/autotuner.rs              — AutoTunerConfig (bounds, constraints, schedule)
crates/storage/src/repos/trial_repo.rs             — TrialRepo (CRUD for trials + experiments)
crates/storage/src/rows/trial.rs                   — TrialRow, ExperimentRow
crates/autotuner/Cargo.toml                        — New crate manifest
crates/autotuner/src/lib.rs                        — Pub exports
crates/autotuner/src/trial.rs                      — Trial, TrialResult, TrialStatus, Experiment, Champion
crates/autotuner/src/evaluator.rs                  — ConstraintEvaluator (multi-metric check, diversity bonus)
crates/autotuner/src/metrics.rs                    — MetricSnapshot, MetricAggregator
crates/autotuner/src/generator.rs                  — VariantGenerator (LLM-guided)
crates/autotuner/src/cycle.rs                      — NightlyCycle (evaluate → promote → generate → activate)
crates/autotuner/src/events.rs                     — AutoTunerEvent enum
crates/autotuner/src/traits.rs                     — ShadowClassifier, MetricSource traits
crates/autotuner/src/config.rs                     — Re-export of AutoTunerConfig with param bounds
crates/agent/src/autotuner/mod.rs                  — AutoTunerOrchestrator
crates/agent/src/autotuner/shadow_classifier.rs    — impl ShadowClassifier
crates/agent/src/autotuner/metric_collector.rs     — impl MetricSource
crates/agent/src/autotuner/hooks.rs                — AutoTunerHook trait
crates/app-core/src/handlers/autotuner.rs          — Tauri-facing handler functions
crates/desktop/src/commands/autotuner.rs           — Tauri command wrappers
crates/desktop-shared/src/events.rs                — AutoTuner event types for frontend (extend existing)
desktop-ui/src/features/autotuner/types.ts         — TypeScript types
desktop-ui/src/features/autotuner/hooks/useAutoTunerStatus.ts
desktop-ui/src/features/autotuner/hooks/useAutoTunerHistory.ts
desktop-ui/src/features/autotuner/components/AutoTunerPanel.tsx
desktop-ui/src/features/autotuner/components/ChampionCard.tsx
desktop-ui/src/features/autotuner/components/ExperimentTimeline.tsx
desktop-ui/src/features/autotuner/components/AmbientIndicator.tsx
```

### Modified files

```
Cargo.toml                                         — Add "crates/autotuner" to workspace members
crates/common/src/lib.rs:8-16                      — Add `pub mod autotuner;` + re-export
crates/tools-core/src/routing.rs:59-82             — Add `champion_params` to RoutingContext
crates/config/src/schema/mod.rs:17-44              — Add `mod autotuner;` + re-export
crates/config/src/schema/core.rs:96+               — Add `autotuner` field to Config struct
crates/storage/src/repos/mod.rs:3-37               — Add `pub mod trial_repo;`
crates/storage/src/rows/mod.rs                     — Add `pub mod trial;`
crates/skill-system/src/router.rs:60-99            — Add weight override params to select_orchestrator_blended()
crates/agent/src/intent_pipeline/analysis.rs:1154+ — Add `overrides` field to IntentAnalyzer
crates/agent/src/agent_runtime/runtime.rs:214+     — Hook AutoTuner into process_message()
crates/agent/src/events.rs:100+                    — Add AutoTuner event variants to AgentEvent
crates/agent/src/lib.rs                            — Add `pub mod autotuner;`
crates/desktop/src/commands/mod.rs                 — Register autotuner commands
crates/desktop/src/dev_server/mod.rs               — Add autotuner to DEV_COMMANDS test coverage
```

---

## Task 1: TrialParams in `common` (L0 foundation type)

**Files:**
- Create: `crates/common/src/autotuner.rs`
- Modify: `crates/common/src/lib.rs:8-16`

- [ ] **Step 1: Write the test**

```rust
// In crates/common/src/autotuner.rs (bottom of file, after the struct)
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn trial_params_default_is_all_none() {
        let params = TrialParams::default();
        assert!(params.skill_keyword_weight.is_none());
        assert!(params.relevance_weight_semantic.is_none());
    }

    #[test]
    fn trial_params_roundtrip_serde() {
        let params = TrialParams {
            skill_keyword_weight: Some(0.65),
            skill_semantic_weight: Some(0.35),
            ..Default::default()
        };
        let json = serde_json::to_string(&params).unwrap();
        let back: TrialParams = serde_json::from_str(&json).unwrap();
        assert_eq!(back.skill_keyword_weight, Some(0.65));
        assert!(back.heuristic_confidence_threshold.is_none());
    }

    #[test]
    fn trial_params_forward_compat_deserialization() {
        // Simulates Phase 2 adding new fields — Phase 1 JSON should still deserialize
        let phase1_json = r#"{"skill_keyword_weight": 0.6}"#;
        let params: TrialParams = serde_json::from_str(phase1_json).unwrap();
        assert_eq!(params.skill_keyword_weight, Some(0.6));
        assert!(params.relevance_weight_semantic.is_none());
    }

    #[test]
    fn normalize_relevance_weights_sums_to_one() {
        let params = TrialParams {
            relevance_weight_semantic: Some(0.40),
            relevance_weight_retrievability: Some(0.25),
            relevance_weight_situation: Some(0.30),
            ..Default::default()
        };
        // CognitiveConfig defaults: semantic=0.30, retrievability=0.20, importance=0.15,
        // frequency=0.10, situation=0.25, temporal=0.05 (sum=1.05)
        let weights = params.resolve_relevance_weights(0.15, 0.10, 0.05);
        let sum: f64 = weights.iter().sum();
        assert!((sum - 1.0).abs() < 1e-10, "Weights must sum to 1.0, got {sum}");
    }
}
```

- [ ] **Step 2: Write the implementation**

```rust
// crates/common/src/autotuner.rs
use serde::{Deserialize, Serialize};

/// Per-request parameter overrides for autotuner experiments.
/// Each field is Option — None means "use Config default."
/// All fields are #[serde(default)] for forward-compatible deserialization.
#[derive(Clone, Debug, Serialize, Deserialize, Default, PartialEq)]
#[serde(default)]
pub struct TrialParams {
    // Phase 1: SkillRouter knobs
    pub skill_keyword_weight: Option<f64>,
    pub skill_semantic_weight: Option<f64>,
    pub skill_activation_threshold: Option<f64>,

    // Phase 1: IntentAnalyzer knobs
    pub heuristic_confidence_threshold: Option<f64>,
    pub llm_classifier_timeout_ms: Option<u64>,

    // Phase 1: Cognitive retrieval relevance weights (3 of 6 tuned in Phase 1)
    pub relevance_weight_semantic: Option<f64>,
    pub relevance_weight_retrievability: Option<f64>,
    pub relevance_weight_situation: Option<f64>,
}

impl TrialParams {
    /// Resolve all 6 relevance weights to a normalized array that sums to 1.0.
    /// Phase 1 tunes 3 weights; the other 3 come from Config defaults.
    /// Returns [semantic, retrievability, importance, frequency, situation, temporal].
    pub fn resolve_relevance_weights(
        &self,
        default_importance: f64,
        default_frequency: f64,
        default_temporal: f64,
    ) -> [f64; 6] {
        let raw = [
            self.relevance_weight_semantic.unwrap_or(0.30),
            self.relevance_weight_retrievability.unwrap_or(0.20),
            default_importance,
            default_frequency,
            self.relevance_weight_situation.unwrap_or(0.25),
            default_temporal,
        ];
        let sum: f64 = raw.iter().sum();
        if sum == 0.0 {
            return [1.0 / 6.0; 6];
        }
        raw.map(|w| w / sum)
    }
}
```

- [ ] **Step 3: Add module to common/lib.rs**

Add `pub mod autotuner;` to `crates/common/src/lib.rs` in the module list (after line 12, before `pub mod ports;`). Add `pub use autotuner::TrialParams;` to the re-export section.

- [ ] **Step 4: Run tests**

Run: `cargo nextest run -p common -E 'test(trial_params)'`
Expected: 4 tests PASS

- [ ] **Step 5: Commit**

```bash
git add crates/common/src/autotuner.rs crates/common/src/lib.rs
git commit -m "feat(common): add TrialParams value object for autotuner experiments"
```

---

## Task 2: AutoTunerConfig in `config`

**Files:**
- Create: `crates/config/src/schema/autotuner.rs`
- Modify: `crates/config/src/schema/mod.rs:17-44`
- Modify: `crates/config/src/schema/core.rs:96+`

- [ ] **Step 1: Write the config struct with tests**

```rust
// crates/config/src/schema/autotuner.rs
use serde::{Deserialize, Serialize};

/// Configuration for the autotuner self-optimization system.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AutoTunerConfig {
    /// Whether the autotuner is enabled (default: false — opt-in).
    #[serde(default)]
    pub enabled: bool,

    /// Cron expression for the nightly experiment cycle (default: "0 2 * * *").
    #[serde(default = "default_schedule")]
    pub schedule: String,

    /// Minimum messages before a trial is eligible for promotion.
    #[serde(default = "default_min_messages")]
    pub min_messages_for_promotion: u32,

    /// Number of consecutive regression days before auto-rollback.
    #[serde(default = "default_rollback_days")]
    pub rollback_after_days: u8,

    /// Experiment pace: "conservative", "balanced", or "bold".
    #[serde(default = "default_pace")]
    pub experiment_pace: String,

    // Promotion constraint thresholds
    /// Minimum correction rate improvement required (default: 0.05 = 5%).
    #[serde(default = "default_correction_improvement")]
    pub min_correction_improvement: f64,
    /// Maximum token cost increase allowed (default: 0.08 = 8%).
    #[serde(default = "default_max_token_increase")]
    pub max_token_cost_increase: f64,
    /// Maximum response time increase allowed (default: 0.15 = 15%).
    #[serde(default = "default_max_response_time_increase")]
    pub max_response_time_increase: f64,
    /// Maximum routing stability decrease allowed (default: 0.10 = 10%).
    #[serde(default = "default_max_stability_decrease")]
    pub max_routing_stability_decrease: f64,
    /// Maximum memory relevance decrease allowed (default: 0.05 = 5%).
    #[serde(default = "default_max_relevance_decrease")]
    pub max_memory_relevance_decrease: f64,
}

impl Default for AutoTunerConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            schedule: default_schedule(),
            min_messages_for_promotion: default_min_messages(),
            rollback_after_days: default_rollback_days(),
            experiment_pace: default_pace(),
            min_correction_improvement: default_correction_improvement(),
            max_token_cost_increase: default_max_token_increase(),
            max_response_time_increase: default_max_response_time_increase(),
            max_routing_stability_decrease: default_max_stability_decrease(),
            max_memory_relevance_decrease: default_max_relevance_decrease(),
        }
    }
}

fn default_schedule() -> String { "0 2 * * *".to_string() }
fn default_min_messages() -> u32 { 50 }
fn default_rollback_days() -> u8 { 3 }
fn default_pace() -> String { "balanced".to_string() }
fn default_correction_improvement() -> f64 { 0.05 }
fn default_max_token_increase() -> f64 { 0.08 }
fn default_max_response_time_increase() -> f64 { 0.15 }
fn default_max_stability_decrease() -> f64 { 0.10 }
fn default_max_relevance_decrease() -> f64 { 0.05 }

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_config_is_disabled() {
        let config = AutoTunerConfig::default();
        assert!(!config.enabled);
        assert_eq!(config.schedule, "0 2 * * *");
        assert_eq!(config.min_messages_for_promotion, 50);
        assert_eq!(config.rollback_after_days, 3);
    }

    #[test]
    fn camel_case_serde_roundtrip() {
        let config = AutoTunerConfig::default();
        let json = serde_json::to_string(&config).unwrap();
        assert!(json.contains("experimentPace"));
        assert!(json.contains("minMessagesForPromotion"));
        let back: AutoTunerConfig = serde_json::from_str(&json).unwrap();
        assert_eq!(back.experiment_pace, "balanced");
    }
}
```

- [ ] **Step 2: Wire into config module**

In `crates/config/src/schema/mod.rs`, add `mod autotuner;` (after `mod agents;` at line 17). Add `pub use autotuner::AutoTunerConfig;` to the re-exports.

In `crates/config/src/schema/core.rs`, add to the Config struct:
```rust
#[serde(default)]
pub autotuner: AutoTunerConfig,
```

- [ ] **Step 3: Run tests**

Run: `cargo nextest run -p config -E 'test(autotuner)'`
Expected: 2 tests PASS

Run: `cargo nextest run -p config` to verify no regressions.

- [ ] **Step 4: Commit**

```bash
git add crates/config/src/schema/autotuner.rs crates/config/src/schema/mod.rs crates/config/src/schema/core.rs
git commit -m "feat(config): add AutoTunerConfig with constraint thresholds and schedule"
```

---

## Task 3: RoutingContext update in `tools-core`

**Files:**
- Modify: `crates/tools-core/src/routing.rs:59-82`

- [ ] **Step 1: Add champion_params field**

Add to the `RoutingContext` struct (after `squad_mode` field):
```rust
pub champion_params: Option<common::TrialParams>,
```

- [ ] **Step 2: Fix all construction sites**

Search for `RoutingContext {` across the codebase and add `champion_params: None,` to each. These are likely in:
- `crates/agent/src/agent_runtime/runtime.rs`
- `crates/agent/src/adapters/`
- `crates/channels/src/`
- Tests in `tests/`

Run: `cargo build --workspace` to find all sites.

- [ ] **Step 3: Verify builds clean**

Run: `cargo build --workspace`
Expected: 0 errors, 0 warnings

- [ ] **Step 4: Commit**

```bash
git add -u
git commit -m "feat(tools-core): add champion_params to RoutingContext for autotuner"
```

---

## Task 4: Storage — TrialRepo + migration

**Files:**
- Create: `crates/storage/src/rows/trial.rs`
- Create: `crates/storage/src/repos/trial_repo.rs`
- Modify: `crates/storage/src/rows/mod.rs`
- Modify: `crates/storage/src/repos/mod.rs:3-37`

- [ ] **Step 1: Write the row types**

```rust
// crates/storage/src/rows/trial.rs
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct TrialRow {
    pub id: String,           // UUID as string
    pub experiment_id: String,
    pub params: String,       // JSON-serialized TrialParams
    pub generation_reasoning: String,
    pub status: String,       // "pending", "active", "completed", "promoted", "reverted"
    pub created_at: String,
    pub completed_at: Option<String>,
    pub result: Option<String>, // JSON-serialized TrialResult
}

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct ExperimentRow {
    pub id: String,
    pub hypothesis: String,
    pub trend_analysis: String,
    pub recommendation_for_next: String,
    pub created_at: String,
}
```

- [ ] **Step 2: Write the TrialRepo with migration SQL**

```rust
// crates/storage/src/repos/trial_repo.rs
use crate::{rows::trial::{ExperimentRow, TrialRow}, SqlitePool, StorageError};

#[derive(Clone)]
pub struct TrialRepo {
    pool: SqlitePool,
}

impl TrialRepo {
    pub fn new(pool: SqlitePool) -> Self {
        Self { pool }
    }

    pub const MIGRATION_SQL: &str = "
        CREATE TABLE IF NOT EXISTS autotuner_experiments (
            id TEXT PRIMARY KEY,
            hypothesis TEXT NOT NULL DEFAULT '',
            trend_analysis TEXT NOT NULL DEFAULT '',
            recommendation_for_next TEXT NOT NULL DEFAULT '',
            created_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%SZ', 'now'))
        );

        CREATE TABLE IF NOT EXISTS autotuner_trials (
            id TEXT PRIMARY KEY,
            experiment_id TEXT NOT NULL REFERENCES autotuner_experiments(id),
            params TEXT NOT NULL DEFAULT '{}',
            generation_reasoning TEXT NOT NULL DEFAULT '',
            status TEXT NOT NULL DEFAULT 'pending',
            created_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%SZ', 'now')),
            completed_at TEXT,
            result TEXT
        );

        CREATE INDEX IF NOT EXISTS idx_trials_status ON autotuner_trials(status);
        CREATE INDEX IF NOT EXISTS idx_trials_experiment ON autotuner_trials(experiment_id);

        CREATE TABLE IF NOT EXISTS autotuner_shadow_log (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            trial_id TEXT NOT NULL,
            message_timestamp TEXT NOT NULL,
            chat_id TEXT NOT NULL,
            predicted_orchestrator TEXT NOT NULL,
            predicted_mode TEXT NOT NULL,
            confidence REAL NOT NULL,
            predicted_iteration_budget INTEGER NOT NULL,
            control_orchestrator TEXT,
            control_mode TEXT,
            user_corrected INTEGER NOT NULL DEFAULT 0,
            created_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%SZ', 'now'))
        );

        CREATE INDEX IF NOT EXISTS idx_shadow_trial ON autotuner_shadow_log(trial_id);
    ";

    pub async fn create_experiment(&self, row: &ExperimentRow) -> Result<(), StorageError> {
        sqlx::query(
            "INSERT INTO autotuner_experiments (id, hypothesis, trend_analysis, recommendation_for_next, created_at)
             VALUES (?, ?, ?, ?, ?)"
        )
        .bind(&row.id)
        .bind(&row.hypothesis)
        .bind(&row.trend_analysis)
        .bind(&row.recommendation_for_next)
        .bind(&row.created_at)
        .execute(self.pool.inner())
        .await?;
        Ok(())
    }

    pub async fn create_trial(&self, row: &TrialRow) -> Result<(), StorageError> {
        sqlx::query(
            "INSERT INTO autotuner_trials (id, experiment_id, params, generation_reasoning, status, created_at)
             VALUES (?, ?, ?, ?, ?, ?)"
        )
        .bind(&row.id)
        .bind(&row.experiment_id)
        .bind(&row.params)
        .bind(&row.generation_reasoning)
        .bind(&row.status)
        .bind(&row.created_at)
        .execute(self.pool.inner())
        .await?;
        Ok(())
    }

    pub async fn update_trial_status(&self, id: &str, status: &str) -> Result<(), StorageError> {
        sqlx::query("UPDATE autotuner_trials SET status = ? WHERE id = ?")
            .bind(status)
            .bind(id)
            .execute(self.pool.inner())
            .await?;
        Ok(())
    }

    pub async fn complete_trial(&self, id: &str, result_json: &str) -> Result<(), StorageError> {
        sqlx::query(
            "UPDATE autotuner_trials SET status = 'completed', completed_at = strftime('%Y-%m-%dT%H:%M:%SZ', 'now'), result = ? WHERE id = ?"
        )
        .bind(result_json)
        .bind(id)
        .execute(self.pool.inner())
        .await?;
        Ok(())
    }

    pub async fn get_active_trials(&self) -> Result<Vec<TrialRow>, StorageError> {
        let rows = sqlx::query_as::<_, TrialRow>(
            "SELECT * FROM autotuner_trials WHERE status = 'active' ORDER BY created_at"
        )
        .fetch_all(self.pool.inner())
        .await?;
        Ok(rows)
    }

    pub async fn get_recent_completed(&self, limit: u32) -> Result<Vec<TrialRow>, StorageError> {
        let rows = sqlx::query_as::<_, TrialRow>(
            "SELECT * FROM autotuner_trials WHERE status IN ('completed', 'promoted', 'reverted') ORDER BY completed_at DESC LIMIT ?"
        )
        .bind(limit)
        .fetch_all(self.pool.inner())
        .await?;
        Ok(rows)
    }

    pub async fn get_experiments(&self, limit: u32) -> Result<Vec<ExperimentRow>, StorageError> {
        let rows = sqlx::query_as::<_, ExperimentRow>(
            "SELECT * FROM autotuner_experiments ORDER BY created_at DESC LIMIT ?"
        )
        .bind(limit)
        .fetch_all(self.pool.inner())
        .await?;
        Ok(rows)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::SqlitePool;

    async fn setup() -> (SqlitePool, TrialRepo) {
        let pool = SqlitePool::connect_in_memory().await.unwrap();
        sqlx::raw_sql(TrialRepo::MIGRATION_SQL)
            .execute(pool.inner())
            .await
            .unwrap();
        let repo = TrialRepo::new(pool.clone());
        (pool, repo)
    }

    #[tokio::test]
    async fn create_and_retrieve_trial() {
        let (_pool, repo) = setup().await;

        let exp = ExperimentRow {
            id: "exp-1".into(),
            hypothesis: "test hypothesis".into(),
            trend_analysis: "".into(),
            recommendation_for_next: "".into(),
            created_at: "2026-03-19T00:00:00Z".into(),
        };
        repo.create_experiment(&exp).await.unwrap();

        let trial = TrialRow {
            id: "trial-1".into(),
            experiment_id: "exp-1".into(),
            params: r#"{"skill_keyword_weight": 0.65}"#.into(),
            generation_reasoning: "Lower keyword weight for research queries".into(),
            status: "active".into(),
            created_at: "2026-03-19T00:00:00Z".into(),
            completed_at: None,
            result: None,
        };
        repo.create_trial(&trial).await.unwrap();

        let active = repo.get_active_trials().await.unwrap();
        assert_eq!(active.len(), 1);
        assert_eq!(active[0].id, "trial-1");
    }

    #[tokio::test]
    async fn complete_trial_sets_result() {
        let (_pool, repo) = setup().await;

        let exp = ExperimentRow {
            id: "exp-1".into(),
            hypothesis: "".into(),
            trend_analysis: "".into(),
            recommendation_for_next: "".into(),
            created_at: "2026-03-19T00:00:00Z".into(),
        };
        repo.create_experiment(&exp).await.unwrap();

        let trial = TrialRow {
            id: "trial-1".into(),
            experiment_id: "exp-1".into(),
            params: "{}".into(),
            generation_reasoning: "".into(),
            status: "active".into(),
            created_at: "2026-03-19T00:00:00Z".into(),
            completed_at: None,
            result: None,
        };
        repo.create_trial(&trial).await.unwrap();
        repo.complete_trial("trial-1", r#"{"correction_rate": 0.05}"#).await.unwrap();

        let completed = repo.get_recent_completed(10).await.unwrap();
        assert_eq!(completed.len(), 1);
        assert_eq!(completed[0].status, "completed");
        assert!(completed[0].result.is_some());
    }
}
```

- [ ] **Step 3: Wire into storage module**

Add `pub mod trial_repo;` to `crates/storage/src/repos/mod.rs` and `pub mod trial;` to `crates/storage/src/rows/mod.rs`. Re-export `TrialRepo` from the storage crate root.

- [ ] **Step 4: Run tests**

Run: `cargo nextest run -p storage -E 'test(trial)'`
Expected: 2 tests PASS

- [ ] **Step 5: Commit**

```bash
git add crates/storage/src/rows/trial.rs crates/storage/src/repos/trial_repo.rs crates/storage/src/rows/mod.rs crates/storage/src/repos/mod.rs
git commit -m "feat(storage): add TrialRepo with autotuner tables and shadow log"
```

---

## Task 5: SkillRouter weight overrides

**Files:**
- Modify: `crates/skill-system/src/router.rs:60-99`

- [ ] **Step 1: Write the test**

```rust
// Add to existing tests in crates/skill-system/src/router.rs
#[test]
fn select_orchestrator_blended_uses_custom_weights() {
    // When keyword_weight = 1.0 and semantic_weight = 0.0,
    // the selection should match keyword-only routing.
    // Exact test depends on existing test fixtures — adapt to use
    // the catalog builder pattern already in the test module.
    // Key assertion: passing Some(1.0), Some(0.0) gives same result as
    // select_orchestrator() (keyword-only).
}
```

- [ ] **Step 2: Modify select_orchestrator_blended signature**

Change the method signature at `crates/skill-system/src/router.rs:60`:
```rust
pub fn select_orchestrator_blended<'a>(
    &self,
    message: &str,
    query_embedding: &[f32],
    catalog: &'a SkillCatalog,
    keyword_weight: Option<f64>,   // NEW — default 0.7 if None
    semantic_weight: Option<f64>,  // NEW — default 0.3 if None
) -> &'a Arc<SkillPackage> {
```

Replace the hardcoded line `let blended = kw_score * 0.7 + sem_score * 0.3;` with:
```rust
let kw_w = keyword_weight.unwrap_or(0.7);
let sem_w = semantic_weight.unwrap_or(0.3);
let blended = kw_score * kw_w + sem_score * sem_w;
```

- [ ] **Step 3: Fix all callers**

Search for `select_orchestrator_blended(` across the codebase. Add `None, None` to existing call sites to preserve current behavior. Likely callers:
- `crates/agent/src/agent_runtime/runtime.rs`
- `crates/agent/src/intent_pipeline/` (if called there)

- [ ] **Step 4: Run tests**

Run: `cargo nextest run -p skill-system`
Expected: All tests PASS (including the new one)

Run: `cargo build --workspace` to verify no broken callers.

- [ ] **Step 5: Commit**

```bash
git add crates/skill-system/src/router.rs
git add -u  # Fix any callers
git commit -m "feat(skill-system): add weight override params to select_orchestrator_blended"
```

---

## Task 6: `autotuner` crate scaffold + core types

**Files:**
- Create: `crates/autotuner/Cargo.toml`
- Create: `crates/autotuner/src/lib.rs`
- Create: `crates/autotuner/src/trial.rs`
- Create: `crates/autotuner/src/events.rs`
- Create: `crates/autotuner/src/traits.rs`
- Modify: `Cargo.toml` (workspace members)

- [ ] **Step 1: Create Cargo.toml**

```toml
[package]
name = "autotuner"
version = "0.1.0"
edition = "2021"

[dependencies]
common = { path = "../common" }
config = { path = "../config" }
storage = { path = "../storage" }
bus = { path = "../bus" }
async-trait = "0.1"
chrono = { version = "0.4", features = ["serde"] }
uuid = { version = "1", features = ["v4", "serde"] }
serde = { version = "1", features = ["derive"] }
serde_json = "1"
tracing = "0.1"

[dev-dependencies]
tokio = { version = "1", features = ["full"] }
```

- [ ] **Step 2: Write core types in trial.rs**

```rust
// crates/autotuner/src/trial.rs
use chrono::{DateTime, Utc};
use common::TrialParams;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum TrialStatus {
    Pending,
    Active,
    Completed,
    Promoted,
    Reverted,
}

impl TrialStatus {
    pub fn as_str(&self) -> &str {
        match self {
            Self::Pending => "pending",
            Self::Active => "active",
            Self::Completed => "completed",
            Self::Promoted => "promoted",
            Self::Reverted => "reverted",
        }
    }

    pub fn from_str(s: &str) -> Self {
        match s {
            "pending" => Self::Pending,
            "active" => Self::Active,
            "completed" => Self::Completed,
            "promoted" => Self::Promoted,
            "reverted" => Self::Reverted,
            _ => Self::Pending,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Trial {
    pub id: Uuid,
    pub experiment_id: Uuid,
    pub params: TrialParams,
    pub generation_reasoning: String,
    pub status: TrialStatus,
    pub created_at: DateTime<Utc>,
    pub completed_at: Option<DateTime<Utc>>,
    pub result: Option<TrialResult>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct TrialResult {
    pub trial_id: Uuid,
    pub messages_scored: u32,
    pub correction_rate: f64,
    pub classification_accuracy: f64,
    pub avg_tokens_per_message: f64,
    pub avg_response_time_ms: f64,
    pub routing_stability: f64,
    pub memory_relevance: f64,
    pub user_satisfaction: Option<f64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Experiment {
    pub id: Uuid,
    pub created_at: DateTime<Utc>,
    pub hypothesis: String,
    pub trend_analysis: String,
    pub recommendation_for_next: String,
    pub trial_ids: Vec<Uuid>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Champion {
    pub trial_id: Option<Uuid>,
    pub params: TrialParams,
    pub promoted_at: DateTime<Utc>,
    pub baseline_metrics: TrialResult,
    pub reason_for_promotion: String,
    pub impact_summary: String,
    pub consecutive_regression_days: u8,
}

impl Default for Champion {
    fn default() -> Self {
        Self {
            trial_id: None,
            params: TrialParams::default(),
            promoted_at: Utc::now(),
            baseline_metrics: TrialResult::default(),
            reason_for_promotion: "Using Config defaults".into(),
            impact_summary: "Baseline configuration".into(),
            consecutive_regression_days: 0,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn trial_status_roundtrip() {
        for status in [TrialStatus::Pending, TrialStatus::Active, TrialStatus::Completed, TrialStatus::Promoted, TrialStatus::Reverted] {
            assert_eq!(TrialStatus::from_str(status.as_str()), status);
        }
    }

    #[test]
    fn champion_default_has_no_trial_id() {
        let c = Champion::default();
        assert!(c.trial_id.is_none());
        assert_eq!(c.reason_for_promotion, "Using Config defaults");
    }

    #[test]
    fn champion_serde_roundtrip() {
        let c = Champion::default();
        let json = serde_json::to_string(&c).unwrap();
        let back: Champion = serde_json::from_str(&json).unwrap();
        assert!(back.trial_id.is_none());
    }
}
```

- [ ] **Step 3: Write traits.rs**

```rust
// crates/autotuner/src/traits.rs
use async_trait::async_trait;
use common::TrialParams;

/// Runs classification-only shadow scoring (Layer 1-2 only).
#[async_trait]
pub trait ShadowClassifier: Send + Sync {
    async fn classify_shadow(
        &self,
        message: &str,
        context: &ShadowContext,
        params: &TrialParams,
    ) -> common::Result<ShadowPrediction>;
}

/// Collects ground truth metrics from the live pipeline.
#[async_trait]
pub trait MetricSource: Send + Sync {
    async fn collect_metrics(
        &self,
        since: chrono::DateTime<chrono::Utc>,
        trial_id: Option<uuid::Uuid>,
    ) -> common::Result<MetricSnapshot>;
}

#[derive(Debug, Clone)]
pub struct ShadowContext {
    pub chat_id: String,
    pub session_key: String,
}

#[derive(Debug, Clone)]
pub struct ShadowPrediction {
    pub predicted_orchestrator: String,
    pub predicted_mode: String,   // "direct" or "reactive"
    pub confidence: f32,
    pub predicted_iteration_budget: u32,
    pub deferred_to_llm: bool,    // true if Layer 1-2 returned None
}

#[derive(Debug, Clone, Default)]
pub struct MetricSnapshot {
    pub correction_rate: f64,
    pub classification_accuracy: f64,
    pub avg_tokens_per_message: f64,
    pub avg_response_time_ms: f64,
    pub routing_stability: f64,
    pub memory_relevance: f64,
    pub user_satisfaction: Option<f64>,
    pub total_messages: u32,
}
```

- [ ] **Step 4: Write events.rs**

```rust
// crates/autotuner/src/events.rs
use serde::{Deserialize, Serialize};
use uuid::Uuid;
use chrono::{DateTime, Utc};

/// Events emitted by the autotuner crate.
/// The L5 orchestrator maps these to AgentEvent variants.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum AutoTunerEvent {
    Report(AutoTunerReport),
    Promotion(AutoTunerPromotion),
    Rollback(AutoTunerRollback),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AutoTunerReport {
    pub champion: ChampionSummary,
    pub active_experiment: Option<ExperimentSummary>,
    pub completed_trials: Vec<TrialSummary>,
    pub trend: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AutoTunerPromotion {
    pub trial_id: Uuid,
    pub reason: String,
    pub impact: String,
    pub params_changed: Vec<ParamChange>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AutoTunerRollback {
    pub reverted_trial_id: Uuid,
    pub reason: String,
    pub reverted_to: ChampionSummary,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChampionSummary {
    pub trial_id: Option<Uuid>,
    pub description: String,
    pub impact: String,
    pub promoted_at: DateTime<Utc>,
    pub days_active: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExperimentSummary {
    pub id: Uuid,
    pub variant_count: u8,
    pub messages_scored: u32,
    pub hypothesis: String,
    pub started_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TrialSummary {
    pub id: Uuid,
    pub status: String,
    pub reasoning: String,
    pub impact: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ParamChange {
    pub name: String,
    pub old_value: f64,
    pub new_value: f64,
}
```

- [ ] **Step 5: Write lib.rs and add to workspace**

```rust
// crates/autotuner/src/lib.rs
pub mod events;
pub mod traits;
pub mod trial;

pub use events::*;
pub use traits::*;
pub use trial::*;
```

Add `"crates/autotuner",` to the workspace `members` list in the root `Cargo.toml`.

- [ ] **Step 6: Run tests**

Run: `cargo nextest run -p autotuner`
Expected: 3 tests PASS (trial_status_roundtrip, champion_default, champion_serde)

- [ ] **Step 7: Commit**

```bash
git add crates/autotuner/ Cargo.toml
git commit -m "feat(autotuner): scaffold crate with Trial, Champion, ShadowClassifier traits"
```

---

## Task 7: ConstraintEvaluator

**Files:**
- Create: `crates/autotuner/src/evaluator.rs`
- Modify: `crates/autotuner/src/lib.rs`

- [ ] **Step 1: Write failing tests**

```rust
// crates/autotuner/src/evaluator.rs (tests at bottom)
#[cfg(test)]
mod tests {
    use super::*;
    use crate::trial::TrialResult;
    use config::AutoTunerConfig;

    fn baseline() -> TrialResult {
        TrialResult {
            correction_rate: 0.20,
            classification_accuracy: 0.80,
            avg_tokens_per_message: 500.0,
            avg_response_time_ms: 1000.0,
            routing_stability: 0.90,
            memory_relevance: 0.70,
            ..Default::default()
        }
    }

    #[test]
    fn passes_when_all_constraints_met() {
        let config = AutoTunerConfig::default();
        let evaluator = ConstraintEvaluator::new(&config);
        let result = TrialResult {
            correction_rate: 0.18, // 10% improvement (>= 5% required)
            avg_tokens_per_message: 520.0, // 4% increase (< 8% max)
            avg_response_time_ms: 1100.0, // 10% increase (< 15% max)
            routing_stability: 0.85, // 5.5% decrease (< 10% max)
            memory_relevance: 0.67, // 4.3% decrease (< 5% max)
            ..Default::default()
        };
        let verdict = evaluator.evaluate(&result, &baseline());
        assert!(verdict.passes_all());
    }

    #[test]
    fn fails_when_correction_improvement_insufficient() {
        let config = AutoTunerConfig::default();
        let evaluator = ConstraintEvaluator::new(&config);
        let result = TrialResult {
            correction_rate: 0.195, // only 2.5% improvement (< 5% required)
            ..baseline()
        };
        let verdict = evaluator.evaluate(&result, &baseline());
        assert!(!verdict.passes_all());
        assert!(verdict.failures.iter().any(|f| f.metric == "correction_rate"));
    }

    #[test]
    fn fails_when_token_cost_regresses() {
        let config = AutoTunerConfig::default();
        let evaluator = ConstraintEvaluator::new(&config);
        let result = TrialResult {
            correction_rate: 0.18, // passes
            avg_tokens_per_message: 550.0, // 10% increase (> 8% max)
            ..baseline()
        };
        let verdict = evaluator.evaluate(&result, &baseline());
        assert!(!verdict.passes_all());
        assert!(verdict.failures.iter().any(|f| f.metric == "token_cost"));
    }

    #[test]
    fn diversity_bonus_prefers_distant_variant() {
        let champion_params = common::TrialParams {
            skill_keyword_weight: Some(0.70),
            ..Default::default()
        };
        let close = common::TrialParams {
            skill_keyword_weight: Some(0.69),
            ..Default::default()
        };
        let far = common::TrialParams {
            skill_keyword_weight: Some(0.50),
            ..Default::default()
        };
        assert!(parameter_distance(&far, &champion_params) > parameter_distance(&close, &champion_params));
    }
}
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo nextest run -p autotuner -E 'test(constraint)'`
Expected: FAIL (ConstraintEvaluator not defined)

- [ ] **Step 3: Write the implementation**

```rust
// crates/autotuner/src/evaluator.rs
use common::TrialParams;
use config::AutoTunerConfig;
use crate::trial::TrialResult;

pub struct ConstraintEvaluator {
    min_correction_improvement: f64,
    max_token_cost_increase: f64,
    max_response_time_increase: f64,
    max_routing_stability_decrease: f64,
    max_memory_relevance_decrease: f64,
}

#[derive(Debug, Clone)]
pub struct ConstraintVerdict {
    pub failures: Vec<ConstraintFailure>,
}

impl ConstraintVerdict {
    pub fn passes_all(&self) -> bool {
        self.failures.is_empty()
    }
}

#[derive(Debug, Clone)]
pub struct ConstraintFailure {
    pub metric: String,
    pub threshold: f64,
    pub actual: f64,
    pub description: String,
}

impl ConstraintEvaluator {
    pub fn new(config: &AutoTunerConfig) -> Self {
        Self {
            min_correction_improvement: config.min_correction_improvement,
            max_token_cost_increase: config.max_token_cost_increase,
            max_response_time_increase: config.max_response_time_increase,
            max_routing_stability_decrease: config.max_routing_stability_decrease,
            max_memory_relevance_decrease: config.max_memory_relevance_decrease,
        }
    }

    pub fn evaluate(&self, trial: &TrialResult, baseline: &TrialResult) -> ConstraintVerdict {
        let mut failures = Vec::new();

        // Correction rate must improve by >= threshold
        let correction_improvement = if baseline.correction_rate > 0.0 {
            (baseline.correction_rate - trial.correction_rate) / baseline.correction_rate
        } else {
            0.0
        };
        if correction_improvement < self.min_correction_improvement {
            failures.push(ConstraintFailure {
                metric: "correction_rate".into(),
                threshold: self.min_correction_improvement,
                actual: correction_improvement,
                description: format!(
                    "Correction rate improved by {:.1}% (need >= {:.1}%)",
                    correction_improvement * 100.0,
                    self.min_correction_improvement * 100.0
                ),
            });
        }

        // Token cost must not increase by more than threshold
        let token_increase = if baseline.avg_tokens_per_message > 0.0 {
            (trial.avg_tokens_per_message - baseline.avg_tokens_per_message) / baseline.avg_tokens_per_message
        } else {
            0.0
        };
        if token_increase > self.max_token_cost_increase {
            failures.push(ConstraintFailure {
                metric: "token_cost".into(),
                threshold: self.max_token_cost_increase,
                actual: token_increase,
                description: format!(
                    "Token cost increased by {:.1}% (max {:.1}%)",
                    token_increase * 100.0,
                    self.max_token_cost_increase * 100.0
                ),
            });
        }

        // Response time must not increase by more than threshold
        let time_increase = if baseline.avg_response_time_ms > 0.0 {
            (trial.avg_response_time_ms - baseline.avg_response_time_ms) / baseline.avg_response_time_ms
        } else {
            0.0
        };
        if time_increase > self.max_response_time_increase {
            failures.push(ConstraintFailure {
                metric: "response_time".into(),
                threshold: self.max_response_time_increase,
                actual: time_increase,
                description: format!(
                    "Response time increased by {:.1}% (max {:.1}%)",
                    time_increase * 100.0,
                    self.max_response_time_increase * 100.0
                ),
            });
        }

        // Routing stability must not decrease by more than threshold
        let stability_decrease = if baseline.routing_stability > 0.0 {
            (baseline.routing_stability - trial.routing_stability) / baseline.routing_stability
        } else {
            0.0
        };
        if stability_decrease > self.max_routing_stability_decrease {
            failures.push(ConstraintFailure {
                metric: "routing_stability".into(),
                threshold: self.max_routing_stability_decrease,
                actual: stability_decrease,
                description: format!(
                    "Routing stability decreased by {:.1}% (max {:.1}%)",
                    stability_decrease * 100.0,
                    self.max_routing_stability_decrease * 100.0
                ),
            });
        }

        // Memory relevance must not drop by more than threshold
        let relevance_decrease = if baseline.memory_relevance > 0.0 {
            (baseline.memory_relevance - trial.memory_relevance) / baseline.memory_relevance
        } else {
            0.0
        };
        if relevance_decrease > self.max_memory_relevance_decrease {
            failures.push(ConstraintFailure {
                metric: "memory_relevance".into(),
                threshold: self.max_memory_relevance_decrease,
                actual: relevance_decrease,
                description: format!(
                    "Memory relevance decreased by {:.1}% (max {:.1}%)",
                    relevance_decrease * 100.0,
                    self.max_memory_relevance_decrease * 100.0
                ),
            });
        }

        ConstraintVerdict { failures }
    }
}

/// Euclidean distance between two TrialParams in normalized parameter space.
/// Used for diversity bonus when multiple trials pass constraints.
pub fn parameter_distance(a: &TrialParams, b: &TrialParams) -> f64 {
    let pairs: Vec<(Option<f64>, Option<f64>)> = vec![
        (a.skill_keyword_weight, b.skill_keyword_weight),
        (a.skill_semantic_weight, b.skill_semantic_weight),
        (a.skill_activation_threshold, b.skill_activation_threshold),
        (a.heuristic_confidence_threshold, b.heuristic_confidence_threshold),
        (a.relevance_weight_semantic, b.relevance_weight_semantic),
        (a.relevance_weight_retrievability, b.relevance_weight_retrievability),
        (a.relevance_weight_situation, b.relevance_weight_situation),
    ];
    let sum_sq: f64 = pairs
        .iter()
        .map(|(a_val, b_val)| {
            let a = a_val.unwrap_or(0.0);
            let b = b_val.unwrap_or(0.0);
            (a - b).powi(2)
        })
        .sum();
    sum_sq.sqrt()
}
```

- [ ] **Step 4: Add to lib.rs**

Add `pub mod evaluator;` and `pub use evaluator::*;` to `crates/autotuner/src/lib.rs`.

- [ ] **Step 5: Run tests**

Run: `cargo nextest run -p autotuner -E 'test(constraint)' -E 'test(diversity)'`
Expected: 4 tests PASS

- [ ] **Step 6: Commit**

```bash
git add crates/autotuner/src/evaluator.rs crates/autotuner/src/lib.rs
git commit -m "feat(autotuner): add ConstraintEvaluator with multi-metric promotion rules"
```

---

## Task 8: VariantGenerator (LLM-guided)

**Files:**
- Create: `crates/autotuner/src/generator.rs`
- Modify: `crates/autotuner/src/lib.rs`

- [ ] **Step 1: Write the test**

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_generation_response_valid_json() {
        let json = r#"{
            "variants": [
                {
                    "hypothesis": "Lower keyword weight for research",
                    "params": {
                        "skill_keyword_weight": 0.62,
                        "skill_semantic_weight": 0.38,
                        "skill_activation_threshold": 0.4,
                        "heuristic_confidence_threshold": 0.82,
                        "llm_classifier_timeout_ms": 2000,
                        "relevance_weight_semantic": 0.32,
                        "relevance_weight_retrievability": 0.22,
                        "relevance_weight_situation": 0.25
                    },
                    "constraint_reasoning": "Should improve corrections",
                    "confidence": "medium",
                    "confidence_reasoning": "Similar to trial #39"
                }
            ],
            "trend_analysis": "Corrections improving",
            "recommendation_for_next_cycle": "Push semantic weight higher"
        }"#;
        let response: GenerationResponse = serde_json::from_str(json).unwrap();
        assert_eq!(response.variants.len(), 1);
        assert_eq!(response.variants[0].params.skill_keyword_weight, Some(0.62));
    }

    #[test]
    fn build_prompt_includes_champion_params() {
        let context = GenerationContext {
            champion_params: common::TrialParams {
                skill_keyword_weight: Some(0.70),
                ..Default::default()
            },
            champion_metrics: crate::trial::TrialResult::default(),
            recent_trials: vec![],
            trend_summary: "Stable".into(),
            behavioral_context: "User does research queries mostly".into(),
            memory_snapshot: "Focused on finance".into(),
            previous_recommendation: None,
            experiment_pace: "balanced".into(),
        };
        let prompt = build_generation_prompt(&context);
        assert!(prompt.contains("skill_keyword_weight: 0.7"));
        assert!(prompt.contains("conservative"));
        assert!(prompt.contains("moderate"));
        assert!(prompt.contains("bold"));
    }
}
```

- [ ] **Step 2: Write the implementation**

```rust
// crates/autotuner/src/generator.rs
use common::TrialParams;
use crate::trial::TrialResult;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone)]
pub struct GenerationContext {
    pub champion_params: TrialParams,
    pub champion_metrics: TrialResult,
    pub recent_trials: Vec<TrialSummaryForPrompt>,
    pub trend_summary: String,
    pub behavioral_context: String,
    pub memory_snapshot: String,
    pub previous_recommendation: Option<String>,
    pub experiment_pace: String,
}

#[derive(Debug, Clone)]
pub struct TrialSummaryForPrompt {
    pub id: String,
    pub params: TrialParams,
    pub result: TrialResult,
    pub reasoning: String,
    pub outcome: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GenerationResponse {
    pub variants: Vec<VariantSuggestion>,
    pub trend_analysis: String,
    pub recommendation_for_next_cycle: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VariantSuggestion {
    pub hypothesis: String,
    pub params: TrialParams,
    pub constraint_reasoning: String,
    pub confidence: String,
    pub confidence_reasoning: String,
}

pub fn build_generation_prompt(ctx: &GenerationContext) -> String {
    let diversity_instruction = match ctx.experiment_pace.as_str() {
        "conservative" => "All three variants should be conservative refinements near the current champion.",
        "bold" => "Two variants should be bold hypotheses based on behavioral patterns. One should be a moderate exploration.",
        _ => "One conservative refinement near the current champion.\nOne moderate exploration shifting 2-3 key parameters.\nOne bold hypothesis based on recent behavioral shifts.",
    };

    let mut prompt = format!(
r#"You are the self-improvement engine for a personal AI assistant. Your job is to suggest parameter adjustments that make the assistant better at understanding and routing this specific user's requests.

You are not optimizing for a population. You are optimizing for one person. Think about what patterns in their behavior suggest about how they think, what they need, and where the current routing falls short.

## Current Champion

Parameters:
  skill_keyword_weight: {}
  skill_semantic_weight: {}
  skill_activation_threshold: {}
  heuristic_confidence_threshold: {}
  llm_classifier_timeout_ms: {}
  relevance_weight_semantic: {}
  relevance_weight_retrievability: {}
  relevance_weight_situation: {}

Champion metrics (baseline):
  correction_rate: {:.4}
  classification_accuracy: {:.4}
  avg_tokens_per_message: {:.1}
  avg_response_time_ms: {:.1}
  routing_stability: {:.4}
  memory_relevance: {:.4}
"#,
        ctx.champion_params.skill_keyword_weight.unwrap_or(0.7),
        ctx.champion_params.skill_semantic_weight.unwrap_or(0.3),
        ctx.champion_params.skill_activation_threshold.unwrap_or(0.4),
        ctx.champion_params.heuristic_confidence_threshold.unwrap_or(0.85),
        ctx.champion_params.llm_classifier_timeout_ms.unwrap_or(2000),
        ctx.champion_params.relevance_weight_semantic.unwrap_or(0.30),
        ctx.champion_params.relevance_weight_retrievability.unwrap_or(0.20),
        ctx.champion_params.relevance_weight_situation.unwrap_or(0.25),
        ctx.champion_metrics.correction_rate,
        ctx.champion_metrics.classification_accuracy,
        ctx.champion_metrics.avg_tokens_per_message,
        ctx.champion_metrics.avg_response_time_ms,
        ctx.champion_metrics.routing_stability,
        ctx.champion_metrics.memory_relevance,
    );

    if !ctx.recent_trials.is_empty() {
        prompt.push_str("\n## Recent Trial History\n\n");
        for t in &ctx.recent_trials {
            prompt.push_str(&format!(
                "Trial {} — {}\n  Reasoning: {}\n  Outcome: {}\n  correction_rate: {:.4}, tokens: {:.1}\n\n",
                t.id, t.outcome, t.reasoning, t.outcome,
                t.result.correction_rate, t.result.avg_tokens_per_message,
            ));
        }
    }

    prompt.push_str(&format!("\n## 7-Day Trend\n\n{}\n", ctx.trend_summary));
    prompt.push_str(&format!("\n## User Behavior Patterns\n\n{}\n", ctx.behavioral_context));
    prompt.push_str(&format!("\n## Recent User Memory Snapshot\n\n{}\n", ctx.memory_snapshot));

    if let Some(rec) = &ctx.previous_recommendation {
        prompt.push_str(&format!("\n## Previous Cycle Recommendation\n\n{}\n", rec));
    }

    prompt.push_str(r#"
## Parameter Bounds

  skill_keyword_weight: [0.30, 0.90]
  skill_semantic_weight: [0.10, 0.70]
  skill_activation_threshold: [0.20, 0.70]
  heuristic_confidence_threshold: [0.60, 0.95]
  llm_classifier_timeout_ms: [500, 5000]
  relevance_weight_semantic: [0.10, 0.60]
  relevance_weight_retrievability: [0.05, 0.50]
  relevance_weight_situation: [0.05, 0.50]

Note: skill_keyword_weight + skill_semantic_weight should sum to 1.0.

## Promotion Constraints

A variant will only be promoted if ALL of these are met:
  - correction_rate: must improve by >= 5%
  - avg_tokens_per_message: must not increase by > 8%
  - avg_response_time_ms: must not increase by > 15%
  - routing_stability: must not decrease by > 10%
  - memory_relevance: must not drop by > 5%

## Your Task

Suggest exactly 3 parameter combinations to test tomorrow. For each:
1. State the hypothesis — what do you expect to improve and why?
2. Provide the full parameter set (all 8 values).
3. Explain how this satisfies the promotion constraints.
4. Rate your confidence (low / medium / high) and explain why.

Make your suggestions meaningfully diverse:
"#);

    prompt.push_str(diversity_instruction);
    prompt.push_str("\nAvoid repeating patterns from the last 5 trials.\n");

    prompt.push_str(r#"
Respond in JSON format:
{
  "variants": [
    {
      "hypothesis": "...",
      "params": { "skill_keyword_weight": ..., "skill_semantic_weight": ..., ... },
      "constraint_reasoning": "...",
      "confidence": "low|medium|high",
      "confidence_reasoning": "..."
    }
  ],
  "trend_analysis": "...",
  "recommendation_for_next_cycle": "..."
}
"#);

    prompt
}
```

- [ ] **Step 3: Add to lib.rs**

Add `pub mod generator;` and `pub use generator::*;` to lib.rs.

- [ ] **Step 4: Run tests**

Run: `cargo nextest run -p autotuner -E 'test(generation)' -E 'test(prompt)'`
Expected: 2 tests PASS

- [ ] **Step 5: Commit**

```bash
git add crates/autotuner/src/generator.rs crates/autotuner/src/lib.rs
git commit -m "feat(autotuner): add VariantGenerator with LLM generation prompt"
```

---

## Task 9: NightlyCycle

**Files:**
- Create: `crates/autotuner/src/cycle.rs`
- Create: `crates/autotuner/src/metrics.rs`
- Modify: `crates/autotuner/src/lib.rs`

- [ ] **Step 1: Write MetricAggregator**

```rust
// crates/autotuner/src/metrics.rs
use crate::traits::MetricSnapshot;
use crate::trial::TrialResult;
use uuid::Uuid;

/// Aggregates MetricSnapshots into a TrialResult.
pub fn aggregate_to_result(trial_id: Uuid, snapshots: &[MetricSnapshot]) -> TrialResult {
    if snapshots.is_empty() {
        return TrialResult { trial_id, ..Default::default() };
    }
    let n = snapshots.len() as f64;
    let total_messages: u32 = snapshots.iter().map(|s| s.total_messages).sum();
    TrialResult {
        trial_id,
        messages_scored: total_messages,
        correction_rate: snapshots.iter().map(|s| s.correction_rate).sum::<f64>() / n,
        classification_accuracy: snapshots.iter().map(|s| s.classification_accuracy).sum::<f64>() / n,
        avg_tokens_per_message: snapshots.iter().map(|s| s.avg_tokens_per_message).sum::<f64>() / n,
        avg_response_time_ms: snapshots.iter().map(|s| s.avg_response_time_ms).sum::<f64>() / n,
        routing_stability: snapshots.iter().map(|s| s.routing_stability).sum::<f64>() / n,
        memory_relevance: snapshots.iter().map(|s| s.memory_relevance).sum::<f64>() / n,
        user_satisfaction: {
            let sats: Vec<f64> = snapshots.iter().filter_map(|s| s.user_satisfaction).collect();
            if sats.is_empty() { None } else { Some(sats.iter().sum::<f64>() / sats.len() as f64) }
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn aggregate_averages_correctly() {
        let snapshots = vec![
            MetricSnapshot { correction_rate: 0.10, avg_tokens_per_message: 400.0, total_messages: 25, ..Default::default() },
            MetricSnapshot { correction_rate: 0.20, avg_tokens_per_message: 600.0, total_messages: 25, ..Default::default() },
        ];
        let result = aggregate_to_result(Uuid::nil(), &snapshots);
        assert_eq!(result.messages_scored, 50);
        assert!((result.correction_rate - 0.15).abs() < 1e-10);
        assert!((result.avg_tokens_per_message - 500.0).abs() < 1e-10);
    }
}
```

- [ ] **Step 2: Write NightlyCycle skeleton**

```rust
// crates/autotuner/src/cycle.rs
use crate::{
    evaluator::{parameter_distance, ConstraintEvaluator},
    events::{AutoTunerEvent, AutoTunerPromotion, AutoTunerReport, AutoTunerRollback, ChampionSummary, ParamChange},
    metrics::aggregate_to_result,
    traits::{MetricSource, ShadowClassifier},
    trial::{Champion, Trial, TrialResult, TrialStatus},
};
use chrono::Utc;
use common::TrialParams;
use config::AutoTunerConfig;
use std::sync::Arc;
use storage::TrialRepo;
use uuid::Uuid;

pub struct NightlyCycle {
    config: AutoTunerConfig,
    evaluator: ConstraintEvaluator,
    repo: TrialRepo,
    metric_source: Arc<dyn MetricSource>,
}

impl NightlyCycle {
    pub fn new(
        config: AutoTunerConfig,
        repo: TrialRepo,
        metric_source: Arc<dyn MetricSource>,
    ) -> Self {
        let evaluator = ConstraintEvaluator::new(&config);
        Self { config, evaluator, repo, metric_source }
    }

    /// Run the full nightly cycle: evaluate → promote → report.
    /// Returns events to emit + optional new champion.
    /// Generation (LLM call) is handled by the caller (L5 orchestrator)
    /// since it needs the LLM provider.
    pub async fn run_evaluation_and_promotion(
        &self,
        current_champion: &Champion,
    ) -> common::Result<CycleResult> {
        // Step 1: Evaluate active trials
        let active_rows = self.repo.get_active_trials().await?;
        let mut completed_results = Vec::new();

        for row in &active_rows {
            let trial_id = Uuid::parse_str(&row.id).unwrap_or_default();
            let snapshot = self.metric_source.collect_metrics(
                Utc::now() - chrono::Duration::hours(24),
                Some(trial_id),
            ).await?;

            if snapshot.total_messages < self.config.min_messages_for_promotion {
                tracing::info!(trial_id = %row.id, messages = snapshot.total_messages, "Trial has insufficient messages, skipping");
                continue;
            }

            let result = aggregate_to_result(trial_id, &[snapshot]);
            let result_json = serde_json::to_string(&result).unwrap_or_default();
            self.repo.complete_trial(&row.id, &result_json).await?;
            completed_results.push((trial_id, result, row.params.clone()));
        }

        // Step 2: Check promotion constraints
        let mut best_candidate: Option<(Uuid, TrialResult, String, f64)> = None;

        for (trial_id, result, params_json) in &completed_results {
            let verdict = self.evaluator.evaluate(result, &current_champion.baseline_metrics);
            if verdict.passes_all() {
                let params: TrialParams = serde_json::from_str(params_json).unwrap_or_default();
                let correction_improvement = if current_champion.baseline_metrics.correction_rate > 0.0 {
                    (current_champion.baseline_metrics.correction_rate - result.correction_rate)
                        / current_champion.baseline_metrics.correction_rate
                } else {
                    0.0
                };
                let diversity = parameter_distance(&params, &current_champion.params);
                let score = correction_improvement + diversity * 0.01; // small diversity bonus

                if best_candidate.as_ref().map_or(true, |(_, _, _, s)| score > *s) {
                    best_candidate = Some((*trial_id, result.clone(), params_json.clone(), score));
                }
            }
        }

        // Step 3: Build result
        let promotion = if let Some((trial_id, result, _params_json, _score)) = best_candidate {
            self.repo.update_trial_status(&trial_id.to_string(), "promoted").await?;
            Some((trial_id, result))
        } else {
            None
        };

        // Step 4: Check regression
        let regression = self.check_regression(current_champion).await?;

        Ok(CycleResult {
            promotion,
            regression,
            completed_count: completed_results.len(),
        })
    }

    async fn check_regression(&self, champion: &Champion) -> common::Result<bool> {
        if champion.trial_id.is_none() {
            return Ok(false);
        }
        let snapshot = self.metric_source.collect_metrics(
            Utc::now() - chrono::Duration::hours(24),
            None,
        ).await?;
        Ok(snapshot.correction_rate > champion.baseline_metrics.correction_rate)
    }
}

pub struct CycleResult {
    pub promotion: Option<(Uuid, TrialResult)>,
    pub regression: bool,
    pub completed_count: usize,
}
```

- [ ] **Step 3: Add to lib.rs**

Add `pub mod cycle;` and `pub mod metrics;` to lib.rs.

- [ ] **Step 4: Run tests**

Run: `cargo nextest run -p autotuner`
Expected: All tests PASS (including metrics::aggregate_averages_correctly)

- [ ] **Step 5: Commit**

```bash
git add crates/autotuner/src/cycle.rs crates/autotuner/src/metrics.rs crates/autotuner/src/lib.rs
git commit -m "feat(autotuner): add NightlyCycle evaluation/promotion and MetricAggregator"
```

---

## Task 10: IntentAnalyzer overrides

**Files:**
- Modify: `crates/agent/src/intent_pipeline/analysis.rs:1154-1188`

- [ ] **Step 1: Add overrides field to IntentAnalyzer struct**

Add to the struct at line ~1168:
```rust
overrides: Option<common::TrialParams>,
```

- [ ] **Step 2: Update constructor**

In `IntentAnalyzer::new()` at line ~1171, add `overrides: None,` to the Self initialization.

Add a builder method after `new()`:
```rust
pub fn with_overrides(mut self, params: common::TrialParams) -> Self {
    self.overrides = Some(params);
    self
}
```

- [ ] **Step 3: Update threshold reading**

In `effective_heuristic_threshold()`, before falling back to `self.config.heuristic_confidence_threshold`, check:
```rust
if let Some(ref overrides) = self.overrides {
    if let Some(threshold) = overrides.heuristic_confidence_threshold {
        return threshold as f32;
    }
}
```

Similar pattern for `llm_classifier_timeout_ms` if used in the cascade.

- [ ] **Step 4: Run tests**

Run: `cargo nextest run -p agent -E 'test(intent)' -E 'test(analysis)'`
Expected: All existing tests PASS (overrides = None preserves behavior)

- [ ] **Step 5: Commit**

```bash
git add crates/agent/src/intent_pipeline/analysis.rs
git commit -m "feat(agent): add TrialParams overrides to IntentAnalyzer"
```

---

## Task 11: agent/autotuner/ module (L5 thin glue)

**Files:**
- Create: `crates/agent/src/autotuner/mod.rs`
- Create: `crates/agent/src/autotuner/shadow_classifier.rs`
- Create: `crates/agent/src/autotuner/metric_collector.rs`
- Create: `crates/agent/src/autotuner/hooks.rs`
- Modify: `crates/agent/src/lib.rs`

- [ ] **Step 1: Write hooks.rs**

```rust
// crates/agent/src/autotuner/hooks.rs
use async_trait::async_trait;
use autotuner::{ShadowContext, ShadowPrediction};
use common::TrialParams;

/// Hook into the agent runtime for shadow scoring.
#[async_trait]
pub trait AutoTunerHook: Send + Sync {
    /// Called after classification, before execution. Runs shadow scoring.
    async fn on_message_received(&self, message: &str, chat_id: &str);

    /// Called after response delivery. Records ground truth.
    async fn on_message_completed(&self, chat_id: &str, user_corrected: bool, tokens_used: u32, response_time_ms: u64);

    /// Returns the current champion params (if any).
    fn current_champion_params(&self) -> Option<TrialParams>;
}
```

- [ ] **Step 2: Write shadow_classifier.rs skeleton**

```rust
// crates/agent/src/autotuner/shadow_classifier.rs
use async_trait::async_trait;
use autotuner::{ShadowClassifier, ShadowContext, ShadowPrediction};
use common::TrialParams;
use crate::intent_pipeline::analysis::IntentAnalyzer;
use skill_system::router::SkillRouter;
use std::sync::Arc;
use tokio::sync::RwLock;

pub struct AgentShadowClassifier {
    // Will hold references to the shared analyzer resources
    // (AC matchers, embedding cache) to construct lightweight
    // IntentAnalyzer instances with overrides.
    config: config::OrchestratorConfig,
    provider: providers::DynProvider,
    model: String,
    skill_router: Arc<RwLock<SkillRouter>>,
}

#[async_trait]
impl ShadowClassifier for AgentShadowClassifier {
    async fn classify_shadow(
        &self,
        message: &str,
        context: &ShadowContext,
        params: &TrialParams,
    ) -> common::Result<ShadowPrediction> {
        // Create a lightweight IntentAnalyzer with overrides
        let analyzer = IntentAnalyzer::new(
            self.provider.clone(),
            &self.model,
            &self.config,
        ).with_overrides(params.clone());

        // Run Layer 1-2 only (heuristic + embedding)
        // The analyze() method runs the full cascade, but with overrides
        // that affect thresholds. For shadow scoring, we only care about
        // the routing prediction, not the full response.
        let analysis = analyzer.analyze(message, &[]).await;

        // Get skill router prediction with weight overrides
        let router = self.skill_router.read().await;
        // Note: actual SkillRouter call depends on having query embedding
        // This is a skeleton — the exact wiring depends on runtime context

        Ok(ShadowPrediction {
            predicted_orchestrator: "general".into(), // placeholder
            predicted_mode: analysis.strategy.execution_mode_str().to_string(),
            confidence: analysis.strategy.confidence,
            predicted_iteration_budget: analysis.strategy.max_iterations.unwrap_or(10),
            deferred_to_llm: false, // TODO: detect from analysis source
        })
    }
}
```

- [ ] **Step 3: Write mod.rs (AutoTunerOrchestrator)**

```rust
// crates/agent/src/autotuner/mod.rs
pub mod hooks;
pub mod shadow_classifier;
pub mod metric_collector;

use autotuner::{Champion, NightlyCycle};
use common::TrialParams;
use std::sync::Arc;
use tokio::sync::RwLock;

pub struct AutoTunerOrchestrator {
    champion: RwLock<Champion>,
    active: bool,
}

impl AutoTunerOrchestrator {
    pub fn new(champion: Champion, enabled: bool) -> Self {
        Self {
            champion: RwLock::new(champion),
            active: enabled,
        }
    }

    pub fn is_active(&self) -> bool {
        self.active
    }

    pub async fn current_champion_params(&self) -> Option<TrialParams> {
        if !self.active {
            return None;
        }
        let champion = self.champion.read().await;
        if champion.trial_id.is_some() {
            Some(champion.params.clone())
        } else {
            None
        }
    }

    pub async fn update_champion(&self, new_champion: Champion) {
        *self.champion.write().await = new_champion;
    }

    pub async fn champion_summary(&self) -> autotuner::ChampionSummary {
        let c = self.champion.read().await;
        let days = (chrono::Utc::now() - c.promoted_at).num_days().max(0) as u32;
        autotuner::ChampionSummary {
            trial_id: c.trial_id,
            description: c.reason_for_promotion.clone(),
            impact: c.impact_summary.clone(),
            promoted_at: c.promoted_at,
            days_active: days,
        }
    }
}
```

- [ ] **Step 4: Add module to agent/lib.rs**

Add `pub mod autotuner;` to `crates/agent/src/lib.rs`.

- [ ] **Step 5: Verify builds**

Run: `cargo build -p agent`
Expected: 0 errors (warnings OK for now — skeleton has unused fields)

- [ ] **Step 6: Commit**

```bash
git add crates/agent/src/autotuner/ crates/agent/src/lib.rs
git commit -m "feat(agent): add autotuner orchestrator, shadow classifier, and hooks"
```

---

## Task 12: AgentEvent variants + app-core handlers

**Files:**
- Modify: `crates/agent/src/events.rs:100+`
- Create: `crates/app-core/src/handlers/autotuner.rs`
- Modify: `crates/app-core/src/handlers/mod.rs`

- [ ] **Step 1: Add AgentEvent variants**

Add to the AgentEvent enum in `crates/agent/src/events.rs`:
```rust
/// AutoTuner nightly report.
AutoTunerReport(autotuner::AutoTunerReport),

/// AutoTuner promoted a new champion.
AutoTunerPromotion(autotuner::AutoTunerPromotion),

/// AutoTuner auto-reverted after regression.
AutoTunerRollback(autotuner::AutoTunerRollback),
```

- [ ] **Step 2: Write app-core handlers**

```rust
// crates/app-core/src/handlers/autotuner.rs
use crate::AppCore;
use common::Result;
use autotuner::{ChampionSummary, ExperimentSummary};
use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AutoTunerStatus {
    pub enabled: bool,
    pub champion: ChampionSummary,
    pub active_experiment: Option<ExperimentSummary>,
    pub paused: bool,
}

pub async fn autotuner_status(core: &AppCore) -> Result<AutoTunerStatus> {
    let orchestrator = core.autotuner_orchestrator();
    let champion = orchestrator.champion_summary().await;
    Ok(AutoTunerStatus {
        enabled: orchestrator.is_active(),
        champion,
        active_experiment: None, // TODO: wire to TrialRepo
        paused: false,
    })
}

pub async fn autotuner_history(core: &AppCore, limit: u32) -> Result<Vec<ExperimentSummary>> {
    // TODO: query TrialRepo for experiment history
    Ok(vec![])
}

pub async fn autotuner_revert(core: &AppCore) -> Result<ChampionSummary> {
    // TODO: revert to previous champion from LearningStateRepo
    let orchestrator = core.autotuner_orchestrator();
    Ok(orchestrator.champion_summary().await)
}

pub async fn autotuner_pause(core: &AppCore) -> Result<()> {
    // TODO: set paused state in LearningStateRepo
    Ok(())
}

pub async fn autotuner_resume(core: &AppCore) -> Result<()> {
    // TODO: clear paused state
    Ok(())
}
```

- [ ] **Step 3: Wire into app-core mod.rs**

Add `pub mod autotuner;` to `crates/app-core/src/handlers/mod.rs`.

- [ ] **Step 4: Verify builds**

Run: `cargo build -p app-core`
Expected: 0 errors

- [ ] **Step 5: Commit**

```bash
git add crates/agent/src/events.rs crates/app-core/src/handlers/autotuner.rs crates/app-core/src/handlers/mod.rs
git commit -m "feat(app-core): add autotuner AgentEvent variants and handler stubs"
```

---

## Task 13: Desktop commands + DEV_COMMANDS

**Files:**
- Create: `crates/desktop/src/commands/autotuner.rs`
- Modify: `crates/desktop/src/commands/mod.rs`
- Modify: `crates/desktop/src/dev_server/mod.rs`

- [ ] **Step 1: Write Tauri commands**

```rust
// crates/desktop/src/commands/autotuner.rs
use crate::AppState;
use tauri::State;

#[cfg(test)]
pub(crate) const DEV_COMMANDS: &[&str] = &[
    "autotuner_status",
    "autotuner_history",
    "autotuner_revert",
    "autotuner_pause",
    "autotuner_resume",
];

#[tauri::command]
pub async fn autotuner_status(
    state: State<'_, AppState>,
) -> Result<serde_json::Value, String> {
    let core = state.core.lock().await;
    let status = app_core::handlers::autotuner::autotuner_status(&core)
        .await
        .map_err(|e| e.to_string())?;
    serde_json::to_value(status).map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn autotuner_history(
    state: State<'_, AppState>,
    limit: Option<u32>,
) -> Result<serde_json::Value, String> {
    let core = state.core.lock().await;
    let history = app_core::handlers::autotuner::autotuner_history(&core, limit.unwrap_or(20))
        .await
        .map_err(|e| e.to_string())?;
    serde_json::to_value(history).map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn autotuner_revert(
    state: State<'_, AppState>,
) -> Result<serde_json::Value, String> {
    let core = state.core.lock().await;
    let result = app_core::handlers::autotuner::autotuner_revert(&core)
        .await
        .map_err(|e| e.to_string())?;
    serde_json::to_value(result).map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn autotuner_pause(
    state: State<'_, AppState>,
) -> Result<(), String> {
    let core = state.core.lock().await;
    app_core::handlers::autotuner::autotuner_pause(&core)
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn autotuner_resume(
    state: State<'_, AppState>,
) -> Result<(), String> {
    let core = state.core.lock().await;
    app_core::handlers::autotuner::autotuner_resume(&core)
        .await
        .map_err(|e| e.to_string())
}
```

- [ ] **Step 2: Register in commands/mod.rs and dev_server**

Add `pub mod autotuner;` to `crates/desktop/src/commands/mod.rs`. Register the commands in the Tauri builder and add to the `dev_server/mod.rs` DEV_COMMANDS test coverage list.

- [ ] **Step 3: Verify the DEV_COMMANDS test passes**

Run: `cargo nextest run -p desktop -E 'test(dev_server_covers)'`
Expected: PASS

- [ ] **Step 4: Commit**

```bash
git add crates/desktop/src/commands/autotuner.rs crates/desktop/src/commands/mod.rs crates/desktop/src/dev_server/mod.rs
git commit -m "feat(desktop): add autotuner Tauri commands with DEV_COMMANDS"
```

---

## Task 14: Frontend — Transparency Panel

**Files:**
- Create: `desktop-ui/src/features/autotuner/types.ts`
- Create: `desktop-ui/src/features/autotuner/hooks/useAutoTunerStatus.ts`
- Create: `desktop-ui/src/features/autotuner/hooks/useAutoTunerHistory.ts`
- Create: `desktop-ui/src/features/autotuner/components/ChampionCard.tsx`
- Create: `desktop-ui/src/features/autotuner/components/ExperimentTimeline.tsx`
- Create: `desktop-ui/src/features/autotuner/components/AutoTunerPanel.tsx`
- Create: `desktop-ui/src/features/autotuner/components/AmbientIndicator.tsx`

- [ ] **Step 1: Write TypeScript types**

```typescript
// desktop-ui/src/features/autotuner/types.ts
export interface ChampionSummary {
  trialId: string | null;
  description: string;
  impact: string;
  promotedAt: string;
  daysActive: number;
}

export interface ExperimentSummary {
  id: string;
  variantCount: number;
  messagesScored: number;
  hypothesis: string;
  startedAt: string;
}

export interface AutoTunerStatus {
  enabled: boolean;
  champion: ChampionSummary;
  activeExperiment: ExperimentSummary | null;
  paused: boolean;
}

export interface ParamChange {
  name: string;
  oldValue: number;
  newValue: number;
}
```

- [ ] **Step 2: Write hooks**

```typescript
// desktop-ui/src/features/autotuner/hooks/useAutoTunerStatus.ts
import { useQuery } from "@shared/hooks/useQuery";
import type { AutoTunerStatus } from "../types";

export function useAutoTunerStatus() {
  return useQuery<AutoTunerStatus>("autotuner_status", {});
}
```

```typescript
// desktop-ui/src/features/autotuner/hooks/useAutoTunerHistory.ts
import { useQuery } from "@shared/hooks/useQuery";
import type { ExperimentSummary } from "../types";

export function useAutoTunerHistory(limit = 20) {
  return useQuery<ExperimentSummary[]>("autotuner_history", { limit });
}
```

- [ ] **Step 3: Write ChampionCard component**

```tsx
// desktop-ui/src/features/autotuner/components/ChampionCard.tsx
import type { ChampionSummary, ExperimentSummary } from "../types";

interface Props {
  champion: ChampionSummary;
  activeExperiment: ExperimentSummary | null;
  onRevert: () => void;
  onPause: () => void;
}

export function ChampionCard({ champion, activeExperiment, onRevert, onPause }: Props) {
  return (
    <div className="glass-panel rounded-xl p-4 space-y-3">
      <h3 className="text-sm font-semibold text-foreground">AI Self-Improvement</h3>

      {champion.trialId ? (
        <div className="space-y-2">
          <p className="text-xs text-muted">
            Current config: Trial #{champion.trialId?.slice(0, 8)} (promoted {champion.daysActive} days ago)
          </p>
          <p className="text-sm text-foreground italic">"{champion.description}"</p>
          <p className="text-xs text-muted">Impact: {champion.impact}</p>
        </div>
      ) : (
        <p className="text-xs text-muted">Using default configuration</p>
      )}

      {activeExperiment && (
        <div className="text-xs text-muted border-t border-border pt-2">
          Testing now: Experiment ({activeExperiment.variantCount} variants)
          <br />
          {activeExperiment.messagesScored} messages scored so far
        </div>
      )}

      <div className="flex gap-2 pt-2">
        <button
          type="button"
          onClick={onRevert}
          className="text-xs px-2 py-1 rounded bg-surface-raised text-muted hover:text-foreground transition-colors"
        >
          Revert to defaults
        </button>
        <button
          type="button"
          onClick={onPause}
          className="text-xs px-2 py-1 rounded bg-surface-raised text-muted hover:text-foreground transition-colors"
        >
          Pause experiments
        </button>
      </div>
    </div>
  );
}
```

- [ ] **Step 4: Write AutoTunerPanel**

```tsx
// desktop-ui/src/features/autotuner/components/AutoTunerPanel.tsx
import { useAutoTunerStatus } from "../hooks/useAutoTunerStatus";
import { useAutoTunerHistory } from "../hooks/useAutoTunerHistory";
import { useMutation } from "@shared/hooks/useMutation";
import { ChampionCard } from "./ChampionCard";
import { ExperimentTimeline } from "./ExperimentTimeline";

export function AutoTunerPanel() {
  const { data: status, isLoading } = useAutoTunerStatus();
  const { data: history } = useAutoTunerHistory();
  const revert = useMutation("autotuner_revert");
  const pause = useMutation("autotuner_pause");

  if (isLoading || !status) return null;
  if (!status.enabled) return null;

  return (
    <div className="space-y-4">
      <ChampionCard
        champion={status.champion}
        activeExperiment={status.activeExperiment}
        onRevert={() => revert.mutate({})}
        onPause={() => pause.mutate({})}
      />
      {history && history.length > 0 && <ExperimentTimeline experiments={history} />}
    </div>
  );
}
```

- [ ] **Step 5: Write AmbientIndicator**

```tsx
// desktop-ui/src/features/autotuner/components/AmbientIndicator.tsx
import { useAutoTunerStatus } from "../hooks/useAutoTunerStatus";

interface Props {
  onClick: () => void;
}

export function AmbientIndicator({ onClick }: Props) {
  const { data: status } = useAutoTunerStatus();

  if (!status?.enabled || !status.champion.trialId) return null;

  return (
    <button
      type="button"
      onClick={onClick}
      className="text-xs text-muted hover:text-foreground transition-colors cursor-pointer"
    >
      Getting to know you better — {status.champion.impact}
    </button>
  );
}
```

- [ ] **Step 6: Write ExperimentTimeline**

```tsx
// desktop-ui/src/features/autotuner/components/ExperimentTimeline.tsx
import type { ExperimentSummary } from "../types";

interface Props {
  experiments: ExperimentSummary[];
}

export function ExperimentTimeline({ experiments }: Props) {
  return (
    <div className="space-y-2">
      <h4 className="text-xs font-semibold text-muted">Recent experiments</h4>
      {experiments.map((exp) => (
        <div key={exp.id} className="glass-panel rounded-lg p-3 text-xs space-y-1">
          <div className="flex items-center gap-2">
            <span className="text-accent">●</span>
            <span className="text-foreground font-medium">
              Experiment ({exp.variantCount} variants)
            </span>
          </div>
          <p className="text-muted italic">"{exp.hypothesis}"</p>
          <p className="text-muted">{exp.messagesScored} messages scored</p>
        </div>
      ))}
    </div>
  );
}
```

- [ ] **Step 7: Run frontend checks**

Run: `cd desktop-ui && bun run lint:fix && bun run build`
Expected: 0 errors

- [ ] **Step 8: Commit**

```bash
git add desktop-ui/src/features/autotuner/
git commit -m "feat(desktop-ui): add Transparency Panel with ChampionCard, timeline, and ambient indicator"
```

---

## Task 15: Wire CronJob for nightly cycle

**Files:**
- Modify: `crates/agent/src/autotuner/mod.rs`
- Depends on: existing CronService wiring pattern in `crates/agent/src/adapters/cron.rs`

- [ ] **Step 1: Add nightly cycle registration**

In `AutoTunerOrchestrator`, add a method to register the nightly cycle with CronService:

```rust
pub async fn register_nightly_cycle(
    &self,
    cron_service: &scheduling::CronService,
    schedule: &str,
) -> common::Result<()> {
    let orchestrator = Arc::new(self.clone()); // or use Arc wrapper
    cron_service.register_system_job(
        "autotuner_nightly_cycle",
        schedule,
        move |_job| {
            let orch = orchestrator.clone();
            Box::pin(async move {
                tracing::info!("Running AutoTuner nightly cycle");
                // Run evaluation and promotion
                // This is the entry point for the full cycle
                Ok(())
            })
        },
    ).await?;
    Ok(())
}
```

- [ ] **Step 2: Register during app startup**

In the agent builder or app-core initialization, after CronService is available:
```rust
if config.autotuner.enabled {
    orchestrator.register_nightly_cycle(&cron_service, &config.autotuner.schedule).await?;
}
```

The exact location depends on where CronService is wired — check `crates/agent/src/adapters/cron.rs` and `crates/app-core/src/init/` for the pattern.

- [ ] **Step 3: Verify builds**

Run: `cargo build --workspace`
Expected: 0 errors

- [ ] **Step 4: Commit**

```bash
git add -u
git commit -m "feat(agent): register autotuner nightly cycle with CronService"
```

---

## Task 16: Integration test

**Files:**
- Create: `crates/autotuner/tests/integration.rs` or add to existing test files

- [ ] **Step 1: Write end-to-end test for the evaluation → promotion flow**

```rust
// In crates/autotuner, tests or inline
#[tokio::test]
async fn full_evaluation_cycle_promotes_winning_trial() {
    // 1. Create in-memory storage + TrialRepo
    // 2. Seed an experiment with 3 trials (one good, two bad)
    // 3. Create a mock MetricSource that returns favorable metrics for the good trial
    // 4. Create a Champion with baseline metrics
    // 5. Run NightlyCycle::run_evaluation_and_promotion()
    // 6. Assert the good trial was promoted
    // 7. Assert the bad trials remain completed (not promoted)
}

#[tokio::test]
async fn evaluation_rejects_trials_that_fail_constraints() {
    // Similar setup but all trials fail one or more constraints
    // Assert no promotion occurred
}
```

- [ ] **Step 2: Run integration test**

Run: `cargo nextest run -p autotuner -E 'test(full_evaluation)'`
Expected: PASS

- [ ] **Step 3: Run full workspace check**

Run: `cargo clippy --workspace --all-targets --all-features`
Expected: 0 warnings

Run: `cargo fmt --all --check`
Expected: 0 formatting issues

- [ ] **Step 4: Final commit**

```bash
git add -u
git commit -m "test(autotuner): add integration tests for evaluation and promotion cycle"
```

---

## Summary

| Task | Component | Key Deliverable |
|------|-----------|-----------------|
| 1 | TrialParams (L0) | Pure value object in common |
| 2 | AutoTunerConfig (L1) | Constraint thresholds and schedule |
| 3 | RoutingContext (L1) | champion_params field |
| 4 | TrialRepo (L2) | Storage tables + CRUD |
| 5 | SkillRouter (L3) | Weight override params |
| 6 | autotuner crate (L4) | Scaffold + Trial/Champion/traits |
| 7 | ConstraintEvaluator (L4) | Multi-metric promotion rules |
| 8 | VariantGenerator (L4) | LLM generation prompt |
| 9 | NightlyCycle (L4) | Evaluate → promote flow |
| 10 | IntentAnalyzer (L5) | Overrides field for shadow scoring |
| 11 | agent/autotuner/ (L5) | Orchestrator + shadow classifier |
| 12 | AgentEvent + handlers | Event variants + app-core handlers |
| 13 | Desktop commands | Tauri wrappers + DEV_COMMANDS |
| 14 | Frontend | Transparency Panel UI |
| 15 | CronJob | Nightly cycle registration |
| 16 | Integration test | End-to-end evaluation cycle |
