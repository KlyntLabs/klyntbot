# Squad Debate Engine Upgrade

**Date:** 2026-03-27
**Status:** Approved
**Scope:** Unified debate engine, active FSRS-5 learning, interaction modes, all gap fixes

## Problem

The squad-based multi-persona system has two parallel execution paths (fan-out and debate) with 10 implementation gaps: dead config fields, no token accounting, silent timeout failures, orphaned FSRS-5 tracking, unbounded blackboard growth, unused relevance scores, bypassed domain selection, missing UI data, inconsistent note perspective generation, and an unused `orchestrator_skill` field. The notes insight system duplicates persona execution logic rather than reusing the chat-facing engine.

## Solution

Unify everything under a single debate engine with a pure-function interface (`DebateConfig` + `DebateContext` -> `DebateResult`). Both chat and notes become thin callers with different config. Add three interaction modes (debate, direct address, squad lead), wire FSRS-5 as an active feedback loop, and close all 10 gaps.

## Architecture: Approach 1 — Pure Function with Rich Result

The debate engine is a stateless computation. Callers assemble input, engine runs, callers handle output. No traits, no state machines.

```
run_debate(config, context, squad, provider, blackboard, accuracy, event_tx, approval_rx, cancel)
  -> DebateResult
```

`approval_rx: Option<oneshot::Receiver<bool>>` — used when budget gate requires user confirmation. Caller creates the channel, sends approval after receiving `BudgetCheck { action: RequiresApproval }` event and getting user input.

Two callers:
- **Chat handler** (`streaming.rs`): builds context from session history, persists synthesis as session messages, records token usage
- **Notes handler** (`insight.rs`): builds context from cognitive enrichment, stores results in `InsightContent`, extracts KnowledgeAtoms

## Section 1: Core Interface

### DebateConfig

```rust
pub struct DebateConfig {
    pub output_mode: OutputMode,
    pub max_rounds: u8,
    pub timeout_seconds: u64,
    pub consensus_threshold: f64,
    pub temperature_override: Option<f32>,
    pub confidence_floor: Option<f32>,
    pub token_budget: Option<TokenBudget>,
    pub accuracy_blend: f32,
    /// When true, respect user overrides from squad editor (pinned personas,
    /// forced speaking order, disabled voices). Default: true.
    pub respect_user_prefs: bool,
}

pub enum OutputMode {
    /// Single merged response (chat)
    Synthesized,
    /// Per-persona sections preserved with ## headers (notes perspectives)
    StructuredPerPersona,
}

pub struct TokenBudget {
    pub remaining_monthly: u64,
    pub daily_squad_cap: Option<u64>,
    pub estimated_tokens_per_round: u64,
}
```

### DebateContext

```rust
pub struct DebateContext {
    pub skill_prompt: String,
    pub conversation_history: Vec<Message>,
    pub user_message: String,
    pub cognitive_context: Option<String>,
    pub domains: Vec<String>,
}
```

### DebateResult

```rust
pub struct DebateResult {
    pub persona_responses: Vec<PersonaResponse>,
    pub synthesis: String,
    pub rounds_completed: u8,
    pub total_rounds_planned: u8,
    pub consensus_reached: bool,
    pub final_consensus_score: f64,
    pub partial_reason: Option<PartialReason>,
    pub token_usage: TokenUsage,
    pub learned_weights_applied: Vec<LearnedWeight>,
    pub accuracy_outcomes: Vec<AccuracyOutcome>,
    pub rounds: Vec<DebateRound>,
}

pub struct PersonaResponse {
    pub persona_id: String,
    pub persona_name: String,
    pub persona_icon: String,
    pub persona_role: String,
    pub content: String,
    pub round: u8,
    pub phase: DebatePhase,
    pub confidence: f64,
    pub effective_confidence: f64,
    /// Only populated on final-round responses after Step 6 consensus scoring.
    /// Earlier rounds have `None`. The f64 is the 0.0-1.0 consensus alignment score.
    pub consensus_alignment: Option<f64>,
}

pub struct DebateRound {
    pub round: u8,
    pub phase: DebatePhase,
    pub responses: Vec<PersonaResponse>,
    pub judge_decision: Option<JudgeDecision>,
    pub persona_order: Vec<String>,
    pub order_reason: String,
}

pub enum PartialReason {
    Timeout { elapsed_seconds: u64 },
    BudgetCap { estimated_cost: u64, remaining: u64 },
    ConsensusEarly { at_round: u8 },
    Cancelled,
}

pub struct LearnedWeight {
    pub persona_id: String,
    pub base_weight: f64,
    pub accuracy_weight: f64,
    pub blended_weight: f64,
    pub domain: String,
    pub debates_used: u32,
}

pub struct AccuracyOutcome {
    pub persona_id: String,
    pub squad_id: String,
    pub domain: String,
    pub consensus_alignment: f64,
    pub fsrs_rating: u8,
}

pub struct TokenUsage {
    pub persona_tokens: u64,
    pub judge_tokens: u64,
    pub synthesis_tokens: u64,
    pub total: u64,
}
```

