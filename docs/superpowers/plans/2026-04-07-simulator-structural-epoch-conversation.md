# Simulator Structural Gaps — Epoch Granularity + Conversation Depth

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Extend the simulator's time resolution from 1-day minimum to sub-hour epochs, and deepen conversation simulation from mostly-independent messages to context-dependent multi-turn exchanges with coherence measurement.

**Architecture:** Epoch granularity extends `EpochStep` with a `Minutes(u32)` variant, updates cron firing logic to avoid over-triggering daily crons, and adjusts message batching. Conversation depth enhances `PersonaRunner` to generate context-dependent follow-ups that reference specific prior exchanges, and adds coherence metrics via the existing embedding engine.

**Tech Stack:** Rust, chrono, simulator crate, embedding engine (bge-small-en-v1.5)

---

## File Map

| File | Change |
|---|---|
| `crates/simulator/src/epoch.rs:10-17` | Add `Minutes(u32)` to `EpochStep`, update `to_duration()`, update cron guards |
| `crates/simulator/src/harness.rs` | Update `parse_epoch_step()`, adjust message batching for sub-day epochs |
| `crates/simulator/src/scenario.rs` | Document epoch_step options |
| `crates/simulator/src/persona/conversation.rs` | Add coherence tracking, semantic drift measurement |
| `crates/simulator/src/persona/mod.rs` | Enhance follow-up generation with context-dependent references |
| `crates/simulator/src/persona/templates.rs` | Add context-referencing follow-up templates |
| `crates/simulator/src/metrics/mod.rs` | Add conversation depth metrics to accumulator/snapshot |
| `crates/simulator/src/metrics/ground_truth.rs` | Wire new metrics |

---

### Task 1: Add Minutes Epoch Step

**Files:**
- Modify: `crates/simulator/src/epoch.rs:10-27`

- [ ] **Step 1: Add Minutes variant to EpochStep**

In `crates/simulator/src/epoch.rs`, change the enum:

```rust
pub enum EpochStep {
    Minutes(u32),    // Advance by specific minutes
    Hours(u32),      // Advance by specific hours
    Day,             // Exactly 24 hours
    Week,            // Exactly 168 hours (7 days)
}
```

Update `to_duration()`:

```rust
pub fn to_duration(&self) -> Duration {
    match self {
        EpochStep::Minutes(m) => Duration::minutes(i64::from(*m)),
        EpochStep::Hours(h) => Duration::hours(i64::from(*h)),
        EpochStep::Day => Duration::hours(24),
        EpochStep::Week => Duration::hours(168),
    }
}
```

- [ ] **Step 2: Update parse_epoch_step in harness.rs**

In `crates/simulator/src/harness.rs`, find `parse_epoch_step()` and add the minutes case:

```rust
fn parse_epoch_step(s: &str) -> EpochStep {
    let s = s.trim().to_lowercase();
    if let Some(mins) = s.strip_suffix("min") {
        let m: u32 = mins.trim().parse().unwrap_or(30);
        return EpochStep::Minutes(m);
    }
    if let Some(hours) = s.strip_suffix('h') {
        let h: u32 = hours.trim().parse().unwrap_or(4);
        return EpochStep::Hours(h);
    }
    match s.as_str() {
        "hour" => EpochStep::Hours(4),
        "day" => EpochStep::Day,
        "week" => EpochStep::Week,
        _ => EpochStep::Day,
    }
}
```

- [ ] **Step 3: Guard daily crons from over-triggering**

In `crates/simulator/src/epoch.rs`, the cron collection functions use `crosses_daily_hour()` and `crosses_midnight()`. These already use half-open interval `(prev, now]` semantics, so they only fire once per crossing — even with minute-granularity steps. No change needed if the step doesn't cross the target hour more than once.

However, add a test to verify:

```rust
#[test]
fn minutes_step_fires_daily_cron_once() {
    // 48 steps of 30min = 24 hours. AtomDecay at 03:00 should fire exactly once.
    let start = Utc.with_ymd_and_hms(2026, 4, 1, 0, 0, 0).unwrap();
    let end = start + Duration::hours(24);
    let mut epoch = SimulatedEpoch::new(start, end, EpochStep::Minutes(30));

    let mut atom_decay_count = 0;
    while let Some(plan) = epoch.advance() {
        for cron in &plan.cron_pre_message {
            if matches!(cron, CronTrigger::AtomDecay) {
                atom_decay_count += 1;
            }
        }
    }
    assert_eq!(atom_decay_count, 1, "AtomDecay should fire exactly once in 24h");
}
```

- [ ] **Step 4: Adjust message batching for sub-day epochs**

In `crates/simulator/src/harness.rs`, the main loop calls `persona_runner.generate_day()` which generates all messages for a full day. With sub-day epochs, we need to batch messages by time window.

The current flow:
1. `advance()` returns an `EpochPlan` with `simulated_now`
2. `generate_day()` produces messages for the whole day
3. Messages are processed in sequence

For sub-day epochs, add logic to filter messages whose `simulated_at` falls within the current epoch's time window:

In the main loop, after `generate_day()`, filter:

