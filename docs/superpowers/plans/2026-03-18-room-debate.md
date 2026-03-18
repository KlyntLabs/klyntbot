# Room-Style Squad Debate Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Replace the parallel-only debate system with a room-style conversation where personas speak sequentially, directly respond to each other, and an LLM judge steers the discussion.

**Architecture:** Remove `DebateMode` enum and Jaccard heuristic. Rewrite `debate.rs` with 4-phase flow (parallel opening → sequential discussion → sequential targeted → parallel final). Add structured LLM judge that returns JSON with consensus score, speaking order, challenges, and decision. Replace `debate_mode` with `squad_mode` ("quick" vs "debate") throughout the stack.

**Tech Stack:** Rust (agent/cognitive/tools-core/desktop-shared/app-core crates), TypeScript/React (desktop-ui), Tauri IPC, SSE streaming.

**Spec:** `docs/superpowers/specs/2026-03-18-room-debate-design.md`

**Deferred to Phase 3.1:**
- `chat_force_final_round` IPC command + `AtomicBool` flag on `StreamingHandle` (the "Force Final Round" button renders but is non-functional until this is built)
- Cognitive memory injection into `call_judge()` via `UnifiedMemoryService` (judge works without user context; memory integration is an accuracy enhancement)

---

## File Map

### New Files
| File | Responsibility |
|------|---------------|
| `desktop-ui/src/features/chat/components/JudgeAnnotation.tsx` | Glass-panel bubble showing judge reasoning, consensus dot, "Force Final Round" button |

### Modified Files
| File | Changes |
|------|---------|
| `crates/agent/src/intent_pipeline/engines/debate.rs` | Rewrite: remove DebateMode/Jaccard/llm_judge, add `JudgeDecision` struct, `call_judge()`, `run_room_debate()` with 4-phase flow, `fan_out_opening()`/`fan_out_final()`, sequential round execution |
| `crates/agent/src/intent_pipeline/engines/squad.rs:112-119` | Add `challenge: None` to `PersonaPerspective` event (compile fix) |
| `crates/agent/src/agent_runtime/runtime.rs:648-697` | Replace DebateMode logic with squad_mode ("quick"/"debate"), remove debate_mode_str param, pass memory_service |
| `crates/agent/src/events.rs:203-252` | Replace `debate_mode` with `phase` on `DebateRoundStarted`, add `challenge` to `PersonaPerspective`, add `DebateJudgeDecision` event |
| `crates/agent/src/agent_loop/mod.rs:706-734` | Replace `debate_mode` param with `squad_mode` on `process_direct_streaming` |
| `crates/desktop-shared/src/events.rs:52-392` | Add `AGENT_DEBATE_JUDGE_DECISION` constant + `DebateJudgeDecisionPayload`, update `DebateRoundStartedPayload` (phase), update `PersonaPerspectivePayload` (+challenge) |
| `crates/desktop-shared/src/commands/chat.rs:52-60` | Replace `debate_mode` with `squad_mode` in `SessionContextInput` |
| `crates/app-core/src/handlers/chat/streaming.rs:847-910` | Handle `DebateJudgeDecision` relay, update `DebateRoundStarted` relay (phase), update `PersonaPerspective` relay (+challenge) |
| `crates/tools-core/src/routing.rs:59-133` | Replace `debate_mode` with `squad_mode` on `RoutingContext` + all constructors |
| `crates/klyntbot-server/src/bridge/registry.rs:68-78` | Update field name |
| `crates/cognitive/src/repos/blackboard.rs:95-115` | Skip `judge_decision` entries in `format_for_prompt()` |
| `crates/agent/src/agent_loop/refactor_tests.rs` | Confirm compilation after rename (call sites already pass `None`) |
| `tests/e2e/agent_loop.rs` | Update `process_direct_streaming` call sites |
| `desktop-ui/src/shared/types/chat.ts` | Add `DebateJudgeDecisionPayload`, `JudgeDecisionEntry`, `DebatePhase`, update `PersonaSegment` (+challenge), update `DebateRound` (+phase) |
| `desktop-ui/src/shared/types/index.ts` | Export new types |
| `desktop-ui/src/shared/stores/chatStreamStore.ts` | Add judge decision handler, replace debateMode with squadMode, add judgeDecisions array, add phase to DebateRound |
| `desktop-ui/src/features/chat/hooks/useAgentStream.ts` | Expose judgeDecisions, squadMode; remove debateMode |
| `desktop-ui/src/features/chat/hooks/useChatSession.ts` | Replace debateMode with squadMode |
| `desktop-ui/src/features/chat/components/DebateView.tsx` | Phase-aware rendering, judge annotations between rounds |
| `desktop-ui/src/features/chat/components/DebateRound.tsx` | Phase-aware layout (cards vs thread), challenge callouts |
| `desktop-ui/src/features/chat/components/ConsensusIndicator.tsx` | Confidence glow colors |
| `desktop-ui/src/features/chat/pages/ChatPage.tsx` | Quick/Debate toggle, pass squadMode |

---

## Task 1: Rename `debate_mode` → `squad_mode` Across the Stack

This is a mechanical rename that touches many files. Do it first to avoid merge conflicts later.

**Files:**
- Modify: `crates/tools-core/src/routing.rs`
- Modify: `crates/desktop-shared/src/commands/chat.rs`
- Modify: `crates/agent/src/agent_loop/mod.rs`
- Modify: `crates/agent/src/agent_loop/refactor_tests.rs`
- Modify: `crates/agent/src/agent_runtime/runtime.rs`
- Modify: `crates/app-core/src/handlers/chat/streaming.rs`
- Modify: `crates/klyntbot-server/src/bridge/registry.rs`
- Modify: `tests/e2e/agent_loop.rs`

- [ ] **Step 1: Rename field in RoutingContext**

In `crates/tools-core/src/routing.rs`, rename `debate_mode: Option<String>` to `squad_mode: Option<String>` on `RoutingContext` and all three constructors (`new`, `with_interaction`, `with_squad`). Update the doc comment to: `/// Squad mode: "quick" (single-pass) or "debate" (room-style conversation).`

