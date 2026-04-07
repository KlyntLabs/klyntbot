# Simulator Missing Signals — 5 Event-Based Metrics

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Capture 5 missing production signals in the simulator: context compression ratio, delegation success, MCP availability, note community health, and debate consensus quality — all by intercepting existing events, no production code changes.

**Architecture:** All 5 signals come from events already emitted by the production system (`AgentEvent` variants for items 1-3, `DomainEvent` variants for items 4-5). The pattern is consistent: extend `AgentResult` or the harness event loop to capture the event, add accumulator counters, compute a metric in `snapshot()`, and wire it into `MetricName`/`get_metric_value()`. Items 1-3 only fire in agent mode (real LLM); items 4-5 fire in heuristic mode too.

**Tech Stack:** Rust, tokio, simulator crate, agent crate (read-only)

---

## File Map

| File | Change |
|---|---|
| `crates/simulator/src/agent_types.rs` | Add compression/delegation/MCP fields to `AgentResult` + `AgentSummary` |
| `crates/simulator/src/agent_harness.rs:346-415` | Capture ContextCompressed, DelegationCompleted, McpStartupComplete in event drain |
| `crates/simulator/src/agent_harness.rs:215-218` | Make context_window configurable from SimulationConfig |
| `crates/simulator/src/scenario.rs` | Add `agent_context_window`, new MetricName variants |
| `crates/simulator/src/metrics/mod.rs` | Add 5 accumulator fields, 5 snapshot fields, compute in `snapshot()` |
| `crates/simulator/src/metrics/ground_truth.rs` | Wire 5 new metrics in `get_metric_value()` |
| `crates/simulator/src/harness.rs` | Count community/debate domain events, pass new AgentResult fields to accumulator |
| `tests/simulation/smoke.rs` | Display new metrics in test output |
| `SIMULATOR.md` | Update missing signals section |

---

### Task 1: Context Compression Ratio — Capture Events + Configurable Context Window

The agent's `MidLoopCompressor` fires when accumulated tokens exceed 70% of the context window (`COMPRESSION_THRESHOLD = 0.70` in `mid_loop_compressor.rs:15`), emitting `AgentEvent::ContextCompressed { before_tokens, after_tokens, iteration }`. The harness event drain at `agent_harness.rs:346-415` currently ignores this event. The simulator hardcodes `context_window: 128_000` at `agent_harness.rs:215`, so compression never triggers (sim uses ~7-9k tokens).

**Files:**
- Modify: `crates/simulator/src/scenario.rs:11-51` (SimulationConfig)
- Modify: `crates/simulator/src/agent_types.rs:31-46` (AgentResult)
- Modify: `crates/simulator/src/agent_harness.rs:215-218,346-415`
- Modify: `crates/simulator/src/metrics/mod.rs`

- [ ] **Step 1: Add `agent_context_window` to SimulationConfig**

In `crates/simulator/src/scenario.rs`, add after `agent_depth_mode`:

```rust
    /// Context window size for agent execution. Lower values trigger
    /// mid-loop compression. Default: 128000.
    #[serde(default = "default_agent_context_window")]
    pub agent_context_window: usize,
```

Add the default function:

```rust
fn default_agent_context_window() -> usize {
    128_000
}
```

Add to `Default for SimulationConfig`:

```rust
            agent_context_window: default_agent_context_window(),
```

- [ ] **Step 2: Add compression fields to AgentResult**

In `crates/simulator/src/agent_types.rs`, add to `AgentResult`:

```rust
    pub context_compressions: u32,
    pub compression_ratio_sum: f64,
```

Add to `AgentSummary`:

```rust
    pub total_context_compressions: u32,
    pub avg_compression_ratio: f64,
```

- [ ] **Step 3: Capture ContextCompressed in event drain**

In `crates/simulator/src/agent_harness.rs`, in the event drain task (the `while let Some(event) = event_rx.recv().await` loop), add a match arm. First, add mutable counters alongside the existing ones (near `let mut tool_calls`, around line 350):

