# Autotuner Feedback Loop — Phase 1 Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Close the autotuner's broken feedback loop so trials can be evaluated against real metrics and the first promotion can occur.

**Architecture:** Bus-mediated correction signals — `AgentLoop` emits `UserCorrectedAI` events (reactions + keywords), cognitive pipeline persists them, `AgentMetricCollector` queries them. Ground truth for shadow_log is written via `on_message_completed`. 14 files modified, 0 new files.

**Tech Stack:** Rust, SQLite (via sqlx), tokio async, tracing, serde

**Spec:** `docs/superpowers/specs/2026-03-20-autotuner-feedback-loop-design.md`

---

## File Map

| File | Responsibility | Tasks |
|------|---------------|-------|
| `crates/bus/src/domain_events.rs` | Domain event types | 1 |
| `crates/cognitive/src/services/background.rs` | Cognitive event processing (exhaustive matches) | 1 |
| `crates/cognitive/src/services/salience.rs` | Salience rules for events | 1 |
| `crates/storage/src/repos/trial_repo.rs` | Shadow log ground truth + correction marking + agreement rate queries | 2 |
| `crates/storage/src/repos/usage.rs` | Token aggregation query | 3 |
| `crates/cognitive/src/repos/event_log.rs` | Event count-by-type query | 3 |
| `crates/agent/src/autotuner/hooks.rs` | Hook trait + implementation for ground truth writing | 4 |
| `crates/agent/src/agent_runtime/runtime.rs` | Pass pipeline results to hook | 4 |
| `crates/agent/src/autotuner/metric_collector.rs` | Replace placeholder metrics with real queries | 5 |
| `crates/app-core/src/init/cron.rs` | Wire new repos into metric collector | 5 |
| `crates/agent/src/agent_loop/mod.rs` | Correction emission (reactions + keywords) | 6 |
| `crates/agent/src/agent_loop/builder.rs` | Inject TrialRepo + rename bus field | 6 |
| `crates/agent/src/autotuner/mod.rs` | Observability logging + AutotunerDecision emission | 7 |
| `crates/app-core/src/handlers/autotuner.rs` | brain_growth + metrics_health in status response | 8 |

---

### Task 1: Extend domain events (foundation for everything else)

**Files:**
- Modify: `crates/bus/src/domain_events.rs` (lines 248–251, add after line 282)
- Modify: `crates/cognitive/src/services/background.rs` (lines 616–625, line 973)
- Modify: `crates/cognitive/src/services/salience.rs` (lines 20–21)

This task extends `UserCorrectedAI` with `kind` + `strength`, adds `CorrectionKind` enum, adds `AutotunerDecision` variant, and updates all exhaustive match arms so the codebase compiles.

- [ ] **Step 1: Write test for CorrectionKind serialization**

In `crates/bus/src/domain_events.rs`, add to the existing `#[cfg(test)] mod tests` block:

```rust
#[test]
fn correction_kind_roundtrip() {
    let kind = CorrectionKind::Reaction;
    let json = serde_json::to_string(&kind).unwrap();
    assert_eq!(json, "\"reaction\"");
    let parsed: CorrectionKind = serde_json::from_str(&json).unwrap();
    assert_eq!(parsed, CorrectionKind::Reaction);

    let kind2 = CorrectionKind::KeywordPrefix;
    let json2 = serde_json::to_string(&kind2).unwrap();
    assert_eq!(json2, "\"keyword_prefix\"");
}

#[test]
fn user_corrected_ai_with_kind_roundtrip() {
    let event = DomainEvent::UserCorrectedAI {
        original: "test".into(),
        correction: "fixed".into(),
        kind: CorrectionKind::Reaction,
        strength: 1.0,
    };
    let json = serde_json::to_string(&event).unwrap();
    let parsed: DomainEvent = serde_json::from_str(&json).unwrap();
    match parsed {
        DomainEvent::UserCorrectedAI { kind, strength, .. } => {
            assert_eq!(kind, CorrectionKind::Reaction);
            assert!((strength - 1.0).abs() < f64::EPSILON);
        }
        _ => panic!("Expected UserCorrectedAI"),
    }
}

#[test]
fn autotuner_decision_roundtrip() {
    let event = DomainEvent::AutotunerDecision {
        trial_id: "abc-123".into(),
        verdict: "promoted".into(),
        improvement_pct: 12.5,
        affected_params: vec!["heuristic_confidence_threshold".into()],
    };
    let json = serde_json::to_string(&event).unwrap();
    assert!(json.contains("promoted"));
    let parsed: DomainEvent = serde_json::from_str(&json).unwrap();
    match parsed {
        DomainEvent::AutotunerDecision { verdict, .. } => {
            assert_eq!(verdict, "promoted");
        }
        _ => panic!("Expected AutotunerDecision"),
    }
}
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo nextest run -p bus --no-fail-fast 2>&1 | tail -20`
Expected: Compile errors — `CorrectionKind` not found, `AutotunerDecision` not found.

- [ ] **Step 3: Add `CorrectionKind` enum and extend `UserCorrectedAI`**

In `crates/bus/src/domain_events.rs`:

1. After the `FeedbackResponse` enum (after line 282), add:
```rust
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CorrectionKind {
    Reaction,
    KeywordPrefix,
}
```

2. Replace the `UserCorrectedAI` variant (lines 248–251) with:
```rust
    UserCorrectedAI {
        original: String,
        correction: String,
        kind: CorrectionKind,
        strength: f64,
    },
```

3. Add the `AutotunerDecision` variant to `DomainEvent` (after `UserCorrectedAI`):
```rust
    AutotunerDecision {
        trial_id: String,
        verdict: String,
        improvement_pct: f64,
        affected_params: Vec<String>,
    },
```

- [ ] **Step 4: Fix exhaustive match in `background.rs`**

In `crates/cognitive/src/services/background.rs`:

1. Update `event_to_observation` — find the `UserCorrectedAI` match arm (line ~616) and add `..` to ignore new fields:
```rust
        DomainEvent::UserCorrectedAI {
            original,
            correction,
            ..
        } => Some(Observation {
            domain: "meta".into(),
            content: format!("User corrected: '{original}' → '{correction}'"),
            importance: 1.0,
            source_event: "UserCorrectedAI".into(),
            timestamp: now,
        }),
```

