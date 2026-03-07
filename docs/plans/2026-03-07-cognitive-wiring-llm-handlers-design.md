# Cognitive Wiring & LLM Handler Implementation

**Date:** 2026-03-07
**Status:** Approved
**Scope:** Wire up cognitive architecture's missing connections + implement LLM-backed handlers

## Problem

The cognitive architecture (design: `2026-03-06-cognitive-architecture-design.md`) has all the infrastructure — traits, repos, background service, coaching components, debug dashboard — but four critical gaps prevent it from functioning:

1. **LLM Extraction** — `ExtractionHandler` trait exists, heuristic impl is crude (copies raw text as fact object)
2. **LLM Consolidation** — `ConsolidationHandler` trait exists, heuristic impl is basic (text match only)
3. **LLM Reflection** — `ReflectionHandler` trait exists, **no implementation at all**, not scheduled
4. **LLM Coaching Reasoner** — `CoachingReasonerHandler` trait exists, heuristic fallback exists, no LLM impl

Additionally, two wiring gaps:
- **Coaching engine not subscribed to DomainEventBus** — `SignalAccumulator` is initialized but nothing feeds events into it
- **Weekly reflection not scheduled** — orchestrator `run_weekly_reflection()` exists but nobody calls it

## Goals

1. Wire the coaching engine to the DomainEventBus via a self-contained `CoachingService`
2. Implement LLM-backed versions of all four handlers with structured JSON output
3. Keep heuristic handlers as fallbacks when LLM is unavailable or fails
4. Add dedicated cognitive model configuration for cost control
5. Schedule weekly reflection via CronService

## Non-Goals

- Changing the cognitive crate's traits, repos, or types (they're correct as-is)
- Changing the debug dashboard frontend (it already consumes the right data)
- Adding new DomainEvent variants (features already emit the right events)
- Building a vector search / embedding pipeline (future work)

## Design

### 1. Configuration: Cognitive Provider

Add `CognitiveConfig` to `crates/config/src/lib.rs`:

```rust
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct CognitiveConfig {
    /// Model for background cognitive tasks. Falls back to agents.defaults.model.
    pub model: Option<String>,
    /// Provider override. Falls back to agents.defaults.provider.
    pub provider: Option<String>,
    /// Temperature for cognitive LLM calls (default: 0.2).
    pub temperature: Option<f64>,
    /// Max tokens per cognitive call (default: 1024).
    pub max_tokens: Option<u32>,
}
```

Config JSON example:
```json
{
  "cognitive": {
    "model": "gpt-4o-mini",
    "temperature": 0.2
  }
}
```

New function in `crates/providers/src/lib.rs`:
```rust
pub fn create_cognitive_provider(config: &Config) -> Result<Option<DynProvider>>
```
Returns `Some(provider)` if cognitive model is configured (or main provider exists), `None` if no API keys are configured. The caller uses this to decide between LLM and heuristic handlers.

### 2. LLM Handler Implementations

All four handlers live in `crates/agent/src/cognitive_handlers.rs` alongside existing heuristics. Each takes `DynProvider` and pre-built `ChatParams`.

#### LlmExtractionHandler

```rust
pub struct LlmExtractionHandler {
    provider: DynProvider,
    params: ChatParams,
    fallback: HeuristicExtractionHandler,
}
```

- **Prompt:** System prompt defines SPO extraction task with domain vocabulary. User message contains the observation text, domain, source event, and importance.
- **Output schema:** `{ "facts": [{ "domain", "subject", "predicate", "object", "confidence", "source" }] }`
- **Fallback:** On LLM error, delegates to `HeuristicExtractionHandler`. Logs failure.

#### LlmConsolidationHandler

```rust
pub struct LlmConsolidationHandler {
    provider: DynProvider,
    params: ChatParams,
    fallback: HeuristicConsolidationHandler,
}
```

- **Prompt:** System prompt explains Mem0 ADD/UPDATE/DELETE/NOOP framework. User message contains candidate fact + existing similar facts.
- **Output schema:** `{ "action": "add|update|delete|noop", "target_id": "...", "reasoning": "...", "confidence": 0.9 }`
- **Fallback:** `HeuristicConsolidationHandler`.

#### LlmReflectionHandler

```rust
pub struct LlmReflectionHandler {
    provider: DynProvider,
    params: ChatParams, // higher max_tokens: 2048
}
```

- **Prompt:** System prompt asks for cross-domain pattern synthesis from episodic memories, user model, and procedural rules over a time period.
- **Output schema:** `{ "fact_updates": [...], "rule_updates": [...], "summary": "..." }`
- **Fallback:** New `HeuristicReflectionHandler` — generates statistical summary (fact counts, rule activations), returns empty updates, stores reflection as episodic memory.

#### LlmCoachingReasonerHandler

```rust
pub struct LlmCoachingReasonerHandler {
    provider: DynProvider,
    params: ChatParams,
    fallback_fn: fn(&ReasonerInput) -> CoachingDecision, // heuristic_reason
}
```

- **Prompt:** System prompt defines coaching philosophy (helpful, not intrusive, respects user preferences). User message contains `ReasonerInput` (situation, trigger, patterns, memories, recent interventions).
- **Output schema:** `{ "should_intervene": bool, "confidence": f64, "message": "...", "intervention_type": "...", "reasoning": "...", "observations": [...] }`
- **Fallback:** Existing `heuristic_reason()` function.

#### Handler Selection Pattern

```rust
let extraction: Arc<dyn ExtractionHandler> = match cognitive_provider {
    Some(ref p) => Arc::new(LlmExtractionHandler::new(p.clone(), params.clone())),
    None => Arc::new(HeuristicExtractionHandler),
};
```