```rust
    let mut context_compressions: u32 = 0;
    let mut compression_ratio_sum: f64 = 0.0;
```

Then add a match arm in the event processing:

```rust
    AgentEvent::ContextCompressed { before_tokens, after_tokens, .. } => {
        context_compressions += 1;
        if before_tokens > 0 {
            compression_ratio_sum += after_tokens as f64 / before_tokens as f64;
        }
    }
```

Populate the new fields in the `AgentResult` construction.

- [ ] **Step 4: Use configurable context_window**

In `crates/simulator/src/agent_harness.rs`, replace the hardcoded `context_window: 128_000` at line ~215:

```rust
    context_window: scenario.simulation.agent_context_window,
```

(The `scenario` is already available via `self.scenario` in the harness constructor.)

- [ ] **Step 5: Add accumulator + snapshot fields**

In `crates/simulator/src/metrics/mod.rs`, add to `EpochAccumulator`:

```rust
    pub context_compressions: u32,
    pub compression_ratio_sum: f64,
```

Add to `MetricSnapshot`:

```rust
    pub context_compression_ratio: f64,
```

In `snapshot()`, compute:

```rust
        let context_compression_ratio = if acc.context_compressions == 0 {
            0.0
        } else {
            acc.compression_ratio_sum / acc.context_compressions as f64
        };
```

Add `context_compression_ratio` to the `MetricSnapshot` construction.

- [ ] **Step 6: Wire MetricName + get_metric_value**

In `scenario.rs`, add to `MetricName`:

```rust
    ContextCompressionRatio,
```

In `ground_truth.rs`, add to `get_metric_value()`:

```rust
        MetricName::ContextCompressionRatio => snapshot.context_compression_ratio,
```

- [ ] **Step 7: Pass AgentResult compression data to accumulator in harness.rs**

In `crates/simulator/src/harness.rs`, in the agent processing section (after `let agent_result = agent.process(...)`) where tool calls and routing are processed, add:

```rust
                    metrics.accumulator_mut().context_compressions += agent_result.context_compressions;
                    metrics.accumulator_mut().compression_ratio_sum += agent_result.compression_ratio_sum;
```

- [ ] **Step 8: Run tests**

Run: `cargo nextest run -p simulator --no-capture`
Expected: All tests pass. Compression metrics will be 0 with 128k window (no compression triggered).

---

### Task 2: Delegation Success Rate — Capture Events

`AgentEvent::DelegationStarted { from_agent, to_agent, query, depth }` and `AgentEvent::DelegationCompleted { from_agent, to_agent, success, duration_ms }` exist at `events.rs:134-152`. The harness ignores these.

**Files:**
- Modify: `crates/simulator/src/agent_types.rs`
- Modify: `crates/simulator/src/agent_harness.rs`
- Modify: `crates/simulator/src/metrics/mod.rs`
- Modify: `crates/simulator/src/scenario.rs`
- Modify: `crates/simulator/src/metrics/ground_truth.rs`
- Modify: `crates/simulator/src/harness.rs`

- [ ] **Step 1: Add delegation fields to AgentResult**

In `crates/simulator/src/agent_types.rs`, add to `AgentResult`:

```rust
    pub delegation_attempts: u32,
    pub delegation_successes: u32,
```

Add to `AgentSummary`:

```rust
    pub total_delegations: u32,
    pub delegation_success_rate: f64,
```

- [ ] **Step 2: Capture delegation events in event drain**

In `crates/simulator/src/agent_harness.rs`, add counters:

```rust
    let mut delegation_attempts: u32 = 0;
    let mut delegation_successes: u32 = 0;
```

Add match arms:

```rust
    AgentEvent::DelegationStarted { .. } => {
        delegation_attempts += 1;
    }
    AgentEvent::DelegationCompleted { success, .. } => {
        if success {
            delegation_successes += 1;
        }
    }
```