```rust
// For sub-day epochs, only process messages in this epoch's window.
let epoch_messages: Vec<_> = day_messages
    .iter()
    .filter(|m| m.simulated_at > plan.previous && m.simulated_at <= plan.simulated_now)
    .cloned()
    .collect();
```

But `generate_day()` is called once per day. For sub-day steps, we need to generate the full day's messages once, then distribute them across epochs. Refactor: generate day messages at the start of each day (when `plan.day_of_simulation` increments), then yield them epoch-by-epoch.

Add a message buffer to the run loop:

```rust
let mut pending_messages: Vec<AnnotatedMessage> = Vec::new();
let mut current_day: u32 = 0;
```

Before processing each epoch:

```rust
if plan.day_of_simulation > current_day {
    current_day = plan.day_of_simulation;
    pending_messages = persona_runner.generate_day(/* ... */);
}

// Drain messages for this epoch's time window
let epoch_messages: Vec<_> = pending_messages
    .drain_filter(|m| m.simulated_at <= plan.simulated_now)
    .collect();
```

Note: `drain_filter` is unstable. Use `retain` + `drain` pattern instead:

```rust
let mut epoch_messages = Vec::new();
pending_messages.retain(|m| {
    if m.simulated_at <= plan.simulated_now {
        epoch_messages.push(m.clone());
        false
    } else {
        true
    }
});
```

- [ ] **Step 5: Add test for minutes parsing**

Add to the existing `parse_epoch_step_variants` test:

```rust
assert!(matches!(parse_epoch_step("30min"), EpochStep::Minutes(30)));
assert!(matches!(parse_epoch_step("15min"), EpochStep::Minutes(15)));
assert!(matches!(parse_epoch_step("5min"), EpochStep::Minutes(5)));
```

- [ ] **Step 6: Run tests**

Run: `cargo nextest run -p simulator --no-capture`
Expected: All pass. Existing tests use `Day` step and remain unchanged.

---

### Task 2: Conversation Coherence Metrics

Measure how well the agent maintains context across turns. The embedding engine (bge-small-en-v1.5) is already loaded in the harness for response quality scoring.

**Files:**
- Modify: `crates/simulator/src/persona/conversation.rs`
- Modify: `crates/simulator/src/metrics/mod.rs`
- Modify: `crates/simulator/src/scenario.rs`
- Modify: `crates/simulator/src/metrics/ground_truth.rs`

- [ ] **Step 1: Add coherence tracking to ConversationTracker**

In `crates/simulator/src/persona/conversation.rs`, add a method to compute semantic drift:

```rust
/// Compute semantic drift between the last two agent responses.
/// Returns None if fewer than 2 turns exist.
/// Uses cosine distance (1 - similarity) — lower = more coherent.
pub fn semantic_drift(&self, engine: &tools::EmbeddingEngine) -> Option<f64> {
    if self.turns.len() < 2 {
        return None;
    }
    let len = self.turns.len();
    let prev_response = &self.turns[len - 2].1;
    let curr_response = &self.turns[len - 1].1;

    let prev_emb = engine.embed(prev_response).ok()?;
    let curr_emb = engine.embed(curr_response).ok()?;
    let similarity = common::helpers::cosine_similarity(&prev_emb, &curr_emb);
    Some(1.0 - similarity) // drift = 1 - similarity
}

/// Return the current turn depth.
pub fn depth(&self) -> usize {
    self.turns.len()
}
```

- [ ] **Step 2: Add conversation metrics to accumulator/snapshot**

In `metrics/mod.rs`, add to `EpochAccumulator`:

```rust
    pub conversation_drift_sum: f64,
    pub conversation_drift_count: u32,
    pub conversation_depth_sum: u32,
    pub conversation_depth_count: u32,
```

Add to `MetricSnapshot`:

```rust
    pub avg_conversation_drift: f64,
    pub avg_conversation_depth: f64,
```

In `snapshot()`:

```rust
        let avg_conversation_drift = if acc.conversation_drift_count == 0 {
            0.0
        } else {
            acc.conversation_drift_sum / acc.conversation_drift_count as f64
        };
        let avg_conversation_depth = if acc.conversation_depth_count == 0 {
            0.0
        } else {
            acc.conversation_depth_sum as f64 / acc.conversation_depth_count as f64
        };
```

- [ ] **Step 3: Wire MetricName**

```rust
    AvgConversationDrift,
    AvgConversationDepth,
```

```rust
        MetricName::AvgConversationDrift => snapshot.avg_conversation_drift,
        MetricName::AvgConversationDepth => snapshot.avg_conversation_depth,
```

- [ ] **Step 4: Record drift after each agent response in harness.rs**

In the harness, after recording the agent response in `conversation_tracker.record()`, measure drift:

```rust
                    // Conversation coherence measurement
                    if let Some(ref engine) = self.embedding_engine {
                        if let Some(drift) = conversation_tracker.semantic_drift(engine) {
                            metrics.accumulator_mut().conversation_drift_sum += drift;
                            metrics.accumulator_mut().conversation_drift_count += 1;
                        }
                    }
                    metrics.accumulator_mut().conversation_depth_sum += conversation_tracker.depth() as u32;
                    metrics.accumulator_mut().conversation_depth_count += 1;
```

