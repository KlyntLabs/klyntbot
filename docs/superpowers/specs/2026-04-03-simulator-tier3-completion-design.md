# Simulator Tier 3 Completion: Multi-Turn, Adversarial, Cross-Feature

## Goal

Complete the simulator's Tier 3 by adding multi-turn conversation context, adversarial stress testing (user-level, tool-level, provider-level), and cross-feature workflow execution (parallel and sequential tool chaining). Together with the existing SimulatedAgentMode, this makes the simulator a comprehensive end-to-end quality infrastructure that tests the full user-facing experience — from realistic user behavior through AI reasoning to tool execution and error recovery.

## Architecture

Four phases, each building on the prior. All share the same in-memory SQLite database and `AgentHarness` infrastructure from the SimulatedAgentMode implementation. New persona templates, a conversation tracker, an error injector, and provider adversarial mode are added as composable layers. 4 new metrics track the quality of each capability independently.

## Phase 1: Multi-Turn Conversations

### Conversation Tracker

A new `ConversationTracker` struct in `crates/simulator/src/persona/conversation.rs` accumulates `(user_message, agent_response)` pairs per session.

```
ConversationTracker {
    turns: VecDeque<(String, String)>,  // (user, agent) pairs
    max_depth: usize,                    // configurable, default 5
}
```

Methods:
- `new(max_depth: usize) -> Self`
- `record(&mut self, user_msg: &str, agent_response: &str)` — push pair, trim to max_depth
- `history_messages(&self) -> Vec<Message>` — convert turns into alternating `Message::user` / `Message::assistant` for the AgentRuntime
- `last_response(&self) -> Option<&str>` — the agent's most recent response

### History Passing

`AgentHarness::process()` accepts an additional `history: &[Message]` parameter. The harness run loop in `harness.rs` maintains a `ConversationTracker`, records each agent response after processing, and passes the tracker's `history_messages()` to the next `process()` call.

### Persona Backreference

After each agent response, the `PersonaRunner` can generate a follow-up message that references it. A new method:

```
PersonaRunner::generate_followup(
    &mut self,
    agent_response: &str,
    simulated_at: DateTime<Utc>,
) -> Option<AnnotatedMessage>
```

Triggered probabilistically based on `followup_rate` (configurable per phase, default 0.15). Uses new backreference templates:
- "You mentioned {previous_context} — can you expand on that?"
- "Going back to what you said about {topic_from_response}, I have a question"
- "Actually, about {previous_context} — I changed my mind"
- "That's helpful. Now based on that, can you help me with {related_topic}?"
- "Wait, you said {previous_context}? That's not what I expected"

### Key Phrase Extraction

A simple `extract_key_phrase(response: &str) -> Option<String>` utility that pulls a short phrase from the agent's response for template insertion. Logic: take the first sentence (split on `.` / `!` / `?`), truncate to 80 chars. No NLP needed — the templates are designed to work with rough extracts.

### Multi-Turn Coherence Metric

`multi_turn_coherence` — After each followup exchange, score whether the agent's response references or builds on the prior context. Uses embedding similarity: compare the agent's followup response embedding against the combined text of `(previous_response + " " + followup_message)`. Higher similarity = more coherent. Accumulated as sum/count, averaged per epoch.

## Phase 2: Cross-Feature Workflows

### Parallel Multi-Tool Calls

New cross-domain templates in the persona system:
- "Create a task to {action} AND add a note about {topic}"
- "Record this expense of {amount} and start a focus session"
- "Set up a reminder for {action} and create flashcards about {topic}"

The `SimulationProvider::generate_tool_calls()` gains a multi-domain detection layer. When the message contains keywords from 2+ domains (e.g., both "task" AND "note"), it returns a `Vec<ToolCall>` with one call per matched domain. The existing `ExecutionCore` already supports parallel tool execution via `join_all(futures)`.

### Sequential Chained Calls

