# Legacy Learning System Cleanup — Implementation Plan

> **For agentic workers:** REQUIRED: Use superpowers:subagent-driven-development (if subagents available) or superpowers:executing-plans to implement this plan. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Remove unused L2 learning tables and migrate `behavioral_patterns` to the L5 cognitive pipeline.

**Architecture:** Two-phase cleanup. Phase 1 deletes zombie tables (`user_profile`, `agent_adaptations`) and all their Rust code. Phase 2 rewrites `PatternAnalyzer` to emit `DomainEvent::BehavioralPatternDetected` instead of writing to `behavioral_patterns`, then replaces the transparency read with L5 `ProceduralRuleRepo`. A single SQLite migration drops all three tables.

**Tech Stack:** Rust, SQLite (sqlx), tokio broadcast channels (`DomainEventBus`), `cargo nextest`

**Spec:** `docs/superpowers/specs/2026-03-11-legacy-learning-cleanup-design.md`

---

## File Structure

| Action | File | Responsibility |
|--------|------|----------------|
| Delete | `crates/storage/src/repos/user_profile.rs` | `UserProfileRepo` (zombie) |
| Delete | `crates/storage/src/repos/agent_adaptation.rs` | `AgentAdaptationRepo` (zombie) |
| Delete | `crates/storage/src/repos/behavioral_pattern.rs` | `BehavioralPatternRepo` (migrated to L5) |
| Modify | `crates/storage/src/repos/mod.rs` | Remove 3 repos from `Repos` struct, module declarations, re-exports |
| Modify | `crates/storage/src/lib.rs` | Remove 6 re-exports (3 repos + 3 row structs) |
| Modify | `crates/storage/src/rows/learning.rs` | Remove `UserProfileRow`, `BehavioralPatternRow`, `AgentAdaptationRow` |
| Create | `crates/storage/migrations/008_drop_legacy_learning.sql` | DROP 3 L2 tables |
| Modify | `crates/bus/src/domain_events.rs` | Add `BehavioralPatternDetected` variant |
| Modify | `crates/cognitive/src/salience.rs` | Map `BehavioralPatternDetected` → `Accumulate` |
| Modify | `crates/cognitive/src/repos/procedural_rule.rs` | Add `list_all_active()` method (no domain filter) |
| Modify | `crates/agent/src/learning/pattern_analyzer.rs` | Rewrite: emit domain events instead of repo writes |
| Modify | `crates/agent/src/learning/service.rs` | Update `PatternAnalyzer` construction |
| Modify | `crates/agent/src/agent_loop/builder.rs` | Wire domain event bus to `PatternAnalyzer`, remove `with_learning_repos` |
| Modify | `crates/agent/src/agent_runtime/runtime.rs` | Remove 3 L2 learning fields, replace transparency with `ProceduralRuleRepo` |

---

## Chunk 1: Phase 1 — Delete Zombie Tables + Phase 2 Domain Event Wiring

### Task 1: Add `BehavioralPatternDetected` Domain Event

**Files:**
- Modify: `crates/bus/src/domain_events.rs:14-158` (add variant to `DomainEvent` enum)

- [ ] **Step 1: Write the failing test**

In `crates/bus/src/domain_events.rs`, add this test inside `mod tests` (after the existing `test_domain_event_serialization` test at line 234):

```rust
#[test]
fn test_behavioral_pattern_detected_serialization() {
    let event = DomainEvent::BehavioralPatternDetected {
        pattern_type: "day_of_week".into(),
        pattern_key: "monday_task".into(),
        sample_count: 15,
        detail: "User uses task agent frequently on Mondays (15 interactions)".into(),
    };
    let json = serde_json::to_string(&event).unwrap();
    let deserialized: DomainEvent = serde_json::from_str(&json).unwrap();
    assert!(matches!(
        deserialized,
        DomainEvent::BehavioralPatternDetected { sample_count: 15, .. }
    ));
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo nextest run -p bus -E 'test(test_behavioral_pattern_detected_serialization)'`
Expected: FAIL — `BehavioralPatternDetected` variant doesn't exist yet

- [ ] **Step 3: Add the variant to DomainEvent**

