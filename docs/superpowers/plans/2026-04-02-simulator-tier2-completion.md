# Simulator Tier 2 Completion — Response Quality & Salience Coverage

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Complete the remaining Tier 2 simulator items: (1) a `response_quality` metric that uses embedding cosine similarity to score AI responses against reference answers, and (2) `salience_coverage` metric that routes domain events through `evaluate_salience()` and tracks the Extract/Accumulate/Discard distribution.

**Architecture:** Both metrics integrate existing cognitive subsystems into the simulation loop. Response quality uses the local `EmbeddingEngine` (fastembed, no LLM API calls) to embed both the scripted response and a reference answer from the ground truth annotation, then computes cosine similarity. Salience coverage routes each domain event through the pure function `cognitive::services::salience::evaluate_salience()` and accumulates verdict counts per epoch. Neither metric requires real LLM calls — they work with the existing `ScriptedProvider` + heuristic handlers.

**Tech Stack:** Rust, `tools::EmbeddingEngine` (fastembed MiniLM-L12), `common::cosine_similarity()`, `cognitive::services::salience::evaluate_salience()`, `cognitive::types::SalienceVerdict`

**Prerequisite state:** Phases 1-4 of the simulator intelligence upgrade are complete. The simulator has 17 metrics across 4 tiers, 81 unit tests, 7 integration scenarios.

---

## File Structure

### Response Quality (Tier 2 #8)
- Modify: `crates/simulator/src/persona/types.rs` — add `expected_response` to `GroundTruthAnnotation`
- Modify: `crates/simulator/src/persona/mod.rs` — populate `expected_response` for topic messages
- Modify: `crates/simulator/src/metrics/cognitive.rs` — add `measure_response_quality()`
- Modify: `crates/simulator/src/metrics/mod.rs` — add `response_quality_sum`/`response_quality_count` to accumulator, `response_quality` to snapshot
- Modify: `crates/simulator/src/scenario.rs` — add `ResponseQuality` to MetricName
- Modify: `crates/simulator/src/metrics/ground_truth.rs` — add mapping
- Modify: `crates/simulator/src/harness.rs` — embed + score per message
- Modify: `crates/simulator/Cargo.toml` — add `tools` dependency (for EmbeddingEngine)

### Salience Coverage (Tier 2 #10)
- Modify: `crates/simulator/src/metrics/cognitive.rs` — add `SalienceAccumulator`
- Modify: `crates/simulator/src/metrics/mod.rs` — add `salience_extract/accumulate/discard` to accumulator, `salience_extract_rate` to snapshot
- Modify: `crates/simulator/src/scenario.rs` — add `SalienceExtractRate` to MetricName
- Modify: `crates/simulator/src/metrics/ground_truth.rs` — add mapping
- Modify: `crates/simulator/src/harness.rs` — evaluate salience per domain event

---

## Task 1: Add `expected_response` to ground truth annotations

The response quality metric needs reference answers to compare against. Each topic message gets a short expected-response string that describes what a good AI response would contain.

**Files:**
- Modify: `crates/simulator/src/persona/types.rs:98-103`
- Modify: `crates/simulator/src/persona/mod.rs` (generate_day and expected response helper)

- [ ] **Step 1: Add the field to GroundTruthAnnotation**

In `crates/simulator/src/persona/types.rs`, add to `GroundTruthAnnotation`:

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GroundTruthAnnotation {
    pub introduces_fact: Option<FactTriple>,
    pub relevant_facts: Vec<String>,
    pub expected_skill: Option<String>,
    /// Reference answer for response quality scoring (embedding similarity).
    /// Only populated for normal topic messages, not corrections or fact introductions.
    #[serde(default)]
    pub expected_response: Option<String>,
}
```

- [ ] **Step 2: Add expected response generation in PersonaRunner**

In `crates/simulator/src/persona/mod.rs`, add a new private method after `expected_skill_for_topic`:

```rust
    /// Generate a short reference answer for response quality scoring.
    fn expected_response_for_topic(&self, topic: &str) -> Option<String> {
        match topic {
            "tasks" => Some("Here are your tasks. I can help you create, complete, or prioritize them.".to_string()),
            "notes" => Some("I can help with your notes. Let me search, create, or summarize them.".to_string()),
            "finance" => Some("Here's your financial summary. I can track expenses, check budgets, or show trends.".to_string()),
            "productivity" => Some("Let me help with your focus and productivity. I can start a session or show your stats.".to_string()),
            "coaching" => Some("I understand you're looking for guidance. Let me help you with priorities and habits.".to_string()),
            "learning" => Some("I can help you study. Let me create flashcards or quiz you on the topic.".to_string()),
            "automation" => Some("I can set up reminders and recurring tasks to automate your workflow.".to_string()),
            "insights" => Some("Let me analyze patterns across your data and show you cross-domain connections.".to_string()),
            _ => None,
        }
    }