The `SimulationProvider` gains iteration awareness. It inspects the messages array for `Message::Tool` entries (results from previous tool executions). When found:
- If the previous tool result is from `notes` and the message mentions "task" or "create", generate a `tasks.create` call referencing the note content
- If the previous tool result is from `tasks` and the message mentions "note" or "document", generate a `notes.create` call referencing the task
- If the previous tool result is from `finance` and the message mentions "note" or "record", generate a `notes.create` call summarizing the financial data

This creates realistic 2-step chains: search → act on result.

### Workflow Pattern Tracking

A `WorkflowPattern` enum in `agent_types.rs`:
```rust
pub enum WorkflowPattern {
    Parallel { expected_tools: Vec<String> },
    Sequential { chain: Vec<String> },
}
```

The `AnnotatedMessage` gains an optional `workflow: Option<WorkflowPattern>` field. When a cross-feature template is selected, the persona sets the expected workflow pattern. The harness compares the agent's actual tool calls against this expectation.

### Cross-Feature Chain Success Metric

`cross_feature_chain_success` — For messages with a `WorkflowPattern`, what percentage complete ALL expected tools? Tracked by comparing `agent_result.tool_calls` against `msg.workflow.expected_tools`. Partial completion counts as failure. Accumulated as success_count/total_count, averaged per epoch.

## Phase 3: Adversarial Scenarios

### Layer 1: Ambiguous/Contradictory User Templates

New template arrays:
- `ADVERSARIAL_AMBIGUOUS` (5 templates): "Do the thing with the stuff from last time", "Can you update that thing I mentioned?", "Handle the usual for this week"
- `ADVERSARIAL_CONTRADICTORY` (5 templates): "Create a task... actually delete it... no wait, keep it", "Record $50 expense — no, make it income — actually it's an expense", "Start a focus session, but cancel it, but actually yes start it"
- `ADVERSARIAL_CONFLICTING_FACTS` (4 templates): "Actually I work as a {wrong_profession}" (contradicts known_facts), "I switched to using {wrong_currency} now", "My project is called {wrong_project}"

Selected via `adversarial_rate` on `PhaseConfig` (default 0.0 — opt-in). When triggered, replaces the normal template for that message. The `AnnotatedMessage` gets an `is_adversarial: bool` flag for metric tracking.

### Layer 2: Tool Failure Injection

A new `ErrorInjector` wrapper in `crates/simulator/src/error_injector.rs`. Wraps the `ActionExecutor` and, based on `error_injection_rate` (configurable per phase, default 0.0), returns a realistic error instead of executing:

Error types (randomly selected):
- `StorageError("table locked — concurrent write in progress")`
- `ToolError("entity not found: no matching note for query")`
- `TimeoutError("tool execution timed out after 30s")`
- `ValidationError("invalid argument: amount must be positive")`

The harness tracks which messages had injected tool failures for the `error_recovery_rate` metric.

### Layer 3: Provider-Level Malformation

The `SimulationProvider` gains an adversarial mode. When `provider_error_rate > 0` (configurable, default 0.0), it occasionally returns:
- Wrong tool name: "taks" instead of "tasks" (typo simulation)
- Invalid JSON arguments: `{"action": "list", "broken":}` (parse failure)
- Empty tool call ID: `id: ""`
- Tool not in registry: `name: "nonexistent_tool"`

Each malformation type is equally weighted. This tests the `ExecutionCore`'s error handling robustness.

### Adversarial Metrics

- `adversarial_resilience` — % of adversarial messages (all 3 layers flagged via `is_adversarial`) that produce no breakpoints. Accumulated as resilient_count/adversarial_count.
- `error_recovery_rate` — After a tool failure injection (Layer 2), does the agent produce a meaningful response (non-empty content, no error) on the same or next reactive iteration? Accumulated as recovered_count/injected_count.

### Adversarial Config

New optional fields on `PhaseConfig`:
```rust
pub adversarial_rate: f64,       // default 0.0
pub error_injection_rate: f64,   // default 0.0
pub provider_error_rate: f64,    // default 0.0
```

TOML:
```toml
[persona.phases.power_user]
adversarial_rate = 0.10
error_injection_rate = 0.05
provider_error_rate = 0.02
```