In `crates/bus/src/domain_events.rs`, add after the `CoachingFeedback` variant (line 157, before the closing `}`):

```rust
    // -- Learning patterns (migrated from L2) --
    BehavioralPatternDetected {
        pattern_type: String,
        pattern_key: String,
        sample_count: i32,
        detail: String,
    },
```

- [ ] **Step 4: Fix exhaustive match in salience.rs**

The new variant will cause a compile error in `crates/cognitive/src/salience.rs` because the match is exhaustive (no wildcard). Add the permanent salience arm at the end of the `match event` block (before the closing `}` at line 66):

```rust
        DomainEvent::BehavioralPatternDetected { .. } => SalienceVerdict::Accumulate,
```

- [ ] **Step 5: Run tests to verify they pass**

Run: `cargo nextest run -p bus -E 'test(test_behavioral_pattern_detected_serialization)'`
Expected: PASS

- [ ] **Step 6: Commit**

```bash
git add crates/bus/src/domain_events.rs crates/cognitive/src/salience.rs
git commit -m "feat(bus): add BehavioralPatternDetected domain event variant"
```

---

### Task 2: Add Salience Test for BehavioralPatternDetected

**Files:**
- Modify: `crates/cognitive/src/salience.rs:69-167` (add test)

- [ ] **Step 1: Write the test**

In `crates/cognitive/src/salience.rs`, add inside `mod tests`:

```rust
#[test]
fn test_behavioral_pattern_detected_is_accumulate() {
    let verdict = evaluate_salience(&DomainEvent::BehavioralPatternDetected {
        pattern_type: "day_of_week".into(),
        pattern_key: "monday_task".into(),
        sample_count: 15,
        detail: "User uses task agent frequently on Mondays".into(),
    });
    assert_eq!(verdict, SalienceVerdict::Accumulate);
}
```

- [ ] **Step 2: Run test to verify it passes**

Run: `cargo nextest run -p cognitive -E 'test(test_behavioral_pattern_detected_is_accumulate)'`
Expected: PASS (we added the arm in Task 1 Step 4)

- [ ] **Step 3: Commit**

```bash
git add crates/cognitive/src/salience.rs
git commit -m "test(cognitive): add salience test for BehavioralPatternDetected"
```

---

### Task 3: Add `list_all_active()` to ProceduralRuleRepo

**Files:**
- Modify: `crates/cognitive/src/repos/procedural_rule.rs`

The existing `list_active(domain)` filters by a single domain. The transparency panel needs to show all learned rules regardless of domain.

- [ ] **Step 1: Write the failing test**

In `crates/cognitive/src/repos/procedural_rule.rs`, add inside `mod tests`:

```rust
#[tokio::test]
async fn test_list_all_active_across_domains() {
    let pool = setup().await;
    let repo = ProceduralRuleRepo::new(pool);

    let r1 = test_rule("r1", "productivity", "Morning is peak time");
    let r2 = test_rule("r2", "finance", "Track daily expenses");
    let r3 = test_rule("r3", "productivity", "Break every 90 minutes");
    repo.upsert(&r1).await.unwrap();
    repo.upsert(&r2).await.unwrap();
    repo.upsert(&r3).await.unwrap();

    // Deactivate one to verify filtering
    repo.deactivate("r3").await.unwrap();

    let all_active = repo.list_all_active().await.unwrap();
    assert_eq!(all_active.len(), 2);
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo nextest run -p cognitive -E 'test(test_list_all_active_across_domains)'`
Expected: FAIL — `list_all_active()` doesn't exist

- [ ] **Step 3: Implement `list_all_active()`**

In `crates/cognitive/src/repos/procedural_rule.rs`, add after the `list_active` method (line 56):

