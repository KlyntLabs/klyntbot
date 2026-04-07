# Simulator Structural Gaps — Error Cascades + Concurrent Sessions

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add realistic error cascade testing (chained failures with dependency-aware injection) and multi-session simulation (concurrent channels sharing state) to the simulator.

**Architecture:** Error cascades replace the flat `error_injection_rate` with a dependency-aware model: when a "root cause" error fires, downstream tool calls in the same execution get elevated failure rates. Concurrent sessions spawn multiple tokio tasks sharing `StoragePool`, `DomainEventBus`, and `MetricCollector`, each with its own `ConversationTracker` and `PersonaRunner`. Metrics are collected per-channel and aggregated.

**Tech Stack:** Rust, tokio, simulator crate, AtomicU32 for shared counters

---

## File Map

| File | Change |
|---|---|
| `crates/simulator/src/error_injector.rs` | Add `CascadeErrorInjector` with dependency graph |
| `crates/simulator/src/agent_harness.rs` | Wire cascade injector, replace `ErrorInjectingTool` |
| `crates/simulator/src/scenario.rs` | Add `error_cascade_config` to SimulationConfig |
| `crates/simulator/src/metrics/mod.rs` | Add cascade + concurrency metrics |
| `crates/simulator/src/harness.rs` | Multi-session spawning, per-channel metrics |
| `crates/simulator/src/persona/types.rs` | Add `ChannelConfig` type |

---

### Task 1: Error Cascade Model

Replace flat-rate error injection with a dependency-aware model. Current system: `ErrorInjectingTool` at `agent_harness.rs:25-73` wraps each tool with a per-tool RNG at a flat `error_injection_rate`. New system: when a "root cause" error fires (e.g. storage), subsequent tool calls in the same ReAct execution get elevated failure rates for dependent error types.

**Files:**
- Modify: `crates/simulator/src/error_injector.rs`
- Modify: `crates/simulator/src/scenario.rs`

- [ ] **Step 1: Define cascade config in scenario**

In `crates/simulator/src/scenario.rs`, add to `SimulationConfig`:

```rust
    /// Error cascade configuration. When a root error fires, dependent errors
    /// get elevated rates for the remainder of that agent execution.
    /// Default: no cascades (flat injection).
    #[serde(default)]
    pub error_cascade_enabled: bool,
    /// Multiplier applied to dependent error rates after a root cause fires.
    /// E.g. 3.0 means downstream errors are 3x more likely. Default: 3.0.
    #[serde(default = "default_cascade_multiplier")]
    pub error_cascade_multiplier: f64,
```

```rust
fn default_cascade_multiplier() -> f64 {
    3.0
}
```

Add to `Default`:

```rust
            error_cascade_enabled: false,
            error_cascade_multiplier: default_cascade_multiplier(),
```

- [ ] **Step 2: Add CascadeState to error_injector.rs**

In `crates/simulator/src/error_injector.rs`, add:

```rust
use std::sync::atomic::{AtomicBool, Ordering};

/// Shared cascade state across all tools in one agent execution.
/// When a root error fires, downstream tools see elevated rates.
#[derive(Default)]
pub struct CascadeState {
    /// Whether a storage error has fired in this execution.
    pub storage_failed: AtomicBool,
    /// Whether a timeout has fired in this execution.
    pub timeout_fired: AtomicBool,
}

impl CascadeState {
    pub fn reset(&self) {
        self.storage_failed.store(false, Ordering::Relaxed);
        self.timeout_fired.store(false, Ordering::Relaxed);
    }
}

/// Compute effective error rate given cascade state.
/// Storage failure → extraction/retrieval tools get elevated rate.
/// Timeout → subsequent tools get elevated rate.
pub fn cascade_adjusted_rate(
    base_rate: f64,
    tool_name: &str,
    state: &CascadeState,
    multiplier: f64,
) -> f64 {
    let mut rate = base_rate;

    // Storage failure elevates extraction and retrieval tools
    if state.storage_failed.load(Ordering::Relaxed) {
        let affected = ["memory", "notes", "tasks", "project", "finance"];
        if affected.iter().any(|t| tool_name.contains(t)) {
            rate = (rate * multiplier).min(0.8);
        }
    }

    // Timeout elevates all subsequent tools
    if state.timeout_fired.load(Ordering::Relaxed) {
        rate = (rate * (multiplier * 0.5)).min(0.5);
    }

    rate
}

/// Enhanced error sampling that updates cascade state.
pub fn sample_cascade_error(
    rng: &mut StdRng,
    rate: f64,
    state: &CascadeState,
) -> Option<common::KlyntbotError> {
    if rate <= 0.0 || rng.random::<f64>() >= rate {
        return None;
    }
    let err = match rng.random_range(0u8..4) {
        0 => {
            state.storage_failed.store(true, Ordering::Relaxed);
            common::KlyntbotError::Storage(
                "table locked — concurrent write in progress".to_string(),
            )
        }
        1 => common::KlyntbotError::Tool(common::ToolError::ExecutionFailed(
            "entity not found: no matching note for query".to_string(),
        )),
        2 => {
            state.timeout_fired.store(true, Ordering::Relaxed);
            common::KlyntbotError::Tool(common::ToolError::ExecutionFailed(
                "tool execution timed out after 30s".to_string(),
            ))
        }
        _ => common::KlyntbotError::Tool(common::ToolError::InvalidParams(
            "invalid argument: amount must be positive".to_string(),
        )),
    };
    Some(err)
}
```

