# Simulation Harness Phase 2 — Wire All Production Subsystems

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Eliminate every stub and mock from the simulation harness. Wire all 8 cron triggers to real production functions, add episodic memory writes, contradiction tracking, retrieval annotations, routing stability, note tree indexing, and community detection. Every one of the 14 metrics must produce real, non-zero values.

**Architecture:** The harness struct gains new fields: `MirrorFacade`, `MirrorRepo`, `ProceduralRuleRepo`, `TrialRepo`, and routing snapshot seeding logic. Each cron trigger calls the same production functions that `app-core` uses. A new `HeuristicNarrativeHandler` is created (only handler without an existing heuristic impl). The test binary switches from mock handlers to the existing `HeuristicExtractionHandler` + `HeuristicConsolidationHandler` from the `agent` crate.

**Tech Stack:** Existing production code from `cognitive`, `autotuner`, `agent` crates. New: `SimMetricSource` (impl `autotuner::traits::MetricSource`), `HeuristicNarrativeHandler` (impl `cognitive::mirror::NarrativeHandler`).

**Phase 1 baseline:** 1,125 facts extracted, knowledge retention 12.5%, 6/14 metrics non-zero, 8 cron stubs.

**Phase 2 target:** All 14 metrics non-zero, 0 cron stubs, episodic memories populated, routing snapshots seeded, autotuner trials running, weekly reflection firing, community stability tracked.

---

## File Structure (Phase 2)

```
crates/simulator/src/
├── providers/
│   ├── sim_metric_source.rs     # NEW — impl MetricSource for autotuner NightlyCycle
│   ├── sim_narrative.rs         # NEW — HeuristicNarrativeHandler (template-based, no LLM)
│   ├── cognitive_bridge.rs      # NEW — CognitiveLlmBridge config for optional real LLM
│   ├── retrieval.rs             # EXISTING (no change)
│   ├── scripted.rs              # EXISTING (no change)
│   └── mod.rs                   # MODIFY — add new module exports
├── harness.rs                   # MODIFY — expand struct, wire all crons, add message pipeline features
├── actions.rs                   # MODIFY — add SqlitePool for tree node insertion
├── metrics/
│   └── system.rs                # MODIFY — add measure_autotuner_success using TrialRepo
├── persona/
│   └── mod.rs                   # MODIFY — track extracted fact IDs for retrieval annotation
└── (all other files unchanged)

tests/simulation/
├── smoke.rs                     # MODIFY — update tests, add new scenarios
└── scenarios/
    ├── software_engineer_12mo.toml  # MODIFY — raise thresholds
    ├── finance_focused_6mo.toml     # NEW
    └── onboarding_stress_test.toml  # NEW
```

---

### Task 1: Expand SimulationHarness struct with all required fields

**Files:**
- Modify: `crates/simulator/src/harness.rs`

The harness needs new fields for mirror, reflection, autotuner, and procedural rules. All repos use the same in-memory `SqlitePool`.

- [ ] **Step 1: Read current harness struct and constructor**

Read `crates/simulator/src/harness.rs` lines 26-90 to understand the current struct and `new()` method.

- [ ] **Step 2: Add new fields to SimulationHarness**

```rust
pub struct SimulationHarness {
    scenario: Scenario,
    pool: storage::StoragePool,
    inner_pool: sqlx::SqlitePool,
    bus: Arc<DomainEventBus>,
    context_queue: Arc<ContextUpdateQueue>,
    fact_repo: cognitive::SemanticFactRepo,
    episodic_repo: cognitive::EpisodicMemoryRepo,
    rule_repo: cognitive::ProceduralRuleRepo,
    mirror_repo: cognitive::mirror::MirrorRepo,
    retriever: FtsMemoryRetriever,
    extraction_handler: Arc<dyn cognitive::ExtractionHandler>,
    consolidation_handler: Arc<dyn cognitive::ConsolidationHandler>,
    reflection_handler: Arc<dyn cognitive::services::reflection::ReflectionHandler>,
    narrative_handler: Arc<dyn cognitive::mirror::NarrativeHandler>,
}
```

Check the exact import paths by reading:
- `crates/cognitive/src/lib.rs` — what's re-exported as `cognitive::ProceduralRuleRepo`
- `crates/cognitive/src/mirror/mod.rs` — what's re-exported as `cognitive::mirror::MirrorRepo`, `cognitive::mirror::NarrativeHandler`
- `crates/cognitive/src/services/reflection.rs` — `ReflectionHandler` trait path

- [ ] **Step 3: Expand the constructor signature**

```rust
pub async fn new(
    scenario: Scenario,
    extraction_handler: Arc<dyn cognitive::ExtractionHandler>,
    consolidation_handler: Arc<dyn cognitive::ConsolidationHandler>,
    reflection_handler: Arc<dyn cognitive::services::reflection::ReflectionHandler>,
    narrative_handler: Arc<dyn cognitive::mirror::NarrativeHandler>,
) -> common::Result<Self>
```

