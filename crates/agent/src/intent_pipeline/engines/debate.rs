//! DebateOrchestrator — multi-round persona debate with consensus detection.
//!
//! Flow:
//! 1. Round 1: parallel persona fan-out (same as squad.rs)
//! 2. Write persona outputs to blackboard
//! 3. Estimate consensus score
//! 4. If consensus < threshold AND round < max_rounds: goto step 1 with blackboard context
//! 5. Final synthesis incorporating all rounds

use cognitive::{BlackboardEntry, BlackboardRepo, NewBlackboardEntry, PersonaRow};
use providers::{ChatParams, DynProvider, Message, UserContent};

/// Default consensus threshold — debate stops when exceeded.
pub const DEFAULT_CONSENSUS_THRESHOLD: f64 = 0.75;
/// Default maximum debate rounds.
pub const DEFAULT_MAX_ROUNDS: u32 = 3;

/// Estimate consensus from persona responses using word-overlap heuristic.
///
/// Computes pairwise Jaccard similarity of response word sets, averaged.
/// Returns 0.0 (no overlap) to 1.0 (identical).
pub fn estimate_consensus(responses: &[(String, String)]) -> f64 {
    if responses.len() < 2 {
        return 1.0;
    }
    let word_sets: Vec<std::collections::HashSet<&str>> = responses
        .iter()
        .map(|(_, content)| {
            content
                .split_whitespace()
                .map(|w| w.trim_matches(|c: char| !c.is_alphanumeric()))
                .filter(|w| w.len() > 3)
                .collect()
        })
        .collect();

    let mut total_sim = 0.0;
    let mut pairs = 0;
    for i in 0..word_sets.len() {
        for j in (i + 1)..word_sets.len() {
            let intersection = word_sets[i].intersection(&word_sets[j]).count();
            let union = word_sets[i].union(&word_sets[j]).count();
            if union > 0 {
                total_sim += intersection as f64 / union as f64;
            }
            pairs += 1;
        }
    }
    if pairs == 0 {
        1.0
    } else {
        total_sim / pairs as f64
    }
}

/// Build a debate-round persona prompt that includes blackboard context.
pub fn build_debate_round_prompt(
    orchestrator_context: &str,
    user_message: &str,
    persona_name: &str,
    persona_role: &str,
    persona_perspective: &str,
    persona_tone: &str,
    blackboard: &[BlackboardEntry],
    current_round: u32,
) -> String {
    let blackboard_context = BlackboardRepo::format_for_prompt(blackboard);
    format!(
        r#"{orchestrator_context}
{blackboard_context}

---

You are now responding as **{persona_name}**, a {persona_role}.
Your perspective: {persona_perspective}
Your tone should be: {persona_tone}

This is **Round {current_round}** of a multi-round debate. You can see what other personas said in prior rounds above.

Rules for this round:
- Reference specific points from other personas' prior contributions
- If you agree with someone, say so explicitly and build on their point
- If you disagree, explain why with evidence
- If your position has changed due to others' arguments, acknowledge the shift
- Be direct and specific. Avoid generic statements.

Respond to: {user_message}"#
    )
}