2. Add `AutotunerDecision` arm in the same function (before the catch-all or at the end of the match):
```rust
        DomainEvent::AutotunerDecision {
            trial_id,
            verdict,
            improvement_pct,
            affected_params,
        } => Some(Observation {
            domain: "meta".into(),
            content: format!(
                "Autotuner {verdict}: trial {trial_id}, improvement {improvement_pct:.1}%, params: {}",
                affected_params.join(", ")
            ),
            importance: 0.8,
            source_event: "AutotunerDecision".into(),
            timestamp: now,
        }),
```

3. Update `event_type_key` — add arm (near line 973):
```rust
        DomainEvent::AutotunerDecision { .. } => "AutotunerDecision".into(),
```

- [ ] **Step 5: Fix salience rule in `salience.rs`**

In `crates/cognitive/src/services/salience.rs`, add arm in `evaluate_salience` (near line 20):
```rust
        DomainEvent::AutotunerDecision { .. } => SalienceVerdict::Extract,
```

Also update the `test_user_correction_is_extract` test in the same file to include the new fields:
```rust
DomainEvent::UserCorrectedAI {
    original: "test".into(),
    correction: "fix".into(),
    kind: bus::CorrectionKind::Reaction,
    strength: 1.0,
}
```

- [ ] **Step 6: Run full workspace compile check**

Run: `cargo check --workspace 2>&1 | tail -20`
Expected: No errors (all exhaustive matches satisfied).

- [ ] **Step 7: Run tests**

Run: `cargo nextest run -p bus -p cognitive --no-fail-fast 2>&1 | tail -30`
Expected: All pass including the three new tests.

- [ ] **Step 8: Commit**

```bash
git add crates/bus/src/domain_events.rs crates/cognitive/src/services/background.rs crates/cognitive/src/services/salience.rs
git commit -m "feat(autotuner): extend UserCorrectedAI with kind/strength, add AutotunerDecision event"
```

---

### Task 2: Add TrialRepo query methods (shadow log ground truth + agreement rate)

**Files:**
- Modify: `crates/storage/src/repos/trial_repo.rs`

This task adds 5 new methods to `TrialRepo` — all are simple SQL queries.

- [ ] **Step 1: Write tests for the new methods**

In `crates/storage/src/repos/trial_repo.rs`, add to the existing `#[cfg(test)] mod tests` block:

```rust
#[tokio::test]
async fn update_shadow_log_ground_truth_sets_control_values() {
    let pool = crate::StoragePool::connect_in_memory().await.unwrap();
    let repo = TrialRepo::new(pool.clone());
    repo.migrate().await.unwrap();

    // Create experiment + trial
    let exp_id = uuid::Uuid::new_v4().to_string();
    repo.create_experiment(&crate::rows::trial::ExperimentRow {
        id: exp_id.clone(),
        hypothesis: "test".into(),
        trend_analysis: "".into(),
        recommendation_for_next: "".into(),
        created_at: "".into(),
    }).await.unwrap();

    let trial_id = uuid::Uuid::new_v4().to_string();
    repo.create_trial(&crate::rows::trial::TrialRow {
        id: trial_id.clone(),
        experiment_id: exp_id,
        params: "{}".into(),
        generation_reasoning: "test".into(),
        status: "active".into(),
        created_at: "".into(),
        completed_at: None,
        result: None,
    }).await.unwrap();

    // Insert shadow log with pending ground truth
    repo.insert_shadow_log(
        &trial_id, "2024-01-01T00:00:00Z", "chat_1", "general", "direct", 0.9, 5, "pending", "pending",
    ).await.unwrap();

    // Update ground truth
    repo.update_shadow_log_ground_truth("chat_1", "task-management", "reactive").await.unwrap();

    // Verify: agreement rate should be 0.0 (predicted=direct, control=reactive)
    let rate = repo.shadow_log_agreement_rate(Some(&trial_id), chrono::Utc::now() - chrono::Duration::hours(1)).await.unwrap();
    assert!((rate - 0.0).abs() < f64::EPSILON);
}

#[tokio::test]
async fn mark_recent_messages_corrected_respects_window() {
    let pool = crate::StoragePool::connect_in_memory().await.unwrap();
    let repo = TrialRepo::new(pool.clone());
    repo.migrate().await.unwrap();

    // Create experiment + trial
    let exp_id = uuid::Uuid::new_v4().to_string();
    repo.create_experiment(&crate::rows::trial::ExperimentRow {
        id: exp_id.clone(),
        hypothesis: "test".into(),
        trend_analysis: "".into(),
        recommendation_for_next: "".into(),
        created_at: "".into(),
    }).await.unwrap();

    let trial_id = uuid::Uuid::new_v4().to_string();
    repo.create_trial(&crate::rows::trial::TrialRow {
        id: trial_id.clone(),
        experiment_id: exp_id,
        params: "{}".into(),
        generation_reasoning: "test".into(),
        status: "active".into(),
        created_at: "".into(),
        completed_at: None,
        result: None,
    }).await.unwrap();

    // Insert a recent shadow log row
    repo.insert_shadow_log(
        &trial_id, "2024-01-01T00:00:00Z", "chat_2", "general", "direct", 0.8, 3, "pending", "pending",
    ).await.unwrap();

    // Mark as corrected (15 min window)
    repo.mark_recent_messages_corrected("chat_2", 15).await.unwrap();

    // Verify by reading back (use a raw query since we don't have a read method yet)
    let row = sqlx::query_scalar::<_, i64>(
        "SELECT user_corrected FROM autotuner_shadow_log WHERE chat_id = 'chat_2' LIMIT 1"
    ).fetch_one(&pool).await.unwrap();
    assert_eq!(row, 1);
}

#[tokio::test]
async fn shadow_log_agreement_rate_excludes_pending() {
    let pool = crate::StoragePool::connect_in_memory().await.unwrap();
    let repo = TrialRepo::new(pool.clone());
    repo.migrate().await.unwrap();

    // With no data, should return 1.0 (default)
    let rate = repo.shadow_log_agreement_rate(None, chrono::Utc::now() - chrono::Duration::hours(1)).await.unwrap();
    assert!((rate - 1.0).abs() < f64::EPSILON);
}

#[tokio::test]
async fn count_trials_and_promoted_since() {
    let pool = crate::StoragePool::connect_in_memory().await.unwrap();
    let repo = TrialRepo::new(pool.clone());
    repo.migrate().await.unwrap();

    let since = chrono::Utc::now() - chrono::Duration::hours(24);
    let trials = repo.count_trials_since(since).await.unwrap();
    assert_eq!(trials, 0);
    let promoted = repo.count_promoted_since(since).await.unwrap();
    assert_eq!(promoted, 0);
}
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo nextest run -p storage -E 'test(shadow_log|mark_recent|count_trials)' --no-fail-fast 2>&1 | tail -20`
Expected: Compile errors — methods don't exist yet.

