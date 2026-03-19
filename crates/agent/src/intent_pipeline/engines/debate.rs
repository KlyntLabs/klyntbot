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
const MAX_ROUNDS: u32 = 6;

/// Consensus threshold — judge score 0-100, debate stops at this level.
const CONSENSUS_THRESHOLD: f64 = 85.0;

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
    let json_str = common::helpers::extract_json_object(text).unwrap_or(text);
    serde_json::from_str(json_str).unwrap_or_default()
}

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
        DebatePhase::Final => "This is the **final round**. Summarize your final position in 2-3 sentences, accounting for the full discussion above.".to_string(),
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

/// Call the LLM judge to evaluate the current round and decide next steps.
pub async fn call_judge(
    provider: &DynProvider,
    responses: &[(String, String)],
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
            JudgeDecision {
                speaking_order: persona_ids.to_vec(),
                ..JudgeDecision::default()
            }
        }
    }
}

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
                orchestrator_context,
                user_message,
                &persona.name,
                &persona.role,
                &persona.perspective,
                &persona.tone,
                blackboard,
                round,
                phase,
                None,
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
                    Message::User {
                        content: UserContent::Text(user_msg),
                    },
                ];
                let text = match provider.chat(&messages, None, &params).await {
                    Ok(r) => r.content.unwrap_or_default(),
                    Err(e) => {
                        tracing::warn!(persona = %persona_name, round, "LLM call failed: {e}");
                        String::new()
                    }
                };
                if let Some(tx) = &tx {
                    let _ = tx
                        .send(crate::AgentEvent::PersonaPerspective {
                            persona_id: persona_id.clone(),
                            persona_name: persona_name.clone(),
                            persona_icon,
                            persona_role,
                            content: text.clone(),
                            challenge: None,
                        })
                        .await;
                }
                (persona_id, persona_name, text)
            }
        })
        .collect();

    futures_util::future::join_all(futures).await
}

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
            orchestrator_context,
            user_message,
            &persona.name,
            &persona.role,
            &persona.perspective,
            &persona.tone,
            &blackboard,
            round,
            phase,
            challenge,
        );

        let messages = vec![
            Message::System { content: system },
            Message::User {
                content: UserContent::Text(user_message.to_string()),
            },
        ];

        let text = match provider.chat(&messages, None, params).await {
            Ok(r) => r.content.unwrap_or_default(),
            Err(e) => {
                tracing::warn!(persona = %persona.name, round, "LLM call failed, skipping: {e}");
                if let Some(tx) = event_tx {
                    let _ = tx
                        .send(crate::AgentEvent::PersonaPerspective {
                            persona_id: persona.id.clone(),
                            persona_name: persona.name.clone(),
                            persona_icon: persona.icon.clone(),
                            persona_role: persona.role.clone(),
                            content: "[Persona unavailable this round]".to_string(),
                            challenge: challenge.map(|s| s.to_string()),
                        })
                        .await;
                }
                continue;
            }
        };

        // Emit event immediately (user sees responses appear one by one)
        if let Some(tx) = event_tx {
            let _ = tx
                .send(crate::AgentEvent::PersonaPerspective {
                    persona_id: persona.id.clone(),
                    persona_name: persona.name.clone(),
                    persona_icon: persona.icon.clone(),
                    persona_role: persona.role.clone(),
                    content: text.clone(),
                    challenge: challenge.map(|s| s.to_string()),
                })
                .await;
        }

        // Write to blackboard so next speaker sees this response
        let _ = blackboard_repo
            .insert(&NewBlackboardEntry {
                session_key,
                squad_id,
                round: round as i64,
                persona_id: &persona.id,
                persona_name: &persona.name,
                entry_type: phase.as_str(),
                content: &text,
                confidence: 0.8,
                references_entry_id: None,
            })
            .await;

        results.push((persona.id.clone(), persona.name.clone(), text));
    }

    results
}