## Phase 4: Metrics & Reporting

### New MetricSnapshot Fields (Tier 6)

| Field | Type | Source |
|-------|------|--------|
| `multi_turn_coherence` | f64 | Embedding similarity avg for followup exchanges |
| `cross_feature_chain_success` | f64 | % of workflows with all tools completed |
| `adversarial_resilience` | f64 | % of adversarial messages without breakpoints |
| `error_recovery_rate` | f64 | % of tool failures with graceful recovery |

### New EpochAccumulator Fields (8 total)

- `multi_turn_coherence_sum: f64`, `multi_turn_coherence_count: u32`
- `cross_feature_success: u32`, `cross_feature_total: u32`
- `adversarial_resilient: u32`, `adversarial_total: u32`
- `error_recovered: u32`, `error_injected: u32`

### New MetricName Variants

```rust
MultiTurnCoherence,
CrossFeatureChainSuccess,
AdversarialResilience,
ErrorRecoveryRate,
```

### SimulationConfig Extensions

```rust
pub multi_turn_history_depth: u32,  // default 5
pub followup_rate: f64,             // default 0.15, global override
```

### AgentSummary Extensions

Add the 4 new metric values plus workflow stats:
```rust
pub multi_turn_coherence: f64,
pub cross_feature_chain_success: f64,
pub adversarial_resilience: f64,
pub error_recovery_rate: f64,
pub total_workflows: u32,
pub parallel_workflows: u32,
pub sequential_workflows: u32,
pub total_adversarial: u32,
pub total_followups: u32,
```

### Smoke Test Extensions

Extend the 12mo test's Agent Path Summary with the new metrics. Add checkpoint assertions for the new metrics in the 12mo scenario TOML.

## File Structure

### New files
- `crates/simulator/src/persona/conversation.rs` — ConversationTracker
- `crates/simulator/src/error_injector.rs` — ErrorInjector wrapping ActionExecutor

### Modified files
- `crates/simulator/src/persona/types.rs` — `workflow` field on AnnotatedMessage, `is_adversarial` flag, adversarial config fields on PhaseConfig
- `crates/simulator/src/persona/templates.rs` — backreference templates, cross-feature templates, adversarial template arrays
- `crates/simulator/src/persona/mod.rs` — `generate_followup()` method, `extract_key_phrase()`, followup_rate logic
- `crates/simulator/src/providers/simulation_provider.rs` — multi-tool detection, sequential chaining, adversarial malformation
- `crates/simulator/src/agent_harness.rs` — history parameter, ConversationTracker integration
- `crates/simulator/src/agent_types.rs` — WorkflowPattern enum, AgentSummary extensions
- `crates/simulator/src/harness.rs` — ConversationTracker in run loop, ErrorInjector, followup generation, new metric accumulation, AgentSummary construction
- `crates/simulator/src/metrics/mod.rs` — 4 new snapshot fields, 8 accumulator fields, computation in snapshot()
- `crates/simulator/src/metrics/ground_truth.rs` — metric value mappings
- `crates/simulator/src/scenario.rs` — new SimulationConfig fields, 4 MetricName variants, adversarial config parsing
- `crates/simulator/src/report.rs` — AgentSummary extensions
- `crates/simulator/src/lib.rs` — export error_injector module
- `tests/simulation/scenarios/software_engineer_12mo.toml` — enable adversarial in power_user phase, add cross-feature topics
- `tests/simulation/smoke.rs` — print new metrics

## Verification

After implementation:
- All existing 85 simulator unit tests pass unchanged
- All 7 simulation integration tests pass
- The 12mo scenario shows non-zero values for all 4 new metrics
- Multi-turn: `multi_turn_coherence > 0.0` (agent sees conversation history)
- Cross-feature: `cross_feature_chain_success > 0.0` (at least some workflows complete)
- Adversarial: `adversarial_resilience > 0.0` (agent handles some adversarial messages)
- Error recovery: `error_recovery_rate > 0.0` (agent recovers from some injected failures)
- Zero clippy warnings, format clean