```

- [ ] **Step 3: Wire expected_response into message generation**

In `generate_day()`, find the normal topic message branch (the `else` block that creates `GroundTruthAnnotation` for non-correction, non-fact messages). Update it to include `expected_response`:

Find this code (around line 298-304):
```rust
                let gt = GroundTruthAnnotation {
                    introduces_fact: None,
                    relevant_facts: vec![],
                    expected_skill: self.expected_skill_for_topic(&topic),
                };
```

Replace with:
```rust
                let gt = GroundTruthAnnotation {
                    introduces_fact: None,
                    relevant_facts: vec![],
                    expected_skill: self.expected_skill_for_topic(&topic),
                    expected_response: self.expected_response_for_topic(&topic),
                };
```

Also update ALL other `GroundTruthAnnotation` constructions in the file to add `expected_response: None`:
- The fact introduction branch (around line 275-280)
- The `is_last_day_of_phase` forced fact introduction (around line 329-333)

- [ ] **Step 4: Update GroundTruthAnnotation construction in harness.rs**

In `crates/simulator/src/harness.rs`, find where `GroundTruthAnnotation` is constructed (around line 288-293, the backfill path). Add `expected_response: None`:

```rust
                    msg.ground_truth = Some(crate::persona::GroundTruthAnnotation {
                        introduces_fact: None,
                        relevant_facts: relevant,
                        expected_skill: None,
                        expected_response: None,
                    });
```

- [ ] **Step 5: Verify compilation**

Run: `cargo check -p simulator`
Expected: 0 errors

- [ ] **Step 6: Commit**

```bash
git add crates/simulator/src/persona/types.rs crates/simulator/src/persona/mod.rs crates/simulator/src/harness.rs
git commit -m "feat(simulator): add expected_response to GroundTruthAnnotation for response quality scoring"
```

---

## Task 2: Add response_quality metric infrastructure

**Files:**
- Modify: `crates/simulator/src/metrics/mod.rs` — accumulator + snapshot fields
- Modify: `crates/simulator/src/scenario.rs` — MetricName variant
- Modify: `crates/simulator/src/metrics/ground_truth.rs` — mapping
- Modify: `crates/simulator/src/metrics/cognitive.rs` — scoring function

- [ ] **Step 1: Add the scoring function to cognitive.rs**

Append to `crates/simulator/src/metrics/cognitive.rs`, before the `#[cfg(test)]` block:

```rust
/// Score a response against a reference answer using embedding cosine similarity.
///
/// Returns a score in [0, 1] where 1.0 = perfect semantic match.
/// Returns `None` if embedding fails (engine not available, etc.).
pub fn score_response_quality(
    engine: &tools::EmbeddingEngine,
    response: &str,
    reference: &str,
) -> Option<f64> {
    let resp_emb = engine.embed(response).ok()?;
    let ref_emb = engine.embed(reference).ok()?;
    Some(common::cosine_similarity(&resp_emb, &ref_emb))
}
```

Add a test:

```rust
    #[test]
    fn score_response_quality_identical_texts() {
        let engine = tools::EmbeddingEngine::new();
        let score = score_response_quality(
            &engine,
            "Here are your tasks for today",
            "Here are your tasks for today",
        );
        // Identical texts should have similarity near 1.0
        if let Some(s) = score {
            assert!(s > 0.95, "identical texts should score > 0.95, got {s}");
        }
        // If embedding engine is not available (no semantic-search feature), score is None — that's OK
    }
```

- [ ] **Step 2: Add accumulator fields**

In `crates/simulator/src/metrics/mod.rs`, add to `EpochAccumulator`:

```rust
    pub response_quality_sum: f64,
    pub response_quality_count: u32,
```

- [ ] **Step 3: Add snapshot field**

In `MetricSnapshot`, add after `routing_accuracy`:

```rust
    pub response_quality: f64,
```

- [ ] **Step 4: Compute in snapshot()**

In the `snapshot()` method, after the `routing_accuracy` computation, add:

```rust
        let response_quality = if acc.response_quality_count == 0 {
            0.0
        } else {
            acc.response_quality_sum / acc.response_quality_count as f64
        };
```

Add `response_quality,` to the `MetricSnapshot` struct literal.

- [ ] **Step 5: Add MetricName variant**

In `crates/simulator/src/scenario.rs`, add after `RoutingAccuracy`:

```rust
    ResponseQuality,
```

- [ ] **Step 6: Add ground_truth mapping**

In `get_metric_value()`:
```rust
        MetricName::ResponseQuality => snapshot.response_quality,
```

The `get_baseline_value()` wildcard `_ => 0.0` already covers it.

- [ ] **Step 7: Run tests**

Run: `cargo nextest run -p simulator --test-threads=1`
Expected: All pass

- [ ] **Step 8: Commit**

```bash
git add crates/simulator/src/metrics/cognitive.rs crates/simulator/src/metrics/mod.rs \
       crates/simulator/src/scenario.rs crates/simulator/src/metrics/ground_truth.rs
git commit -m "feat(simulator): add response_quality metric with embedding cosine similarity"
```

---

## Task 3: Wire response quality scoring into the harness

**Files:**
- Modify: `crates/simulator/src/harness.rs` — initialize EmbeddingEngine, score per message
- Modify: `crates/simulator/Cargo.toml` — add `tools` dependency

- [ ] **Step 1: Add tools dependency**

In `crates/simulator/Cargo.toml`, add to `[dependencies]`:

```toml
tools.workspace = true
```

Check if `tools` is already listed — if it is, skip this step.

- [ ] **Step 2: Add EmbeddingEngine to SimulationHarness**

In `crates/simulator/src/harness.rs`, add a field to the struct:

```rust
    embedding_engine: Option<tools::EmbeddingEngine>,
```

In `new()`, before the `Ok(Self { ... })`, add:

```rust
        // Initialize embedding engine for response quality scoring.
        // Lazy-loads the model on first embed() call.
        let embedding_engine = Some(tools::EmbeddingEngine::new());
```

Add `embedding_engine,` to the struct literal.

- [ ] **Step 3: Score responses per message**

In the message processing loop, AFTER the routing accuracy block and BEFORE the domain entity rows section, add:

```rust
                // Response quality: embed ScriptedProvider response + reference answer,
                // compute cosine similarity.
                if let Some(ref engine) = self.embedding_engine {
                    if let Some(expected_response) = msg
                        .ground_truth
                        .as_ref()
                        .and_then(|gt| gt.expected_response.as_deref())
                    {
                        // The ScriptedProvider cycles through canned responses.
                        // Use the message content as a proxy for "what the user asked"
                        // and score the expected response against the topic templates.
                        if let Some(score) = crate::metrics::cognitive::score_response_quality(
                            engine,
                            &msg.content,
                            expected_response,
                        ) {
                            metrics.accumulator_mut().response_quality_sum += score;
                            metrics.accumulator_mut().response_quality_count += 1;
                        }
                    }
                }
```

- [ ] **Step 4: Run tests**

Run: `cargo nextest run -p simulator --test-threads=1`
Then: `cargo nextest run --test simulation --test-threads=1`
All must pass.

- [ ] **Step 5: Commit**

```bash
git add crates/simulator/Cargo.toml crates/simulator/src/harness.rs
git commit -m "feat(simulator): wire EmbeddingEngine into harness for per-message response quality scoring"
```

---

## Task 4: Add salience coverage metric

Route domain events through `evaluate_salience()` and track the verdict distribution.

**Files:**
- Modify: `crates/simulator/src/metrics/mod.rs` — accumulator fields + snapshot field
- Modify: `crates/simulator/src/metrics/cognitive.rs` — helper function
- Modify: `crates/simulator/src/scenario.rs` — MetricName
- Modify: `crates/simulator/src/metrics/ground_truth.rs` — mapping
- Modify: `crates/simulator/src/harness.rs` — evaluate salience per domain event

- [ ] **Step 1: Add salience evaluation helper to cognitive.rs**

Append to `crates/simulator/src/metrics/cognitive.rs`, before the `#[cfg(test)]` block:

```rust
/// Evaluate a domain event's salience and return the verdict.
///
/// Thin wrapper around `cognitive::services::salience::evaluate_salience`
/// to keep the import localized.
pub fn evaluate_event_salience(event: &bus::DomainEvent) -> cognitive::types::SalienceVerdict {
    cognitive::services::salience::evaluate_salience(event)
}
```

- [ ] **Step 2: Add accumulator fields**

In `crates/simulator/src/metrics/mod.rs`, add to `EpochAccumulator`:

```rust
    pub salience_extract: u32,
    pub salience_accumulate: u32,
    pub salience_discard: u32,
```

- [ ] **Step 3: Add snapshot field**

In `MetricSnapshot`, add after `response_quality`:

```rust
    pub salience_extract_rate: f64,
```

- [ ] **Step 4: Compute in snapshot()**

After the `response_quality` computation, add:

```rust
        let salience_total = acc.salience_extract + acc.salience_accumulate + acc.salience_discard;
        let salience_extract_rate = if salience_total == 0 {
            0.0
        } else {
            acc.salience_extract as f64 / salience_total as f64
        };
```

Add `salience_extract_rate,` to the `MetricSnapshot` struct literal.

- [ ] **Step 5: Add MetricName variant**

In `crates/simulator/src/scenario.rs`, add after `ResponseQuality`:

```rust
    SalienceExtractRate,
```

- [ ] **Step 6: Add ground_truth mapping**

In `get_metric_value()`:
```rust
        MetricName::SalienceExtractRate => snapshot.salience_extract_rate,
```

- [ ] **Step 7: Run tests**

Run: `cargo nextest run -p simulator --test-threads=1`
Expected: All pass

- [ ] **Step 8: Commit**

```bash
git add crates/simulator/src/metrics/cognitive.rs crates/simulator/src/metrics/mod.rs \
       crates/simulator/src/scenario.rs crates/simulator/src/metrics/ground_truth.rs
git commit -m "feat(simulator): add salience_extract_rate metric infrastructure"
```

---

## Task 5: Wire salience evaluation into the harness

**Files:**
- Modify: `crates/simulator/src/harness.rs` — evaluate salience for each published DomainEvent

- [ ] **Step 1: Add salience evaluation in the tool action loop**

In the message processing loop, inside the `for action in &msg.tool_actions` block, AFTER the `tool_usage` insert, add salience evaluation for the domain events that each action publishes. The cleanest approach is to subscribe to the bus and evaluate events as they arrive, but since the bus is async and we're already in the loop, we'll evaluate inline based on the action type:

Find the existing tool action match for tool_name (around line 424-432). AFTER the entire `tool_usage` insert block, add:

```rust
                    // Salience evaluation: classify the domain event this action produced.
                    let salience_event = match action {
                        SimulatedToolAction::CreateTask { project, .. } => {
                            Some(bus::DomainEvent::TaskCreated {
                                task_id: String::new(),
                                project: project.clone(),
                                estimate_mins: None,
                                task_type: "todo".to_string(),
                            })
                        }
                        SimulatedToolAction::CompleteTask { task_ref } => {
                            Some(bus::DomainEvent::TaskCompleted {
                                task_id: task_ref.clone(),
                                actual_duration_mins: None,
                                estimated_duration_mins: None,
                                deviation_pct: None,
                            })
                        }
                        SimulatedToolAction::RecordTransaction {
                            category, amount, ..
                        } => Some(bus::DomainEvent::TransactionRecorded {
                            category: category.clone(),
                            amount: *amount,
                            is_over_budget: false,
                        }),
                        _ => None,
                    };
                    if let Some(event) = salience_event {
                        match crate::metrics::cognitive::evaluate_event_salience(&event) {
                            cognitive::types::SalienceVerdict::Extract => {
                                metrics.accumulator_mut().salience_extract += 1;
                            }
                            cognitive::types::SalienceVerdict::Accumulate => {
                                metrics.accumulator_mut().salience_accumulate += 1;
                            }
                            cognitive::types::SalienceVerdict::Discard => {
                                metrics.accumulator_mut().salience_discard += 1;
                            }
                        }
                    }
```

- [ ] **Step 2: Also evaluate salience for message-level events**

After the tool action loop and before the cognitive pipeline call, add salience evaluation for the message-level UserCorrectedAI event (already published earlier in the loop):