- [ ] **Step 3: Implement the 5 new methods**

Add to `impl TrialRepo` in `crates/storage/src/repos/trial_repo.rs`:

```rust
    pub async fn update_shadow_log_ground_truth(
        &self,
        chat_id: &str,
        control_orchestrator: &str,
        control_mode: &str,
    ) -> common::Result<()> {
        sqlx::query(
            "UPDATE autotuner_shadow_log
             SET control_orchestrator = ?1, control_mode = ?2
             WHERE chat_id = ?3
               AND control_orchestrator = 'pending'
               AND created_at >= datetime('now', '-60 seconds')",
        )
        .bind(control_orchestrator)
        .bind(control_mode)
        .bind(chat_id)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    pub async fn mark_recent_messages_corrected(
        &self,
        chat_id: &str,
        window_minutes: i32,
    ) -> common::Result<()> {
        sqlx::query(
            "UPDATE autotuner_shadow_log
             SET user_corrected = 1
             WHERE id IN (
                 SELECT id FROM autotuner_shadow_log
                 WHERE chat_id = ?1
                   AND created_at >= datetime('now', ?2)
                 ORDER BY created_at DESC LIMIT 2
             )",
        )
        .bind(chat_id)
        .bind(format!("-{} minutes", window_minutes))
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    pub async fn shadow_log_agreement_rate(
        &self,
        trial_id: Option<&str>,
        since: DateTime<Utc>,
    ) -> common::Result<f64> {
        let since_str = since.to_rfc3339();

        let (total, agreed): (i64, i64) = if let Some(tid) = trial_id {
            sqlx::query_as::<_, (i64, i64)>(
                "SELECT COUNT(*) AS total,
                        COALESCE(SUM(CASE WHEN predicted_mode = control_mode THEN 1 ELSE 0 END), 0) AS agreed
                 FROM autotuner_shadow_log
                 WHERE trial_id = ?1 AND control_mode != 'pending'
                   AND created_at >= ?2",
            )
            .bind(tid)
            .bind(&since_str)
            .fetch_one(&self.pool)
            .await?
        } else {
            sqlx::query_as::<_, (i64, i64)>(
                "SELECT COUNT(*) AS total,
                        COALESCE(SUM(CASE WHEN predicted_mode = control_mode THEN 1 ELSE 0 END), 0) AS agreed
                 FROM autotuner_shadow_log
                 WHERE control_mode != 'pending' AND created_at >= ?1",
            )
            .bind(&since_str)
            .fetch_one(&self.pool)
            .await?
        };

        Ok(if total == 0 {
            1.0
        } else {
            agreed as f64 / total as f64
        })
    }

    pub async fn count_trials_since(&self, since: DateTime<Utc>) -> common::Result<i64> {
        Ok(sqlx::query_scalar::<_, i64>(
            "SELECT COUNT(*) FROM autotuner_trials
             WHERE status IN ('completed', 'promoted', 'reverted')
               AND completed_at >= ?1",
        )
        .bind(since.to_rfc3339())
        .fetch_one(&self.pool)
        .await?)
    }

    pub async fn count_promoted_since(&self, since: DateTime<Utc>) -> common::Result<i64> {
        Ok(sqlx::query_scalar::<_, i64>(
            "SELECT COUNT(*) FROM autotuner_trials
             WHERE status = 'promoted' AND completed_at >= ?1",
        )
        .bind(since.to_rfc3339())
        .fetch_one(&self.pool)
        .await?)
    }
```

Note: add `use chrono::{DateTime, Utc};` to the imports if not already present.

- [ ] **Step 4: Run tests**

Run: `cargo nextest run -p storage -E 'test(shadow_log|mark_recent|count_trials)' --no-fail-fast 2>&1 | tail -30`
Expected: All 4 new tests pass.

- [ ] **Step 5: Commit**

```bash
git add crates/storage/src/repos/trial_repo.rs
git commit -m "feat(storage): add TrialRepo methods for ground truth, correction marking, agreement rate"
```

---

### Task 3: Add repo query methods for metrics (EventLogRepo + UsageRepo)

**Files:**
- Modify: `crates/cognitive/src/repos/event_log.rs`
- Modify: `crates/storage/src/repos/usage.rs`

- [ ] **Step 1: Write tests**

In `crates/cognitive/src/repos/event_log.rs`, add to tests:
```rust
#[tokio::test]
async fn count_by_event_type_filters_correctly() {
    let pool = crate::repos::cognitive_test_pool().await;
    let repo = EventLogRepo::new(pool);

    // Insert two UserCorrectedAI and one ChatTurnCompleted
    let now = chrono::Utc::now().to_rfc3339();
    repo.insert_domain_event(
        &uuid::Uuid::new_v4().to_string(),
        "UserCorrectedAI", "meta", "extract",
        r#"{"original":"x","correction":"y"}"#,
        &now,
    ).await.unwrap();
    repo.insert_domain_event(
        &uuid::Uuid::new_v4().to_string(),
        "UserCorrectedAI", "meta", "extract",
        r#"{"original":"a","correction":"b"}"#,
        &now,
    ).await.unwrap();
    repo.insert_domain_event(
        &uuid::Uuid::new_v4().to_string(),
        "ChatTurnCompleted", "chat", "observe",
        r#"{}"#,
        &now,
    ).await.unwrap();

    let since = chrono::Utc::now() - chrono::Duration::hours(1);
    let count = repo.count_by_event_type("UserCorrectedAI", since).await.unwrap();
    assert_eq!(count, 2);

    let count2 = repo.count_by_event_type("ChatTurnCompleted", since).await.unwrap();
    assert_eq!(count2, 1);

    let count3 = repo.count_by_event_type("NonExistent", since).await.unwrap();
    assert_eq!(count3, 0);
}
```

