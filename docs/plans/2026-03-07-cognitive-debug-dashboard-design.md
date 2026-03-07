# Cognitive Debug Dashboard

**Date:** 2026-03-07
**Status:** Approved
**Scope:** Desktop UI — full debug/inspection dashboard for the cognitive architecture

## Problem

The cognitive architecture (memory system + coaching engine) is fully implemented in Rust but invisible to the developer. There's no way to see what the AI knows about the user, what events are flowing, what facts are being extracted/consolidated, what coaching decisions are being made, or which components are wired vs stub. Debugging requires reading logs or writing ad-hoc queries.

## Goals

1. Full visibility into every layer of the cognitive architecture from the desktop UI
2. Interactive controls: add/edit/delete facts and rules, trigger reflection/compaction, reset coaching state
3. Live event stream showing DomainEvents with salience classification in real-time
4. Implementation completeness matrix showing what's wired, what's stub, what needs work
5. Works in both Tauri and dev-api (browser) modes

## Non-Goals

- End-user-facing feature (this is a developer debug tool)
- Mobile or responsive layout optimization
- Persisting dashboard preferences across sessions

## Architecture

### Navigation

Top-level route `/debug` with its own sidebar entry (bug/terminal icon). 5 tabbed sections:

```
/debug
  Tab: Memory | Coaching | Events | Pipeline | System
```

### Data Flow

```
[cognitive crate repos] ──> [Tauri commands] ──> [useQuery hooks] ──> [React tabs]
[DomainEventBus]        ──> [Tauri emit]     ──> [useEvent hook]  ──> [Events tab live stream]
[coaching in-memory]    ──> [Arc<Mutex<>>]   ──> [Tauri commands] ──> [Coaching tab]
```

### Backend Pattern

Follow the established pattern: DTOs in `desktop-shared` -> commands in `desktop/src/commands/cognitive.rs` -> register in `main.rs` -> mirror in `dev-api`.

Coaching engine state (SignalAccumulator, PatternDetector, FeedbackTracker, InterventionRouter) is currently not on `AppCore`. Must be added as `Option<Arc<Mutex<CoachingEngine>>>` on `AppCore`, similar to `focus_manager`.

Live events: the `BackgroundConsolidationService` already subscribes to the `DomainEventBus`. Add a second subscriber that forwards events as Tauri events (`cognitive:domain_event`) with salience verdict attached.

### Frontend Pattern

- Route: `/debug` in `App.tsx` router, lazy-loaded
- Page component: `desktop-ui/src/components/views/DebugDashboard.tsx`
- Tab components: `desktop-ui/src/components/debug/` directory
- Data fetching: existing `useQuery` hook with command names like `cognitive_facts_list`, `coaching_situation`, etc.
- Live events: existing `useEvent` hook listening on `cognitive:domain_event`
- Mutations: existing `useMutation` hook for actions (add fact, run compaction, etc.)

## Tab Designs

### Tab 1: Memory

**Top: UserModel Summary**
- 6 small cards (Identity, Energy, Work, Finance, Learning, Preferences)
- Each shows fact count + top 2-3 facts as `predicate = object`
- Click card filters the facts table to that domain

**Semantic Facts Table**
- Columns: Domain, Subject, Predicate, Object, Confidence (bar), Stability (bar), Source, Retrievability (computed), Access Count, Valid From, Status
- Inline editing: click Object to edit, confidence slider
- Row actions: Delete (supersede), Archive
- Toolbar: Add Fact button, domain filter, sort selector
- Faded rows for retrievability < 0.3

**Episodic Memories**
- Chronological list with importance badges, expandable content
- Domain filter, date range picker

**Procedural Rules**
- Table: Domain, Rule Text, Confidence, Signal Count, Active toggle
- Inline edit, Add Rule button

**Stats Bar**
- Total active facts / archived / episodic / rules counts
- Last compaction result
- "Run Compaction" button

### Tab 2: Coaching

