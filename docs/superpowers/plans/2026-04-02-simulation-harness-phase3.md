# Simulation Harness Phase 3 — Fill All Tables, Wire All Data

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Make the simulation harness a complete replica of production data flow — every table that would have data after 9 months of real usage must have data in simulation. All 14 metrics must produce real non-zero values, including `autotuner_promotion_success` and `brain_version_velocity` (the last two at zero).

**Architecture:** Three categories of work: (1) Fix the autotuner pipeline end-to-end (migrate tables → seed trials → write shadow logs → enable promotions → brain versions), (2) Write actual DB rows for domain entities (tasks, finance transactions, usage records, interaction log, tool usage, strategy records), (3) Improve retrieval precision/recall by fixing the backfill timing. No new files — all changes are in existing harness code.

**Tech Stack:** Existing `storage::TrialRepo`, `storage::ExperimentRow`, `storage::TrialRow` types. Direct SQL INSERTs for domain tables where no repo is available from L4.

**Phase 2 baseline:** 12/14 metrics non-zero, 0 cron stubs, ~19 empty tables, autotuner shadow log empty.

**Phase 3 target:** 14/14 metrics non-zero, all domain tables populated, autotuner promotions flowing, brain versions tracked.

---

## File Structure (Phase 3)

```
crates/simulator/src/
├── harness.rs          # MODIFY — autotuner table migration, shadow log writes,
│                       #   domain entity INSERTs, retrieval backfill fix
├── actions.rs          # MODIFY — INSERT rows into tasks + finance_transactions tables
├── providers/
│   └── mod.rs          # MODIFY — re-export TrialRepo setup helpers if needed
└── (all other files unchanged)

tests/simulation/
├── smoke.rs                             # MODIFY — verify non-zero autotuner metrics
└── scenarios/
    └── software_engineer_12mo.toml      # MODIFY — add autotuner checkpoint assertions
```

---

### Task 1: Migrate autotuner tables and seed initial trials

**Files:**
- Modify: `crates/simulator/src/harness.rs`

The autotuner tables don't exist in the in-memory DB because they're created by `TrialRepo::migrate()`, not by the base migrations. Without them, every autotuner query silently fails.

- [ ] **Step 1: Read the TrialRepo migration API**

Read `crates/storage/src/repos/trial_repo.rs` to find:
- `TrialRepo::new(pool)` constructor
- `TrialRepo::migrate()` method
- `create_experiment(&ExperimentRow)` method
- `create_trial(&TrialRow)` method
- `ExperimentRow` and `TrialRow` struct field names

Also read `crates/common/src/autotuner.rs` for `TrialParams` — you need to know how to serialize it to JSON for the `params` column.

- [ ] **Step 2: Add autotuner migration to SimulationHarness::new()**

After the existing `run_feature_migrations` call (which creates cognitive tables), add:

```rust
// Migrate autotuner tables (experiments, trials, shadow logs)
let trial_repo = storage::TrialRepo::new(inner_pool.clone());
trial_repo.migrate().await?;
```

- [ ] **Step 3: Run feature-tasks migration**

The `tasks` table is in `crates/feature-tasks/migrations/001_create_tasks.sql`. Run it:

```rust
// Run feature-tasks migration for tasks table
storage::StoragePool::run_feature_migrations(
    &inner_pool,
    &feature_tasks::task_migrations(),  // check the actual function name
).await?;
```

Read `crates/feature-tasks/src/lib.rs` or `crates/feature-tasks/src/migrations.rs` to find how the feature migration is exposed. It may be `feature_tasks::migrations()` or similar. If not publicly exported, use raw SQL:

```rust
let tasks_ddl = include_str!("../../../feature-tasks/migrations/001_create_tasks.sql");
sqlx::raw_sql(tasks_ddl).execute(&inner_pool).await?;
```

Check if the `feature-tasks` crate is a dependency of `simulator`. If not, add it to `crates/simulator/Cargo.toml`, or use `include_str!` with a relative path, or inline the minimal CREATE TABLE.

- [ ] **Step 4: Seed initial autotuner experiment and trials**

After migration, seed one experiment with 2 active trials so the nightly cycle has something to evaluate:

```rust
// Seed autotuner experiment with 2 active trials
let experiment_id = uuid::Uuid::new_v4().to_string();
let trial_repo = storage::TrialRepo::new(inner_pool.clone());

trial_repo.create_experiment(&storage::ExperimentRow {
    id: experiment_id.clone(),
    hypothesis: "Simulation baseline experiment".to_string(),
    trend_analysis: "Initial simulation run".to_string(),
    recommendation_for_next: "Continue monitoring".to_string(),
    created_at: chrono::Utc::now().to_rfc3339(),
}).await?;

// Trial A: default params
let trial_a_id = uuid::Uuid::new_v4().to_string();
trial_repo.create_trial(&storage::TrialRow {
    id: trial_a_id.clone(),
    experiment_id: experiment_id.clone(),
    params: serde_json::to_string(&common::TrialParams::default()).unwrap(),
    generation_reasoning: "Baseline trial with default parameters".to_string(),
    status: "active".to_string(),
    created_at: chrono::Utc::now().to_rfc3339(),
    completed_at: None,
    result: None,
}).await?;

// Trial B: slightly modified params
let trial_b_id = uuid::Uuid::new_v4().to_string();
let mut params_b = common::TrialParams::default();
params_b.skill_keyword_weight = Some(0.8);
params_b.heuristic_confidence_threshold = Some(0.6);
trial_repo.create_trial(&storage::TrialRow {
    id: trial_b_id,
    experiment_id: experiment_id.clone(),
    params: serde_json::to_string(&params_b).unwrap(),
    generation_reasoning: "Variant trial with adjusted routing weights".to_string(),
    status: "active".to_string(),
    created_at: chrono::Utc::now().to_rfc3339(),
    completed_at: None,
    result: None,
}).await?;
```

**IMPORTANT:** Check the exact field names of `ExperimentRow` and `TrialRow` in `crates/storage/src/repos/trial_repo.rs`. They may use `&str` parameters instead of owned `String`. Also check if `TrialParams::default()` exists in `crates/common/src/autotuner.rs`.

Store `trial_a_id` on the harness struct so the message loop can reference it for shadow log writes:

```rust
// Add to SimulationHarness struct:
active_trial_id: String,
```

- [ ] **Step 5: Verify compilation**

Run: `cargo build -p simulator --lib`

- [ ] **Step 6: Commit**

```bash
git commit -m "feat(simulator): migrate autotuner tables and seed initial trials"
```

---

### Task 2: Write shadow log entries per message

**Files:**
- Modify: `crates/simulator/src/harness.rs`

The autotuner NightlyCycle requires `messages_scored >= 50` to evaluate a trial. `SimMetricSource` reads from `autotuner_shadow_log`. We need to write a row per message.

- [ ] **Step 1: Read insert_shadow_log signature**

Read `crates/storage/src/repos/trial_repo.rs` for `insert_shadow_log` — it takes 10 string/numeric parameters.

- [ ] **Step 2: Add shadow log write to the message loop**

In `harness.rs`, inside the per-message loop (after session message write, before cognitive pipeline), add:

```rust
// Write shadow log entry for the active trial
let trial_repo = storage::TrialRepo::new(self.inner_pool.clone());
let predicted_skill = crate::harness::expected_skill_for_topic(&msg.topic);
let _ = trial_repo.insert_shadow_log(
    &self.active_trial_id,
    &msg.simulated_at.to_rfc3339(),
    "sim-session",
    &uuid::Uuid::new_v4().to_string(),  // message_id
    predicted_skill,                      // predicted_orchestrator
    "reactive",                           // predicted_mode
    0.85,                                 // confidence
    10,                                   // predicted_iteration_budget
    predicted_skill,                      // control_orchestrator (same as predicted for sim)
    "reactive",                           // control_mode
).await;
```

Check the actual `insert_shadow_log` parameter names and order. The function may also need `user_corrected: bool` — check the actual signature.

**Optimization:** Don't construct `TrialRepo` on every message — construct it once in `run()` and reuse. Or store it on the harness struct.

- [ ] **Step 3: Add a topic-to-skill mapping helper**

If `expected_skill_for_topic` doesn't already exist (check `message_matches_topic_keywords`), add:

```rust
fn expected_skill_for_topic(topic: &str) -> &'static str {
    match topic {
        "tasks" => "task-management",
        "finance" => "finance-management",
        "automation" => "automation",
        "notes" | "learning" | "productivity" | "insights" | "chat" => "general",
        _ => "general",
    }
}
```