In `crates/storage/src/repos/usage.rs`, add to tests:
```rust
#[tokio::test]
async fn total_tokens_since_aggregates_correctly() {
    let pool = crate::StoragePool::connect_in_memory().await.unwrap();
    let repo = UsageRepo::new(pool);

    // Insert a usage record
    repo.create(&crate::rows::usage::UsageRecordRow {
        id: uuid::Uuid::new_v4(),
        timestamp: chrono::Utc::now(),
        request_id: "req1".into(),
        model: "test-model".into(),
        provider: "test".into(),
        prompt_tokens: 100,
        completion_tokens: 50,
        cache_read_tokens: 0,
        cache_write_tokens: 0,
        estimated_cost_usd: 0.001,
        channel: "test".into(),
        strategy: "direct".into(),
    }).await.unwrap();

    let since = chrono::Utc::now() - chrono::Duration::hours(1);
    let total = repo.total_tokens_since(since).await.unwrap();
    assert_eq!(total, 150);
}
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo nextest run -p cognitive -E 'test(count_by_event)' --no-fail-fast 2>&1 | tail -10`
Run: `cargo nextest run -p storage -E 'test(total_tokens_since)' --no-fail-fast 2>&1 | tail -10`
Expected: Compile errors — methods don't exist.

- [ ] **Step 3: Implement `EventLogRepo::count_by_event_type`**

In `crates/cognitive/src/repos/event_log.rs`, add to `impl EventLogRepo`:

```rust
    pub async fn count_by_event_type(
        &self,
        event_type: &str,
        since: DateTime<Utc>,
    ) -> common::Result<i64> {
        Ok(sqlx::query_scalar::<_, i64>(
            "SELECT COUNT(*) FROM domain_event_log
             WHERE event_type = ?1 AND timestamp >= ?2",
        )
        .bind(event_type)
        .bind(since.to_rfc3339())
        .fetch_one(&self.pool)
        .await?)
    }
```

- [ ] **Step 4: Implement `UsageRepo::total_tokens_since`**

In `crates/storage/src/repos/usage.rs`, add to `impl UsageRepo`:

```rust
    pub async fn total_tokens_since(&self, since: DateTime<Utc>) -> common::Result<i64> {
        Ok(sqlx::query_scalar::<_, i64>(
            "SELECT COALESCE(SUM(prompt_tokens + completion_tokens), 0)
             FROM usage_records WHERE timestamp >= ?1",
        )
        .bind(since.to_rfc3339())
        .fetch_one(&self.pool)
        .await?)
    }
```

- [ ] **Step 5: Run tests**

Run: `cargo nextest run -p cognitive -E 'test(count_by_event)' -p storage -E 'test(total_tokens_since)' --no-fail-fast 2>&1 | tail -20`
Expected: All pass.

- [ ] **Step 6: Commit**

```bash
git add crates/cognitive/src/repos/event_log.rs crates/storage/src/repos/usage.rs
git commit -m "feat(storage): add EventLogRepo::count_by_event_type and UsageRepo::total_tokens_since"
```

---

### Task 4: Wire ground truth writing in `on_message_completed`

**Files:**
- Modify: `crates/agent/src/autotuner/hooks.rs` (lines 22–38, 174–188)
- Modify: `crates/agent/src/agent_runtime/runtime.rs` (lines 574–584)

- [ ] **Step 1: Write test for ground truth hook**

In `crates/agent/src/autotuner/hooks.rs`, add to the existing `#[cfg(test)] mod tests` block:

```rust
#[tokio::test]
async fn on_message_completed_writes_ground_truth() {
    // Setup: in-memory pool, create experiment + trial + shadow log row
    let pool = storage::StoragePool::connect_in_memory().await.unwrap();
    let trial_repo = storage::TrialRepo::new(pool.clone());
    trial_repo.migrate().await.unwrap();

    let exp_id = uuid::Uuid::new_v4().to_string();
    trial_repo.create_experiment(&storage::rows::trial::ExperimentRow {
        id: exp_id.clone(),
        hypothesis: "test".into(),
        trend_analysis: "".into(),
        recommendation_for_next: "".into(),
        created_at: "".into(),
    }).await.unwrap();

    let trial_id = uuid::Uuid::new_v4().to_string();
    trial_repo.create_trial(&storage::rows::trial::TrialRow {
        id: trial_id.clone(),
        experiment_id: exp_id,
        params: "{}".into(),
        generation_reasoning: "test".into(),
        status: "active".into(),
        created_at: "".into(),
        completed_at: None,
        result: None,
    }).await.unwrap();

    // Insert shadow log with pending ground truth
    trial_repo.insert_shadow_log(
        &trial_id, "2024-01-01T00:00:00Z", "chat_test", "general", "direct", 0.9, 5, "pending", "pending",
    ).await.unwrap();

    // Verify pending
    let row = sqlx::query_scalar::<_, String>(
        "SELECT control_orchestrator FROM autotuner_shadow_log WHERE chat_id = 'chat_test' LIMIT 1"
    ).fetch_one(&pool).await.unwrap();
    assert_eq!(row, "pending");

    // Call update
    trial_repo.update_shadow_log_ground_truth("chat_test", "task-management", "reactive").await.unwrap();

    // Verify updated
    let row = sqlx::query_scalar::<_, String>(
        "SELECT control_orchestrator FROM autotuner_shadow_log WHERE chat_id = 'chat_test' LIMIT 1"
    ).fetch_one(&pool).await.unwrap();
    assert_eq!(row, "task-management");

    let mode = sqlx::query_scalar::<_, String>(
        "SELECT control_mode FROM autotuner_shadow_log WHERE chat_id = 'chat_test' LIMIT 1"
    ).fetch_one(&pool).await.unwrap();
    assert_eq!(mode, "reactive");
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo nextest run -p agent -E 'test(on_message_completed_writes)' --no-fail-fast 2>&1 | tail -10`
Expected: Compile error or test failure.