**Top: UserSituation Gauges**
- 6 arc gauges: Energy, Focus, Deadline Pressure, Distraction Risk, Coaching Receptivity, Hours Active
- Task avoidance indicator badge
- "Last computed" timestamp + "Recompute" button

**Left Column: Signals & Patterns**

Signal Accumulator:
- Window size indicator (e.g., "14 signals in 30min window")
- Recent signals list: event type, timestamp, metadata
- Trigger conditions table: name, cooldown remaining, last fired
- "Evaluate Triggers" button

Detected Patterns:
- Cards: name, confidence bar, signal count, description, domain badge

**Right Column: Interventions & Feedback**

Recent Interventions:
- List: type badge, message, trigger name, timestamp
- Rate limit status: "2/3 hourly, 5/10 daily"
- Per-trigger dismissal counts and cooldown status

Feedback Tracker:
- Strategy effectiveness table: type, times used, acceptance rate, effectiveness, behavioral +/-
- Receptivity adjustment indicator
- "Reset Dismissals" button per trigger

**Controls:** Coaching intensity selector, "Reset All Dismissals", "Clear Signal Window"

### Tab 3: Events

**Filter Bar**
- Salience toggles: Extract (green) / Accumulate (yellow) / Discard (gray)
- Domain filter, event type multi-select
- Pause/Resume, Clear

**Live Event Stream**
- Reverse-chronological, live-updating via Tauri events
- Each row: relative timestamp, event type badge, salience badge, domain badge, expandable payload
- Extract events have accent left border
- Max 200 events in view

**Accumulation Buffers (collapsible)**
- Table: event type, count, distinct days, promoted status
- Progress bar toward promotion threshold (5 events / 3 days)

### Tab 4: Pipeline

**Extraction Log**
- Recent extractions: timestamp, source observation, extracted facts count, facts preview
- Expandable for full content
- Handler type badge ("Heuristic" / "LLM")

**Consolidation Log**
- Recent ops: timestamp, operation badge (ADD/UPDATE/DELETE/NOOP), fact summary, old vs new
- Running counters since startup

**Reflection History**
- Past reflections: date range, summary, fact/rule update counts
- Expandable detail
- "Run Reflection Now" button + next scheduled timestamp

**Background Service Status**
- BackgroundConsolidation: status, events processed, errors, last timestamps
- Accumulated buffer sizes

### Tab 5: System

**Service Health Cards**
- DomainEvent Bus: subscriber count, total published
- Background Consolidation: Running/Stopped, uptime, events processed
- Coaching Engine: active triggers, interventions today, feedback pending
- Memory System: active facts, episodic count, rules count, archive size

**Implementation Completeness Matrix**
- Table with columns: Component, Status (Wired/Built/Stub), Handler Type, Notes
- Color coded: green = wired, yellow = built not wired, gray = stub

Components tracked:
- DomainEvent Bus, Salience Filter, Extraction, Consolidation, FSRS Decay, Compaction, Reflection, Context Source, UserSituation, Signal Accumulator, Pattern Detector, Coaching Reasoner, Intervention Router, Feedback Tracker, LLM Extraction, LLM Consolidation, LLM Reflection, LLM Coaching Reasoner

**Configuration**
- Handler types display
- Editable thresholds: accumulate promotion (5/3), compaction (90 days), max facts (10K), coaching rate limits (3/hr, 10/day)
- Save button

## Tauri Commands Required