### DebateEvent (lightweight stream for callers)

```rust
pub enum DebateEvent {
    BudgetCheck { estimated_cost: u64, remaining: u64, action: BudgetAction },
    RoundStarted { round: u8, phase: DebatePhase, persona_order: Vec<String>, order_reason: String },
    PersonaPerspective { persona_id: String, name: String, icon: String, role: String, content: String, round: u8, challenge: Option<String> },
    JudgeDecision { round: u8, consensus_score: f64, speaking_order: Vec<String>, decision: String },
    RoundCompleted { round: u8, phase: DebatePhase },
    ConsensusReached { round: u8, score: f64 },
    DebatePartial { reason: PartialReason, completed_rounds: u8 },
    SynthesisStarted,
    SynthesisChunk { content: String },
    Complete,
}

pub enum BudgetAction {
    Proceed,
    ReducedRounds { from: u8, to: u8 },
    /// Engine pauses and waits on `approval_rx`. Caller sends `true` to proceed
    /// or `false` to cancel. Uses a `tokio::sync::oneshot` channel passed
    /// alongside the event_tx in `run_debate`'s parameters.
    RequiresApproval,
}
// run_debate also accepts: approval_rx: Option<oneshot::Receiver<bool>>
```

### Interaction Modes

```rust
pub enum SquadInteractionMode {
    Debate,
    DirectAddress { persona_id: String },
    LeadResponse,
}

pub enum DefaultInteractionMode {
    Lead,
    Debate,
    Smart,
}

pub struct DirectAddressResult {
    pub response: PersonaResponse,
    pub token_usage: TokenUsage,
    pub context_used: Vec<String>,
}
```

## Section 2: Debate Execution Flow

### Step-by-step

**Step 1 — Budget pre-check:**
- Estimate cost: `personas.len() x max_rounds x avg_tokens`
- If over budget: calculate `reduced_rounds` to fit, emit `BudgetCheck { action: ReducedRounds | RequiresApproval }`
- If `RequiresApproval`: wait for caller response via cancel token
- Adjust `effective_max_rounds` accordingly

**Step 2 — Load learned weights:**
- For each persona: query `persona_accuracy(persona_id, squad_id, domain)`
- Compute `accuracy_score = stability / 10.0` (normalized 0.0-1.0)
- Compute `confidence_in_score = min(total_debates / 20.0, 1.0)` (ramps over 20 debates)
- `effective_blend = config.accuracy_blend * confidence_in_score`
- `blended = (1.0 - effective_blend) * relevance_score + effective_blend * accuracy_score`
- Record `LearnedWeight` for each persona

Ramp-up curve:

| Debates | Effective blend (config=0.7) | Behavior |
|---|---|---|
| 0 | 0.0 | Pure relevance_score |
| 5 | 0.175 | Slight accuracy nudge |
| 10 | 0.35 | Moderate influence |
| 20+ | 0.70 | Full learning applied |

User overrides always win: manual pins keep slot, manual reorder overrides sort, "Reset learning" zeros the accuracy row.

**Step 3 — Build persona system prompts:**
- Base: `context.skill_prompt` (from `orchestrator_skill` via `SkillCatalog`)
- Persona block: name, role, expertise, perspective, tone, questioning_style, cognitive_bias, analysis_frameworks
- Cognitive context (if `Some`)
- User message
- Apply `temperature_override` and `confidence_floor` to `ChatParams`

**Step 4 — Debate loop:**
- Generate `debate_session_key = "debate:{squad_id}:{uuid}"`
- Each round determines phase via `determine_phase(round, max_rounds, last_judge_decision)`

Persona ordering per round:
- Opening: sorted by `blended_weight DESC` (strongest voices frame the debate)
- Discussion/Targeted: judge's `speaking_order`, fallback `blended_weight ASC` (weakest first, corrected by stronger)
- User pins always override their slot