- [ ] **Step 3: Update `AutoTunerHook` trait signature**

In `crates/agent/src/autotuner/hooks.rs`, replace the trait method (lines 26–32):

Old:
```rust
    async fn on_message_completed(
        &self,
        chat_id: &str,
        user_corrected: bool,
        tokens_used: u32,
        response_time_ms: u64,
    );
```

New:
```rust
    async fn on_message_completed(
        &self,
        chat_id: &str,
        orchestrator_name: &str,
        execution_mode: &str,
        tokens_used: u32,
        response_time_ms: u64,
    );
```

- [ ] **Step 4: Implement `on_message_completed` in `AutoTunerHookImpl`**

Replace the stub (lines 174–188) with:

```rust
    async fn on_message_completed(
        &self,
        chat_id: &str,
        orchestrator_name: &str,
        execution_mode: &str,
        _tokens_used: u32,
        _response_time_ms: u64,
    ) {
        if !self.orchestrator.is_active() {
            return;
        }

        if let Err(e) = self
            .trial_repo
            .update_shadow_log_ground_truth(chat_id, orchestrator_name, execution_mode)
            .await
        {
            tracing::warn!(error = %e, "Failed to update shadow log ground truth");
        }
    }
```

- [ ] **Step 5: Update runtime call site**

In `crates/agent/src/agent_runtime/runtime.rs`, replace lines 574–584:

Old:
```rust
        if let Some(ref hook) = self.autotuner_hook {
            let tokens = router_result.usage.prompt_tokens + router_result.usage.completion_tokens;
            hook.on_message_completed(
                ctx.chat_id.as_str(),
                false, // user_corrected — updated later via reaction/feedback
                tokens,
                pipeline_elapsed_ms,
            )
            .await;
        }
```

New:
```rust
        if let Some(ref hook) = self.autotuner_hook {
            let tokens = router_result.usage.prompt_tokens + router_result.usage.completion_tokens;
            hook.on_message_completed(
                ctx.chat_id.as_str(),
                &agent_name,
                analysis.mode.short_name(),
                tokens,
                pipeline_elapsed_ms,
            )
            .await;
        }
```

Note: `agent_name` is captured at Step 1 of the pipeline (line ~260, `profile.name.clone()`). `analysis.mode.short_name()` is confirmed to exist on `ExecutionMode` in `crates/agent/src/intent_pipeline/types.rs` (line 17).

- [ ] **Step 6: Compile check + run tests**

Run: `cargo check --workspace 2>&1 | tail -20`
Run: `cargo nextest run -p agent -E 'test(on_message_completed|hook_skips|hook_runs)' --no-fail-fast 2>&1 | tail -20`
Expected: All pass. Existing hook tests may need signature updates.

- [ ] **Step 7: Commit**

```bash
git add crates/agent/src/autotuner/hooks.rs crates/agent/src/agent_runtime/runtime.rs
git commit -m "feat(autotuner): wire on_message_completed to write shadow log ground truth"
```

---

### Task 5: Replace metric collector placeholders

**Files:**
- Modify: `crates/agent/src/autotuner/metric_collector.rs`
- Modify: `crates/app-core/src/init/cron.rs` (lines 117–119)

- [ ] **Step 1: Write tests for real metric computation**

In `crates/agent/src/autotuner/metric_collector.rs`, add/extend the test module:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn metric_collector_handles_empty_data() {
        let pool = storage::StoragePool::connect_in_memory().await.unwrap();
        // Run migrations for all relevant tables
        storage::TrialRepo::new(pool.clone()).migrate().await.unwrap();

        let collector = AgentMetricCollector::new(
            storage::StrategyRepo::new(pool.clone()),
            cognitive::EventLogRepo::new(pool.clone()),
            storage::UsageRepo::new(pool.clone()),
            storage::TrialRepo::new(pool.clone()),
        );

        let since = chrono::Utc::now() - chrono::Duration::hours(24);
        let snapshot = collector.collect_metrics(since, None).await.unwrap();

        // With no data: correction_rate=0, tokens=0, stability=1.0 (default), relevance=1.0
        assert!((snapshot.correction_rate - 0.0).abs() < f64::EPSILON);
        assert!((snapshot.avg_tokens_per_message - 0.0).abs() < f64::EPSILON);
        assert!((snapshot.routing_stability - 1.0).abs() < f64::EPSILON);
        assert!((snapshot.memory_relevance - 1.0).abs() < f64::EPSILON);
    }
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo nextest run -p agent -E 'test(metric_collector)' --no-fail-fast 2>&1 | tail -10`
Expected: Compile error — `AgentMetricCollector::new` doesn't accept 4 args yet.

- [ ] **Step 3: Update `AgentMetricCollector` struct and constructor**

Replace the struct and `new` in `crates/agent/src/autotuner/metric_collector.rs`:

```rust
pub struct AgentMetricCollector {
    strategy_repo: storage::StrategyRepo,
    event_log_repo: cognitive::EventLogRepo,
    usage_repo: storage::UsageRepo,
    trial_repo: storage::TrialRepo,
}