```rust
    /// List all active rules across all domains.
    pub async fn list_all_active(&self) -> Result<Vec<ProceduralRule>, sqlx::Error> {
        sqlx::query_as::<_, ProceduralRule>(
            "SELECT * FROM procedural_rules WHERE active = 1 ORDER BY confidence DESC",
        )
        .fetch_all(&self.pool)
        .await
    }
```

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo nextest run -p cognitive -E 'test(test_list_all_active_across_domains)'`
Expected: PASS

- [ ] **Step 5: Commit**

```bash
git add crates/cognitive/src/repos/procedural_rule.rs
git commit -m "feat(cognitive): add list_all_active() to ProceduralRuleRepo"
```

---

### Task 4: Rewrite PatternAnalyzer to Use DomainEventBus

**Files:**
- Modify: `crates/agent/src/learning/pattern_analyzer.rs` (full rewrite)

- [ ] **Step 1: Write the failing test**

Replace the entire `#[cfg(test)] mod tests` block in `crates/agent/src/learning/pattern_analyzer.rs` with:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_pattern_analyzer_emits_domain_events() {
        let pool = storage::StoragePool::connect_in_memory().await.unwrap();
        let repos = storage::Repos::from_pool(&pool);

        // Insert 15 interactions on a Monday morning with agent="task"
        for _ in 0..15 {
            repos
                .interaction_log
                .create_with_timestamp(
                    "task",
                    &["task"],
                    "telegram",
                    Some(100),
                    "2026-03-02 10:00:00", // Monday morning
                )
                .await
                .unwrap();
        }

        let event_bus = Arc::new(bus::DomainEventBus::new(64));
        let mut rx = event_bus.subscribe();

        let analyzer = PatternAnalyzer::new(
            repos.interaction_log.clone(),
            Arc::clone(&event_bus),
        );
        analyzer.analyze().await.unwrap();

        // Collect all published events
        let mut events = Vec::new();
        while let Ok(event) = rx.try_recv() {
            events.push(event);
        }

        // Should have: day_of_week + time_of_day + agent_usage = at least 3 events
        assert!(events.len() >= 3, "Expected at least 3 events, got {}", events.len());

        let has_day = events.iter().any(|e| matches!(
            e, bus::DomainEvent::BehavioralPatternDetected { pattern_type, .. }
            if pattern_type == "day_of_week"
        ));
        assert!(has_day, "Expected a day_of_week pattern event");

        let has_time = events.iter().any(|e| matches!(
            e, bus::DomainEvent::BehavioralPatternDetected { pattern_type, .. }
            if pattern_type == "time_of_day"
        ));
        assert!(has_time, "Expected a time_of_day pattern event");

        let has_agent = events.iter().any(|e| matches!(
            e, bus::DomainEvent::BehavioralPatternDetected { pattern_type, .. }
            if pattern_type == "agent_usage"
        ));
        assert!(has_agent, "Expected an agent_usage pattern event");
    }

    #[tokio::test]
    async fn test_insufficient_data_emits_no_events() {
        let pool = storage::StoragePool::connect_in_memory().await.unwrap();
        let repos = storage::Repos::from_pool(&pool);

        // Only 3 interactions — below threshold
        for _ in 0..3 {
            repos
                .interaction_log
                .create("task", &["task"], "telegram", Some(100))
                .await
                .unwrap();
        }

        let event_bus = Arc::new(bus::DomainEventBus::new(64));
        let mut rx = event_bus.subscribe();

        let analyzer = PatternAnalyzer::new(
            repos.interaction_log.clone(),
            Arc::clone(&event_bus),
        );
        analyzer.analyze().await.unwrap();

        assert!(rx.try_recv().is_err(), "Should not emit events with insufficient data");
    }
}
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo nextest run -p agent -E 'test(pattern_analyzer)'`
Expected: FAIL — `PatternAnalyzer::new` signature mismatch (still takes `BehavioralPatternRepo`)

- [ ] **Step 3: Rewrite the PatternAnalyzer struct and constructor**

Replace the entire `PatternAnalyzer` struct and `impl` (lines 19-125) with:

```rust
/// Analyzes interaction logs to detect behavioral patterns.
///
/// Emits `DomainEvent::BehavioralPatternDetected` events instead of writing
/// to the (now-removed) `behavioral_patterns` table. The cognitive pipeline
/// processes these events into procedural rules via salience → extraction →
/// consolidation.
pub struct PatternAnalyzer {
    log_repo: storage::InteractionLogRepo,
    event_bus: Arc<bus::DomainEventBus>,
}