- [ ] **Step 3: Update ErrorInjectingTool to use cascade state**

In `crates/simulator/src/agent_harness.rs`, modify `ErrorInjectingTool`:

```rust
struct ErrorInjectingTool {
    inner: tools::DynTool,
    base_rate: f64,
    cascade_state: Arc<CascadeState>,
    cascade_multiplier: f64,
    rng: std::sync::Mutex<rand::rngs::StdRng>,
}
```

Update `execute()`:

```rust
async fn execute(
    &self,
    args: serde_json::Value,
    ctx: &RoutingContext,
) -> common::Result<String> {
    let effective_rate = crate::error_injector::cascade_adjusted_rate(
        self.base_rate,
        &self.inner.name(),
        &self.cascade_state,
        self.cascade_multiplier,
    );
    let injected = {
        let mut rng = self.rng.lock().unwrap();
        crate::error_injector::sample_cascade_error(&mut rng, effective_rate, &self.cascade_state)
    };
    if let Some(err) = injected {
        return Err(err);
    }
    self.inner.execute(args, ctx).await
}
```

- [ ] **Step 4: Reset cascade state per agent execution**

In the harness's `process()` method, before each agent call:

```rust
// Reset cascade state for this execution
self.cascade_state.reset();
```

Add `cascade_state: Arc<CascadeState>` as a field of the agent harness struct.

- [ ] **Step 5: Add cascade metrics**

In `metrics/mod.rs`, add to `EpochAccumulator`:

```rust
    pub cascade_triggered: u32,
    pub cascade_depth_sum: u32,
```

In `MetricSnapshot`:

```rust
    pub cascade_rate: f64,
    pub avg_cascade_depth: f64,
```

In `snapshot()`:

```rust
        let cascade_rate = if acc.error_injected == 0 {
            0.0
        } else {
            acc.cascade_triggered as f64 / acc.error_injected as f64
        };
        let avg_cascade_depth = if acc.cascade_triggered == 0 {
            0.0
        } else {
            acc.cascade_depth_sum as f64 / acc.cascade_triggered as f64
        };
```

- [ ] **Step 6: Wire MetricName**

```rust
    CascadeRate,
    AvgCascadeDepth,
```

```rust
        MetricName::CascadeRate => snapshot.cascade_rate,
        MetricName::AvgCascadeDepth => snapshot.avg_cascade_depth,
```

- [ ] **Step 7: Run tests**

Run: `cargo nextest run -p simulator --no-capture`
Expected: All pass. Cascade metrics will be 0 with `error_cascade_enabled: false` (default).

---

### Task 2: Concurrent Sessions — Multi-Channel Simulation

Add support for running multiple simultaneous sessions (e.g. Telegram + CLI + Discord) that share `StoragePool`, `DomainEventBus`, and `MetricCollector`.

**Files:**
- Modify: `crates/simulator/src/scenario.rs`
- Modify: `crates/simulator/src/harness.rs`
- Modify: `crates/simulator/src/metrics/mod.rs`

- [ ] **Step 1: Add channel config to scenario**