- [ ] **Step 2: Rename field in SessionContextInput**

In `crates/desktop-shared/src/commands/chat.rs`, rename `debate_mode` to `squad_mode` on `SessionContextInput`. Update doc comment.

- [ ] **Step 3: Rename in agent_loop**

In `crates/agent/src/agent_loop/mod.rs`, rename `debate_mode` parameter on `process_direct_streaming` to `squad_mode`. Update the line `routing_ctx.debate_mode = debate_mode;` to `routing_ctx.squad_mode = squad_mode;`.

- [ ] **Step 4: Rename in runtime**

In `crates/agent/src/agent_runtime/runtime.rs`:
- Rename `debate_mode_str` parameter on `run_squad_execution` to `squad_mode_str`.
- Update the mode selection block to use `squad_mode_str` and remove the `DebateMode` enum references (just check for `"debate"` vs default to quick):
```rust
let use_debate = match squad_mode_str {
    Some("debate") => blackboard_repo.is_some(),
    _ => false,  // "quick" or None → single-pass
};
```

- [ ] **Step 5: Rename in streaming handler**

In `crates/app-core/src/handlers/chat/streaming.rs`, update the `debate_mode` variable extraction:
```rust
let squad_mode = context.as_ref().and_then(|c| c.squad_mode.clone());
```
And the `process_direct_streaming` call.

- [ ] **Step 6: Rename in klyntbot-server**

In `crates/klyntbot-server/src/bridge/registry.rs`, rename `debate_mode: None` to `squad_mode: None`.

- [ ] **Step 7: Update test call sites**

In `crates/agent/src/agent_loop/refactor_tests.rs` and `tests/e2e/agent_loop.rs`, the third parameter to `process_direct_streaming` is already `None` — just confirm it compiles after the rename.

- [ ] **Step 8: Verify compilation**

Run: `cargo build --workspace 2>&1 | tail -10`
Expected: Clean build.

- [ ] **Step 9: Run tests**

Run: `cargo nextest run --workspace 2>&1 | tail -5`
Expected: All pass.

- [ ] **Step 10: Commit**

```bash
git add -A && git commit -m "refactor: rename debate_mode to squad_mode across the stack"
```

---

## Task 2: Update Events — Phase, Challenge, JudgeDecision

**Files:**
- Modify: `crates/agent/src/events.rs`
- Modify: `crates/agent/src/intent_pipeline/engines/squad.rs`
- Modify: `crates/desktop-shared/src/events.rs`
- Modify: `crates/app-core/src/handlers/chat/streaming.rs`

- [ ] **Step 1: Update AgentEvent variants**

In `crates/agent/src/events.rs`:

Replace `DebateRoundStarted`:
```rust
DebateRoundStarted {
    round: u32,
    #[serde(rename = "totalRounds")]
    total_rounds: u32,
    phase: String,  // "opening" | "discussion" | "targeted" | "final"
},
```

Add `challenge` to `PersonaPerspective`:
```rust
PersonaPerspective {
    #[serde(rename = "personaId")]
    persona_id: String,
    #[serde(rename = "personaName")]
    persona_name: String,
    #[serde(rename = "personaIcon")]
    persona_icon: String,
    #[serde(rename = "personaRole")]
    persona_role: String,
    content: String,
    /// What the judge asked this persona to address (targeted phase only).
    #[serde(rename = "challenge", skip_serializing_if = "Option::is_none")]
    challenge: Option<String>,
},
```

Add new `DebateJudgeDecision` variant:
```rust
/// Emitted after the judge evaluates each round.
DebateJudgeDecision {
    round: u32,
    #[serde(rename = "consensusScore")]
    consensus_score: f64,
    decision: String,
    #[serde(rename = "speakingOrder")]
    speaking_order: Vec<String>,
    reasoning: String,
},
```

- [ ] **Step 2: Fix squad.rs compile error**

In `crates/agent/src/intent_pipeline/engines/squad.rs`, add `challenge: None` to the `PersonaPerspective` emission (around line 112):
```rust
.send(crate::AgentEvent::PersonaPerspective {
    persona_id: persona_id.clone(),
    persona_name: persona_name.clone(),
    persona_icon: persona_icon.clone(),
    persona_role: persona_role.clone(),
    content: text.clone(),
    challenge: None,
})
```

- [ ] **Step 3: Update desktop-shared payloads**

In `crates/desktop-shared/src/events.rs`:

Add event name constant:
```rust
pub const AGENT_DEBATE_JUDGE_DECISION: &str = "agent:debate_judge_decision";
```

Update `DebateRoundStartedPayload` — replace `debate_mode` with `phase`:
```rust
pub struct DebateRoundStartedPayload {
    pub session_key: String,
    pub round: u32,
    pub total_rounds: u32,
    pub phase: String,
}
```

Add `challenge` to `PersonaPerspectivePayload`:
```rust
pub struct PersonaPerspectivePayload {
    pub session_key: String,
    pub persona_id: String,
    pub persona_name: String,
    pub persona_icon: String,
    pub persona_role: String,
    pub content: String,
    pub challenge: Option<String>,
}
```

Add `DebateJudgeDecisionPayload`:
```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DebateJudgeDecisionPayload {
    pub session_key: String,
    pub round: u32,
    pub consensus_score: f64,
    pub decision: String,
    pub speaking_order: Vec<String>,
    pub reasoning: String,
}
```

- [ ] **Step 4: Update streaming relay**

In `crates/app-core/src/handlers/chat/streaming.rs`:

Update `PersonaPerspective` relay to include `challenge`:
```rust
AgentEvent::PersonaPerspective { persona_id, persona_name, persona_icon, persona_role, content, challenge } => {
    emit!(
        events::AGENT_PERSONA_PERSPECTIVE,
        events::PersonaPerspectivePayload {
            session_key: sk.to_string(),
            persona_id, persona_name, persona_icon, persona_role, content, challenge,
        }
    );
}
```

Update `DebateRoundStarted` relay — replace `debate_mode` with `phase`:
```rust
AgentEvent::DebateRoundStarted { round, total_rounds, phase } => {
    emit!(
        events::AGENT_DEBATE_ROUND_STARTED,
        events::DebateRoundStartedPayload {
            session_key: sk.to_string(), round, total_rounds, phase,
        }
    );
}
```