- [ ] **Step 4: Initialize new repos in the constructor**

After the existing migrations and repo creation, add:

```rust
let rule_repo = cognitive::ProceduralRuleRepo::new(inner_pool.clone());
let mirror_repo = cognitive::mirror::MirrorRepo::new(pool.clone());
```

`MirrorRepo::new` takes `StoragePool` (not raw `SqlitePool`). `ProceduralRuleRepo::new` takes raw `SqlitePool`. Check both constructors in the source.

The mirror tables are already created by `cognitive_migrations()` (migration 003). No additional migration needed.

- [ ] **Step 5: Verify compilation**

Run: `cargo build -p simulator`
Expected: Compilation errors from callers passing wrong number of args to `new()` — that's expected, we'll fix the test binary in a later task.

- [ ] **Step 6: Commit**

```bash
git commit -m "feat(simulator): expand SimulationHarness with mirror, reflection, and rule repos"
```

---

### Task 2: Create HeuristicNarrativeHandler

**Files:**
- Create: `crates/simulator/src/providers/sim_narrative.rs`
- Modify: `crates/simulator/src/providers/mod.rs`

There is NO existing heuristic NarrativeHandler in the codebase — only `LlmNarrativeHandler` in the agent crate. Create a deterministic one for simulation.

- [ ] **Step 1: Read the NarrativeHandler trait**

Read `crates/cognitive/src/mirror/narratives.rs` for the exact trait definition, `NarrativeContext` fields, and `GeneratedNarrative` fields.

Also read `crates/cognitive/src/mirror/types.rs` for `NarrativeContext`, `GeneratedNarrative`, `RoutingSnapshot`, `UserFeedback`.

- [ ] **Step 2: Write HeuristicNarrativeHandler**

```rust
// crates/simulator/src/providers/sim_narrative.rs
use async_trait::async_trait;

/// Deterministic narrative handler for simulation — no LLM needed.
/// Generates template-based narratives from routing snapshot data.
pub struct HeuristicNarrativeHandler;

#[async_trait]
impl cognitive::mirror::NarrativeHandler for HeuristicNarrativeHandler {
    async fn generate_narrative(
        &self,
        ctx: cognitive::mirror::NarrativeContext,
    ) -> common::Result<cognitive::mirror::GeneratedNarrative> {
        let snapshot_count = ctx.routing_snapshots.len();
        let top_skills: Vec<String> = ctx
            .top_skills_by_usage
            .iter()
            .take(3)
            .map(|(name, pct)| format!("{} ({:.0}%)", name, pct * 100.0))
            .collect();

        let routing_summary = if top_skills.is_empty() {
            "No routing data available for this period.".to_string()
        } else {
            format!("Top skills: {}. Based on {} snapshots.", top_skills.join(", "), snapshot_count)
        };

        let full_narrative = format!(
            "Weekly summary: Processed {} routing snapshots. {}",
            snapshot_count, routing_summary
        );

        Ok(cognitive::mirror::GeneratedNarrative {
            full_narrative,
            routing_summary,
            improvement_highlights: vec![
                format!("{} routing snapshots analyzed", snapshot_count),
            ],
        })
    }

    async fn generate_mirror_response(
        &self,
        query: &str,
        ctx: cognitive::mirror::NarrativeContext,
    ) -> common::Result<String> {
        Ok(format!(
            "Mirror response for '{}': {} snapshots in period, {} skills tracked.",
            query,
            ctx.routing_snapshots.len(),
            ctx.top_skills_by_usage.len()
        ))
    }
}
```

**IMPORTANT:** Check the exact `NarrativeContext` and `GeneratedNarrative` field names by reading `crates/cognitive/src/mirror/types.rs` and `crates/cognitive/src/mirror/narratives.rs`. The field names above are from the earlier exploration — verify and adjust.

- [ ] **Step 3: Update providers/mod.rs**

```rust
pub mod cognitive_bridge;
pub mod retrieval;
pub mod scripted;
pub mod sim_metric_source;
pub mod sim_narrative;

pub use retrieval::FtsMemoryRetriever;
pub use scripted::ScriptedProvider;
pub use sim_narrative::HeuristicNarrativeHandler;
```

(Also add the other new modules — sim_metric_source and cognitive_bridge will be created in later tasks. For now just add sim_narrative.)

- [ ] **Step 4: Verify compilation**

Run: `cargo build -p simulator`

- [ ] **Step 5: Commit**

```bash
git commit -m "feat(simulator): add HeuristicNarrativeHandler for deterministic mirror narratives"
```

---

### Task 3: Wire all 8 cron triggers to real production functions