In `crates/simulator/src/scenario.rs`, add:

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChannelConfig {
    pub name: String,
    /// Fraction of total messages routed to this channel (0.0-1.0).
    pub message_share: f64,
    /// Seed offset for this channel's persona RNG (deterministic per-channel).
    #[serde(default)]
    pub seed_offset: u64,
}
```

Add to `SimulationConfig`:

```rust
    /// Concurrent channels to simulate. Default: single channel.
    /// When specified, messages are distributed across channels by share.
    #[serde(default)]
    pub channels: Vec<ChannelConfig>,
```

Default: empty vec (single session mode, backward compatible).

- [ ] **Step 2: Add per-channel session keys**

In `crates/simulator/src/harness.rs`, instead of hardcoded `"sim-session"`, generate per-channel keys:

```rust
fn channel_session_key(channel: &str) -> String {
    format!("sim-{channel}")
}
```

When channels are configured, create one `ConversationTracker` per channel.

- [ ] **Step 3: Distribute messages across channels**

In the main run loop, after `generate_day()`, distribute messages:

```rust
if self.scenario.simulation.channels.is_empty() {
    // Single-channel mode (backward compatible)
    process_messages(messages, "sim-session", &mut conversation_tracker, &mut metrics);
} else {
    // Multi-channel mode: assign each message to a channel by share
    let mut rng = StdRng::seed_from_u64(self.scenario.persona.seed + day as u64);
    for msg in &messages {
        let channel = select_channel(&self.scenario.simulation.channels, &mut rng);
        let session_key = channel_session_key(&channel.name);
        // Process with channel-specific tracker
    }
}
```

Channel selection by weighted random:

```rust
fn select_channel<'a>(channels: &'a [ChannelConfig], rng: &mut StdRng) -> &'a ChannelConfig {
    let total: f64 = channels.iter().map(|c| c.message_share).sum();
    let mut roll = rng.random::<f64>() * total;
    for ch in channels {
        roll -= ch.message_share;
        if roll <= 0.0 {
            return ch;
        }
    }
    channels.last().unwrap()
}
```

- [ ] **Step 4: Add per-channel metrics**

In `metrics/mod.rs`, add to `MetricSnapshot`:

```rust
    pub channel_message_distribution: HashMap<String, u32>,
```

In `EpochAccumulator`:

```rust
    pub channel_messages: HashMap<String, u32>,
```

In `snapshot()`:

```rust
        let channel_message_distribution = std::mem::take(&mut acc.channel_messages);
```

- [ ] **Step 5: Concurrent processing with tokio**

For true concurrency, spawn per-channel tasks. But this requires making `MetricCollector` thread-safe. Simpler approach: process channels sequentially within each epoch (interleaved, not parallel). This still tests cross-channel state effects (shared DB, shared event bus) without async complexity.

The key insight: concurrency bugs come from shared mutable state, not from literal parallelism. Sequential interleaving of channel messages achieves the same effect for the simulator's purposes.

```rust
// Interleave messages across channels chronologically
let mut all_messages: Vec<(String, AnnotatedMessage)> = Vec::new();
for msg in messages {
    let channel = select_channel(&channels, &mut rng);
    all_messages.push((channel.name.clone(), msg));
}
// Sort by simulated_at to interleave
all_messages.sort_by_key(|(_, m)| m.simulated_at);

// Process in chronological order, switching channels as messages arrive
for (channel_name, msg) in all_messages {
    let session_key = channel_session_key(&channel_name);
    let tracker = channel_trackers.get_mut(&channel_name).unwrap();
    // Process message with channel-specific tracker but shared pool/bus/metrics
    metrics.accumulator_mut().channel_messages
        .entry(channel_name)
        .and_modify(|c| *c += 1)
        .or_insert(1);
}
```

- [ ] **Step 6: Maintain per-channel ConversationTrackers**

```rust
let mut channel_trackers: HashMap<String, ConversationTracker> = HashMap::new();
for ch in &self.scenario.simulation.channels {
    channel_trackers.insert(
        ch.name.clone(),
        ConversationTracker::new(self.scenario.simulation.multi_turn_history_depth as usize),
    );
}
// Default single-channel tracker for backward compat
if channel_trackers.is_empty() {
    channel_trackers.insert("default".to_string(), ConversationTracker::new(depth));
}
```

- [ ] **Step 7: Wire MetricName**

No new MetricName needed — `channel_message_distribution` is a map stored in the snapshot for analysis, not a single metric value.

- [ ] **Step 8: Add a multi-channel test scenario**

Create `tests/simulation/scenarios/multi_channel_test.toml`:

```toml
[persona]
name = "multi_channel_test"
timezone = "UTC"
language = "en"
seed = 42