Add `DebateJudgeDecision` relay:
```rust
AgentEvent::DebateJudgeDecision { round, consensus_score, decision, speaking_order, reasoning } => {
    emit!(
        events::AGENT_DEBATE_JUDGE_DECISION,
        events::DebateJudgeDecisionPayload {
            session_key: sk.to_string(), round, consensus_score, decision, speaking_order, reasoning,
        }
    );
}
```

- [ ] **Step 5: Verify compilation**

Run: `cargo build -p agent -p app-core -p desktop-shared 2>&1 | tail -10`

- [ ] **Step 6: Commit**

```bash
git add -A && git commit -m "feat(events): add phase/challenge/JudgeDecision, remove debate_mode from events"
```

---

## Task 3: Update Blackboard — Skip Judge Entries in Prompt

**Files:**
- Modify: `crates/cognitive/src/repos/blackboard.rs`

- [ ] **Step 1: Write test**

Add to the test module in `blackboard.rs`:
```rust
#[test]
fn test_format_for_prompt_skips_judge_decisions() {
    let entries = vec![
        BlackboardEntry {
            id: "1".into(), session_key: "s".into(), squad_id: "sq".into(),
            round: 1, persona_id: "p1".into(), persona_name: "Analyst".into(),
            entry_type: "opening".into(), content: "My analysis".into(),
            confidence: 0.9, references_entry_id: None, created_at: "now".into(),
        },
        BlackboardEntry {
            id: "2".into(), session_key: "s".into(), squad_id: "sq".into(),
            round: 1, persona_id: "judge".into(), persona_name: "Judge".into(),
            entry_type: "judge_decision".into(), content: r#"{"decision":"continue"}"#.into(),
            confidence: 1.0, references_entry_id: None, created_at: "now".into(),
        },
        BlackboardEntry {
            id: "3".into(), session_key: "s".into(), squad_id: "sq".into(),
            round: 2, persona_id: "p2".into(), persona_name: "Skeptic".into(),
            entry_type: "discussion".into(), content: "I disagree".into(),
            confidence: 0.8, references_entry_id: None, created_at: "now".into(),
        },
    ];
    let prompt = BlackboardRepo::format_for_prompt(&entries);
    assert!(prompt.contains("Analyst"));
    assert!(prompt.contains("Skeptic"));
    assert!(!prompt.contains("Judge"));
    assert!(!prompt.contains("judge_decision"));
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo nextest run -p cognitive -E 'test(test_format_for_prompt_skips)'`
Expected: FAIL — judge entry is currently included.

- [ ] **Step 3: Update format_for_prompt**

Add a filter to skip judge entries:
```rust
pub fn format_for_prompt(entries: &[BlackboardEntry]) -> String {
    if entries.is_empty() {
        return String::new();
    }
    let mut rounds: std::collections::BTreeMap<i64, Vec<&BlackboardEntry>> =
        std::collections::BTreeMap::new();
    for e in entries {
        if e.entry_type == "judge_decision" {
            continue;  // Skip judge entries — they're not persona contributions
        }
        rounds.entry(e.round).or_default().push(e);
    }
    // ... rest unchanged
```

- [ ] **Step 4: Run test**

Run: `cargo nextest run -p cognitive -E 'test(test_format_for_prompt)'`
Expected: All pass.

- [ ] **Step 5: Commit**

```bash
git add crates/cognitive/src/repos/blackboard.rs
git commit -m "fix(blackboard): skip judge_decision entries in format_for_prompt"
```

---

## Task 4: Rewrite debate.rs — JudgeDecision + Room Debate Core

This is the main rewrite. Replace the entire debate orchestrator.

**Files:**
- Modify: `crates/agent/src/intent_pipeline/engines/debate.rs`

- [ ] **Step 1: Write tests for JudgeDecision parsing**

Replace the existing test module with new tests. Add at the bottom of `debate.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_judge_decision_valid() {
        let json = r#"{"consensus_score":72,"decision":"continue","speaking_order":["p1","p2","p3"],"challenges":{"p1":"Address X","p2":"Respond to Y","p3":"Integrate Z"},"reasoning":"P1 was most challenged","summary":"Converging on index funds","quick_synthesis_hint":null}"#;
        let decision = parse_judge_json(json);
        assert_eq!(decision.consensus_score, 72.0);
        assert_eq!(decision.decision, "continue");
        assert_eq!(decision.speaking_order, vec!["p1", "p2", "p3"]);
        assert_eq!(decision.challenges.get("p1").unwrap(), "Address X");
        assert_eq!(decision.reasoning, "P1 was most challenged");
        assert_eq!(decision.summary, "Converging on index funds");
        assert!(decision.quick_synthesis_hint.is_none());
    }

    #[test]
    fn test_parse_judge_decision_with_synthesis_hint() {
        let json = r#"{"consensus_score":92,"decision":"stop","speaking_order":[],"challenges":{},"reasoning":"Full agreement","summary":"All agree on diversification","quick_synthesis_hint":"Recommend diversified portfolio"}"#;
        let decision = parse_judge_json(json);
        assert_eq!(decision.consensus_score, 92.0);
        assert_eq!(decision.decision, "stop");
        assert_eq!(decision.quick_synthesis_hint.as_deref(), Some("Recommend diversified portfolio"));
    }

    #[test]
    fn test_parse_judge_decision_malformed_returns_default() {
        let decision = parse_judge_json("not json at all");
        assert_eq!(decision.decision, "continue");
        assert_eq!(decision.consensus_score, 0.0);
    }

    #[test]
    fn test_build_opening_prompt() {
        let prompt = build_phase_prompt(
            "System context",
            "Should I invest?",
            "Analyst",
            "Financial expert",
            "Data-driven",
            "analytical",
            &[],
            1,
            DebatePhase::Opening,
            None,
        );
        assert!(prompt.contains("Analyst"));
        assert!(prompt.contains("initial position"));
        assert!(!prompt.contains("Prior Debate"));
    }

    #[test]
    fn test_build_targeted_prompt_includes_challenge() {
        let blackboard = vec![BlackboardEntry {
            id: "1".into(), session_key: "s".into(), squad_id: "sq".into(),
            round: 1, persona_id: "p1".into(), persona_name: "Analyst".into(),
            entry_type: "opening".into(), content: "Index funds win.".into(),
            confidence: 0.9, references_entry_id: None, created_at: "now".into(),
        }];
        let prompt = build_phase_prompt(
            "System",
            "Invest?",
            "Skeptic",
            "Critical",
            "Questions claims",
            "direct",
            &blackboard,
            2,
            DebatePhase::Targeted,
            Some("What evidence supports index funds over active management?"),
        );
        assert!(prompt.contains("Prior Debate"));
        assert!(prompt.contains("Address this specific point"));
        assert!(prompt.contains("What evidence supports"));
    }
}
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo nextest run -p agent -E 'test(test_parse_judge) | test(test_build_opening) | test(test_build_targeted)'`
Expected: FAIL — types don't exist yet.