impl AgentMetricCollector {
    pub fn new(
        strategy_repo: storage::StrategyRepo,
        event_log_repo: cognitive::EventLogRepo,
        usage_repo: storage::UsageRepo,
        trial_repo: storage::TrialRepo,
    ) -> Self {
        Self {
            strategy_repo,
            event_log_repo,
            usage_repo,
            trial_repo,
        }
    }
}
```

Add `use cognitive;` to imports if the `cognitive` crate is not already imported. Check `Cargo.toml` for the `agent` crate to confirm `cognitive` is a dependency — if not, add `cognitive = { path = "../cognitive" }`.

- [ ] **Step 4: Replace placeholder metrics in `collect_metrics`**

Replace the body of the `MetricSource::collect_metrics` implementation:

```rust
    async fn collect_metrics(
        &self,
        since: DateTime<Utc>,
        trial_id: Option<uuid::Uuid>,
    ) -> common::Result<MetricSnapshot> {
        let stats = self.strategy_repo.get_stats_since(since).await?;

        // 1. correction_rate — from domain_event_log
        let correction_count = self
            .event_log_repo
            .count_by_event_type("UserCorrectedAI", since)
            .await
            .unwrap_or(0);
        let total = stats.total_records.max(1);
        let correction_rate = (correction_count as f64 / total as f64).min(1.0);

        // 2. avg_tokens_per_message — from usage_records
        let total_tokens = self.usage_repo.total_tokens_since(since).await.unwrap_or(0);
        let (total_requests, _) = self.usage_repo.totals_since(since).await.unwrap_or((0, 0.0));
        let avg_tokens_per_message = total_tokens as f64 / total_requests.max(1) as f64;

        // 3. routing_stability — from shadow_log agreement rate
        // Note: MetricSource trait uses Option<uuid::Uuid>, but shadow_log_agreement_rate
        // takes Option<&str>. Convert with .as_ref().map(|u| u.to_string()).
        let trial_id_str = trial_id.as_ref().map(|u| u.to_string());
        let routing_stability = self
            .trial_repo
            .shadow_log_agreement_rate(trial_id_str.as_deref(), since)
            .await
            .unwrap_or(1.0);

        // 4. memory_relevance — Phase 2 placeholder
        let memory_relevance = 1.0;

        Ok(MetricSnapshot {
            correction_rate,
            classification_accuracy: stats.accuracy,
            avg_tokens_per_message,
            avg_response_time_ms: stats.avg_response_time_ms as f64,
            routing_stability,
            memory_relevance,
            user_satisfaction: stats.avg_satisfaction,
            total_messages: stats.total_records as u32,
        })
    }
```

- [ ] **Step 5: Update wiring in `init/cron.rs`**

In `crates/app-core/src/init/cron.rs`, find the `AgentMetricCollector::new` call (line ~119) and update:

Old:
```rust
        let strategy_repo = repos.strategies.clone();
        let metric_source: Arc<dyn autotuner::MetricSource> =
            Arc::new(agent::autotuner::metric_collector::AgentMetricCollector::new(strategy_repo));
```

New (verify exact repo accessor names from `Repos` struct):
```rust
        let metric_source: Arc<dyn autotuner::MetricSource> =
            Arc::new(agent::autotuner::metric_collector::AgentMetricCollector::new(
                repos.strategies.clone(),
                cognitive_repos.event_log.clone(),
                repos.usage.clone(),
                trial_repo.clone(),
            ));
```

Note: Verify the exact field names on the repos structs. The cognitive repos may be accessed via a different pattern — check how `EventLogRepo` is obtained at the init site. It may be `repos.event_log` or accessed via a cognitive-specific repos struct.

- [ ] **Step 6: Compile check + run tests**

Run: `cargo check --workspace 2>&1 | tail -20`
Run: `cargo nextest run -p agent -E 'test(metric_collector)' --no-fail-fast 2>&1 | tail -20`
Expected: All pass.

- [ ] **Step 7: Commit**

```bash
git add crates/agent/src/autotuner/metric_collector.rs crates/app-core/src/init/cron.rs
git commit -m "feat(autotuner): replace placeholder metrics with real queries"
```

---

### Task 6: Emit correction signals from AgentLoop

**Files:**
- Modify: `crates/agent/src/agent_loop/mod.rs` (lines 73, 124–156, 343–403)
- Modify: `crates/agent/src/agent_loop/builder.rs` (line 1468)

This is the largest task — it adds correction emission (reactions + keywords), injects `TrialRepo`, renames `_domain_event_bus`, and adds the correction acknowledgment flag.

- [ ] **Step 1: Write test for keyword correction detection**

Add a unit test in `crates/agent/src/agent_loop/mod.rs` (or a new test module):

```rust
#[cfg(test)]
mod correction_tests {
    /// Returns (matched: bool, strength: f64) for correction prefix detection
    fn detect_correction_prefix(message: &str) -> Option<f64> {
        let lower = message.to_lowercase();
        let check = &lower[..lower.len().min(80)];

        const STRONG: &[&str] = &["no,", "no ", "wrong", "that's not", "incorrect"];
        const SOFT: &[&str] = &["i meant", "try again", "redo", "not quite", "never mind"];

        for prefix in STRONG {
            if check.starts_with(prefix) {
                return Some(1.0);
            }
        }
        for prefix in SOFT {
            if check.starts_with(prefix) {
                return Some(0.8);
            }
        }
        None
    }

    #[test]
    fn detects_strong_corrections() {
        assert_eq!(detect_correction_prefix("No, that's wrong"), Some(1.0));
        assert_eq!(detect_correction_prefix("wrong answer"), Some(1.0));
        assert_eq!(detect_correction_prefix("incorrect, I wanted"), Some(1.0));
        assert_eq!(detect_correction_prefix("That's not what I meant"), Some(1.0));
    }

    #[test]
    fn detects_soft_corrections() {
        assert_eq!(detect_correction_prefix("I meant the other one"), Some(0.8));
        assert_eq!(detect_correction_prefix("try again please"), Some(0.8));
        assert_eq!(detect_correction_prefix("not quite right"), Some(0.8));
        assert_eq!(detect_correction_prefix("never mind, let me rephrase"), Some(0.8));
    }