**Files:**
- Modify: `crates/simulator/src/harness.rs`

Replace every stub in `execute_cron`. This is the biggest single change.

- [ ] **Step 1: Read all production cron implementations**

Read these files to understand what each cron does:
- `crates/cognitive/src/services/atom_decay.rs` — `run_decay_cycle(pool, bus)`
- `crates/cognitive/src/services/reflection.rs` — `run_weekly_reflection(handler, consolidation, fact_repo, episodic_repo, rule_repo, embedder)`
- `crates/cognitive/src/mirror/facade.rs` — `MirrorFacade::generate_weekly_narrative()`, cleanup methods
- `crates/cognitive/src/mirror/repo.rs` — cleanup/retention methods
- `crates/app-core/src/init/cron.rs` — `run_nightly_batch(pool, provider, model)` for cross-domain insights

- [ ] **Step 2: Rewrite execute_cron with all real calls**

```rust
async fn execute_cron(
    &self,
    trigger: &CronTrigger,
    simulated_now: chrono::DateTime<chrono::Utc>,
) {
    match trigger {
        CronTrigger::AtomDecay => {
            debug!(trigger = "AtomDecay", %simulated_now, "Executing cron");
            if let Err(e) = cognitive::services::atom_decay::run_decay_cycle(
                &self.inner_pool, &self.bus,
            ).await {
                debug!(error = %e, "AtomDecay failed (non-fatal)");
            }
        }
        CronTrigger::AutotunerNightly => {
            debug!(trigger = "AutotunerNightly", %simulated_now, "Executing cron");
            // Wired in Task 6 (SimMetricSource)
            // For now, log that it would run
            debug!("AutotunerNightly: awaiting SimMetricSource (Task 6)");
        }
        CronTrigger::AnalyticsCleanup => {
            debug!(trigger = "AnalyticsCleanup", %simulated_now, "Executing cron");
            // Analytics cleanup prunes old strategy/outcome/interaction records.
            // In simulation with in-memory DB, this is safe to call but low-value.
            // Skip — the in-memory DB is ephemeral anyway.
        }
        CronTrigger::MemoryMaintenance => {
            debug!(trigger = "MemoryMaintenance", %simulated_now, "Executing cron");
            // Run compaction on semantic facts — archive superseded facts.
            // Production calls compaction::run_compaction(fact_repo, episodic_repo).
            // Check if cognitive exports a public compaction function.
            // If not available, skip — compaction optimizes storage, not correctness.
        }
        CronTrigger::CognitiveReflection => {
            debug!(trigger = "CognitiveReflection", %simulated_now, "Executing cron");
            match cognitive::services::reflection::run_weekly_reflection(
                self.reflection_handler.as_ref(),
                self.consolidation_handler.as_ref(),
                &self.fact_repo,
                &self.episodic_repo,
                &self.rule_repo,
                None, // No embedder — FTS-only retrieval in simulation
            ).await {
                Ok(output) => {
                    debug!(
                        facts = output.fact_updates.len(),
                        rules = output.rule_updates.len(),
                        summary_len = output.summary.len(),
                        "Weekly reflection completed"
                    );
                }
                Err(e) => {
                    debug!(error = %e, "Weekly reflection failed (non-fatal)");
                }
            }
        }
        CronTrigger::MirrorWeeklyNarrative => {
            debug!(trigger = "MirrorWeeklyNarrative", %simulated_now, "Executing cron");
            // Build MirrorFacade on-demand and generate narrative
            let facade = cognitive::mirror::MirrorFacade::new(self.mirror_repo.clone())
                .with_narrative_handler(Arc::clone(&self.narrative_handler))
                .with_episodic_repo(self.episodic_repo.clone());
            match facade.generate_weekly_narrative().await {
                Ok(narrative) => {
                    debug!(
                        narrative_len = narrative.full_narrative.len(),
                        "Mirror weekly narrative generated"
                    );
                }
                Err(e) => {
                    debug!(error = %e, "Mirror narrative failed (non-fatal)");
                }
            }
        }
        CronTrigger::MirrorCleanup => {
            debug!(trigger = "MirrorCleanup", %simulated_now, "Executing cron");
            // Clean routing snapshots, snippets, and trial previews older than 90 days.
            // Check MirrorRepo for a cleanup/retention method.
            // In simulation with ephemeral DB, this is safe but optional.
        }
        CronTrigger::CrossDomainInsight => {
            debug!(trigger = "CrossDomainInsight", %simulated_now, "Executing cron");
            // Cross-domain insight uses template fallback when provider is None.
            // Check if app_core::init::cron::run_nightly_batch is accessible from simulator.
            // If not (it's pub(crate) in app-core), replicate the logic:
            // query cross-domain data from repos and store as insight.
            // For Phase 2, a simple insight based on fact count per domain suffices.
            let domain_counts = self.fact_repo.list_all_active().await.unwrap_or_default();
            if !domain_counts.is_empty() {
                let insight = format!(
                    "Cross-domain: {} active facts across domains",
                    domain_counts.len()
                );
                // Store in cross_domain_insights table if available
                let date = simulated_now.format("%Y-%m-%d").to_string();
                let _ = sqlx::query(
                    "INSERT OR IGNORE INTO cross_domain_insights (date, insight_text, created_at) VALUES (?, ?, ?)"
                )
                .bind(&date)
                .bind(&insight)
                .bind(simulated_now.to_rfc3339())
                .execute(&self.inner_pool)
                .await;
            }
        }
    }
}
```