- [ ] **Step 3: Implement JudgeDecision struct and parser**

Replace the top of `debate.rs` (remove `DebateMode` enum, `estimate_consensus`, `llm_judge_consensus`, `DEFAULT_*` constants):

```rust
//! Room-style debate orchestrator — multi-round persona conversation with LLM judge.
//!
//! Four phases:
//! 1. Opening (parallel): all personas give initial positions
//! 2. Discussion (sequential): personas respond to each other, full responses
//! 3. Targeted (sequential): judge gives specific challenges, short responses
//! 4. Final (parallel): concise final position statements

use std::collections::HashMap;

use cognitive::{BlackboardEntry, BlackboardRepo, NewBlackboardEntry, PersonaRow};
use providers::{ChatParams, DynProvider, Message, UserContent};
use serde::{Deserialize, Serialize};

/// Safety cap — maximum total rounds including opening and final.
pub const MAX_ROUNDS: u32 = 6;

/// Consensus threshold — judge score 0-100, debate stops at this level.
pub const CONSENSUS_THRESHOLD: f64 = 85.0;

/// Debate phase — controls prompt style and execution pattern.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DebatePhase {
    Opening,
    Discussion,
    Targeted,
    Final,
}

impl DebatePhase {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Opening => "opening",
            Self::Discussion => "discussion",
            Self::Targeted => "targeted",
            Self::Final => "final",
        }
    }
}

/// Structured decision from the LLM judge.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JudgeDecision {
    pub consensus_score: f64,
    pub decision: String,
    pub speaking_order: Vec<String>,
    #[serde(default)]
    pub challenges: HashMap<String, String>,
    pub reasoning: String,
    #[serde(default)]
    pub summary: String,
    pub quick_synthesis_hint: Option<String>,
}

impl Default for JudgeDecision {
    fn default() -> Self {
        Self {
            consensus_score: 0.0,
            decision: "continue".to_string(),
            speaking_order: Vec::new(),
            challenges: HashMap::new(),
            reasoning: "Judge unavailable".to_string(),
            summary: String::new(),
            quick_synthesis_hint: None,
        }
    }
}

/// Parse judge JSON response, returning default on failure.
pub fn parse_judge_json(text: &str) -> JudgeDecision {
    // Try to extract JSON from the response (judge may wrap it in markdown)
    let json_str = if let Some(start) = text.find('{') {
        if let Some(end) = text.rfind('}') {
            &text[start..=end]
        } else {
            text
        }
    } else {
        text
    };
    serde_json::from_str(json_str).unwrap_or_default()
}
```

- [ ] **Step 4: Implement `build_phase_prompt`**

```rust
/// Build a persona prompt appropriate for the current debate phase.
pub fn build_phase_prompt(
    orchestrator_context: &str,
    user_message: &str,
    persona_name: &str,
    persona_role: &str,
    persona_perspective: &str,
    persona_tone: &str,
    blackboard: &[BlackboardEntry],
    current_round: u32,
    phase: DebatePhase,
    challenge: Option<&str>,
) -> String {
    let blackboard_context = BlackboardRepo::format_for_prompt(blackboard);

    let phase_instructions = match phase {
        DebatePhase::Opening => format!(
            "This is **Round {current_round}** — the opening round. Share your initial position on the topic. Be clear about your stance and reasoning."
        ),
        DebatePhase::Discussion => format!(
            "This is **Round {current_round}** of a live discussion. You can see what other personas said above.\n\n\
             Rules:\n\
             - Respond directly to specific points others have made\n\
             - If you agree with someone, say so and build on their point\n\
             - If you disagree, explain why with evidence\n\
             - If your position has changed, acknowledge the shift\n\
             - Be direct and specific"
        ),
        DebatePhase::Targeted => {
            let challenge_text = challenge.unwrap_or("Continue the discussion.");
            format!(
                "This is **Round {current_round}** — a targeted exchange. Address this specific point concisely (50-150 words):\n\n\
                 > {challenge_text}"
            )
        }
        DebatePhase::Final => format!(
            "This is the **final round**. Summarize your final position in 2-3 sentences, accounting for the full discussion above."
        ),
    };

    format!(
        r#"{orchestrator_context}
{blackboard_context}

---

You are responding as **{persona_name}**, a {persona_role}.
Your perspective: {persona_perspective}
Your tone: {persona_tone}

{phase_instructions}

Respond to: {user_message}"#
    )
}
```

- [ ] **Step 5: Implement `call_judge`**