Round execution:
- Opening / Final: `fan_out_parallel()` via `join_all` — all personas simultaneously
- Discussion / Targeted: sequential in persona order — each reads blackboard before speaking
- Each response: `blackboard.insert()`, emit `PersonaPerspective` event
- Low-confidence responses (below `confidence_floor`) flagged but NOT dropped

After each non-final round: judge evaluation (temp=0.1, 500 tokens) -> `JudgeDecision { consensus_score, decision, speaking_order, challenges }`. If `consensus_score >= threshold` or `decision == "stop"/"final_round"`: next phase becomes Final.

Exit conditions: `phase == Final`, `round > effective_max_rounds`, timeout, cancellation.

On timeout/cancel: emit `DebatePartial`, break, proceed to synthesis with partial results.

**Step 5 — Synthesis:**
- Collect last round responses (or all if partial)
- Build synthesis prompt with: all persona responses with attribution, learned weights, partial reason if applicable
- Output mode instruction: `Synthesized` -> "Integrate into single coherent response"; `StructuredPerPersona` -> "Preserve ## {Name} -- {Role} headers"
- Stream synthesis via `provider.chat_stream()`, emit `SynthesisChunk` events
- Accumulate synthesis tokens

**Step 6 — Compute accuracy outcomes (post-synthesis consensus judge):**
- This is a SEPARATE call from the per-round judge in Step 4. The per-round judge evaluates debate flow (consensus score, speaking order, challenges). This post-synthesis judge evaluates each persona's alignment with the FINAL output.
- Lightweight LLM call (temp=0.0, 300 tokens): given the synthesis and each persona's final-round response, rate 0.0-1.0 how much each persona's core position was reflected
- Map to FSRS-5 rating: 0.8+ = 4 "Easy", 0.5-0.8 = 3 "Good", 0.3-0.5 = 2 "Hard", <0.3 = 1 "Again"
- Backfill `consensus_alignment` on final-round `PersonaResponse` entries
- Return `AccuracyOutcome` in `DebateResult` (caller persists)

**Step 7 — Blackboard cleanup:**
- Delete debate-scoped entries: `blackboard.delete_by_session_key(debate_session_key)`
- Debate working memory is fully captured in `DebateResult.rounds`

## Section 3: Caller Integration

### Chat Caller (`streaming.rs`)

1. Extract `squad_id` from `SessionContextInput`, stamp on session via `upsert_session`
2. Load squad: `squad_repo.resolve_squad(squad_id)` (includes `default_interaction_mode`)
3. Detect interaction mode:
   - Scan for group triggers at message start: "everyone", "squad", "team", "all of you" -> `Debate`
   - Scan for persona name mentions with strict matching:
     - `@PersonaName` anywhere in message -> `DirectAddress`
     - `"PersonaName,"` or `"PersonaName:"` at message start (first word) -> `DirectAddress`
     - Full exact name match only (not substring: "skeptical" does NOT match "Skeptic")
   - Fallback: `squad.default_interaction_mode` (Lead / Debate / Smart)
   - Smart: query interaction history, decide based on usage patterns

4a. **Debate path:**
- `DebateConfig`: `output_mode: Synthesized`, `max_rounds: 6`, `timeout_seconds: 120`, `temperature_override: None`
- `DebateContext`: `skill_prompt` from `SkillCatalog.get_skill(orchestrator_skill)`, `conversation_history` from session, `cognitive_context: None`
- Event relay: `DebateEvent` -> `AgentEvent` -> Tauri emit
- Post-process: persist synthesis as session message, store `rounds` as JSON metadata, record `token_usage` via `CostTracker`, persist `accuracy_outcomes`, emit `DomainEvent::SquadDebateCompleted`

4b. **DirectAddress / LeadResponse path:**
- Resolve target persona (by name match or `role_in_squad == "lead"`)
- Build prompt: `skill_prompt` + persona block + conversation history + user message
- `provider.chat_stream()` -> emit content chunks
- Persist response as session message with `persona_id` set
- Append to shared blackboard session (`"session:{session_key}:{squad_id}"`) so future debates see this context
- Record token usage, emit `DomainEvent::SquadInteractionPattern`

### Notes Caller (`insight.rs`)