Populate in AgentResult.

- [ ] **Step 3: Add accumulator + snapshot fields**

In `metrics/mod.rs`, add to `EpochAccumulator`:

```rust
    pub delegation_attempts: u32,
    pub delegation_successes: u32,
```

Add to `MetricSnapshot`:

```rust
    pub delegation_success_rate: f64,
```

In `snapshot()`:

```rust
        let delegation_success_rate = if acc.delegation_attempts == 0 {
            0.0
        } else {
            acc.delegation_successes as f64 / acc.delegation_attempts as f64
        };
```

- [ ] **Step 4: Wire MetricName + get_metric_value**

In `scenario.rs`:

```rust
    DelegationSuccessRate,
```

In `ground_truth.rs`:

```rust
        MetricName::DelegationSuccessRate => snapshot.delegation_success_rate,
```

- [ ] **Step 5: Pass to accumulator in harness.rs**

After agent result processing:

```rust
                    metrics.accumulator_mut().delegation_attempts += agent_result.delegation_attempts;
                    metrics.accumulator_mut().delegation_successes += agent_result.delegation_successes;
```

- [ ] **Step 6: Run tests**

Run: `cargo nextest run -p simulator --no-capture`
Expected: All pass. Delegation metrics will be 0 (delegation not triggered in current scenarios).

---

### Task 3: MCP Tool Availability — Capture Events

`AgentEvent::McpStartupComplete { ready, failed, skipped }` at `events.rs:169-173`. Only fires in agent mode with real MCP connections.

**Files:** Same pattern as Task 2.

- [ ] **Step 1: Add MCP fields to AgentResult**

```rust
    pub mcp_ready: u32,
    pub mcp_failed: u32,
```

Add to `AgentSummary`:

```rust
    pub mcp_availability: f64,
```

- [ ] **Step 2: Capture in event drain**

```rust
    let mut mcp_ready: u32 = 0;
    let mut mcp_failed: u32 = 0;
```

```rust
    AgentEvent::McpStartupComplete { ready, failed, .. } => {
        mcp_ready += ready as u32;
        mcp_failed += failed as u32;
    }
```

- [ ] **Step 3: Add accumulator + snapshot**

`EpochAccumulator`:

```rust
    pub mcp_ready: u32,
    pub mcp_failed: u32,
```

`MetricSnapshot`:

```rust
    pub mcp_availability: f64,
```

`snapshot()`:

```rust
        let mcp_total = acc.mcp_ready + acc.mcp_failed;
        let mcp_availability = if mcp_total == 0 {
            1.0 // No MCP usage = fully available by default
        } else {
            acc.mcp_ready as f64 / mcp_total as f64
        };
```

- [ ] **Step 4: Wire MetricName**

```rust
    McpAvailability,
```

```rust
        MetricName::McpAvailability => snapshot.mcp_availability,
```

- [ ] **Step 5: Pass to accumulator in harness.rs**

```rust
                    metrics.accumulator_mut().mcp_ready += agent_result.mcp_ready;
                    metrics.accumulator_mut().mcp_failed += agent_result.mcp_failed;
```

- [ ] **Step 6: Run tests**

Run: `cargo nextest run -p simulator --no-capture`
Expected: All pass. MCP metrics default to 1.0 availability.

---

### Task 4: Note Community Health — Domain Event Subscription

`DomainEvent::CommunityDiscovered { community_id, name, member_count }`, `CommunityUpdated { community_id, member_count, stability }`, `CommunityWeakened { community_id, stability }` at `domain_events.rs:504-519`. These fire when the Knowledge Fabric detects note graph clusters. The harness already has a domain event subscriber for coaching events — add community events to it.

**Files:**
- Modify: `crates/simulator/src/harness.rs` (coaching listener task)
- Modify: `crates/simulator/src/metrics/mod.rs`
- Modify: `crates/simulator/src/scenario.rs`
- Modify: `crates/simulator/src/metrics/ground_truth.rs`