```rust
/// Call the LLM judge to evaluate the current round and decide next steps.
pub async fn call_judge(
    provider: &DynProvider,
    responses: &[(String, String)],
    blackboard: &[BlackboardEntry],
    params: &ChatParams,
    user_message: &str,
    round: u32,
    persona_ids: &[String],
) -> JudgeDecision {
    let mut response_text = String::new();
    for (name, content) in responses {
        let truncated: String = content.chars().take(500).collect();
        response_text.push_str(&format!("**{name}**: {truncated}\n\n"));
    }

    let persona_list = persona_ids.join(", ");

    let judge_prompt = format!(
        r#"You are a debate judge evaluating a multi-persona discussion.

**User's question:** {user_message}

**Round {round} responses:**
{response_text}

**Available persona IDs:** {persona_list}

Evaluate the discussion and return a JSON object with exactly these fields:
{{
  "consensus_score": <0-100, how much the personas agree>,
  "decision": <"continue" if unresolved disagreements, "final_round" if near consensus, "stop" if fully agreed>,
  "speaking_order": [<persona IDs ordered by who should speak next — most challenged/disagreed first>],
  "challenges": {{<persona_id: "specific question or challenge for this persona">}},
  "reasoning": "<1-2 sentences explaining your decision>",
  "summary": "<1 sentence summarizing the current state of the debate>",
  "quick_synthesis_hint": <null, or a 1-2 sentence synthesis if consensus_score > 85>
}}

Rules:
- speaking_order: put the persona who was most challenged or had the weakest argument first
- challenges: give each persona a specific, pointed question that advances the debate
- decision: use "continue" unless personas are clearly converging (70+) or stuck repeating themselves
- quick_synthesis_hint: only set if consensus_score > 85

Output ONLY the JSON object, no other text."#
    );

    let messages = vec![Message::User {
        content: UserContent::Text(judge_prompt),
    }];

    let judge_params = ChatParams {
        max_tokens: Some(500),
        temperature: Some(0.1),
        ..params.clone()
    };

    match provider.chat(&messages, None, &judge_params).await {
        Ok(result) => {
            let text = result.content.unwrap_or_default();
            parse_judge_json(&text)
        }
        Err(e) => {
            tracing::warn!("LLM judge call failed: {e}");
            // Fallback: continue with original persona order
            JudgeDecision {
                speaking_order: persona_ids.to_vec(),
                ..JudgeDecision::default()
            }
        }
    }
}
```

- [ ] **Step 6: Run tests**

Run: `cargo nextest run -p agent -E 'test(test_parse_judge) | test(test_build_opening) | test(test_build_targeted)'`
Expected: All pass.

- [ ] **Step 7: Commit**

```bash
git add crates/agent/src/intent_pipeline/engines/debate.rs
git commit -m "feat(debate): add JudgeDecision struct, call_judge, phase-aware prompts"
```

---

## Task 5: Implement `run_room_debate` — The 4-Phase Flow

**Files:**
- Modify: `crates/agent/src/intent_pipeline/engines/debate.rs`

- [ ] **Step 1: Implement parallel helpers**

Add `fan_out_opening` and `fan_out_final` functions:

```rust
/// Phase 1 (Opening) and Phase Final (Closing): parallel fan-out.
async fn fan_out_parallel(
    provider: &DynProvider,
    orchestrator_context: &str,
    user_message: &str,
    personas: &[PersonaRow],
    params: &ChatParams,
    blackboard: &[BlackboardEntry],
    round: u32,
    phase: DebatePhase,
    event_tx: Option<&tokio::sync::mpsc::Sender<crate::AgentEvent>>,
) -> Vec<(String, String, String)> {
    let futures: Vec<_> = personas
        .iter()
        .map(|persona| {
            let provider = provider.clone();
            let params = params.clone();
            let system = build_phase_prompt(
                orchestrator_context, user_message,
                &persona.name, &persona.role, &persona.perspective, &persona.tone,
                blackboard, round, phase, None,
            );
            let user_msg = user_message.to_string();
            let persona_name = persona.name.clone();
            let persona_id = persona.id.clone();
            let persona_icon = persona.icon.clone();
            let persona_role = persona.role.clone();
            let tx = event_tx.cloned();

            async move {
                let messages = vec![
                    Message::System { content: system },
                    Message::User { content: UserContent::Text(user_msg) },
                ];
                let text = match provider.chat(&messages, None, &params).await {
                    Ok(r) => r.content.unwrap_or_default(),
                    Err(e) => {
                        tracing::warn!(persona = %persona_name, round, "LLM call failed: {e}");
                        String::new()
                    }
                };
                if let Some(tx) = &tx {
                    let _ = tx.send(crate::AgentEvent::PersonaPerspective {
                        persona_id: persona_id.clone(),
                        persona_name: persona_name.clone(),
                        persona_icon, persona_role,
                        content: text.clone(),
                        challenge: None,
                    }).await;
                }
                (persona_id, persona_name, text)
            }
        })
        .collect();

    futures_util::future::join_all(futures).await
}
```

- [ ] **Step 2: Implement sequential round**

```rust
/// Phase 2 (Discussion) and Phase 3+ (Targeted): sequential execution.
///
/// Each persona speaks in turn, seeing all prior speakers in this round.
async fn run_sequential_round(
    provider: &DynProvider,
    orchestrator_context: &str,
    user_message: &str,
    personas: &[PersonaRow],
    params: &ChatParams,
    blackboard_repo: &BlackboardRepo,
    session_key: &str,
    squad_id: &str,
    round: u32,
    phase: DebatePhase,
    speaking_order: &[String],
    challenges: &HashMap<String, String>,
    event_tx: Option<&tokio::sync::mpsc::Sender<crate::AgentEvent>>,
) -> Vec<(String, String, String)> {
    let mut results = Vec::new();

    for persona_id in speaking_order {
        // Find the persona by ID
        let Some(persona) = personas.iter().find(|p| p.id == *persona_id) else {
            continue;
        };

        // Reload blackboard (includes prior speakers this round)
        let blackboard = blackboard_repo
            .list_for_session(session_key)
            .await
            .unwrap_or_default();

        let challenge = challenges.get(persona_id.as_str()).map(|s| s.as_str());

        let system = build_phase_prompt(
            orchestrator_context, user_message,
            &persona.name, &persona.role, &persona.perspective, &persona.tone,
            &blackboard, round, phase, challenge,
        );

        let messages = vec![
            Message::System { content: system },
            Message::User { content: UserContent::Text(user_message.to_string()) },
        ];

        let text = match provider.chat(&messages, None, params).await {
            Ok(r) => r.content.unwrap_or_default(),
            Err(e) => {
                tracing::warn!(persona = %persona.name, round, "LLM call failed, skipping");
                // Emit placeholder so frontend shows something
                if let Some(tx) = event_tx {
                    let _ = tx.send(crate::AgentEvent::PersonaPerspective {
                        persona_id: persona.id.clone(),
                        persona_name: persona.name.clone(),
                        persona_icon: persona.icon.clone(),
                        persona_role: persona.role.clone(),
                        content: "[Persona unavailable this round]".to_string(),
                        challenge: challenge.map(|s| s.to_string()),
                    }).await;
                }
                continue; // Skip — don't write empty entry to blackboard
            }
        };

        // Emit event immediately (user sees responses appear one by one)
        if let Some(tx) = event_tx {
            let _ = tx.send(crate::AgentEvent::PersonaPerspective {
                persona_id: persona.id.clone(),
                persona_name: persona.name.clone(),
                persona_icon: persona.icon.clone(),
                persona_role: persona.role.clone(),
                content: text.clone(),
                challenge: challenge.map(|s| s.to_string()),
            }).await;
        }

        // Write to blackboard so next speaker sees this response
        let _ = blackboard_repo.insert(&NewBlackboardEntry {
            session_key, squad_id,
            round: round as i64,
            persona_id: &persona.id,
            persona_name: &persona.name,
            entry_type: phase.as_str(),
            content: &text,
            confidence: 0.8,
            references_entry_id: None,
        }).await;

        results.push((persona.id.clone(), persona.name.clone(), text));
    }

    results
}
```