- [ ] **Step 5: Run tests**

Run: `cargo nextest run -p simulator --no-capture`
Expected: All pass.

---

### Task 3: Richer Follow-Up Generation

The current `generate_followup()` in `persona/mod.rs:565-591` only triggers at `followup_rate` (10-15%) and uses a simple `extract_key_phrase()` from the agent response. Enhance to create context-dependent messages that reference specific prior exchanges.

**Files:**
- Modify: `crates/simulator/src/persona/templates.rs`
- Modify: `crates/simulator/src/persona/mod.rs`

- [ ] **Step 1: Add context-referencing templates**

In `crates/simulator/src/persona/templates.rs`, add:

```rust
pub const CONTEXT_REFERENCE_TEMPLATES: &[&str] = &[
    "Going back to what you said about {previous_context}, can you elaborate on that?",
    "Earlier you mentioned {previous_context}. How does that relate to my {topic}?",
    "I was thinking about your point on {previous_context}. Actually, I changed my mind about {topic}.",
    "Remember when we discussed {previous_context}? I have a follow-up question about {topic}.",
    "You said {previous_context} before. Does that still apply if I {action}?",
];

pub const CORRECTION_FOLLOWUP_TEMPLATES: &[&str] = &[
    "Actually, that's not quite right. I meant {correct_value}, not what you assumed.",
    "No, I want you to {correct_value} instead. Please correct that.",
    "That's wrong — when I said {previous_context}, I meant {correct_value}.",
];
```

- [ ] **Step 2: Enhance generate_followup with context dependency**

In `crates/simulator/src/persona/mod.rs`, enhance `generate_followup()`:

```rust
    pub fn generate_followup(
        &mut self,
        agent_response: &str,
        simulated_at: DateTime<Utc>,
        followup_rate: f64,
    ) -> Option<AnnotatedMessage> {
        if self.rng.random::<f64>() >= followup_rate {
            return None;
        }

        let key_phrase = extract_key_phrase(agent_response)?;

        // 70% context reference, 30% correction follow-up
        let (content, is_correction) = if self.rng.random::<f64>() < 0.7 {
            let template = pick_template(templates::CONTEXT_REFERENCE_TEMPLATES, &mut self.rng);
            let last_topic = self.topic_history.back().cloned().unwrap_or_else(|| "that".to_string());
            let actions = ["review it", "change the deadline", "add a note", "update the task"];
            let action = actions[self.rng.random_range(0..actions.len())];
            let text = fill_template(template, &[
                ("previous_context", &key_phrase),
                ("topic", &last_topic),
                ("action", action),
            ]);
            (text, false)
        } else {
            let template = pick_template(templates::CORRECTION_FOLLOWUP_TEMPLATES, &mut self.rng);
            let correct_values = ["the updated version", "next Monday instead", "the other project"];
            let correct_value = correct_values[self.rng.random_range(0..correct_values.len())];
            let text = fill_template(template, &[
                ("previous_context", &key_phrase),
                ("correct_value", correct_value),
            ]);
            (text, true)
        };

        Some(AnnotatedMessage {
            content,
            phase: self.current_phase,
            simulated_at,
            ground_truth: None,
            tool_actions: vec![],
            is_correction,
            topic: "followup".to_string(),
            is_followup: true,
            workflow: None,
            is_adversarial: false,
        })
    }
```

- [ ] **Step 3: Run tests**

Run: `cargo nextest run -p simulator --no-capture`
Expected: All pass. Follow-ups now include context references and occasional corrections.

---

### Task 4: Display Metrics + Update SIMULATOR.md

- [ ] **Step 1: Add new metrics to smoke.rs**

```rust
    eprintln!("  CONVERSATION DEPTH");
    eprintln!("    Avg drift:            {:.3}", fm.avg_conversation_drift);
    eprintln!("    Avg depth:            {:.1}", fm.avg_conversation_depth);
```

- [ ] **Step 2: Update SIMULATOR.md structural gaps**

Mark "Time granularity" and "Conversation depth" as resolved:

```markdown
### Recently Resolved Structural Gaps

- **Time granularity** — `EpochStep::Minutes(u32)` added. Scenarios can use `epoch_step = "30min"` for sub-hour resolution. Daily crons still fire correctly (once per crossing).
- **Conversation depth** — Follow-ups now reference specific prior context (70% context reference, 30% correction). Semantic drift measured via embedding cosine distance. `avg_conversation_drift` and `avg_conversation_depth` metrics added.
```

- [ ] **Step 3: Run full tests**

Run: `cargo nextest run -p simulator --no-capture && cargo nextest run --test simulation -E 'test(smoke_test_7_day)' --no-capture`
Expected: All pass.

---

## Verification

```bash
# Unit tests
cargo nextest run -p simulator --no-capture

# Smoke test with default Day step
cargo nextest run --test simulation -E 'test(smoke_test_7_day)' --no-capture

# To test minutes step, temporarily set epoch_step = "30min" in a scenario TOML
```