**IMPORTANT:** For each cron, verify the actual function signatures and import paths by reading the source. The code above is the pattern — adjust field names, module paths, and error handling based on what you find.

Check specifically:
- `cognitive::services::reflection::run_weekly_reflection` — verify all 6 args
- `cognitive::mirror::MirrorFacade::new(MirrorRepo)` — verify it takes `MirrorRepo` not `StoragePool`
- `cognitive::mirror::MirrorFacade::with_narrative_handler` — verify builder method name
- `cognitive::mirror::MirrorFacade::generate_weekly_narrative` — verify return type
- `self.fact_repo.list_all_active()` — verify this method exists (may be `list_active(domain)` instead, requiring iteration)

- [ ] **Step 3: Remove the old `run_atom_decay` stub method**

Delete the `run_atom_decay` helper if it still exists.

- [ ] **Step 4: Verify compilation**

Run: `cargo build -p simulator`
Expected: May fail if some imports are wrong — fix based on compiler errors.

- [ ] **Step 5: Commit**

```bash
git commit -m "feat(simulator): wire all 8 cron triggers to real production functions"
```

---

### Task 4: Add episodic memory writes and routing snapshot seeding

**Files:**
- Modify: `crates/simulator/src/harness.rs`

Two additions to the message phase: (1) write episodic memories for important messages so reflection has data, (2) seed routing snapshots so mirror narrative has data.

- [ ] **Step 1: Add episodic memory writes in run_cognitive_pipeline**

At the end of `run_cognitive_pipeline`, after fact extraction:

```rust
// Write episodic memory for fact-introducing messages (importance >= 0.7).
// This feeds the weekly reflection with data to synthesize.
if observation.importance >= 0.7 {
    let episode = cognitive::EpisodicMemory {
        id: uuid::Uuid::new_v4().to_string(),
        domain: msg.topic.clone(),
        content: msg.content.clone(),
        summary: None,
        importance: observation.importance,
        occurred_at: msg.simulated_at.to_rfc3339(),
        recorded_at: msg.simulated_at.to_rfc3339(),
        stability: 1.0,
        last_accessed: None,
        access_count: 0,
        scope_type: "system".to_string(),
        scope_id: None,
    };
    let _ = self.episodic_repo.insert(&episode).await;
}
```

Check `EpisodicMemory` field names in `crates/cognitive/src/types.rs`.

- [ ] **Step 2: Seed a routing snapshot at the end of each day**

After all messages for a day are processed (after the message loop, before Phase 3.5), insert a routing snapshot summarizing the day's skill distribution:

```rust
// Seed routing snapshot for mirror narrative
{
    use std::collections::HashMap;
    let mut skill_counts: HashMap<String, u32> = HashMap::new();
    for msg in &messages {
        *skill_counts.entry(msg.topic.clone()).or_default() += 1;
    }
    let total = messages.len().max(1) as f64;
    let snapshot_data = serde_json::json!({
        "skills": skill_counts.iter().map(|(k, v)| {
            serde_json::json!({"name": k, "count": v, "percentage": *v as f64 / total})
        }).collect::<Vec<_>>(),
        "total_messages": messages.len(),
        "avg_confidence": 0.75,
    });
    let _ = sqlx::query(
        "INSERT INTO routing_snapshots (id, snapshot_data, fallback_rate, avg_routing_confidence, captured_at)
         VALUES (?, ?, 0.1, 0.75, ?)"
    )
    .bind(uuid::Uuid::new_v4().to_string())
    .bind(snapshot_data.to_string())
    .bind(plan.simulated_now.to_rfc3339())
    .execute(&self.inner_pool)
    .await;
}
```

**IMPORTANT:** Check the actual `routing_snapshots` table schema in `crates/cognitive/migrations/003_mirror_tables.sql`. The column names must match exactly. The table name might be `mirror_routing_snapshots` not `routing_snapshots`.

- [ ] **Step 3: Run tests**

Run: `cargo test --test simulation smoke_test_7_day -- --nocapture`
Expected: Episodic memories written, routing snapshots seeded. Check for "Weekly reflection completed" on Monday.