- [ ] **Step 3: Implement `run_room_debate`**

```rust
/// Run a room-style multi-round debate with 4 phases.
///
/// Returns: Vec of (round_number, persona_responses, consensus_score) per round.
pub async fn run_room_debate(
    provider: &DynProvider,
    orchestrator_context: &str,
    user_message: &str,
    personas: &[PersonaRow],
    params: &ChatParams,
    blackboard_repo: &BlackboardRepo,
    session_key: &str,
    squad_id: &str,
    event_tx: Option<&tokio::sync::mpsc::Sender<crate::AgentEvent>>,
) -> Vec<(u32, Vec<(String, String)>, f64)> {
    let persona_ids: Vec<String> = personas.iter().map(|p| p.id.clone()).collect();
    let mut all_rounds: Vec<(u32, Vec<(String, String)>, f64)> = Vec::new();
    let mut last_judge: Option<JudgeDecision> = None;
    let mut round: u32 = 0;

    loop {
        round += 1;

        // Determine phase
        let phase = if round == 1 {
            DebatePhase::Opening
        } else if let Some(ref judge) = last_judge {
            if judge.decision == "final_round" || round >= MAX_ROUNDS {
                DebatePhase::Final
            } else if round == 2 {
                DebatePhase::Discussion
            } else {
                DebatePhase::Targeted
            }
        } else {
            DebatePhase::Discussion
        };

        // Emit round started
        if let Some(tx) = event_tx {
            let _ = tx.send(crate::AgentEvent::DebateRoundStarted {
                round,
                total_rounds: MAX_ROUNDS,
                phase: phase.as_str().to_string(),
            }).await;
        }

        // Execute round based on phase
        let round_results = match phase {
            DebatePhase::Opening | DebatePhase::Final => {
                let blackboard = blackboard_repo.list_for_session(session_key).await.unwrap_or_default();
                let results = fan_out_parallel(
                    provider, orchestrator_context, user_message,
                    personas, params, &blackboard, round, phase, event_tx,
                ).await;

                // Write to blackboard
                for (pid, pname, content) in &results {
                    if !content.is_empty() {
                        let _ = blackboard_repo.insert(&NewBlackboardEntry {
                            session_key, squad_id, round: round as i64,
                            persona_id: pid, persona_name: pname,
                            entry_type: phase.as_str(), content,
                            confidence: 0.8, references_entry_id: None,
                        }).await;
                    }
                }
                results
            }
            DebatePhase::Discussion | DebatePhase::Targeted => {
                let speaking_order = last_judge.as_ref()
                    .map(|j| j.speaking_order.clone())
                    .unwrap_or_else(|| persona_ids.clone());
                let challenges = last_judge.as_ref()
                    .map(|j| j.challenges.clone())
                    .unwrap_or_default();

                run_sequential_round(
                    provider, orchestrator_context, user_message,
                    personas, params, blackboard_repo, session_key, squad_id,
                    round, phase, &speaking_order, &challenges, event_tx,
                ).await
            }
        };

        // Build (name, content) pairs
        let responses: Vec<(String, String)> = round_results
            .iter()
            .map(|(_, name, content)| (name.clone(), content.clone()))
            .collect();

        // Final phase: no judge needed, emit completion and break
        if phase == DebatePhase::Final {
            if let Some(tx) = event_tx {
                let _ = tx.send(crate::AgentEvent::DebateRoundCompleted {
                    round, consensus_score: 100.0,
                }).await;
                let summary = last_judge.as_ref()
                    .map(|j| j.summary.clone())
                    .unwrap_or_else(|| "Debate complete.".to_string());
                let _ = tx.send(crate::AgentEvent::ConsensusReached {
                    round, consensus_score: 100.0, summary,
                }).await;
            }
            all_rounds.push((round, responses, 100.0));
            break;
        }

        // Call judge
        let blackboard = blackboard_repo.list_for_session(session_key).await.unwrap_or_default();
        let judge = call_judge(
            provider, &responses, &blackboard, params,
            user_message, round, &persona_ids,
        ).await;

        // Emit judge decision BEFORE acting on it
        if let Some(tx) = event_tx {
            let _ = tx.send(crate::AgentEvent::DebateJudgeDecision {
                round,
                consensus_score: judge.consensus_score,
                decision: judge.decision.clone(),
                speaking_order: judge.speaking_order.clone(),
                reasoning: judge.reasoning.clone(),
            }).await;
        }

        // Store judge decision on blackboard
        let judge_json = serde_json::to_string(&judge).unwrap_or_default();
        let _ = blackboard_repo.insert(&NewBlackboardEntry {
            session_key, squad_id, round: round as i64,
            persona_id: "judge", persona_name: "Judge",
            entry_type: "judge_decision", content: &judge_json,
            confidence: 1.0, references_entry_id: None,
        }).await;

        // Emit round completed
        if let Some(tx) = event_tx {
            let _ = tx.send(crate::AgentEvent::DebateRoundCompleted {
                round, consensus_score: judge.consensus_score,
            }).await;
        }

        all_rounds.push((round, responses, judge.consensus_score));

        // Check for early termination
        if judge.decision == "stop" || judge.consensus_score >= CONSENSUS_THRESHOLD {
            if let Some(tx) = event_tx {
                let _ = tx.send(crate::AgentEvent::ConsensusReached {
                    round,
                    consensus_score: judge.consensus_score,
                    summary: judge.summary.clone(),
                }).await;
            }
            break;
        }

        // Safety cap
        if round >= MAX_ROUNDS - 1 {
            // Next round will be forced final
        }

        last_judge = Some(judge);
    }

    all_rounds
}
```