1. Validate note, compute content hash, check cache (return early if cached)
2. Resolve squad: explicit `squad_id` -> `resolve_squad()`, else `"builtin-squad-general"`, fallback `select_for_note()`
3. `DebateConfig`: `output_mode: StructuredPerPersona`, `max_rounds: 3`, `timeout_seconds: 240`, `temperature_override: Some(0.3)`, `confidence_floor: Some(0.6)`
4. `DebateContext`: `skill_prompt` from `SkillCatalog`, `conversation_history: []`, `user_message: note.title + note.body`, `cognitive_context: Some(InsightService::prepare_context())`
5. Notes always runs full debate (no direct address detection)
6. Event relay: `DebateEvent` -> insight-specific Tauri events (`insight:persona-perspective`, `insight:synthesis-chunk`, `insight:round-started`, `insight:tab-done`)
7. Post-process: store `persona_responses` as `InsightContent.perspectives`, store `rounds` as `debate_transcript` JSON, persist `accuracy_outcomes`, record `token_usage`, spawn atom extraction
8. Regeneration (`note_insight_regenerate_tab "perspectives"`) also uses `run_debate` now — same engine, same quality. Eliminates the single-call `perspectives_prompt` inconsistency.

### Files Deleted

| File/Function | Reason |
|---|---|
| `engines/squad.rs` (entire file) | Fan-out removed, debate-only |
| `squad::fan_out_personas()` | Replaced by debate Opening phase |
| `squad::build_squad_synthesis_prompt()` | Moved into debate engine Step 5 |
| `squad::format_multi_voice()` | Output format now in `OutputMode` enum |
| `insight_prompts::perspectives_prompt()` | Regeneration uses `run_debate` |
| `insight_prompts::single_persona_prompt()` | Replaced by debate persona prompt builder |
| `insight_chat::relay_squad_chat()` | Replaced by chat caller debate/direct path |
| `run_squad_execution()` in `runtime.rs` | Replaced by new routing in chat handler |
| `RoutingContext.squad_mode` | Dead config removed |
| `SessionContextInput.squad_mode` | Dead config removed |

## Section 4: FSRS-5 Active Feedback Loop

### When outcomes are recorded

**After every debate:** graduated consensus scoring via cheap LLM judge pass (temp=0.0). Rates 0.0-1.0 per persona. Mapped to FSRS-5 rating. Returned in `DebateResult.accuracy_outcomes`, caller persists via `accuracy_repo.record_outcome()`.

**After direct/lead responses:** no accuracy recording (no consensus to measure), but thumbs up/down on the response adjusts `relevance_score` (the base weight) immediately.

**Mini-consensus for direct/lead:** after 3-5 direct replies in the same domain, auto-trigger a lightweight comparison against the last full debate transcript for a soft accuracy nudge.

### How accuracy influences behavior

1. **Opening round order:** sorted by `blended_weight DESC` (strongest frame the debate)
2. **Discussion fallback order:** `blended_weight ASC` (weakest first, corrected by stronger)
3. **Judge prompt:** receives blended weights, weighs experienced personas' positions higher
4. **Smart mode:** high-accuracy personas on specific domains suggested for `DirectAddress`

### relevance_score upgrade

`relevance_score` becomes the base weight in the blend formula. Thumbs up/down adjusts by +/-0.1, clamped [0.0, 1.0]. Immediate user feedback (short-term) complements FSRS-5 learning (long-term).

### User override

- Manual pin -> keeps slot regardless of weight
- Manual reorder in squad editor -> overrides blended sort
- "Reset learning" button -> `accuracy_repo.reset()` sets `stability=1.0, difficulty=5.0, total_debates=0`
- Thumbs up/down -> immediate `relevance_score` adjustment

### Transparency

- Show each persona's current reliability score (stability from FSRS-5) and recent consensus alignment in the squad editor and Perspectives tab
- `learned_weights_applied` in `DebateResult` -> UI shows "Skeptic was boosted 22% -- strong track record on finance domains"
- Squad Learning Summary in squad editor: "Squad has learned 3 new patterns this month"
- Per-response consensus alignment badge: "Consensus alignment: 82%"

## Section 5: Blackboard Lifecycle

### Dual-purpose blackboard

**Debate working memory (ephemeral):** scoped to `"debate:{squad_id}:{uuid}"`. Personas read during sequential rounds. Deleted after debate completes (captured in `DebateResult.rounds`).

