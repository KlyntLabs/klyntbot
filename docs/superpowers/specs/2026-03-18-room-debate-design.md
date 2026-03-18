# Room-Style Squad Debate — Design Spec

## Problem

The current debate system runs all personas in parallel every round. Personas never directly respond to each other — they only see a formatted blackboard summary of prior rounds. This produces responses that talk past each other rather than genuinely engaging. The Jaccard consensus heuristic (Basic mode) is fundamentally broken for natural language (17% even when personas explicitly agree). The system needs to feel like a real brainstorming room.

## Goal

Replace the current parallel-only debate with a **room-style conversation** where personas speak sequentially, directly respond to each other, and an LLM judge steers the discussion toward resolution. Remove Basic mode entirely.

## Architecture

### Four Phases

```
Phase 1 — OPENING (parallel)
  All personas respond simultaneously to get initial positions fast.

Phase 2 — DISCUSSION (sequential, full responses)
  Judge determines speaking order based on who was most challenged.
  Each persona speaks in turn, seeing all prior speakers in this round.
  Full responses (200+ words).

Phase 3+ — TARGETED (sequential, short exchanges)
  Judge gives each persona a specific question/challenge to address.
  Short focused responses (50-150 words).
  Repeats until judge says "final_round" or safety cap (6 rounds).

Phase FINAL — CLOSING (parallel)
  Each persona gives 2-3 sentence final position statement.
  All in parallel (fast).
```

### User Modes

Two modes, controlled by a frontend toggle:

- **Quick**: Single-pass parallel fan-out + synthesis. No rounds, no judge. For simple questions where debate is overkill.
- **Debate**: Full room-style conversation with all four phases.

### The Judge

A lightweight LLM call after each round that returns structured JSON:

```json
{
  "consensus_score": 72,
  "decision": "continue",
  "speaking_order": ["builtin-skeptic", "builtin-deep-analyst", "builtin-strategist"],
  "challenges": {
    "builtin-skeptic": "Analyst provided evidence for X. Does this change your position?",
    "builtin-deep-analyst": "Skeptic raised concern about Y. How do you address it?",
    "builtin-strategist": "Both agreed on Z but disagreed on implementation. Propose a resolution."
  },
  "reasoning": "Skeptic was most challenged by Analyst's evidence — speaks first to defend or revise.",
  "summary": "Personas converging on index funds with some disagreement on allocation.",
  "quick_synthesis_hint": null
}
```

Fields:
- `consensus_score`: 0-100 (0 = total disagreement, 100 = full consensus)
- `decision`: `"continue"` | `"final_round"` | `"stop"`
  - `continue` → next round with targeted challenges
  - `final_round` → trigger final position statements, then synthesize
  - `stop` → consensus reached, skip final round, go straight to synthesis. Emits `ConsensusReached` with `summary` from judge.
- `speaking_order`: persona IDs ordered by who should speak first (most challenged first)
- `challenges`: per-persona question/challenge for the next round (only used in targeted phase)
- `reasoning`: human-readable explanation of the judge's decision (displayed in frontend)
- `summary`: human-readable summary of the debate state. Used as `ConsensusReached.summary` when decision is `"stop"` or `"final_round"`.
- `quick_synthesis_hint`: when consensus > 85, a 1-2 sentence hint the synthesis LLM can use directly, skipping the final round

### Judge Context: Cognitive Memory Integration

Before calling the judge, inject 3-5 most relevant semantic facts from the squad's scope chain. The judge function accepts an `Option<&cognitive::UnifiedMemoryService>` parameter (cognitive crate is Layer 3, agent crate is Layer 5 — dependency is valid).

```rust
// In call_judge(), debate.rs:
async fn call_judge(
    provider: &DynProvider,
    responses: &[(String, String)],
    blackboard: &[BlackboardEntry],
    params: &ChatParams,
    memory_service: Option<&cognitive::UnifiedMemoryService>,
    user_message: &str,
    squad_id: &str,
) -> JudgeDecision { ... }
```

Memory retrieval:
```rust
if let Some(memory) = memory_service {
    let scope_chain = vec![
        ("system".to_string(), None),
        ("squad".to_string(), Some(squad_id.to_string())),
    ];
    let memories = memory.retrieve_scoped(user_message, 5, scope_chain).await;
    // Append to judge prompt as "--- Relevant Context ---"
}
```

### Error Handling: Mid-Sequential Persona Failure

If a persona's LLM call fails during a sequential round:

1. **Skip the failed persona for this round** — do NOT write an empty entry to the blackboard.
2. Log a warning: `warn!(persona = %name, round, "Persona LLM call failed, skipping this round")`
3. Emit `PersonaPerspective` with `content: "[Persona unavailable this round]"` so the frontend shows a placeholder.
4. Continue with the next persona in the speaking order.
5. The judge will see the missing persona and may re-prioritize them in the next round's speaking order.

This avoids corrupting the blackboard with empty entries and keeps the debate flowing.

### Round Flow

#### Round 1 (Opening — parallel)
1. Emit `DebateRoundStarted { phase: "opening" }`
2. Fan out all personas in parallel via a dedicated `fan_out_opening()` helper in `debate.rs` (NOT `squad::fan_out_personas`, since that doesn't support the `challenge: None` field on `PersonaPerspective`)
3. Each emits `PersonaPerspective { challenge: None }` event
4. Write all to blackboard with `entry_type: "opening"`
5. Call judge → get decision + speaking order
6. Emit `DebateJudgeDecision` event (BEFORE acting on decision)
7. Store judge decision as blackboard entry (`entry_type: "judge_decision"`, `persona_id: "judge"`, `persona_name: "Judge"`)
8. Emit `DebateRoundCompleted`

#### Round 2 (Discussion — sequential full)
1. Emit `DebateRoundStarted { phase: "discussion" }`
2. For each persona in judge's speaking order:
   a. Build prompt with FULL conversation history (all prior rounds + all prior speakers this round)
   b. Prompt includes: "You are in a live discussion. Respond directly to what others have said."
   c. Call LLM → get response (on failure: skip persona, see Error Handling above)
   d. Emit `PersonaPerspective { challenge: None }` immediately (user sees responses appear one by one)
   e. Write to blackboard with `entry_type: "discussion"`
3. Call judge → get decision + challenges
4. Emit `DebateJudgeDecision` event (BEFORE acting on decision)
5. Store judge decision as blackboard entry
6. Emit `DebateRoundCompleted`

#### Round 3+ (Targeted — sequential short)
1. Emit `DebateRoundStarted { phase: "targeted" }`
2. For each persona in judge's speaking order:
   a. Build prompt with conversation history + judge's specific challenge for this persona
   b. Prompt includes: "Address this specific point concisely (50-150 words): {challenge}"
   c. Call LLM → get response (on failure: skip persona)
   d. Emit `PersonaPerspective { challenge: Some("...") }` (includes the challenge text)
   e. Write to blackboard with `entry_type: "targeted"`
3. Call judge → get decision
4. Emit `DebateJudgeDecision` event (BEFORE acting on decision)
5. Store judge decision as blackboard entry
6. If `quick_synthesis_hint` is set (consensus > 85): skip final round, emit `ConsensusReached` with judge's `summary`, go to synthesis
7. Emit `DebateRoundCompleted`

#### Final Round (Closing — parallel)
Triggered when judge returns `decision: "final_round"` or safety cap hit.
1. Emit `DebateRoundStarted { phase: "final" }`
2. Fan out all personas in parallel via `fan_out_final()` helper with prompt:
   "Given the full discussion above, summarize your final position in 2-3 sentences."
   (This helper builds the prompt from blackboard history + the final-position instruction)
3. Each emits `PersonaPerspective { challenge: None }`
4. Write to blackboard with `entry_type: "final_position"`
5. Emit `DebateRoundCompleted`
6. Emit `ConsensusReached` with judge's last `summary`
7. → Synthesis

### Safety Cap

Maximum 6 rounds total (including opening and final). If the judge keeps returning `"continue"` past round 5, force `"final_round"` on round 6.

## Events

### Modified Events

```rust
DebateRoundStarted {
    round: u32,
    total_rounds: u32,           // safety cap (6)
    phase: String,               // "opening" | "discussion" | "targeted" | "final"
}

PersonaPerspective {
    persona_id: String,
    persona_name: String,
    persona_icon: String,
    persona_role: String,
    content: String,
    challenge: Option<String>,   // NEW: what the judge asked this persona to address (None for opening/discussion/final)
}
```

### New Events

```rust
/// Emitted after the judge evaluates each round.
DebateJudgeDecision {
    round: u32,
    consensus_score: f64,
    decision: String,            // "continue" | "final_round" | "stop"
    speaking_order: Vec<String>, // persona IDs
    reasoning: String,           // human-readable explanation
}
```

Event name constant: `AGENT_DEBATE_JUDGE_DECISION = "agent:debate_judge_decision"`

### ConsensusReached — source of `summary`

When `decision == "stop"` or `"final_round"`: `ConsensusReached.summary` is populated from the judge's `summary` field. This ensures the frontend always has a meaningful summary string.

### Removed

- `DebateMode` enum — no more Basic/Deep distinction
- `estimate_consensus()` (Jaccard function) — dead code
- `llm_judge_consensus()` — replaced by the structured `call_judge()` that returns the full decision
- `debate_mode` field from `DebateRoundStarted`, `RoutingContext`, `SessionContextInput`
- `DEFAULT_MAX_ROUNDS`, `DEFAULT_CONSENSUS_THRESHOLD` constants — replaced by phase-aware logic

## Blackboard Schema

No schema changes needed. Existing `blackboard_entries` table is sufficient. New `entry_type` values:
- `"opening"` — Round 1 parallel responses
- `"discussion"` — Round 2 full sequential responses
- `"targeted"` — Round 3+ short targeted responses
- `"final_position"` — Final round position statements
- `"judge_decision"` — Judge's structured JSON stored as content

Judge decision entries use `persona_id: "judge"` and `persona_name: "Judge"` to distinguish them from persona entries. The `format_for_prompt()` function must be updated to **skip** entries with `entry_type == "judge_decision"` so they don't appear as persona contributions in prompts.

The `references_entry_id` field (already exists, currently unused) can link targeted responses to the specific prior entry they're responding to.

## Frontend

### Toggle

Replace "Basic / Deep Debate" button with:
- **Quick** (default for simple messages): single lightning icon, muted style
- **Debate**: discussion icon, purple highlight when active

### Debate View Rendering

Render phases differently:

- **Opening**: side-by-side persona cards (current `PersonaMessageList`)
- **Discussion**: conversation thread layout — responses flow vertically with left-aligned persona avatars, like a chat thread
- **Targeted**: each response prefixed with the judge's challenge in a subtle callout box
- **Final**: compact summary cards, side-by-side

### Judge Annotation (JudgeAnnotation.tsx — NEW)

Between rounds, show a `glass-panel` bubble with:
- Judge's reasoning text (italic, small)
- Consensus score as a colored dot (green > 85, yellow 60-85, red < 60)
- "Force Final Round" button (appears after round 2)

Props:
```typescript
interface JudgeAnnotationProps {
  reasoning: string;
  consensusScore: number;
  round: number;
  onForceFinalize: () => void;
}
```

### "Force Final Round" User Action

The "Force Final Round" button calls a new IPC command `chat_force_final_round` which:
1. Sets a flag on the active debate session (via a new field on `StreamingHandle` or a shared `AtomicBool`)
2. The debate loop checks this flag before starting the next round
3. If set, skips to final round regardless of judge decision

### Confidence Glow

Persona cards get a subtle border glow based on consensus_score:
- `> 85`: green glow (`border-green-500/30`)
- `60-85`: yellow glow (`border-yellow-500/30`)
- `< 60`: red glow (`border-red-400/30`)

(Threshold 85 aligns with `quick_synthesis_hint` trigger for consistency.)

### StreamSnapshot Shape Update

```typescript
interface StreamSnapshot {
  // ... existing fields ...
  debateRounds: DebateRound[];
  currentDebateRound: number | null;
  totalDebateRounds: number | null;
  squadMode: "quick" | "debate" | null;  // replaces debateMode
  consensusReached: boolean;
  consensusSummary: string | null;
  judgeDecisions: JudgeDecisionEntry[];  // NEW
}

interface JudgeDecisionEntry {
  round: number;
  consensusScore: number;
  decision: "continue" | "final_round" | "stop";
  speakingOrder: string[];
  reasoning: string;
}

interface DebateRound {
  round: number;
  phase: "opening" | "discussion" | "targeted" | "final";  // NEW
  personaMessages: PersonaSegment[];
  consensusScore: number | null;
}

interface PersonaSegment {
  personaId: string;
  personaName: string;
  personaIcon?: string;
  personaRole?: string;
  content: string;
  challenge?: string;  // NEW
}
```

## Files Changed

### Rust (backend)
| File | Changes |
|------|---------|
| `crates/agent/src/intent_pipeline/engines/debate.rs` | Rewrite: remove DebateMode enum, remove estimate_consensus/llm_judge_consensus, add `run_room_debate()` with 4-phase flow, add `call_judge()` returning structured JudgeDecision, add `fan_out_opening()`/`fan_out_final()` helpers, add sequential round execution |
| `crates/agent/src/intent_pipeline/engines/squad.rs` | Add `challenge: None` to the `PersonaPerspective` event emission (compile fix for new field) |
| `crates/agent/src/agent_runtime/runtime.rs` | Simplify: remove DebateMode selection logic, remove debate_mode_str param, use "quick" vs "debate" from `ctx.squad_mode`, inject `memory_service` into debate |
| `crates/agent/src/events.rs` | Add `DebateJudgeDecision` event, replace `debate_mode` with `phase` on `DebateRoundStarted`, add `challenge: Option<String>` to `PersonaPerspective` |
| `crates/agent/src/agent_loop/mod.rs` | Replace `debate_mode` param with `squad_mode` on `process_direct_streaming` |
| `crates/desktop-shared/src/events.rs` | Add `AGENT_DEBATE_JUDGE_DECISION` constant, add `DebateJudgeDecisionPayload`, update `DebateRoundStartedPayload` (phase replaces debate_mode), update `PersonaPerspectivePayload` (+challenge) |
| `crates/desktop-shared/src/commands/chat.rs` | Replace `debate_mode` with `squad_mode: Option<String>` ("quick" or "debate") in `SessionContextInput` |
| `crates/app-core/src/handlers/chat/streaming.rs` | Handle new `DebateJudgeDecision` event in SSE relay, pass `phase` instead of `debate_mode` in `DebateRoundStarted` relay, pass `challenge` in `PersonaPerspective` relay |
| `crates/tools-core/src/routing.rs` | Replace `debate_mode` with `squad_mode` on `RoutingContext` |
| `crates/klyntbot-server/src/bridge/registry.rs` | Update RoutingContext field name (debate_mode → squad_mode) |
| `crates/cognitive/src/repos/blackboard.rs` | Update `format_for_prompt()` to skip `entry_type == "judge_decision"` entries |

### TypeScript (frontend)
| File | Changes |
|------|---------|
| `desktop-ui/src/shared/types/chat.ts` | Add `DebateJudgeDecisionPayload`, `JudgeDecisionEntry`, `DebatePhase` type, update `PersonaSegment` (+challenge), update `DebateRound` (+phase), remove DebateMode types |
| `desktop-ui/src/shared/types/index.ts` | Export new types |
| `desktop-ui/src/shared/stores/chatStreamStore.ts` | Add `onDebateJudgeDecision` handler, track phase per round, replace debateMode with squadMode, add judgeDecisions array to StreamSnapshot, add `"agent:debate_judge_decision"` to SSE_AGENT_EVENTS |
| `desktop-ui/src/features/chat/hooks/useAgentStream.ts` | Expose judgeDecisions, squadMode; remove debateMode, totalDebateRounds |
| `desktop-ui/src/features/chat/hooks/useChatSession.ts` | Replace debateMode with squadMode in interface and context payload |
| `desktop-ui/src/features/chat/components/DebateView.tsx` | Phase-aware rendering (thread for discussion, challenge callouts for targeted, cards for opening/final), render JudgeAnnotation between rounds |
| `desktop-ui/src/features/chat/components/DebateRound.tsx` | Phase-aware rendering, show challenge callout for targeted entries |
| `desktop-ui/src/features/chat/components/ConsensusIndicator.tsx` | Confidence glow colors (green > 85, yellow 60-85, red < 60) |
| `desktop-ui/src/features/chat/components/JudgeAnnotation.tsx` | NEW: glass-panel bubble with judge reasoning, consensus dot, "Force Final Round" button |
| `desktop-ui/src/features/chat/pages/ChatPage.tsx` | Replace Basic/Deep toggle with Quick/Debate toggle, pass squadMode, wire "Force Final Round" action |

## Cost Analysis

For a typical 4-round debate with 3 personas:

| Phase | LLM Calls | Token Note | Latency Pattern |
|-------|-----------|------------|-----------------|
| Opening | 3 parallel + 1 judge | Base context each | ~2s (parallel) + ~1s (judge) |
| Discussion | 3 sequential + 1 judge | Growing context: P1 gets base, P2 gets base+P1, P3 gets base+P1+P2 | ~6-8s (serial, growing) + ~1s (judge) |
| Targeted | 3 sequential + 1 judge | Shorter responses but full history | ~3-4s (serial) + ~1s (judge) |
| Final | 3 parallel | Full history + short output | ~2s (parallel) |
| Synthesis | 1 | All final positions | ~2s |
| **Total** | **~15 calls** | **O(n²) within sequential rounds** | **~18-22s** |

Note: Sequential rounds have O(n²) token growth within a round (each persona sees all prior speakers). With 3 personas at ~200 words each, the third speaker's prompt is ~400 words larger than the first's. This is acceptable for 3-5 personas but would be expensive at 10+.

Quick mode: 3 parallel + 1 synthesis = 4 calls, ~3s. Same as current single-pass.