- [ ] **Step 4: Verify compilation**

Run: `cargo build -p agent 2>&1 | tail -10`

- [ ] **Step 5: Commit**

```bash
git add crates/agent/src/intent_pipeline/engines/debate.rs
git commit -m "feat(debate): implement run_room_debate with 4-phase flow — parallel opening, sequential discussion, targeted challenges, parallel final"
```

---

## Task 6: Update Runtime Integration

**Files:**
- Modify: `crates/agent/src/agent_runtime/runtime.rs`

- [ ] **Step 1: Simplify squad execution to use room debate**

In `run_squad_execution`, replace the DebateMode logic with:

```rust
// 3. Persona fan-out (quick or room debate)
let blackboard_repo = self.squad_deps.as_ref().and_then(|d| d.blackboard_repo.as_ref());
let use_debate = match squad_mode_str {
    Some("debate") => blackboard_repo.is_some(),
    _ => false,
};

let persona_responses = if use_debate {
    let blackboard_repo = blackboard_repo.unwrap();
    let debate_session_key = format!("debate:{}:{}", squad_id, uuid::Uuid::new_v4());
    let debate_results = engines::debate::run_room_debate(
        provider,
        &orchestrator_context,
        message,
        &resolved.personas,
        params,
        blackboard_repo,
        &debate_session_key,
        squad_id,
        event_tx.as_ref(),
    ).await;

    debate_results
        .last()
        .map(|(_, responses, _)| responses.clone())
        .unwrap_or_default()
} else {
    squad::fan_out_personas(
        provider, &orchestrator_context, message,
        &resolved.personas, params, event_tx.as_ref(),
    ).await
};
```

Keep the `use crate::intent_pipeline::engines` import — it's still needed for `engines::debate::run_room_debate`.

- [ ] **Step 2: Verify compilation**

Run: `cargo build -p agent -p app-core 2>&1 | tail -10`

- [ ] **Step 3: Run full test suite**

Run: `cargo nextest run --workspace 2>&1 | tail -5`

- [ ] **Step 4: Commit**

```bash
git add crates/agent/src/agent_runtime/runtime.rs
git commit -m "feat(runtime): wire run_room_debate, simplify squad_mode selection"
```

---

## Task 7: Frontend Types + Store

**Files:**
- Modify: `desktop-ui/src/shared/types/chat.ts`
- Modify: `desktop-ui/src/shared/types/index.ts`
- Modify: `desktop-ui/src/shared/stores/chatStreamStore.ts`
- Modify: `desktop-ui/src/features/chat/hooks/useAgentStream.ts`
- Modify: `desktop-ui/src/features/chat/hooks/useChatSession.ts`

- [ ] **Step 1: Update types**

In `chat.ts`:

Update `DebateRoundStartedPayload` — replace `debateMode` with `phase`:
```typescript
export interface DebateRoundStartedPayload {
  sessionKey: string;
  round: number;
  totalRounds: number;
  phase: "opening" | "discussion" | "targeted" | "final";
}
```

Add `DebateJudgeDecisionPayload`:
```typescript
export interface DebateJudgeDecisionPayload {
  sessionKey: string;
  round: number;
  consensusScore: number;
  decision: "continue" | "final_round" | "stop";
  speakingOrder: string[];
  reasoning: string;
}
```

Add `JudgeDecisionEntry` (for store state):
```typescript
export interface JudgeDecisionEntry {
  round: number;
  consensusScore: number;
  decision: "continue" | "final_round" | "stop";
  speakingOrder: string[];
  reasoning: string;
}
```

Update `PersonaSegment` — add `challenge`:
```typescript
export interface PersonaSegment {
  personaId: string;
  personaName: string;
  personaIcon?: string;
  personaRole?: string;
  content: string;
  challenge?: string;
}
```

Update `DebateRound` — add `phase`:
```typescript
export interface DebateRound {
  round: number;
  phase: "opening" | "discussion" | "targeted" | "final";
  personaMessages: PersonaSegment[];
  consensusScore: number | null;
}
```

Remove `MemoryPromotedPayload` type if unused. Export new types from `index.ts`.

- [ ] **Step 2: Update store**

In `chatStreamStore.ts`:

Update `StreamSnapshot`:
```typescript
squadMode: "quick" | "debate" | null;  // replaces debateMode
judgeDecisions: JudgeDecisionEntry[];  // NEW
```

Update `DEFAULT_SNAPSHOT`:
```typescript
squadMode: null,
judgeDecisions: [],
```

Add `"agent:debate_judge_decision"` to `SSE_AGENT_EVENTS` array.

Update `onDebateRoundStarted` to use `phase` instead of `debateMode`:
```typescript
private onDebateRoundStarted(payload: DebateRoundStartedPayload): void {
    if (!this.isActive(payload.sessionKey)) return;
    this.updateState(payload.sessionKey, (s) => ({
        ...s,
        currentDebateRound: payload.round,
        totalDebateRounds: payload.totalRounds,
        squadMode: "debate",
        debateRounds: [
            ...s.debateRounds,
            { round: payload.round, phase: payload.phase, personaMessages: [], consensusScore: null },
        ],
    }));
}
```