- [ ] **Step 1: Add accumulator + snapshot fields**

In `metrics/mod.rs`, add to `EpochAccumulator`:

```rust
    pub communities_discovered: u32,
    pub communities_weakened: u32,
```

Add to `MetricSnapshot`:

```rust
    pub community_churn_rate: f64,
```

In `snapshot()`:

```rust
        let community_churn_rate = if acc.communities_discovered == 0 {
            0.0
        } else {
            acc.communities_weakened as f64 / acc.communities_discovered as f64
        };
```

- [ ] **Step 2: Count community events in harness coaching listener**

In `crates/simulator/src/harness.rs`, in the coaching listener task (the `tokio::spawn` block that subscribes to the domain event bus), add match arms alongside the existing `FocusSessionStarted`, `FocusSessionEnded`, etc.:

```rust
                    DomainEvent::CommunityDiscovered { .. } => {
                        // Tracked via accumulator in main loop
                    }
                    DomainEvent::CommunityWeakened { .. } => {
                        // Tracked via accumulator in main loop
                    }
```

Actually, the coaching listener processes events asynchronously, but the accumulator is owned by the main loop. The simpler approach: count communities at epoch boundary from the DB. The `measure_community_stability()` function in `metrics/system.rs` already queries the `communities` table.

Better approach — add a `count_communities()` query to `metrics/system.rs`:

In `crates/simulator/src/metrics/system.rs`, add:

```rust
pub async fn count_communities(pool: &sqlx::SqlitePool) -> (u32, u32) {
    let total: (i64,) = sqlx::query_as(
        "SELECT COUNT(*) FROM communities"
    )
    .fetch_one(pool)
    .await
    .unwrap_or((0,));

    let weakened: (i64,) = sqlx::query_as(
        "SELECT COUNT(*) FROM communities WHERE stability < 0.3"
    )
    .fetch_one(pool)
    .await
    .unwrap_or((0,));

    (total.0 as u32, weakened.0 as u32)
}
```

Then in `harness.rs`, call it at epoch boundary (alongside `measure_community_stability`):

```rust
let (total_communities, weakened_communities) = crate::metrics::system::count_communities(&self.inner_pool).await;
metrics.accumulator_mut().communities_discovered = total_communities;
metrics.accumulator_mut().communities_weakened = weakened_communities;
```

- [ ] **Step 3: Wire MetricName**

```rust
    CommunityChurnRate,
```

```rust
        MetricName::CommunityChurnRate => snapshot.community_churn_rate,
```

- [ ] **Step 4: Run tests**

Run: `cargo nextest run -p simulator --no-capture`
Expected: All pass.

---

### Task 5: Debate Consensus Quality — Domain Event Subscription

`DomainEvent::SquadDebateCompleted { squad_id, rounds_completed, consensus_score, persona_accuracies, was_partial, token_cost, average_consensus_score, top_performer_persona_id, .. }` at `domain_events.rs:523-533`. Debates fire when the squad system processes ambiguous queries.

**Files:**
- Modify: `crates/simulator/src/metrics/mod.rs`
- Modify: `crates/simulator/src/scenario.rs`
- Modify: `crates/simulator/src/metrics/ground_truth.rs`
- Modify: `crates/simulator/src/harness.rs`

- [ ] **Step 1: Add accumulator + snapshot fields**

In `metrics/mod.rs`, add to `EpochAccumulator`:

```rust
    pub debate_count: u32,
    pub debate_consensus_sum: f64,
    pub debate_token_cost: u64,
```

Add to `MetricSnapshot`:

```rust
    pub debate_avg_consensus: f64,
    pub debate_count: u32,
```

In `snapshot()`:

```rust
        let debate_avg_consensus = if acc.debate_count == 0 {
            0.0
        } else {
            acc.debate_consensus_sum / acc.debate_count as f64
        };
        let debate_count = acc.debate_count;
```

- [ ] **Step 2: Subscribe to debate events in harness coaching listener**