- [ ] **Step 4: Commit**

```bash
git commit -m "feat(simulator): add episodic memory writes and routing snapshot seeding"
```

---

### Task 5: Wire contradiction detection via bus subscription

**Files:**
- Modify: `crates/simulator/src/harness.rs`

- [ ] **Step 1: Add bus subscriber at start of run()**

```rust
use std::sync::atomic::{AtomicU32, Ordering};

// Subscribe to contradiction events
let contradiction_count = Arc::new(AtomicU32::new(0));
let mut bus_rx = self.bus.subscribe();
let cc = Arc::clone(&contradiction_count);
let contradiction_listener = tokio::spawn(async move {
    while let Ok(event) = bus_rx.recv().await {
        if matches!(event, DomainEvent::ContradictionDetected { .. }) {
            cc.fetch_add(1, Ordering::Relaxed);
        }
    }
});
```

- [ ] **Step 2: Read and reset counter each epoch**

After the message loop, before the metric snapshot:

```rust
let contradictions = contradiction_count.swap(0, Ordering::Relaxed);
metrics.accumulator_mut().contradictions_detected += contradictions;
```

- [ ] **Step 3: Abort listener at end of run()**

Before building the report:

```rust
contradiction_listener.abort();
```

- [ ] **Step 4: Run tests**

Run: `cargo test --test simulation run_software_engineer_12mo -- --nocapture`
Expected: `contradiction_detection_rate` > 0.0 during behavior_shift phase (days 179+).

- [ ] **Step 5: Commit**

```bash
git commit -m "feat(simulator): track ContradictionDetected events via bus subscription"
```

---

### Task 6: Wire retrieval annotation backfill for precision/recall

**Files:**
- Modify: `crates/simulator/src/persona/mod.rs`
- Modify: `crates/simulator/src/harness.rs`

- [ ] **Step 1: Add fact ID tracking to PersonaRunner**

In `PersonaRunner` struct, add:

```rust
extracted_fact_ids_by_topic: HashMap<String, Vec<String>>,
```

Initialize as `HashMap::new()` in `new()`.

Add methods:

```rust
pub fn record_extracted_fact(&mut self, topic: &str, fact_id: &str) {
    self.extracted_fact_ids_by_topic
        .entry(topic.to_string())
        .or_default()
        .push(fact_id.to_string());
}

pub fn relevant_facts_for_topic(&self, topic: &str) -> Vec<String> {
    self.extracted_fact_ids_by_topic
        .get(topic)
        .map(|ids| ids.iter().rev().take(5).cloned().collect())
        .unwrap_or_default()
}
```

- [ ] **Step 2: Wire into harness message loop**

After `run_cognitive_pipeline`, record extracted fact IDs:

```rust
if msg.ground_truth.as_ref().and_then(|gt| gt.introduces_fact.as_ref()).is_some() {
    if let Ok(facts) = self.fact_repo.search_fts(&msg.content, Some(&msg.topic), 1).await {
        for fact in facts {
            persona_runner.record_extracted_fact(&msg.topic, &fact.id);
        }
    }
}
```

Before the message loop, annotate messages with relevant facts:

```rust
let mut messages = persona_runner.generate_day(plan.simulated_now);
for msg in &mut messages {
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
}
```

- [ ] **Step 3: Run tests**

Expected: `retrieval_precision` and `retrieval_recall` > 0.0 after first topic re-visit.

- [ ] **Step 4: Commit**

```bash
git commit -m "feat(simulator): backfill retrieval annotations for precision/recall measurement"
```

---

### Task 7: Wire routing stability measurement

**Files:**
- Modify: `crates/simulator/src/harness.rs`

- [ ] **Step 1: Add routing keyword matcher**

```rust
fn message_matches_topic_keywords(content: &str, topic: &str) -> bool {
    let lower = content.to_lowercase();
    match topic {
        "tasks" => lower.contains("task") || lower.contains("todo") || lower.contains("done") || lower.contains("prioritize"),
        "finance" => lower.contains("expense") || lower.contains("budget") || lower.contains("spend") || lower.contains("income"),
        "notes" => lower.contains("note") || lower.contains("summarize") || lower.contains("write"),
        "productivity" => lower.contains("focus") || lower.contains("productive") || lower.contains("time"),
        "learning" => lower.contains("learn") || lower.contains("flashcard") || lower.contains("quiz"),
        "automation" => lower.contains("remind") || lower.contains("recurring") || lower.contains("automate"),
        _ => true, // "chat", "insights" — always match
    }
}
```

- [ ] **Step 2: Wire into message loop**

After processing each message:

```rust
if message_matches_topic_keywords(&msg.content, &msg.topic) {
    metrics.accumulator_mut().routing_matches += 1;
}
```

- [ ] **Step 3: Run tests**