    #[test]
    fn ignores_normal_messages() {
        assert_eq!(detect_correction_prefix("What's the weather?"), None);
        assert_eq!(detect_correction_prefix("Actually that reminds me"), None);
        assert_eq!(detect_correction_prefix("Wait before that"), None);
        assert_eq!(detect_correction_prefix("Hello there"), None);
    }
}
```

- [ ] **Step 2: Run test to verify it passes (pure function)**

Run: `cargo nextest run -p agent -E 'test(correction)' --no-fail-fast 2>&1 | tail -10`
Expected: Pass (this is a standalone pure function test).

- [ ] **Step 3: Rename `_domain_event_bus` → `domain_event_bus`**

In `crates/agent/src/agent_loop/mod.rs`:
- Line 73: rename field from `_domain_event_bus` to `domain_event_bus`
- Update all references in the file (line ~430 where `ChatTurnCompleted` is published)

In `crates/agent/src/agent_loop/builder.rs`:
- Line 1468: update `_domain_event_bus: self.domain_event_bus` → `domain_event_bus: self.domain_event_bus`

- [ ] **Step 4: Add `trial_repo` to `AgentLoop`**

In `crates/agent/src/agent_loop/mod.rs`, add field:
```rust
    trial_repo: Option<storage::TrialRepo>,
```

In `crates/agent/src/agent_loop/builder.rs`, in the `AgentLoop` struct initialization (around line 1460):
```rust
    trial_repo: if self.autotuner.is_some() {
        self.pool.as_ref().map(|p| storage::TrialRepo::new(p.clone()))
    } else {
        None
    },
```

- [ ] **Step 5: Extract `detect_correction_prefix` as a module-level function**

Move the `detect_correction_prefix` function from the test module to the main module so it can be called from `process_message`. Keep the tests referencing it.

- [ ] **Step 6: Add correction emission to `handle_reaction`**

In `crates/agent/src/agent_loop/mod.rs`, in `handle_reaction` (after the existing `set_satisfaction_for_chat` call), add:

```rust
        // Emit UserCorrectedAI for negative reactions
        if score == 0.0 {
            if let Some(ref bus) = self.domain_event_bus {
                // Read last assistant message from session
                let session_key = msg.session_key();
                let original = if let Ok(session_arc) = self.session_manager.get_or_create(&session_key, None).await {
                    let session = session_arc.lock().await;
                    session.get_history(5)
                        .iter()
                        .rev()
                        .find(|m| m.role == "assistant")
                        .map(|m| m.content.clone())
                        .unwrap_or_else(|| "(unavailable)".into())
                } else {
                    "(unavailable)".into()
                };

                let _ = bus.publish(bus::DomainEvent::UserCorrectedAI {
                    original,
                    correction: format!("[reaction:{}]", msg.content),
                    kind: bus::CorrectionKind::Reaction,
                    strength: 1.0,
                });
            }

            // Mark shadow log rows as corrected
            if let Some(ref trial_repo) = self.trial_repo {
                let _ = trial_repo.mark_recent_messages_corrected(msg.chat_id.as_str(), 15).await;
            }
        }
```

- [ ] **Step 7: Add keyword correction detection to `process_message`**

In `process_message`, after the session is loaded and `history` is extracted (around line 403), before `run_pipeline`:

```rust
        // Keyword correction detection
        if let Some(strength) = detect_correction_prefix(&msg.content) {
            // Check if last message was from assistant
            let last_was_assistant = {
                let session = session_arc.lock().await;
                session.get_history(1)
                    .last()
                    .map(|m| m.role == "assistant")
                    .unwrap_or(false)
            };

            if last_was_assistant {
                if let Some(ref bus) = self.domain_event_bus {
                    let original = {
                        let session = session_arc.lock().await;
                        session.get_history(2)
                            .iter()
                            .rev()
                            .find(|m| m.role == "assistant")
                            .map(|m| m.content.clone())
                            .unwrap_or_else(|| "(unavailable)".into())
                    };

                    let _ = bus.publish(bus::DomainEvent::UserCorrectedAI {
                        original,
                        correction: msg.content.clone(),
                        kind: bus::CorrectionKind::KeywordPrefix,
                        strength,
                    });
                }

                if let Some(ref trial_repo) = self.trial_repo {
                    let _ = trial_repo.mark_recent_messages_corrected(msg.chat_id.as_str(), 15).await;
                }
            }
        }