impl PatternAnalyzer {
    pub fn new(
        log_repo: storage::InteractionLogRepo,
        event_bus: Arc<bus::DomainEventBus>,
    ) -> Self {
        Self {
            log_repo,
            event_bus,
        }
    }

    /// Run all pattern analyses.
    pub async fn analyze(&self) -> common::Result<()> {
        let logs = self.log_repo.list_recent(1000).await?;

        if logs.len() < MIN_INTERACTIONS_FOR_ANALYSIS {
            return Ok(());
        }

        // Single pass: accumulate all three pattern types simultaneously
        let mut day_agent_counts: HashMap<(String, String), i32> = HashMap::new();
        let mut time_counts: HashMap<String, i32> = HashMap::new();

        for log in &logs {
            if let Ok(dt) =
                chrono::NaiveDateTime::parse_from_str(&log.timestamp, "%Y-%m-%d %H:%M:%S")
            {
                // Day-of-week × agent
                let day = match dt.weekday() {
                    chrono::Weekday::Mon => "monday",
                    chrono::Weekday::Tue => "tuesday",
                    chrono::Weekday::Wed => "wednesday",
                    chrono::Weekday::Thu => "thursday",
                    chrono::Weekday::Fri => "friday",
                    chrono::Weekday::Sat => "saturday",
                    chrono::Weekday::Sun => "sunday",
                };
                *day_agent_counts
                    .entry((day.to_string(), log.agent_name.clone()))
                    .or_default() += 1;

                // Time-of-day
                let period = match dt.hour() {
                    5..=11 => "morning",
                    12..=17 => "afternoon",
                    18..=22 => "evening",
                    _ => "night",
                };
                *time_counts.entry(period.to_string()).or_default() += 1;
            }
        }

        // Emit day-of-week patterns
        for ((day, agent), count) in &day_agent_counts {
            if *count >= MIN_PATTERN_OCCURRENCES {
                self.event_bus.publish(bus::DomainEvent::BehavioralPatternDetected {
                    pattern_type: "day_of_week".into(),
                    pattern_key: format!("{}_{}", day, agent),
                    sample_count: *count,
                    detail: format!(
                        "User uses {} agent frequently on {}s ({} interactions)",
                        agent, day, count
                    ),
                });
            }
        }

        // Emit time-of-day patterns
        for (period, count) in &time_counts {
            if *count >= MIN_PATTERN_OCCURRENCES {
                self.event_bus.publish(bus::DomainEvent::BehavioralPatternDetected {
                    pattern_type: "time_of_day".into(),
                    pattern_key: period.clone(),
                    sample_count: *count,
                    detail: format!(
                        "User is most active in the {} ({} interactions)",
                        period, count
                    ),
                });
            }
        }

        // Agent usage: use the repo's SQL GROUP BY
        let agent_counts = self.log_repo.count_by_agent().await?;
        for (agent, count) in &agent_counts {
            let count_i32 = *count as i32;
            if count_i32 >= MIN_PATTERN_OCCURRENCES {
                self.event_bus.publish(bus::DomainEvent::BehavioralPatternDetected {
                    pattern_type: "agent_usage".into(),
                    pattern_key: agent.clone(),
                    sample_count: count_i32,
                    detail: format!(
                        "{} agent is used frequently ({} total uses)",
                        agent, count
                    ),
                });
            }
        }

        Ok(())
    }
}
```

- [ ] **Step 4: Update imports**

At the top of `crates/agent/src/learning/pattern_analyzer.rs`:

1. **Remove** `use tracing::warn;` (line 11) — the rewrite no longer uses `warn!()`, and clippy will flag unused imports.

2. **Add** `use std::sync::Arc;` after the existing `use` block.

- [ ] **Step 5: Run tests to verify they pass**

Run: `cargo nextest run -p agent -E 'test(pattern_analyzer)'`
Expected: PASS

- [ ] **Step 6: Commit**

```bash
git add crates/agent/src/learning/pattern_analyzer.rs
git commit -m "refactor(agent): rewrite PatternAnalyzer to emit domain events"
```

---

### Task 5: Update LearningService + Builder Wiring

**Files:**
- Modify: `crates/agent/src/learning/service.rs:65-67`
- Modify: `crates/agent/src/agent_loop/builder.rs:952-956`

- [ ] **Step 1: Update builder to pass event bus to PatternAnalyzer**

In `crates/agent/src/agent_loop/builder.rs`, replace the `PatternAnalyzer` construction and service wiring (lines 952-965). The original code is:

```rust
            let pattern_analyzer = crate::learning::PatternAnalyzer::new(
                repos.interaction_log.clone(),
                repos.behavioral_patterns.clone(),
            );
            let mut service = crate::learning::LearningService::new(
                Arc::clone(store),
                adaptive,
                threshold_handle,
                Duration::from_secs(config.learning.analysis_interval_secs),
            )
            .with_event_bus(event_bus)
            .with_pattern_analyzer(pattern_analyzer);
            service.start();