### Read Commands
- `cognitive_user_model()` -> UserModel with fact counts per domain
- `cognitive_facts_list(domain?, limit?)` -> Vec<SemanticFactResponse>
- `cognitive_fact_get(id)` -> SemanticFactResponse
- `cognitive_episodic_list(domain?, start?, end?, limit?)` -> Vec<EpisodicMemoryResponse>
- `cognitive_rules_list(domain?)` -> Vec<ProceduralRuleResponse>
- `cognitive_memory_stats()` -> MemoryStatsResponse (counts, last compaction)
- `cognitive_retrieval_score(domain, limit)` -> Vec<ScoredFactResponse> (with computed retrievability)
- `coaching_situation()` -> UserSituationResponse
- `coaching_signals()` -> SignalWindowResponse (signals, trigger states)
- `coaching_patterns()` -> Vec<DetectedPatternResponse>
- `coaching_interventions()` -> Vec<DeliveredInterventionResponse>
- `coaching_feedback_stats()` -> Vec<StrategyFeedbackResponse>
- `coaching_router_status()` -> RouterStatusResponse (rate limits, dismissals)
- `cognitive_pipeline_status()` -> PipelineStatusResponse (extraction/consolidation logs, service health)
- `cognitive_system_status()` -> SystemStatusResponse (implementation matrix, config, service health)

### Write Commands
- `cognitive_fact_create(params)` -> SemanticFactResponse
- `cognitive_fact_update(id, params)` -> SemanticFactResponse
- `cognitive_fact_delete(id)` -> bool
- `cognitive_rule_create(params)` -> ProceduralRuleResponse
- `cognitive_rule_update(id, params)` -> ProceduralRuleResponse
- `cognitive_rule_toggle(id)` -> ProceduralRuleResponse
- `cognitive_run_compaction()` -> CompactionResultResponse
- `cognitive_run_reflection()` -> ReflectionResultResponse
- `coaching_recompute_situation()` -> UserSituationResponse
- `coaching_evaluate_triggers()` -> Vec<TriggerFiredResponse>
- `coaching_reset_dismissals(trigger_name?)` -> bool
- `coaching_clear_signals()` -> bool
- `coaching_update_config(params)` -> CoachingConfigResponse

### Events (Tauri emit)
- `cognitive:domain_event` — payload: { event, salience, timestamp }
- `cognitive:extraction` — payload: { observation, facts_extracted }
- `cognitive:consolidation` — payload: { operation, fact }

## Frontend File Structure

```
desktop-ui/src/components/
  debug/
    DebugDashboard.tsx          -- Main page with tab navigation
    tabs/
      MemoryTab.tsx             -- UserModel + facts + episodic + rules
      CoachingTab.tsx           -- Situation + signals + interventions + feedback
      EventsTab.tsx             -- Live stream + accumulation buffers
      PipelineTab.tsx           -- Extraction + consolidation + reflection logs
      SystemTab.tsx             -- Health + completeness matrix + config
    components/
      FactsTable.tsx            -- Semantic facts table with inline edit
      RulesTable.tsx            -- Procedural rules table
      EpisodicList.tsx          -- Episodic memories list
      SituationGauges.tsx       -- Arc gauges for UserSituation
      EventStream.tsx           -- Live event stream
      AccumulationBuffers.tsx   -- Promotion progress table
      CompletionMatrix.tsx      -- Implementation status matrix
      ArcGauge.tsx              -- Reusable gauge component
      SalienceBadge.tsx         -- Extract/Accumulate/Discard badge
```

## Backend File Structure

```
crates/desktop-shared/src/
  cognitive_commands.rs         -- All cognitive/coaching DTOs

crates/desktop/src/commands/
  cognitive.rs                  -- Read + write Tauri commands

crates/dev-api/src/
  main.rs                      -- Mirror routes in dispatch()
```

## Key Design Decisions

1. **Tauri commands per existing pattern** — no new transport layers, works in both Tauri and dev-api mode
2. **Coaching state on AppCore** — expose in-memory coaching state via `Arc<Mutex<>>`, same as focus_manager
3. **Live events via Tauri emit** — second DomainEventBus subscriber forwards to frontend, no WebSocket needed
4. **Pipeline logs in-memory** — extraction/consolidation logs kept in a bounded ring buffer on the background service, not persisted to SQLite
5. **Implementation matrix hardcoded** — the completeness status is a static mapping in the System tab, updated manually as components get wired
6. **Tabbed layout** — keeps each domain focused, avoids information overload