### 3. CoachingService

New file: `crates/feature-coaching/src/service.rs`

```rust
pub struct CoachingService {
    cancel_token: CancellationToken,
    task_handle: Option<JoinHandle<()>>,
}

impl CoachingService {
    pub fn start(
        event_rx: broadcast::Receiver<DomainEvent>,
        accumulator: Arc<Mutex<SignalAccumulator>>,
        detector: Arc<Mutex<PatternDetector>>,
        router: Arc<Mutex<InterventionRouter>>,
        feedback: Arc<Mutex<FeedbackTracker>>,
        situation: Arc<Mutex<UserSituation>>,
        reasoner: Arc<dyn CoachingReasonerHandler>,
        intervention_tx: mpsc::Sender<DeliveredIntervention>,
        cancel: CancellationToken,
    ) -> Self

    pub async fn stop(&mut self)
}
```

**Event loop:**
1. Receive `DomainEvent` from bus
2. Push into `SignalAccumulator`
3. Incrementally update `UserSituation` from signal type
4. Evaluate trigger conditions against situation
5. For each fired trigger: record in `PatternDetector`, detect patterns, call reasoner, route via `InterventionRouter`, record in `FeedbackTracker`
6. Send `DeliveredIntervention` through `intervention_tx` channel

**Decoupling:** The service sends interventions via `mpsc::Sender` — it doesn't know about Tauri. The consumer in `app_core.rs` emits them as Tauri events.

### 4. Wiring in AppCore

In `AppCore::init()`:

1. Create cognitive provider via `create_cognitive_provider(&config)`
2. Select LLM or heuristic handlers based on provider availability
3. Start `BackgroundConsolidationService` with selected extraction + consolidation handlers (existing, just swap handlers)
4. Start `CoachingService` with selected coaching reasoner + shared coaching state
5. Spawn intervention forwarder (receives from `intervention_rx`, emits Tauri events)
6. Schedule weekly reflection via `CronService` with `CronSchedule::Cron { expr: "0 9 * * 1" }`

### 5. AgentLoopBuilder Changes

Add `.with_cognitive_provider(provider: Option<DynProvider>)` method. The builder uses it to select LLM vs heuristic handlers when creating `BackgroundConsolidationService`.

### 6. Dev-API Parity

`crates/dev-api/src/main.rs` gets the same cognitive provider creation, so LLM handlers work in browser dev mode. Falls back to heuristics if no API key configured.

### 7. Reflection Scheduling

Weekly reflection triggered by `CronService` callback:
- Load episodic memories from past 7 days
- Load current user model + procedural rules
- Call `ReflectionHandler` (LLM or heuristic)
- Consolidate fact updates via `ConsolidationHandler`
- Apply rule updates
- Store reflection as episodic memory

The cron job ID is `cognitive_weekly_reflection`. Schedule: Monday 9am in user's configured timezone.

## Files Changed/Created

| File | Action | What |
|------|--------|------|
| `crates/config/src/lib.rs` | Modify | Add `CognitiveConfig` struct |
| `crates/providers/src/lib.rs` | Modify | Add `create_cognitive_provider()` |
| `crates/agent/src/cognitive_handlers.rs` | Modify | Add 4 LLM handlers + `HeuristicReflectionHandler` |
| `crates/feature-coaching/src/service.rs` | Create | `CoachingService` with full pipeline loop |
| `crates/feature-coaching/src/lib.rs` | Modify | Export `CoachingService` |
| `crates/agent/src/agent_loop/builder.rs` | Modify | Wire cognitive provider, handler selection |
| `crates/desktop/src/app_core.rs` | Modify | Start `CoachingService`, schedule reflection, forward interventions |
| `crates/dev-api/src/main.rs` | Modify | Cognitive provider wiring for browser dev mode |

## What Does NOT Change

- `crates/cognitive/src/` — all traits, repos, background service, types unchanged
- `crates/feature-coaching/src/` — existing accumulator, detector, router, feedback, reasoner types unchanged
- `desktop-ui/` — no frontend changes, dashboard already consumes the right events/queries
- Feature crates that emit DomainEvents — no changes

## Testing Strategy

**Unit tests (per handler):**
- Each LLM handler tested with `MockProvider` returning pre-defined JSON
- Verify prompt construction, JSON schema parsing, fallback on LLM error
- `CoachingService` tested with mock event bus — push events, verify trigger → intervention flow

**Integration tests:**
- `BackgroundConsolidationService` + LLM handlers + mock provider: event → fact storage
- `CoachingService` end-to-end: push `DistractionDetected` events → verify intervention delivered
- Reflection: populate episodic memories → run reflection → verify facts/rules consolidated

All tests use in-memory SQLite and mock providers. No real LLM calls in CI.

## Key Design Decisions

1. **Dedicated cognitive provider** — cost control, cheap model for background tasks
2. **`CoachingService` as self-contained struct** — follows `BackgroundConsolidationService` pattern, keeps `app_core.rs` clean
3. **Heuristic fallbacks on every handler** — graceful degradation, works without API keys
4. **`ResponseFormat::JsonSchema`** — strict structured output, no fragile regex parsing
5. **`mpsc` channel for intervention delivery** — decouples coaching from Tauri/UI layer
6. **Cron-scheduled reflection** — automated weekly cycle, no manual trigger needed

## References

- `docs/plans/2026-03-06-cognitive-architecture-design.md` — parent architecture design
- `docs/plans/2026-03-07-cognitive-debug-dashboard-plan.md` — debug dashboard (completed)
- `crates/cognitive/src/` — trait definitions and orchestrators
- `crates/agent/src/cognitive_handlers.rs` — existing heuristic implementations