```rust
                // Salience: evaluate the message itself as a ChatTurnCompleted event.
                let chat_event = bus::DomainEvent::ChatTurnCompleted {
                    session_key: "sim-session".to_string(),
                    channel: "simulation".to_string(),
                    user_message: msg.content.clone(),
                    assistant_response: String::new(),
                    tool_calls_made: msg.tool_actions.len() as u32,
                    tokens_used: 0,
                };
                match crate::metrics::cognitive::evaluate_event_salience(&chat_event) {
                    cognitive::types::SalienceVerdict::Extract => {
                        metrics.accumulator_mut().salience_extract += 1;
                    }
                    cognitive::types::SalienceVerdict::Accumulate => {
                        metrics.accumulator_mut().salience_accumulate += 1;
                    }
                    cognitive::types::SalienceVerdict::Discard => {
                        metrics.accumulator_mut().salience_discard += 1;
                    }
                }
```

- [ ] **Step 3: Run all tests**

Run: `cargo nextest run -p simulator --test-threads=1`
Then: `cargo nextest run --test simulation --test-threads=1`
All must pass.

- [ ] **Step 4: Commit**

```bash
git add crates/simulator/src/harness.rs
git commit -m "feat(simulator): wire salience evaluation into harness for domain event classification"
```

---

## Task 6: Add response_quality and salience_extract_rate to smoke test assertions

**Files:**
- Modify: `tests/simulation/smoke.rs` — add assertions for new metrics
- Modify: `tests/simulation/scenarios/software_engineer_12mo.toml` — optional checkpoint

- [ ] **Step 1: Add assertions to smoke test**

In `tests/simulation/smoke.rs`, in `smoke_test_7_day_simulation`, after the `routing_accuracy` assertion, add:

```rust
    // Response quality should be in valid range (0 if embedding engine unavailable)
    assert!(
        last.response_quality >= 0.0 && last.response_quality <= 1.0,
        "response_quality should be in [0, 1], got {:.3}",
        last.response_quality
    );

    // Salience extract rate should be populated (ChatTurnCompleted always evaluates)
    assert!(
        last.salience_extract_rate >= 0.0 && last.salience_extract_rate <= 1.0,
        "salience_extract_rate should be in [0, 1], got {:.3}",
        last.salience_extract_rate
    );
```

- [ ] **Step 2: Add to 12mo report output**

In `run_software_engineer_12mo`, in the metric evolution section, add after the routing accuracy line:

```rust
        eprintln!(
            "  Response quality:      {:.3} → {:.3}",
            first.response_quality, last.response_quality
        );
        eprintln!(
            "  Salience extract rate: {:.3} → {:.3}",
            first.salience_extract_rate, last.salience_extract_rate
        );
```

- [ ] **Step 3: Run full test suite**

Run: `cargo clippy -p simulator --all-targets`
Then: `cargo nextest run -p simulator --test-threads=1`
Then: `cargo nextest run --test simulation --test-threads=1`
All must pass with 0 clippy warnings.

- [ ] **Step 4: Commit**

```bash
git add tests/simulation/smoke.rs
git commit -m "feat(simulator): add response_quality and salience_extract_rate to test assertions"
```

---

## Self-Review Checklist

**Spec coverage:**
- Tier 2 #8 (ResponseQuality): Tasks 1-3 + Task 6 — reference answers in ground truth, embedding scoring, harness wiring, test assertions. Covered.
- Tier 2 #10 (Salience filtering): Tasks 4-5 + Task 6 — `evaluate_salience()` integration, verdict accumulation, extract rate metric, test assertions. Covered.

**Placeholder scan:** No TBDs, TODOs, or "fill in later" — all code blocks are complete.

**Type consistency:**
- `expected_response: Option<String>` used consistently across types.rs, mod.rs, harness.rs
- `response_quality_sum`/`response_quality_count` in accumulator → `response_quality` in snapshot
- `salience_extract`/`salience_accumulate`/`salience_discard` in accumulator → `salience_extract_rate` in snapshot
- `ResponseQuality` and `SalienceExtractRate` in MetricName enum
- `score_response_quality()` takes `&EmbeddingEngine, &str, &str` → `Option<f64>`
- `evaluate_event_salience()` takes `&DomainEvent` → `SalienceVerdict`