- [ ] **Step 4: Verify**

Run: `cargo build -p simulator --lib`

- [ ] **Step 5: Commit**

```bash
git commit -m "feat(simulator): write autotuner shadow log entries per message"
```

---

### Task 3: Write domain entity rows (tasks, finance, usage, interaction, tool_usage, strategy)

**Files:**
- Modify: `crates/simulator/src/actions.rs`
- Modify: `crates/simulator/src/harness.rs`

Currently `CreateTask` and `RecordTransaction` only publish bus events. Write actual DB rows too, plus add usage/interaction/tool_usage/strategy records per message.

- [ ] **Step 1: Insert task rows for CreateTask**

In `actions.rs`, in the `CreateTask` arm, after publishing the event and upserting the tree node, INSERT into the `tasks` table:

```rust
let _ = sqlx::query(
    "INSERT OR IGNORE INTO tasks (id, title, status, area_id, priority, created_at, updated_at, completed) \
     VALUES (?, ?, 'pending', NULL, 'medium', ?, ?, 0)"
)
.bind(&task_id)
.bind(title)
.bind(simulated_now.to_rfc3339())
.bind(simulated_now.to_rfc3339())
.execute(&self.pool)
.await;
```

For `CompleteTask`, UPDATE the task:

```rust
let _ = sqlx::query(
    "UPDATE tasks SET status = 'completed', completed = 1, completed_at = ?, updated_at = ? WHERE id = ? OR title = ?"
)
.bind(simulated_now.to_rfc3339())
.bind(simulated_now.to_rfc3339())
.bind(task_ref)
.bind(task_ref)
.execute(&self.pool)
.await;
```

Check the actual `tasks` table schema — it may require `status_label_id` or other NOT NULL columns. Use defaults or NULL for optional columns.

- [ ] **Step 2: Insert finance_transactions rows for RecordTransaction**

In the `RecordTransaction` arm:

```rust
let tx_id = uuid::Uuid::new_v4().to_string();
let _ = sqlx::query(
    "INSERT INTO finance_transactions (id, account_id, tx_type, amount, currency, category, notes, tx_date, created_at, updated_at) \
     VALUES (?, 'sim-account', 'expense', ?, 'VND', ?, ?, ?, ?, ?)"
)
.bind(&tx_id)
.bind(amount)
.bind(category)
.bind(description)
.bind(simulated_now.format("%Y-%m-%d").to_string())
.bind(simulated_now.to_rfc3339())
.bind(simulated_now.to_rfc3339())
.execute(&self.pool)
.await;
```

Check `finance_transactions` schema for required columns.

- [ ] **Step 3: Insert usage_records per message in harness**

In `harness.rs`, in the message loop, after the cognitive pipeline, insert a usage record:

```rust
let _ = sqlx::query(
    "INSERT INTO usage_records (id, timestamp, model, provider, prompt_tokens, completion_tokens, channel, strategy) \
     VALUES (?, ?, 'scripted-sim', 'simulator', 100, 50, 'simulation', 'reactive')"
)
.bind(uuid::Uuid::new_v4().to_string())
.bind(msg.simulated_at.to_rfc3339())
.execute(&self.inner_pool)
.await;
```

- [ ] **Step 4: Insert interaction_log per message**

```rust
let _ = sqlx::query(
    "INSERT INTO interaction_log (timestamp, agent_name, tool_names, channel, duration_ms) \
     VALUES (?, 'general', ?, 'simulation', 50)"
)
.bind(msg.simulated_at.to_rfc3339())
.bind(&msg.topic)
.execute(&self.inner_pool)
.await;
```

- [ ] **Step 5: Insert tool_usage for tool actions**

In the tool action loop, after executing each action:

```rust
let tool_name = match action {
    SimulatedToolAction::CreateTask { .. } => "tasks",
    SimulatedToolAction::CompleteTask { .. } => "tasks",
    SimulatedToolAction::CreateNote { .. } => "notes",
    SimulatedToolAction::UpdateNote { .. } => "notes",
    SimulatedToolAction::RecordTransaction { .. } => "finance",
    SimulatedToolAction::StartFocus { .. } => "productivity",
    SimulatedToolAction::CreateObjective { .. } => "okr",
    SimulatedToolAction::RecordProductivityEvent { .. } => "productivity",
};
let _ = sqlx::query(
    "INSERT INTO tool_usage (id, tool_name, action, session_key, channel, success, duration_ms, created_at) \
     VALUES (?, ?, 'execute', 'sim-session', 'simulation', 1, 10, ?)"
)
.bind(uuid::Uuid::new_v4().to_string())
.bind(tool_name)
.bind(msg.simulated_at.to_rfc3339())
.execute(&self.inner_pool)
.await;
```