```

Replace with (`self.domain_event_bus` is accessible at `self` scope — the `PatternAnalyzer::new()` now requires `Arc<DomainEventBus>`, so we wrap in `Option` when the bus is absent):

```rust
            let pattern_analyzer = if let Some(ref domain_bus) = self.domain_event_bus {
                Some(crate::learning::PatternAnalyzer::new(
                    repos.interaction_log.clone(),
                    Arc::clone(domain_bus),
                ))
            } else {
                None
            };
            let mut service = crate::learning::LearningService::new(
                Arc::clone(store),
                adaptive,
                threshold_handle,
                Duration::from_secs(config.learning.analysis_interval_secs),
            )
            .with_event_bus(event_bus);
            if let Some(pa) = pattern_analyzer {
                service = service.with_pattern_analyzer(pa);
            }
            service.start();
```

- [ ] **Step 2: Run workspace build to verify**

Run: `cargo build -p agent 2>&1 | head -30`
Expected: Compiles without errors (storage changes haven't happened yet, so `behavioral_patterns` still exists in `Repos` — this step only changes the `PatternAnalyzer` construction path)

- [ ] **Step 3: Run tests**

Run: `cargo nextest run -p agent -E 'test(pattern_analyzer)'`
Expected: PASS

Run: `cargo nextest run -p agent -E 'test(learning_service)'`
Expected: PASS (LearningService tests don't wire a PatternAnalyzer)

- [ ] **Step 4: Commit**

```bash
git add crates/agent/src/agent_loop/builder.rs
git commit -m "refactor(agent): wire DomainEventBus to PatternAnalyzer in builder"
```

---

## Chunk 2: Phase 1 — Remove Zombie Code + Phase 2 — Remove behavioral_patterns

> **Dependency:** Chunk 2 requires Chunk 1 to be completed first (Task 3 adds `list_all_active()` used in Task 7).
>
> **Non-compiling window:** Tasks 6, 7, and 8 form an atomic group — the workspace will NOT compile between them. The `storage` crate breaks after Task 6 (deleted repos), `agent` crate breaks until Task 8 replaces `with_learning_repos`. Only after Task 8 Step 2 does the workspace compile cleanly. Individual commits are fine for tracking progress but expect build failures until Task 8 completes.

### Task 6: Delete Zombie Repo Files

**Files:**
- Delete: `crates/storage/src/repos/user_profile.rs`
- Delete: `crates/storage/src/repos/agent_adaptation.rs`
- Delete: `crates/storage/src/repos/behavioral_pattern.rs`

- [ ] **Step 1: Delete the three files**

```bash
rm crates/storage/src/repos/user_profile.rs
rm crates/storage/src/repos/agent_adaptation.rs
rm crates/storage/src/repos/behavioral_pattern.rs
```

- [ ] **Step 2: Remove module declarations and re-exports from repos/mod.rs**

In `crates/storage/src/repos/mod.rs`:

Remove these module declarations (lines 4, 7, 33):
```rust
pub mod agent_adaptation;
pub mod behavioral_pattern;
pub mod user_profile;
```

Remove these re-exports (lines 39, 42, 67):
```rust
pub use agent_adaptation::AgentAdaptationRepo;
pub use behavioral_pattern::BehavioralPatternRepo;
pub use user_profile::UserProfileRepo;
```

Remove these fields from the `Repos` struct (lines 90-92):
```rust
    pub user_profile: UserProfileRepo,
    pub behavioral_patterns: BehavioralPatternRepo,
    pub agent_adaptations: AgentAdaptationRepo,