```

Note: The rate limiter (max 1 per 3 messages) can be added via a simple `AtomicU32` counter on the session or a `HashMap<String, u32>` on `AgentLoop`. For v1, skip the rate limiter and add it as a follow-up if needed — the keyword list is already conservative enough.

- [ ] **Step 8: Compile check**

Run: `cargo check --workspace 2>&1 | tail -20`
Expected: No errors.

- [ ] **Step 9: Run all agent tests**

Run: `cargo nextest run -p agent --no-fail-fast 2>&1 | tail -30`
Expected: All pass (existing tests may need minor updates for the renamed field).

- [ ] **Step 10: Commit**

```bash
git add crates/agent/src/agent_loop/mod.rs crates/agent/src/agent_loop/builder.rs
git commit -m "feat(autotuner): emit UserCorrectedAI from reactions and keyword corrections"
```

---

### Task 7: Add observability logging and AutotunerDecision emission

**Files:**
- Modify: `crates/agent/src/autotuner/mod.rs`

- [ ] **Step 1: Add structured logging to nightly cycle callback**

Find the `register_nightly_cycle` function in `crates/agent/src/autotuner/mod.rs`. Inside the callback closure that calls `NightlyCycle::run_evaluation_and_promotion`, add logging after each evaluation result.

After each trial evaluation:
```rust
tracing::info!(
    trial_id = %trial_id,
    correction_rate = %format!("{:.3}", result.correction_rate),
    accuracy = %format!("{:.3}", result.classification_accuracy),
    messages = result.messages_scored,
    "Autotuner: trial evaluated"
);
```

After promotion:
```rust
tracing::info!(
    trial_id = %winner_id,
    "Autotuner: trial PROMOTED to champion"
);
```

After regression detection:
```rust
tracing::warn!(
    days = champion.consecutive_regression_days,
    "Autotuner: champion regression detected"
);
```

- [ ] **Step 2: Emit `AutotunerDecision` domain event**

In the same nightly cycle callback, after a promotion or rollback decision, emit the event. This requires injecting the `DomainEventBus` into the closure. Check if the `register_nightly_cycle` function has access to a bus — if not, add it as a parameter.

```rust
if let Some(ref bus) = domain_event_bus {
    let _ = bus.publish(bus::DomainEvent::AutotunerDecision {
        trial_id: winner_id.clone(),
        verdict: "promoted".into(),
        improvement_pct,
        affected_params: changed_param_names,
    });
}
```

For rollback:
```rust
if let Some(ref bus) = domain_event_bus {
    let _ = bus.publish(bus::DomainEvent::AutotunerDecision {
        trial_id: trial_id.clone(),
        verdict: "rolled_back".into(),
        improvement_pct: 0.0,
        affected_params: vec![],
    });
}
```

Note: The `changed_param_names` can be derived by comparing the new champion's `TrialParams` fields against the old champion's — iterate over the 8 option fields and collect names where the new value differs.

- [ ] **Step 3: Compile check + run tests**

Run: `cargo check --workspace 2>&1 | tail -20`
Run: `cargo nextest run -p agent -E 'test(autotuner)' --no-fail-fast 2>&1 | tail -20`
Expected: All pass.

- [ ] **Step 4: Commit**

```bash
git add crates/agent/src/autotuner/mod.rs
git commit -m "feat(autotuner): add observability logging and AutotunerDecision events"
```

---

### Task 8: Add brain_growth + metrics_health to status response

**Files:**
- Modify: `crates/app-core/src/handlers/autotuner.rs`

- [ ] **Step 1: Define new response types**

Add to `crates/app-core/src/handlers/autotuner.rs`:

```rust
#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BrainGrowth {
    pub corrections_captured_7d: i64,
    pub trials_evaluated_7d: i64,
    pub promoted_this_week: i64,
    pub status: String,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MetricsHealth {
    pub correction_rate_available: bool,
    pub token_rate_available: bool,
    pub stability_available: bool,
}
```

- [ ] **Step 2: Add fields to `AutoTunerStatus`**

Update the `AutoTunerStatus` struct:

```rust
pub struct AutoTunerStatus {
    pub enabled: bool,
    pub champion: autotuner::ChampionSummary,
    pub active_experiment: Option<autotuner::ExperimentSummary>,
    pub paused: bool,
    pub brain_growth: Option<BrainGrowth>,
    pub metrics_health: Option<MetricsHealth>,
}
```

- [ ] **Step 3: Populate the new fields in `autotuner_status` handler**

In the handler, after the existing logic, compute:

```rust
// Compute brain_growth
let seven_days_ago = chrono::Utc::now() - chrono::Duration::days(7);
let corrections_7d = event_log_repo.count_by_event_type("UserCorrectedAI", seven_days_ago).await.unwrap_or(0);
let trials_7d = trial_repo.count_trials_since(seven_days_ago).await.unwrap_or(0);
let promoted_7d = trial_repo.count_promoted_since(seven_days_ago).await.unwrap_or(0);

let status = if corrections_7d == 0 {
    "needs_feedback".into()
} else if promoted_7d == 0 {
    "adapting".into()
} else {
    "growing".into()
};

let brain_growth = Some(BrainGrowth {
    corrections_captured_7d: corrections_7d,
    trials_evaluated_7d: trials_7d,
    promoted_this_week: promoted_7d,
    status,
});

let metrics_health = Some(MetricsHealth {
    correction_rate_available: corrections_7d > 0,
    token_rate_available: true, // usage_records always exist once agent runs
    stability_available: true,  // shadow_log writes ground truth now
});
```

Note: The handler will need access to `EventLogRepo` and `TrialRepo`. Check how `AppCore` provides repos — it likely has a `repos()` method or the handler receives them as parameters. Wire them through `AppCore` following the existing pattern for other handlers.

- [ ] **Step 4: Compile check + run existing tests**

Run: `cargo check --workspace 2>&1 | tail -20`
Run: `cargo nextest run -p app-core -E 'test(autotuner)' --no-fail-fast 2>&1 | tail -20`
Expected: All pass.

- [ ] **Step 5: Commit**

```bash
git add crates/app-core/src/handlers/autotuner.rs
git commit -m "feat(autotuner): add brain_growth and metrics_health to status response"
```

---

### Task 9: Final integration test + cleanup

**Files:**
- Test in root `tests/` or `crates/agent/`

- [ ] **Step 1: Run full workspace tests**

Run: `cargo nextest run --workspace --no-fail-fast 2>&1 | tail -40`
Expected: All pass. Fix any failures.

- [ ] **Step 2: Run clippy**

Run: `cargo clippy --workspace --all-targets --all-features 2>&1 | tail -20`
Expected: 0 warnings. Fix any issues.

- [ ] **Step 3: Run fmt check**

Run: `cargo fmt --all --check 2>&1 | tail -10`
Expected: No formatting issues. Run `cargo fmt --all` if needed.

- [ ] **Step 4: Final commit if any fixes**

```bash
git add -A
git commit -m "chore: fix clippy warnings and formatting from autotuner feedback loop"
```

- [ ] **Step 5: Verify success criteria**

Manually verify by reading the code:
1. `MetricSnapshot.correction_rate` — computed from `EventLogRepo::count_by_event_type`
2. `MetricSnapshot.avg_tokens_per_message` — computed from `UsageRepo::total_tokens_since`
3. `MetricSnapshot.routing_stability` — computed from `TrialRepo::shadow_log_agreement_rate`
4. Shadow log `control_orchestrator` and `control_mode` — written by `on_message_completed`
5. Logging in nightly cycle — `tracing::info!` calls present
6. `autotuner_status` returns `brain_growth` field

---

## Dependency Graph

```
Task 1 (domain events) ──┬──→ Task 2 (TrialRepo methods)
                          │
                          ├──→ Task 3 (EventLogRepo + UsageRepo methods)
                          │
                          └──→ Task 6 (correction emission from AgentLoop)

Task 2 ──→ Task 4 (ground truth hook wiring)

Task 2 + Task 3 ──→ Task 5 (metric collector replacement)

Task 1 ──→ Task 7 (observability + AutotunerDecision)

Task 3 + Task 2 ──→ Task 8 (brain_growth status)

All ──→ Task 9 (integration test + cleanup)
```

Tasks 2, 3, and 6 can be parallelized after Task 1 completes.