**Thread context continuity (persistent):** Direct/lead responses write to `"session:{session_key}:{squad_id}"`. Persists for the life of the session so future debates in the same thread see prior exchanges.

### Cleanup strategy

1. **Per-debate (immediate):** `blackboard.delete_by_session_key("debate:...")` after `run_debate` completes
2. **Session lifecycle:** `blackboard.delete_by_session_prefix("session:{key}:")` when session is archived/deleted
3. **Safety-net cron (weekly):** `JOB_BLACKBOARD_CLEANUP` (Sunday 4am UTC) deletes entries older than 30 days

## Section 6: Remaining Gap Fixes

### squad_name/icon in thread listings

LEFT JOIN against `squads` table in `chat_list_sessions_by_project`. Populate `squad_name`, `squad_icon`, `squad_default_mode` on `ChatThreadResponse`. Make squad name/icon clickable to jump to squad editor.

### Smart mode learning

Query `session_messages` for interaction mode counts over last 30 days per squad. Cache in `squads.last_smart_mode` + `last_smart_updated` columns. Detection: <5 interactions -> LeadResponse default; >60% direct -> LeadResponse; >50% debate -> Debate; else LeadResponse.

Visibility: squad editor shows "Smart mode -- currently defaulting to Lead (68% of your interactions are quick replies)". One tap to override or reset.

### orchestrator_skill activation

Load skill instructions from `SkillCatalog.get_skill(squad.orchestrator_skill)` when building `DebateContext.skill_prompt`. Graceful fallback to empty string if skill not found. Add `orchestrator_skill` to `UpdateSquadParams` so it can be changed after creation.

### squad_mode removal

Delete `RoutingContext.squad_mode`, `SessionContextInput.squad_mode`, and the `_squad_mode_str` parameter. Interaction mode is detected from message content + `squad.default_interaction_mode`.

### Mode indicator on responses

Session message metadata includes: `squad_mode` ("debate"/"direct"/"lead"), `squad_id`, `persona_id`, `persona_name`, `debate_rounds`, `partial`, `partial_reason`. Frontend renders appropriate badge.

### Coaching integration hooks

New domain events:
- `SquadDebateCompleted`: squad_id, session_key, rounds_completed, consensus_score, persona_accuracies, was_partial, token_cost, average_consensus_score, top_performer_persona_id
- `SquadInteractionPattern`: squad_id, mode, persona_id, domain_hint

Enables coaching suggestions: budget warnings, persona promotion, squad creation, mode switching.

## Schema Changes

```sql
-- squads table additions
ALTER TABLE squads ADD COLUMN default_interaction_mode TEXT NOT NULL DEFAULT 'lead';
ALTER TABLE squads ADD COLUMN last_smart_mode TEXT;
ALTER TABLE squads ADD COLUMN last_smart_updated TEXT;

-- insight_reviews addition
ALTER TABLE insight_reviews ADD COLUMN debate_transcript JSON;

-- New indexes for hot query paths
CREATE INDEX IF NOT EXISTS idx_persona_accuracy_lookup
  ON persona_accuracy(persona_id, squad_id, domain);
CREATE INDEX IF NOT EXISTS idx_blackboard_session_key
  ON blackboard_entries(session_key);
CREATE INDEX IF NOT EXISTS idx_blackboard_created_at
  ON blackboard_entries(created_at);
```

## Gap Resolution Summary

| # | Gap | Resolution |
|---|---|---|
| 0 | Fan-out exists alongside debate | Deleted -- debate-only engine |
| 1 | `squad_mode` dead config | Removed -- replaced by interaction mode detection |
| 2 | No token accounting | `TokenUsage` in `DebateResult`, persisted via `CostTracker` |
| 3 | 120s timeout -> empty result | Graceful degradation + `PartialReason` + synthesis on partial |
| 4 | PersonaAccuracy orphaned | Active FSRS-5 loop with graduated consensus scoring |
| 5 | Blackboard unbounded | Three-tier cleanup: per-debate, session lifecycle, weekly cron |
| 6 | `relevance_score` unused | Base weight in FSRS-5 blend formula |
| 7 | `builtin-squad-general` always wins | Domain selection as fallback; squads explicitly chosen |
| 8 | `squad_name/icon` None | LEFT JOIN in thread listing |
| 9 | Perspectives tab inconsistency | Regeneration uses `run_debate` |
| 10 | `orchestrator_skill` dead | Loaded from `SkillCatalog`, injected as domain grounding |