```

Remove these lines from `Repos::from_pool()` (lines 121-123):
```rust
            user_profile: UserProfileRepo::new(db.clone()),
            behavioral_patterns: BehavioralPatternRepo::new(db.clone()),
            agent_adaptations: AgentAdaptationRepo::new(db.clone()),
```

- [ ] **Step 3: Remove row structs from rows/learning.rs**

In `crates/storage/src/rows/learning.rs`, remove:

Lines 80-93 (`UserProfileRow`):
```rust
/// Row struct for the `user_profile` table.
#[derive(Debug, Clone, FromRow, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct UserProfileRow {
    pub id: i32,
    pub category: String,
    pub key: String,
    pub value: String,
    pub source: String,
    pub confidence: f64,
    pub agent_name: Option<String>,
    pub last_confirmed: String,
    pub created_at: String,
}
```

Lines 95-105 (`BehavioralPatternRow`):
```rust
/// Row struct for the `behavioral_patterns` table.
#[derive(Debug, Clone, FromRow, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BehavioralPatternRow {
    pub id: i32,
    pub pattern_type: String,
    pub pattern_key: String,
    pub pattern_value: String,
    pub sample_count: i32,
    pub last_updated: String,
}
```

Lines 107-118 (`AgentAdaptationRow`):
```rust
/// Row struct for the `agent_adaptations` table.
#[derive(Debug, Clone, FromRow, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AgentAdaptationRow {
    pub id: i32,
    pub agent_name: String,
    pub preference_key: String,
    pub preference_value: String,
    pub source: String,
    pub confidence: f64,
    pub last_updated: String,
}
```

- [ ] **Step 4: Remove re-exports from storage/src/lib.rs**

In `crates/storage/src/lib.rs`:

Remove these 3 repo re-exports (lines 30, 32, 42):
```rust
pub use repos::AgentAdaptationRepo;
pub use repos::BehavioralPatternRepo;
pub use repos::UserProfileRepo;
```

Update the row re-export block (lines 62-66) from:
```rust
pub use rows::learning::{
    AgentAdaptationRow, BehavioralPatternRow, DecisionLogRow, EnrichmentFeedbackRow,
    InteractionLogRow, LearningStateRow, OutcomeRow, StrategyRecordRow, StrategySummaryRow,
    UserProfileRow,
};
```

To:
```rust
pub use rows::learning::{
    DecisionLogRow, EnrichmentFeedbackRow, InteractionLogRow, LearningStateRow, OutcomeRow,
    StrategyRecordRow, StrategySummaryRow,
};
```

- [ ] **Step 5: Verify storage crate compiles**

Run: `cargo build -p storage 2>&1 | head -30`
Expected: May show errors from other crates that reference these types — storage itself should compile.

- [ ] **Step 6: Commit**

```bash
git add -A crates/storage/src/repos/ crates/storage/src/rows/learning.rs crates/storage/src/lib.rs
git commit -m "refactor(storage): delete zombie L2 repos (user_profile, agent_adaptations, behavioral_patterns)"
```

---

### Task 7: Update AgentRuntime — Remove L2 Fields, Add ProceduralRuleRepo

**Files:**
- Modify: `crates/agent/src/agent_runtime/runtime.rs`

- [ ] **Step 1: Remove L2 learning fields from AgentRuntime struct**

In `crates/agent/src/agent_runtime/runtime.rs`, remove these fields from the `AgentRuntime` struct (lines 77-79):

```rust
    learning_user_profile: Option<storage::UserProfileRepo>,
    learning_patterns: Option<storage::BehavioralPatternRepo>,
    learning_adaptations: Option<storage::AgentAdaptationRepo>,
```

Replace with:

```rust
    /// Procedural rules repo for transparency (L5 cognitive rules).
    procedural_rule_repo: Option<cognitive::ProceduralRuleRepo>,