[persona.messages_per_day]
onboarding = 4
routine = 3
power_user = 3
shift = 3

[persona.profile]
known_facts = [
    { subject = "user", predicate = "works_as", object = "engineer" },
]

[persona.phases.onboarding]
duration_days = 3
correction_rate = 0.1
topic_weights = { tasks = 0.5, chat = 0.5 }
new_fact_introduction_rate = 0.3
tool_action_rate = 0.3

[persona.phases.routine]
duration_days = 2
correction_rate = 0.1
topic_weights = { tasks = 0.5, chat = 0.5 }
new_fact_introduction_rate = 0.1
tool_action_rate = 0.5

[persona.phases.power_user]
duration_days = 1
correction_rate = 0.05
topic_weights = { tasks = 0.5, chat = 0.5 }
new_fact_introduction_rate = 0.05
tool_action_rate = 0.7

[persona.phases.behavior_shift]
duration_days = 1
correction_rate = 0.15
shift_description = "switches focus"
new_facts = [{ subject = "user", predicate = "learning", object = "Python" }]
topic_weights = { tasks = 0.3, notes = 0.4, chat = 0.3 }
new_fact_introduction_rate = 0.4
tool_action_rate = 0.5

[simulation]
channels = [
    { name = "telegram", message_share = 0.5 },
    { name = "cli", message_share = 0.3 },
    { name = "discord", message_share = 0.2 },
]

[[checkpoints]]
at_day = 7
assertions = [
    { type = "metric_above", metric = "knowledge_retention", threshold = 0.0 },
]
```

- [ ] **Step 9: Add smoke test for multi-channel**

In `tests/simulation/smoke.rs`:

```rust
#[tokio::test]
async fn run_multi_channel_test() {
    let report = run_scenario(include_str!("scenarios/multi_channel_test.toml")).await;
    assert!(report.summary.total_messages > 0);
    eprintln!(
        "Multi-channel: {} msgs, {:.2}s",
        report.summary.total_messages,
        report.wall_time_secs,
    );
    // Verify messages were distributed across channels
    if let Some(last) = report.metric_timeline.last() {
        if !last.channel_message_distribution.is_empty() {
            eprintln!("  Channel distribution:");
            for (ch, count) in &last.channel_message_distribution {
                eprintln!("    {}: {}", ch, count);
            }
        }
    }
}
```

- [ ] **Step 10: Run tests**

Run: `cargo nextest run -p simulator --no-capture && cargo nextest run --test simulation -E 'test(smoke_test_7_day)' --no-capture`
Expected: All pass. Single-channel scenarios unchanged (empty `channels` vec).

---

### Task 3: Display Metrics + Update SIMULATOR.md

- [ ] **Step 1: Add new metrics to smoke.rs output**

```rust
    eprintln!("  RESILIENCE");
    eprintln!("    Cascade rate:         {:.3}", fm.cascade_rate);
    eprintln!("    Avg cascade depth:    {:.1}", fm.avg_cascade_depth);
```

- [ ] **Step 2: Update SIMULATOR.md structural gaps**

Mark "Error cascades" and "Concurrent sessions" as resolved.

- [ ] **Step 3: Run full tests**

Run: `cargo nextest run -p simulator --no-capture && cargo nextest run --test simulation -E 'test(smoke_test_7_day)' --no-capture`
Expected: All pass.

---

## Verification

```bash
# Unit tests
cargo nextest run -p simulator --no-capture

# Smoke test (single channel, no cascades)
cargo nextest run --test simulation -E 'test(smoke_test_7_day)' --no-capture

# Multi-channel test
cargo nextest run --test simulation -E 'test(run_multi_channel_test)' --no-capture

# To test cascades, use a scenario with:
# error_injection_rate = 0.05
# error_cascade_enabled = true
# error_cascade_multiplier = 3.0
```