/// Run a room-style multi-round debate with 4 phases.
///
/// Returns: Vec of (round_number, persona_responses as (name, content), consensus_score) per round.
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
            let _ = tx
                .send(crate::AgentEvent::DebateRoundStarted {
                    round,
                    total_rounds: MAX_ROUNDS,
                    phase: phase.as_str().to_string(),
                })
                .await;
        }

        // Execute round based on phase
        let round_results = match phase {
            DebatePhase::Opening | DebatePhase::Final => {
                let blackboard = blackboard_repo
                    .list_for_session(session_key)
                    .await
                    .unwrap_or_default();
                let results = fan_out_parallel(
                    provider,
                    orchestrator_context,
                    user_message,
                    personas,
                    params,
                    &blackboard,
                    round,
                    phase,
                    event_tx,
                )
                .await;

                // Write to blackboard
                for (pid, pname, content) in &results {
                    if !content.is_empty() {
                        let _ = blackboard_repo
                            .insert(&NewBlackboardEntry {
                                session_key,
                                squad_id,
                                round: round as i64,
                                persona_id: pid,
                                persona_name: pname,
                                entry_type: phase.as_str(),
                                content,
                                confidence: 0.8,
                                references_entry_id: None,
                            })
                            .await;
                    }
                }
                results
            }
            DebatePhase::Discussion | DebatePhase::Targeted => {
                let speaking_order = last_judge
                    .as_ref()
                    .map(|j| j.speaking_order.clone())
                    .unwrap_or_else(|| persona_ids.clone());
                let challenges = last_judge
                    .as_ref()
                    .map(|j| j.challenges.clone())
                    .unwrap_or_default();

                run_sequential_round(
                    provider,
                    orchestrator_context,
                    user_message,
                    personas,
                    params,
                    blackboard_repo,
                    session_key,
                    squad_id,
                    round,
                    phase,
                    &speaking_order,
                    &challenges,
                    event_tx,
                )
                .await
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
                let _ = tx
                    .send(crate::AgentEvent::DebateRoundCompleted {
                        round,
                        consensus_score: 100.0,
                    })
                    .await;
                let summary = last_judge
                    .as_ref()
                    .map(|j| j.summary.clone())
                    .unwrap_or_else(|| "Debate complete.".to_string());
                let _ = tx
                    .send(crate::AgentEvent::ConsensusReached {
                        round,
                        consensus_score: 100.0,
                        summary,
                    })
                    .await;
            }
            all_rounds.push((round, responses, 100.0));
            break;
        }

        // Call judge
        let judge = call_judge(
            provider,
            &responses,
            params,
            user_message,
            round,
            &persona_ids,
        )
        .await;

        // Emit judge decision BEFORE acting on it
        if let Some(tx) = event_tx {
            let _ = tx
                .send(crate::AgentEvent::DebateJudgeDecision {
                    round,
                    consensus_score: judge.consensus_score,
                    decision: judge.decision.clone(),
                    speaking_order: judge.speaking_order.clone(),
                    reasoning: judge.reasoning.clone(),
                })
                .await;
        }

        // Store judge decision on blackboard
        let judge_json = serde_json::to_string(&judge).unwrap_or_default();
        let _ = blackboard_repo
            .insert(&NewBlackboardEntry {
                session_key,
                squad_id,
                round: round as i64,
                persona_id: "judge",
                persona_name: "Judge",
                entry_type: "judge_decision",
                content: &judge_json,
                confidence: 1.0,
                references_entry_id: None,
            })
            .await;

        // Emit round completed
        if let Some(tx) = event_tx {
            let _ = tx
                .send(crate::AgentEvent::DebateRoundCompleted {
                    round,
                    consensus_score: judge.consensus_score,
                })
                .await;
        }

        all_rounds.push((round, responses, judge.consensus_score));

        // Check for early termination
        if judge.decision == "stop" || judge.consensus_score >= CONSENSUS_THRESHOLD {
            if let Some(tx) = event_tx {
                let _ = tx
                    .send(crate::AgentEvent::ConsensusReached {
                        round,
                        consensus_score: judge.consensus_score,
                        summary: judge.summary.clone(),
                    })
                    .await;
            }
            break;
        }

        last_judge = Some(judge);
    }

    all_rounds
}

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
        assert_eq!(
            decision.quick_synthesis_hint.as_deref(),
            Some("Recommend diversified portfolio")
        );
    }

    #[test]
    fn test_parse_judge_decision_malformed_returns_default() {
        let decision = parse_judge_json("not json at all");
        assert_eq!(decision.decision, "continue");
        assert_eq!(decision.consensus_score, 0.0);
    }

    #[test]
    fn test_parse_judge_decision_wrapped_in_markdown() {
        let text = "```json\n{\"consensus_score\":80,\"decision\":\"final_round\",\"speaking_order\":[],\"challenges\":{},\"reasoning\":\"Near consensus\",\"summary\":\"Agreed\",\"quick_synthesis_hint\":null}\n```";
        let decision = parse_judge_json(text);
        assert_eq!(decision.consensus_score, 80.0);
        assert_eq!(decision.decision, "final_round");
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
            id: "1".into(),
            session_key: "s".into(),
            squad_id: "sq".into(),
            round: 1,
            persona_id: "p1".into(),
            persona_name: "Analyst".into(),
            entry_type: "opening".into(),
            content: "Index funds win.".into(),
            confidence: 0.9,
            references_entry_id: None,
            created_at: "now".into(),
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

    #[test]
    fn test_build_final_prompt() {
        let prompt = build_phase_prompt(
            "System",
            "Question",
            "Persona",
            "Role",
            "Perspective",
            "Tone",
            &[],
            4,
            DebatePhase::Final,
            None,
        );
        assert!(prompt.contains("final round"));
        assert!(prompt.contains("2-3 sentences"));
    }

    #[test]
    fn test_debate_phase_as_str() {
        assert_eq!(DebatePhase::Opening.as_str(), "opening");
        assert_eq!(DebatePhase::Discussion.as_str(), "discussion");
        assert_eq!(DebatePhase::Targeted.as_str(), "targeted");
        assert_eq!(DebatePhase::Final.as_str(), "final");
    }
}