```

- [ ] **Step 2: Update constructor defaults**

In `AgentRuntime::new()` (lines 98-116), remove:

```rust
            learning_user_profile: None,
            learning_patterns: None,
            learning_adaptations: None,
```

Replace with:

```rust
            procedural_rule_repo: None,
```

- [ ] **Step 3: Replace `with_learning_repos` with `with_procedural_rule_repo`**

Remove the `with_learning_repos` method (lines 140-145):

```rust
    pub fn with_learning_repos(mut self, repos: &storage::Repos) -> Self {
        self.learning_user_profile = Some(repos.user_profile.clone());
        self.learning_patterns = Some(repos.behavioral_patterns.clone());
        self.learning_adaptations = Some(repos.agent_adaptations.clone());
        self
    }
```

Replace with:

```rust
    pub fn with_procedural_rule_repo(mut self, repo: cognitive::ProceduralRuleRepo) -> Self {
        self.procedural_rule_repo = Some(repo);
        self
    }
```

- [ ] **Step 4: Remove unused constants**

Remove these constants at the top of the file (lines 21-24):

```rust
/// Minimum confidence for user profile entries to appear in context.
const PROFILE_MIN_CONFIDENCE: f64 = 0.5;

/// Minimum sample count for behavioral patterns to appear in context.
const PATTERN_MIN_SAMPLES: i32 = 5;
```

- [ ] **Step 5: Rewrite `emit_learning_summary`**

Replace the entire `emit_learning_summary` method (lines 504-587) with:

```rust
    /// Emit learning context summary events for transparency panel.
    async fn emit_learning_summary(
        &self,
        tx: &tokio::sync::mpsc::Sender<AgentEvent>,
        _agent_name: &str,
    ) {
        // Learned procedural rules (from L5 cognitive pipeline)
        if let Some(ref rule_repo) = self.procedural_rule_repo {
            if let Ok(rules) = rule_repo.list_all_active().await {
                if !rules.is_empty() {
                    let previews: Vec<&str> =
                        rules.iter().take(3).map(|r| r.rule_text.as_str()).collect();
                    let _ = tx
                        .send(AgentEvent::LearningEvent {
                            event_type: "patterns".into(),
                            detail: format!(
                                "{} learned rules ({})",
                                rules.len(),
                                previews.join(", ")
                            ),
                        })
                        .await;
                }
            }
        }

        // Confidence threshold (in-memory read, no DB hit)
        if let Some(ref evaluator) = self.confidence_evaluator {
            let threshold = evaluator.threshold();
            let _ = tx
                .send(AgentEvent::LearningEvent {
                    event_type: "confidence".into(),
                    detail: format!("threshold: {:.0}%", threshold * 100.0),
                })
                .await;
        }
    }
```

- [ ] **Step 6: Add cognitive import**

At the top of `crates/agent/src/agent_runtime/runtime.rs`, verify `cognitive` is available. The `agent` crate already depends on `cognitive`. If there's no existing `use cognitive::...` import, the type path `cognitive::ProceduralRuleRepo` works directly since `cognitive` is a crate dependency.

- [ ] **Step 7: Verify the agent crate compiles**

Run: `cargo build -p agent 2>&1 | head -50`
Expected: Errors from builder.rs (still calls `with_learning_repos`) — we fix that next.

- [ ] **Step 8: Commit (partial — may not compile yet)**

```bash
git add crates/agent/src/agent_runtime/runtime.rs
git commit -m "refactor(agent): replace L2 learning repos with ProceduralRuleRepo in AgentRuntime"
```

---

### Task 8: Update Builder Wiring

**Files:**
- Modify: `crates/agent/src/agent_loop/builder.rs:1056-1057`

- [ ] **Step 1: Replace `with_learning_repos` call**

In `crates/agent/src/agent_loop/builder.rs`, find line 1057:

```rust
        runtime = runtime.with_learning_repos(&repos);
```

Replace with (`storage_pool` is always in scope — created unconditionally at line 168 of the builder):

```rust
        // Inject procedural rule repo for transparency (L5 cognitive rules)
        let rule_repo = cognitive::ProceduralRuleRepo::new(storage_pool.inner().clone());
        runtime = runtime.with_procedural_rule_repo(rule_repo);