/// Run a full multi-round debate.
///
/// Returns: Vec of (round_number, persona_responses, consensus_score) per round.
pub async fn run_debate(
    provider: &DynProvider,
    orchestrator_context: &str,
    user_message: &str,
    personas: &[PersonaRow],
    params: &ChatParams,
    blackboard_repo: &BlackboardRepo,
    session_key: &str,
    squad_id: &str,
    max_rounds: u32,
    consensus_threshold: f64,
    event_tx: Option<&tokio::sync::mpsc::Sender<crate::AgentEvent>>,
) -> Vec<(u32, Vec<(String, String)>, f64)> {
    let mut all_rounds: Vec<(u32, Vec<(String, String)>, f64)> = Vec::new();

    for round in 1..=max_rounds {
        // Emit round started
        if let Some(tx) = event_tx {
            let _ = tx
                .send(crate::AgentEvent::DebateRoundStarted {
                    round,
                    total_rounds: max_rounds,
                })
                .await;
        }

        // Load blackboard from prior rounds
        let blackboard = blackboard_repo
            .list_for_session(session_key)
            .await
            .unwrap_or_default();

        // Fan out to all personas with debate-aware prompts
        let futures: Vec<_> = personas
            .iter()
            .map(|persona| {
                let provider = provider.clone();
                let params = params.clone();
                let system = build_debate_round_prompt(
                    orchestrator_context,
                    user_message,
                    &persona.name,
                    &persona.role,
                    &persona.perspective,
                    &persona.tone,
                    &blackboard,
                    round,
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
                    let result = provider.chat(&messages, None, &params).await;
                    let text = match result {
                        Ok(r) => r.content.unwrap_or_default(),
                        Err(e) => {
                            tracing::warn!(persona = %persona_name, round, "Debate LLM call failed: {e}");
                            String::new()
                        }
                    };

                    if let Some(tx) = &tx {
                        let _ = tx
                            .send(crate::AgentEvent::PersonaPerspective {
                                persona_id: persona_id.clone(),
                                persona_name: persona_name.clone(),
                                persona_icon: persona_icon.clone(),
                                persona_role: persona_role.clone(),
                                content: text.clone(),
                            })
                            .await;
                    }

                    (persona_id, persona_name, text)
                }
            })
            .collect();

        let round_results = futures_util::future::join_all(futures).await;

        // Write to blackboard
        for (pid, pname, content) in &round_results {
            if !content.is_empty() {
                let _ = blackboard_repo
                    .insert(&NewBlackboardEntry {
                        session_key,
                        squad_id,
                        round: round as i64,
                        persona_id: pid,
                        persona_name: pname,
                        entry_type: if round == 1 { "observation" } else { "response" },
                        content,
                        confidence: 0.8,
                        references_entry_id: None,
                    })
                    .await;
            }
        }

        // Build (name, content) pairs for consensus check
        let responses: Vec<(String, String)> = round_results
            .into_iter()
            .map(|(_, name, content)| (name, content))
            .collect();

        let consensus = estimate_consensus(&responses);

        // Emit round completed
        if let Some(tx) = event_tx {
            let _ = tx
                .send(crate::AgentEvent::DebateRoundCompleted {
                    round,
                    consensus_score: consensus,
                })
                .await;
        }

        all_rounds.push((round, responses, consensus));

        // Check for early termination
        if consensus >= consensus_threshold {
            if let Some(tx) = event_tx {
                let _ = tx
                    .send(crate::AgentEvent::ConsensusReached {
                        round,
                        consensus_score: consensus,
                        summary: format!(
                            "Consensus reached after {round} rounds (score: {consensus:.2})"
                        ),
                    })
                    .await;
            }
            break;
        }
    }

    all_rounds
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_consensus_score_agreement() {
        // Responses that share many identical key terms
        let responses = vec![
            (
                "Analyst".into(),
                "Index funds provide strong long-term returns with lower fees than active management funds overall.".into(),
            ),
            (
                "Skeptic".into(),
                "Index funds provide strong long-term returns with lower risk than individual stock picking overall.".into(),
            ),
            (
                "Strategist".into(),
                "Index funds provide strong long-term returns with lower volatility than alternatives overall.".into(),
            ),
        ];
        let score = estimate_consensus(&responses);
        // High word overlap → high consensus
        assert!(score > 0.4, "Expected high consensus, got {score}");
    }

    #[test]
    fn test_consensus_score_disagreement() {
        let responses = vec![
            (
                "Analyst".into(),
                "Cryptocurrency mining rigs generate passive blockchain revenue through proof-of-work algorithms.".into(),
            ),
            (
                "Skeptic".into(),
                "Municipal government bonds provide guaranteed coupon payments backed by taxing authority.".into(),
            ),
            (
                "Strategist".into(),
                "Agricultural farmland produces rental income while appreciating through topsoil development.".into(),
            ),
        ];
        let score = estimate_consensus(&responses);
        assert!(score < 0.4, "Expected low consensus, got {score}");
    }

    #[test]
    fn test_consensus_single_response() {
        let responses = vec![("Analyst".into(), "Some response.".into())];
        assert!((estimate_consensus(&responses) - 1.0).abs() < f64::EPSILON);
    }

    #[test]
    fn test_consensus_empty_responses() {
        let responses: Vec<(String, String)> = vec![];
        assert!((estimate_consensus(&responses) - 1.0).abs() < f64::EPSILON);
    }

    #[test]
    fn test_build_debate_prompt_includes_blackboard() {
        let blackboard = vec![BlackboardEntry {
            id: "1".into(),
            session_key: "s".into(),
            squad_id: "sq".into(),
            round: 1,
            persona_id: "p1".into(),
            persona_name: "Analyst".into(),
            entry_type: "observation".into(),
            content: "Index funds beat 80% of managers.".into(),
            confidence: 0.9,
            references_entry_id: None,
            created_at: "now".into(),
        }];
        let prompt = build_debate_round_prompt(
            "System context",
            "Should I invest in index funds?",
            "Skeptic",
            "Critical analyst",
            "Questions claims",
            "direct",
            &blackboard,
            2,
        );
        assert!(prompt.contains("Prior Debate Rounds"));
        assert!(prompt.contains("Analyst"));
        assert!(prompt.contains("Index funds beat 80%"));
        assert!(prompt.contains("Round 2"));
    }

    #[test]
    fn test_build_debate_prompt_no_blackboard() {
        let prompt = build_debate_round_prompt(
            "System",
            "Question",
            "Persona",
            "Role",
            "Perspective",
            "Tone",
            &[],
            1,
        );
        assert!(!prompt.contains("Prior Debate Rounds"));
        assert!(prompt.contains("Round 1"));
    }
}