- [ ] **Step 6: Insert strategy_records per message**

```rust
let _ = sqlx::query(
    "INSERT INTO strategy_records (id, timestamp, predicted_strategy, actual_strategy, success, execution_mode, chat_id) \
     VALUES (?, ?, 'reactive', 'reactive', 1, 'reactive', 'sim-session')"
)
.bind(uuid::Uuid::new_v4().to_string())
.bind(msg.simulated_at.to_rfc3339())
.execute(&self.inner_pool)
.await;
```

**IMPORTANT for all SQL above:** Read the actual table schemas in `crates/storage/migrations/001_initial.sql` and `crates/feature-tasks/migrations/001_create_tasks.sql`. Column names, types, and NOT NULL constraints must match exactly. Adjust the queries based on what you find.

- [ ] **Step 7: Verify**

Run: `cargo build -p simulator --lib`

- [ ] **Step 8: Commit**

```bash
git commit -m "feat(simulator): write domain entity rows (tasks, finance, usage, interaction, tool_usage, strategy)"
```

---

### Task 4: Fix retrieval precision/recall backfill timing

**Files:**
- Modify: `crates/simulator/src/harness.rs`

Currently the backfill annotates all messages for a day BEFORE any of them are processed. This means a fact extracted from message N on day D can only be a relevant fact for messages on day D+1, not for message N+1 on the same day.

- [ ] **Step 1: Move backfill into per-message processing**

Instead of annotating all messages at once before the loop, annotate each message just before it's processed:

```rust
// Remove the pre-loop backfill block (lines ~171-190)
// Instead, inside the per-message loop, before processing:

let relevant = persona_runner.relevant_facts_for_topic(&msg.topic);
if !relevant.is_empty() {
    if let Some(ref mut gt) = msg.ground_truth {
        gt.relevant_facts = relevant;
    } else {
        msg.ground_truth = Some(crate::persona::GroundTruthAnnotation {
            introduces_fact: None,
            relevant_facts: relevant,
            expected_skill: None,
        });
    }
}
// ... then process the message, run cognitive pipeline, record fact IDs ...
```

This way, message 3 on day 5 can have relevant_facts populated from messages 1-2 on the same day.

Note: The `messages` variable needs to be iterated mutably. Change from `for msg in &messages` to `for msg in &mut messages`.

- [ ] **Step 2: Verify retrieval count increases**

Run: `cargo test --test simulation run_software_engineer_12mo -- --nocapture`
Check: `retrieval_precision` should be non-zero in more than 14/269 epochs.

- [ ] **Step 3: Commit**

```bash
git commit -m "fix(simulator): move retrieval backfill to per-message for better precision/recall coverage"
```

---

### Task 5: Wire AutotunerNightly to use persisted trial_repo

**Files:**
- Modify: `crates/simulator/src/harness.rs`

Now that autotuner tables are migrated and shadow logs are written, update the AutotunerNightly cron to read the seeded trials.

- [ ] **Step 1: Store trial_repo on harness struct**

Add `trial_repo: storage::TrialRepo` to `SimulationHarness`. Initialize in `new()`:

```rust
let trial_repo = storage::TrialRepo::new(inner_pool.clone());
```

(Reuse the one created for migration.)

- [ ] **Step 2: Update AutotunerNightly cron to use stored trial_repo**

Replace the cron's inline `TrialRepo::new(...)` with `self.trial_repo.clone()` (or `&self.trial_repo` depending on the API).

- [ ] **Step 3: Set min_messages_for_promotion lower for simulation**

The default config requires 50 messages. After ~10 days of simulation (50+ messages), the trial should be evaluable. Create a custom `AutoTunerConfig` with a lower threshold if needed:

```rust
let mut config = config::AutoTunerConfig::default();
config.min_messages_for_promotion = 20; // Lower for simulation speed
```