```

- [ ] **Step 2: Verify full workspace compiles**

Run: `cargo build --workspace 2>&1 | tail -20`
Expected: Clean build (0 errors). Some warnings about unused imports may appear — fix any that relate to removed types.

- [ ] **Step 3: Fix any remaining compile errors**

Common issues:
- Other files that import `UserProfileRepo`, `BehavioralPatternRepo`, or `AgentAdaptationRepo` — search and remove
- Row types referenced elsewhere — search and remove

Run: `cargo build --workspace 2>&1 | grep "error\[" | head -20`

If errors found, fix them. If clean, proceed.

- [ ] **Step 4: Run all tests**

Run: `cargo nextest run --workspace 2>&1 | tail -20`
Expected: All tests pass. The old PatternAnalyzer tests that referenced `repos.behavioral_patterns` are gone (replaced in Task 4).

- [ ] **Step 5: Commit**

```bash
git add crates/agent/src/agent_loop/builder.rs
git commit -m "refactor(agent): wire ProceduralRuleRepo in builder, remove with_learning_repos"
```

---

### Task 9: Add Migration to Drop L2 Tables

**Files:**
- Create: `crates/storage/migrations/008_drop_legacy_learning.sql`

- [ ] **Step 1: Create the migration file**

Create `crates/storage/migrations/008_drop_legacy_learning.sql`:

```sql
-- Drop legacy L2 learning tables (migrated to L5 cognitive pipeline).
-- user_profile: superseded by L5 semantic_facts
-- agent_adaptations: unused, never written in production
-- behavioral_patterns: migrated to DomainEvent → cognitive pipeline
DROP TABLE IF EXISTS user_profile;
DROP TABLE IF EXISTS agent_adaptations;
DROP TABLE IF EXISTS behavioral_patterns;
```

- [ ] **Step 2: Verify migration runs**

Run: `cargo nextest run -p storage`
Expected: PASS — `StoragePool::connect_in_memory()` runs all migrations including the new one. The tables were created by `003_learning_system.sql` and now dropped by `008`.

- [ ] **Step 3: Run full workspace tests**

Run: `cargo nextest run --workspace`
Expected: All tests pass.

- [ ] **Step 4: Commit**

```bash
git add crates/storage/migrations/008_drop_legacy_learning.sql
git commit -m "feat(storage): add migration to drop legacy L2 learning tables"
```

---

### Task 10: Final Verification + Cleanup

- [ ] **Step 1: Run clippy**

Run: `cargo clippy --workspace --all-targets --all-features 2>&1 | tail -20`
Expected: 0 warnings. Fix any warnings from unused imports, dead code, etc.

- [ ] **Step 2: Run formatting check**

Run: `cargo fmt --all --check`
Expected: Clean. If not, run `cargo fmt --all` and commit.

- [ ] **Step 3: Run full test suite**

Run: `cargo nextest run --workspace`
Expected: All tests pass.

Run: `cargo test --workspace --doc`
Expected: All doctests pass.

- [ ] **Step 4: Verify no references to removed types remain**

Run these searches — all should return 0 results:

```bash
rg "UserProfileRepo" --type rust crates/
rg "AgentAdaptationRepo" --type rust crates/
rg "BehavioralPatternRepo" --type rust crates/
rg "UserProfileRow" --type rust crates/
rg "AgentAdaptationRow" --type rust crates/
rg "BehavioralPatternRow" --type rust crates/
rg "learning_user_profile" --type rust crates/
rg "learning_adaptations" --type rust crates/
rg "learning_patterns" --type rust crates/
rg "PROFILE_MIN_CONFIDENCE" --type rust crates/
rg "PATTERN_MIN_SAMPLES" --type rust crates/
rg "repos\.behavioral_patterns" --type rust crates/
rg "repos\.user_profile" --type rust crates/
rg "repos\.agent_adaptations" --type rust crates/
rg "with_learning_repos" --type rust crates/
```

If any matches found, clean them up and commit.

- [ ] **Step 5: Final commit (if any cleanup)**

```bash
git add -A
git commit -m "chore: cleanup remaining references to removed L2 learning types"
```