Add `onDebateJudgeDecision` handler:
```typescript
private onDebateJudgeDecision(payload: DebateJudgeDecisionPayload): void {
    if (!this.isActive(payload.sessionKey)) return;
    this.updateState(payload.sessionKey, (s) => ({
        ...s,
        judgeDecisions: [
            ...s.judgeDecisions,
            {
                round: payload.round,
                consensusScore: payload.consensusScore,
                decision: payload.decision,
                speakingOrder: payload.speakingOrder,
                reasoning: payload.reasoning,
            },
        ],
    }));
}
```

Update `onPersonaPerspective` to include `challenge`:
```typescript
const segment: PersonaSegment = {
    personaId: payload.personaId,
    personaName: payload.personaName,
    personaIcon: payload.personaIcon,
    personaRole: payload.personaRole,
    content: payload.content,
    challenge: payload.challenge ?? undefined,
};
```

Register new event handlers in both browser and Tauri modes.

- [ ] **Step 3: Update hooks**

In `useAgentStream.ts`: expose `judgeDecisions`, `squadMode`; remove `debateMode`.
In `useChatSession.ts`: replace `debateMode` with `squadMode` in interface and payload.

- [ ] **Step 4: Lint**

Run: `cd desktop-ui && bun run lint:fix`

- [ ] **Step 5: Commit**

```bash
git add desktop-ui/src/shared/ desktop-ui/src/features/chat/hooks/
git commit -m "feat(ui): update types + store for room debate — phase, judgeDecisions, squadMode"
```

---

## Task 8: Frontend Components — JudgeAnnotation + Phase-Aware Rendering

**Files:**
- Create: `desktop-ui/src/features/chat/components/JudgeAnnotation.tsx`
- Modify: `desktop-ui/src/features/chat/components/DebateView.tsx`
- Modify: `desktop-ui/src/features/chat/components/DebateRound.tsx`
- Modify: `desktop-ui/src/features/chat/components/ConsensusIndicator.tsx`
- Modify: `desktop-ui/src/features/chat/pages/ChatPage.tsx`

- [ ] **Step 1: Create JudgeAnnotation**

```tsx
import type { JudgeDecisionEntry } from "@shared/types";

interface JudgeAnnotationProps {
  decision: JudgeDecisionEntry;
  onForceFinalize?: () => void;
  showForceButton: boolean;
}

export function JudgeAnnotation({ decision, onForceFinalize, showForceButton }: JudgeAnnotationProps) {
  const dotColor =
    decision.consensusScore > 85
      ? "bg-green-400"
      : decision.consensusScore > 60
        ? "bg-yellow-400"
        : "bg-red-400";

  return (
    <div className="glass-panel rounded-lg px-3 py-2 flex items-start gap-2 text-[10px]">
      <div className={`w-2 h-2 rounded-full mt-0.5 shrink-0 ${dotColor}`} />
      <div className="flex-1 min-w-0">
        <p className="text-dim italic">{decision.reasoning}</p>
        <p className="text-muted-foreground mt-0.5">
          Consensus: {Math.round(decision.consensusScore)}% — {decision.decision === "continue" ? "Continuing" : decision.decision === "final_round" ? "Moving to final round" : "Consensus reached"}
        </p>
      </div>
      {showForceButton && decision.decision === "continue" && onForceFinalize && (
        <button
          type="button"
          onClick={onForceFinalize}
          className="shrink-0 px-2 py-0.5 rounded text-[9px] bg-white/[0.06] hover:bg-white/[0.1] text-dim hover:text-muted-foreground transition-colors"
        >
          Force Final
        </button>
      )}
    </div>
  );
}
```

- [ ] **Step 2: Update DebateRound for phase-aware rendering**

Update `DebateRound.tsx` to render differently based on phase:
- `opening` / `final`: side-by-side cards (existing `PersonaMessageList`)
- `discussion`: vertical thread layout
- `targeted`: show challenge callout above each persona response

- [ ] **Step 3: Update DebateView to show judge annotations between rounds**

After each round (except the last), render `JudgeAnnotation` using the corresponding entry from `judgeDecisions`.

- [ ] **Step 4: Update ChatPage toggle**

Replace the Basic/Deep toggle with Quick/Debate:
```tsx
{squadId && (
  <button
    type="button"
    onClick={() => setSquadMode((m) => (m === "debate" ? "quick" : "debate"))}
    className={`px-2 py-0.5 rounded text-[10px] font-medium transition-colors ${
      squadMode === "debate"
        ? "bg-purple-500/20 text-purple-300 border border-purple-500/30"
        : "bg-white/[0.04] text-dim hover:text-muted-foreground border border-transparent"
    }`}
  >
    {squadMode === "debate" ? "Debate" : "Quick"}
  </button>
)}
```

Update state name from `debateMode` to `squadMode` and pass to `useChatSession`.

- [ ] **Step 5: Lint**

Run: `cd desktop-ui && bun run lint:fix`

- [ ] **Step 6: Commit**

```bash
git add desktop-ui/src/features/chat/
git commit -m "feat(ui): add JudgeAnnotation, phase-aware DebateRound, Quick/Debate toggle"
```

---

## Task 9: Integration Test + Cleanup

- [ ] **Step 1: Full workspace build**

Run: `cargo build --workspace`

- [ ] **Step 2: Full test suite**

Run: `cargo nextest run --workspace`

- [ ] **Step 3: Clippy**

Run: `cargo clippy --workspace --all-targets --all-features`

- [ ] **Step 4: Frontend lint**

Run: `cd desktop-ui && bun run lint:fix`

- [ ] **Step 5: Remove dead code**

Remove any remaining references to:
- `DebateMode` enum
- `estimate_consensus()` (Jaccard)
- `llm_judge_consensus()`
- `DEFAULT_MAX_ROUNDS` / `DEFAULT_CONSENSUS_THRESHOLD`
- `debateMode` in frontend types/stores

- [ ] **Step 6: Commit**

```bash
git add -A && git commit -m "feat(squads): room-style debate complete — parallel opening, sequential discussion, LLM judge, phase-aware UI"
```