Check if `AutoTunerConfig` fields are public. If not, the default of 50 should be fine since the simulation runs 269 days × ~5 messages = 1,345+ shadow log entries.

- [ ] **Step 4: Verify autotuner metrics**

Run: `cargo test --test simulation run_software_engineer_12mo -- --nocapture`
Check: `autotuner_promotion_success` should be non-zero after day 30+. `brain_version_velocity` should be non-zero when promotions happen (the `AutotunerDecision` bus event triggers `ConfigArchiver` which writes `mirror_brain_versions` — but `ConfigArchiver` is a mirror subscriber that only runs when `MirrorEngine::start()` is called, which the harness doesn't do).

If `brain_version_velocity` is still zero, add an explicit brain version insert when a promotion happens:

```rust
if let Some(ref promo) = result.promotion {
    // Write brain version directly
    let _ = sqlx::query(
        "INSERT INTO mirror_brain_versions (version, trial_id, promoted_at, params, reason, parent_version, reverted) \
         VALUES ((SELECT COALESCE(MAX(version),0)+1 FROM mirror_brain_versions), ?, ?, '{}', 'Simulation promotion', \
         (SELECT MAX(version) FROM mirror_brain_versions), 0)"
    )
    .bind(&promo.trial_id.to_string())
    .bind(simulated_now.to_rfc3339())
    .execute(&self.inner_pool)
    .await;
}
```

Check `mirror_brain_versions` schema in `crates/cognitive/migrations/003_mirror_tables.sql`.

- [ ] **Step 5: Commit**

```bash
git commit -m "feat(simulator): wire AutotunerNightly with seeded trials and brain version tracking"
```

---

### Task 6: Update scenarios and verify all 14 metrics

**Files:**
- Modify: `tests/simulation/scenarios/software_engineer_12mo.toml`
- Modify: `tests/simulation/smoke.rs`

- [ ] **Step 1: Run the full simulation and observe all 14 metrics**

Run: `cargo test --test simulation run_software_engineer_12mo -- --nocapture`

Observe the actual values for all 14 metrics. Every metric should now be non-zero in at least some epochs.

- [ ] **Step 2: Add autotuner checkpoint assertions**

Update `software_engineer_12mo.toml` to add autotuner checkpoints:

```toml
[[checkpoints]]
at_day = 90
assertions = [
    { type = "metric_above", metric = "knowledge_retention", threshold = 0.05 },
    { type = "metric_above", metric = "routing_stability", threshold = 0.3 },
    { type = "metric_above", metric = "community_stability", threshold = 0.01 },
]
```

Set thresholds to ~50% of observed values (conservative, won't flake in CI).

- [ ] **Step 3: Add a metric census to the 12mo test**

In `smoke.rs`, add a post-run check that verifies all 14 metrics were non-zero at least once:

```rust
let metrics_names = [
    "knowledge_retention", "retrieval_precision", "retrieval_recall",
    "fact_extraction_accuracy", "contradiction_detection_rate", "correction_rate",
    "token_efficiency", "personalization_score", "task_completion_rate",
    "routing_stability", "insight_usefulness", "autotuner_promotion_success",
    "community_stability", "brain_version_velocity"
];
// Check each metric was non-zero at least once in the timeline
// This is the Phase 3 gate: ALL 14 must fire
```

- [ ] **Step 4: Run all scenarios**

Run: `cargo test --test simulation -- --nocapture`
All 5 tests must pass.

- [ ] **Step 5: Commit**

```bash
git commit -m "feat(simulator): verify all 14 metrics non-zero, add autotuner checkpoints"
```

---

### Task 7: Final verification and cleanup

**Files:** All simulator files

- [ ] **Step 1: Run clippy**

Run: `cargo clippy -p simulator --all-targets`
Fix any warnings.

- [ ] **Step 2: Run formatting**

Run: `cargo fmt -p simulator --check`

- [ ] **Step 3: Run all simulator unit tests**

Run: `cargo nextest run -p simulator`

- [ ] **Step 4: Run all integration tests with output**

Run: `cargo test --test simulation -- --nocapture`

Print the full 14-metric census from the JSON report to confirm 14/14.

- [ ] **Step 5: Verify workspace builds**

Run: `cargo build --workspace`

- [ ] **Step 6: Commit any fixes**

```bash
git commit -m "fix(simulator): Phase 3 final cleanup"
```