In the coaching listener's match block in `harness.rs`, add:

```rust
                    DomainEvent::SquadDebateCompleted {
                        consensus_score,
                        token_cost,
                        ..
                    } => {
                        // Debate events are rare; track for observability.
                        // Metrics counted at epoch boundary via accumulator.
                        tracing::debug!(consensus_score, token_cost, "Debate completed in sim");
                    }
```

Since the coaching listener can't easily write to the accumulator (different task), count debates from the bus at epoch boundary instead. Add a field to track debate events via a shared counter (like `CoachingCounters`):

Add to `CoachingCounters` (the `AtomicU32` struct in harness.rs):

```rust
    debate_count: AtomicU32,
    debate_consensus_sum_x1000: AtomicU32, // consensus * 1000, summed
    debate_token_cost: AtomicU64,
```

In the coaching listener, when `SquadDebateCompleted` is received:

```rust
                    DomainEvent::SquadDebateCompleted {
                        consensus_score,
                        token_cost,
                        ..
                    } => {
                        coaching_counters.debate_count.fetch_add(1, Ordering::Relaxed);
                        coaching_counters.debate_consensus_sum_x1000.fetch_add(
                            (consensus_score * 1000.0) as u32, Ordering::Relaxed
                        );
                        coaching_counters.debate_token_cost.fetch_add(*token_cost, Ordering::Relaxed);
                    }
```

At epoch boundary, drain into accumulator:

```rust
                let debate_count = coaching_counters.debate_count.swap(0, Ordering::Relaxed);
                let debate_consensus_x1000 = coaching_counters.debate_consensus_sum_x1000.swap(0, Ordering::Relaxed);
                metrics.accumulator_mut().debate_count += debate_count;
                metrics.accumulator_mut().debate_consensus_sum += debate_consensus_x1000 as f64 / 1000.0;
                metrics.accumulator_mut().debate_token_cost += coaching_counters.debate_token_cost.swap(0, Ordering::Relaxed);
```

- [ ] **Step 3: Wire MetricName**

```rust
    DebateAvgConsensus,
```

```rust
        MetricName::DebateAvgConsensus => snapshot.debate_avg_consensus,
```

- [ ] **Step 4: Run tests**

Run: `cargo nextest run -p simulator --no-capture`
Expected: All pass. Debate metrics will be 0 (no debates triggered in current scenarios).

---

### Task 6: Display New Metrics + Update SIMULATOR.md

- [ ] **Step 1: Add new metrics to smoke.rs output**

In `tests/simulation/smoke.rs`, in both the `run_software_engineer_1mo` and `run_agent_validation_1week` test functions, add after the existing "NEW METRICS" block:

```rust
    eprintln!("  SIGNAL COVERAGE");
    eprintln!("    Compression ratio:    {:.3}", fm.context_compression_ratio);
    eprintln!("    Delegation success:   {:.3}", fm.delegation_success_rate);
    eprintln!("    MCP availability:     {:.3}", fm.mcp_availability);
    eprintln!("    Community churn:      {:.3}", fm.community_churn_rate);
    eprintln!("    Debate consensus:     {:.3}", fm.debate_avg_consensus);
```

- [ ] **Step 2: Update SIMULATOR.md**

Move items 6, 11-14 from "Missing Signal Categories" to "Completed Improvements" with checkmarks.

- [ ] **Step 3: Run full test suite**

Run: `cargo nextest run -p simulator --no-capture && cargo nextest run --test simulation -E 'test(smoke_test_7_day)' --no-capture`
Expected: All pass.

---

## Verification

After all tasks:

```bash
cargo nextest run -p simulator --no-capture
cargo nextest run --test simulation -E 'test(smoke_test_7_day)' --no-capture
```

To verify compression triggers, add to a scenario TOML:

```toml
agent_context_window = 16000
```

This will trigger compression when accumulated ReAct tool results exceed ~11k tokens (70% of 16k).