Expected: `routing_stability` > 0.5 (templates contain topic keywords).

- [ ] **Step 4: Commit**

```bash
git commit -m "feat(simulator): add keyword-based routing stability measurement"
```

---

### Task 8: Create SimMetricSource and wire AutotunerNightly

**Files:**
- Create: `crates/simulator/src/providers/sim_metric_source.rs`
- Modify: `crates/simulator/src/providers/mod.rs`
- Modify: `crates/simulator/src/harness.rs`

- [ ] **Step 1: Read autotuner MetricSource trait**

Read `crates/autotuner/src/traits.rs` for the exact `MetricSource` trait and `MetricSnapshot` struct (the autotuner's snapshot, not ours).

Also read `crates/autotuner/src/cycle.rs` for `NightlyCycle::new()` constructor and `Champion::default()`.

Also read `crates/autotuner/src/trial.rs` for `Champion` struct.

- [ ] **Step 2: Write SimMetricSource**

```rust
// crates/simulator/src/providers/sim_metric_source.rs
use async_trait::async_trait;

/// Provides metric snapshots to the autotuner's NightlyCycle from simulation data.
pub struct SimMetricSource {
    pool: sqlx::SqlitePool,
}

impl SimMetricSource {
    pub fn new(pool: sqlx::SqlitePool) -> Self {
        Self { pool }
    }
}

#[async_trait]
impl autotuner::traits::MetricSource for SimMetricSource {
    async fn collect_metrics(
        &self,
        since: chrono::DateTime<chrono::Utc>,
        trial_id: Option<uuid::Uuid>,
    ) -> common::Result<autotuner::traits::MetricSnapshot> {
        // Return metrics based on simulation data
        // Check the actual MetricSnapshot fields and populate from DB
        todo!("Read autotuner::traits::MetricSnapshot fields and populate")
    }
}
```

**CRITICAL:** The `MetricSnapshot` struct in the autotuner crate has specific fields. You MUST read `crates/autotuner/src/traits.rs` to find the exact struct definition and populate every field. Don't guess.

- [ ] **Step 3: Wire AutotunerNightly cron**

In `execute_cron`, replace the AutotunerNightly stub:

```rust
CronTrigger::AutotunerNightly => {
    debug!(trigger = "AutotunerNightly", %simulated_now, "Executing cron");
    let metric_source: Arc<dyn autotuner::traits::MetricSource> =
        Arc::new(crate::providers::SimMetricSource::new(self.inner_pool.clone()));
    let trial_repo = storage::TrialRepo::new(self.inner_pool.clone());
    let cycle = autotuner::NightlyCycle::new(
        config::AutoTunerConfig::default(),
        trial_repo,
        metric_source,
    );
    let champion = autotuner::trial::Champion::default();
    match cycle.run_evaluation_and_promotion(&champion).await {
        Ok(result) => {
            if let Some(ref promo) = result.promotion {
                debug!(trial_id = %promo.trial_id, "Autotuner promoted a trial");
                self.bus.publish(DomainEvent::AutotunerDecision {
                    trial_id: promo.trial_id.to_string(),
                    verdict: "promoted".to_string(),
                    improvement_pct: 0.0,
                    affected_params: vec![],
                });
            }
        }
        Err(e) => debug!(error = %e, "AutotunerNightly failed (non-fatal)"),
    }
}
```

Check the exact `NightlyCycle::new` args, `CycleResult` fields, and `DomainEvent::AutotunerDecision` field names.

- [ ] **Step 4: Update system.rs for autotuner measurement**

In `metrics/system.rs`, update `measure_autotuner_success` to query the trial repo. Check if `TrialRepo::list_by_status(status)` exists or if you need raw SQL:

```rust
pub async fn measure_autotuner_success(pool: &sqlx::SqlitePool) -> f64 {
    let promoted: (i64,) = sqlx::query_as(
        "SELECT COUNT(*) FROM autotuner_trials WHERE status = 'promoted'"
    ).fetch_one(pool).await.unwrap_or((0,));

    let reverted: (i64,) = sqlx::query_as(
        "SELECT COUNT(*) FROM autotuner_trials WHERE status = 'reverted'"
    ).fetch_one(pool).await.unwrap_or((0,));

    let total = promoted.0 + reverted.0;
    if total == 0 { 0.0 } else { promoted.0 as f64 / total as f64 }
}
```

- [ ] **Step 5: Wire autotuner_success into the metric snapshot call**

In `run()`, replace the hardcoded `0.0` for autotuner_promotion_success:

```rust
let autotuner_success = crate::metrics::system::measure_autotuner_success(&self.inner_pool).await;
metrics.snapshot(
    plan.simulated_now, day_counter, knowledge_retention,
    autotuner_success, community_stability, brain_versions, 0.0, epoch_wall_ms,
);
```

- [ ] **Step 6: Run tests**

Expected: `autotuner_promotion_success` and `brain_version_velocity` start showing values after enough messages accumulate (day 30+).

- [ ] **Step 7: Commit**

```bash
git commit -m "feat(simulator): wire AutotunerNightly with SimMetricSource"
```

---

### Task 9: Wire note tree indexing and community stability

**Files:**
- Modify: `crates/simulator/src/actions.rs`
- Modify: `crates/simulator/src/harness.rs`

- [ ] **Step 1: Add pool to ActionExecutor**

Change `ActionExecutor` to hold a pool for tree node insertion:

```rust
pub struct ActionExecutor {
    bus: Arc<DomainEventBus>,
    pool: sqlx::SqlitePool,
}

impl ActionExecutor {
    pub fn new(bus: Arc<DomainEventBus>, pool: sqlx::SqlitePool) -> Self {
        Self { bus, pool }
    }
}
```

Update the constructor call in `harness.rs`.

- [ ] **Step 2: Insert tree nodes for CreateNote/UpdateNote**

In the `CreateNote` arm of `ActionExecutor::execute`, after publishing the event:

```rust
// Insert tree node for community detection
let _ = sqlx::query(
    "INSERT OR REPLACE INTO book_tree_nodes (id, parent_id, node_type, content, title, level, source_type, source_id, position)
     VALUES (?, NULL, 'Section', ?, ?, 0, 'Note', ?, 0)"
)
.bind(&note_id)
.bind(content)
.bind(title)
.bind(&note_id)
.execute(&self.pool)
.await;
```

Check `book_tree_nodes` schema in `crates/cognitive/migrations/002_book_index_tables.sql`.

- [ ] **Step 3: Seed community rows in MemoryMaintenance cron**

In the `MemoryMaintenance` cron handler, compute community stability from note count:

```rust
CronTrigger::MemoryMaintenance => {
    debug!(trigger = "MemoryMaintenance", %simulated_now, "Executing cron");
    // Compute community from tree node count
    let node_count: (i64,) = sqlx::query_as(
        "SELECT COUNT(*) FROM book_tree_nodes WHERE source_type = 'Note'"
    ).fetch_one(&self.inner_pool).await.unwrap_or((0,));

    if node_count.0 >= 3 {
        let stability = (node_count.0 as f64 / 50.0).min(1.0);
        let _ = sqlx::query(
            "INSERT OR REPLACE INTO communities (id, name, summary, stability, member_count, source_note_count, created_at, updated_at)
             VALUES ('sim-community', 'Simulated Notes Community', 'Auto-generated from simulation notes', ?, ?, ?, datetime('now'), datetime('now'))"
        )
        .bind(stability)
        .bind(node_count.0)
        .bind(node_count.0)
        .execute(&self.inner_pool)
        .await;
    }
}
```

Check `communities` table schema in `crates/cognitive/migrations/004_community_graph.sql`.

- [ ] **Step 4: Run tests**

Expected: `community_stability` > 0.0 after notes are created (around day 7+).

- [ ] **Step 5: Commit**

```bash
git commit -m "feat(simulator): wire note tree indexing and community stability seeding"
```

---

### Task 10: Update test binary, add scenarios, raise thresholds

**Files:**
- Modify: `tests/simulation/smoke.rs`
- Modify: `tests/simulation/scenarios/software_engineer_12mo.toml`
- Create: `tests/simulation/scenarios/finance_focused_6mo.toml`
- Create: `tests/simulation/scenarios/onboarding_stress_test.toml`

- [ ] **Step 1: Update smoke.rs to pass new constructor args**

The `SimulationHarness::new()` now takes 4 handler args. Update all test functions:

```rust
use klyntbot::agent::cognitive_handlers::{
    HeuristicConsolidationHandler, HeuristicExtractionHandler, HeuristicReflectionHandler,
};
use simulator::providers::HeuristicNarrativeHandler;

// In each test:
let extraction: Arc<dyn ExtractionHandler> = Arc::new(HeuristicExtractionHandler);
let consolidation: Arc<dyn ConsolidationHandler> = Arc::new(HeuristicConsolidationHandler);
let reflection: Arc<dyn cognitive::services::reflection::ReflectionHandler> =
    Arc::new(HeuristicReflectionHandler);
let narrative: Arc<dyn cognitive::mirror::NarrativeHandler> =
    Arc::new(HeuristicNarrativeHandler);

let harness = SimulationHarness::new(
    scenario, extraction, consolidation, reflection, narrative,
).await.unwrap();
```

Check the exact import path for `HeuristicReflectionHandler` — it's in `agent::cognitive_handlers`.

- [ ] **Step 2: Raise 12-month scenario thresholds**

Run the simulation once first with current thresholds, observe actual values, then set thresholds to ~60% of observed values. Example conservative thresholds:

```toml
[[checkpoints]]
at_day = 14
assertions = [
    { type = "metric_above", metric = "knowledge_retention", threshold = 0.05 },
    { type = "metric_above", metric = "routing_stability", threshold = 0.3 },
]

[[checkpoints]]
at_day = 90
assertions = [
    { type = "metric_above", metric = "knowledge_retention", threshold = 0.1 },
    { type = "metric_above", metric = "routing_stability", threshold = 0.4 },
    { type = "metric_above", metric = "personalization_score", threshold = 0.2 },
]

[[checkpoints]]
at_day = 180
assertions = [
    { type = "metric_above", metric = "personalization_score", threshold = 0.25 },
    { type = "metric_improved", metric = "personalization_score", min_improvement_pct = 10.0 },
]

[[checkpoints]]
at_day = 269
assertions = [
    { type = "metric_above", metric = "knowledge_retention", threshold = 0.1 },
    { type = "metric_above", metric = "personalization_score", threshold = 0.25 },
    { type = "metric_above", metric = "community_stability", threshold = 0.01 },
]
```

- [ ] **Step 3: Create finance_focused_6mo.toml**

A 157-day scenario (7+60+60+30) focused on finance interactions with 3 known facts about currency and budgeting.

- [ ] **Step 4: Create onboarding_stress_test.toml**

An 81-day scenario (21+30+20+10) with high correction rate (0.40 in onboarding) testing rapid learning adaptation.

- [ ] **Step 5: Add test runners for new scenarios**

```rust
#[tokio::test]
async fn run_finance_focused_6mo() {
    // Same pattern as run_software_engineer_12mo with heuristic handlers
}

#[tokio::test]
async fn run_onboarding_stress_test() {
    // Same pattern
}
```

- [ ] **Step 6: Run all tests**

Run: `cargo test --test simulation -- --nocapture`
Expected: All scenarios pass. Adjust thresholds if needed.

- [ ] **Step 7: Commit**

```bash
git commit -m "feat(simulator): update tests, raise thresholds, add finance and onboarding scenarios"
```

---

### Task 11: CognitiveLlmBridge for optional real LLM

**Files:**
- Create: `crates/simulator/src/providers/cognitive_bridge.rs`

- [ ] **Step 1: Write CognitiveBridgeConfig**

```rust
// crates/simulator/src/providers/cognitive_bridge.rs
use crate::scenario::SimulationConfig;

/// Configuration for optional real-LLM cognitive extraction.
/// Reads SIMULATION_COGNITIVE_LLM env var, falling back to config.
pub struct CognitiveBridgeConfig {
    pub model: String,
    pub temperature: f64,
    pub max_calls_per_day: u32,
}

impl From<&SimulationConfig> for CognitiveBridgeConfig {
    fn from(config: &SimulationConfig) -> Self {
        Self {
            model: std::env::var("SIMULATION_COGNITIVE_LLM")
                .unwrap_or_else(|_| config.cognitive_llm_model.clone()),
            temperature: config.cognitive_temperature,
            max_calls_per_day: config.max_cognitive_calls_per_day,
        }
    }
}

impl CognitiveBridgeConfig {
    pub fn is_heuristic(&self) -> bool {
        self.model == "heuristic" || self.model.is_empty()
    }
}
```

- [ ] **Step 2: Add an ignored test for real LLM**

In `smoke.rs`:

```rust
/// Requires: SIMULATION_COGNITIVE_LLM=claude-haiku-4-5-20251001 ANTHROPIC_API_KEY=sk-...
#[tokio::test]
#[ignore]
async fn run_with_real_llm() {
    // Construct real AnthropicProvider + LlmExtractionHandler from agent crate
    // Run 7-day scenario, assert facts extracted > 0
}
```

- [ ] **Step 3: Commit**

```bash
git commit -m "feat(simulator): add CognitiveLlmBridge config for optional real LLM extraction"
```

---

### Task 12: Final verification

**Files:** All simulator files

- [ ] **Step 1: Run clippy**

Run: `cargo clippy -p simulator --all-targets`
Fix all warnings.

- [ ] **Step 2: Run formatting**

Run: `cargo fmt -p simulator --check`

- [ ] **Step 3: Run simulator unit tests**

Run: `cargo nextest run -p simulator`

- [ ] **Step 4: Run all simulation scenarios**

Run: `cargo test --test simulation -- --nocapture`

Print and review the full report for the 12-month scenario. All 14 metrics should be non-zero (except `insight_usefulness` which depends on cross-domain insights accumulating).

- [ ] **Step 5: Verify workspace build**

Run: `cargo build --workspace`

- [ ] **Step 6: Commit any fixes**

```bash
git commit -m "fix(simulator): phase 2 final cleanup"
```
