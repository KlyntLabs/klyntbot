//! Room-style debate orchestrator — multi-round persona conversation with LLM judge.
//!
//! Two APIs:
//! - `run_debate` (new) — unified engine using `DebateConfig`, `DebateContext`, `DebateResult`
//!   with learned weights, token tracking, budget checks, and accuracy outcomes.
//! - `_run_room_debate_legacy` — the original orchestrator kept temporarily for callers
//!   that haven't migrated yet.
//!
//! Four phases:
//! 1. Opening (parallel): all personas give initial positions
//! 2. Discussion (sequential): personas respond to each other, full responses
//! 3. Targeted (sequential): judge gives specific challenges, short responses
//! 4. Final (parallel): concise final position statements

use std::collections::HashMap;

use cognitive::{
    alignment_to_fsrs_rating, BlackboardEntry, BlackboardRepo, NewBlackboardEntry,
    PersonaAccuracyRepo, PersonaRow, ResolvedSquad,
};
use providers::{ChatParams, DynProvider, Message, UserContent};
use serde::{Deserialize, Serialize};
use tokio::sync::{mpsc, oneshot};
use tokio_util::sync::CancellationToken;

use super::debate_types::*;

// ── Legacy types (kept for `_run_room_debate_legacy`) ──────────────────────

/// (round_number, persona_responses as (name, content), consensus_score) per round.
type DebateRounds = Vec<(u32, Vec<(String, String)>, f64)>;

/// Safety cap — maximum total rounds including opening and final.
const MAX_ROUNDS: u32 = 6;

/// Consensus threshold — judge score 0-100, debate stops at this level.
const CONSENSUS_THRESHOLD: f64 = 85.0;

/// Average tokens per persona per round (for budget estimation).
const AVG_TOKENS_PER_PERSONA_ROUND: u64 = 800;

/// Legacy debate phase — used by `_run_room_debate_legacy`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LegacyDebatePhase {
    Opening,
    Discussion,
    Targeted,
    Final,
}

impl LegacyDebatePhase {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Opening => "opening",
            Self::Discussion => "discussion",
            Self::Targeted => "targeted",
            Self::Final => "final",
        }
    }
}

/// Legacy structured decision from the LLM judge.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LegacyJudgeDecision {
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

impl Default for LegacyJudgeDecision {
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

// ── Shared helpers ─────────────────────────────────────────────────────────

/// Parse judge JSON response, returning default on failure.
pub fn parse_judge_json(text: &str) -> LegacyJudgeDecision {
    let json_str = common::helpers::extract_json_object(text).unwrap_or(text);
    serde_json::from_str(json_str).unwrap_or_default()
}

/// Parse judge JSON into the new `JudgeDecision` type.
fn parse_judge_json_v2(text: &str) -> JudgeDecision {
    let json_str = common::helpers::extract_json_object(text).unwrap_or(text);
    serde_json::from_str(json_str).unwrap_or(JudgeDecision {
        consensus_score: 0.0,
        decision: "continue".to_string(),
        speaking_order: Vec::new(),
        challenges: HashMap::new(),
    })
}

/// Build a persona system prompt with full persona context.
///
/// Combines the orchestrator skill prompt with persona-specific information
/// including expertise, cognitive bias, analysis frameworks, and optional
/// cognitive context from the memory system.
pub fn build_persona_system_prompt(
    skill_prompt: &str,
    persona: &PersonaRow,
    cognitive_context: Option<&str>,
) -> String {
    let mut prompt = format!(
        r#"{skill_prompt}

---

You are responding as **{name}**, a {role}.
Expertise: {expertise}
Perspective: {perspective}
Tone: {tone}
Questioning style: {questioning_style}
Cognitive bias awareness: {cognitive_bias}
Analysis frameworks: {analysis_frameworks}"#,
        name = persona.name,
        role = persona.role,
        expertise = persona.expertise,
        perspective = persona.perspective,
        tone = persona.tone,
        questioning_style = persona.questioning_style,
        cognitive_bias = persona.cognitive_bias,
        analysis_frameworks = persona.analysis_frameworks,
    );

    if let Some(ctx) = cognitive_context {
        if !ctx.is_empty() {
            prompt.push_str(&format!(
                "\n\n<cognitive_context>\n{ctx}\n</cognitive_context>"
            ));
        }
    }

    prompt
}

/// Build a phase-specific prompt that layers on top of the persona system prompt.
fn build_debate_phase_prompt(
    base_system: &str,
    user_message: &str,
    blackboard: &[BlackboardEntry],
    round: u8,
    phase: DebatePhase,
    challenge: Option<&str>,
) -> String {
    let blackboard_context = BlackboardRepo::format_for_prompt(blackboard);

    let phase_instructions = match phase {
        DebatePhase::Opening => format!(
            "This is **Round {round}** — the opening round. Share your initial position on the topic. Be clear about your stance and reasoning."
        ),
        DebatePhase::Discussion => format!(
            "This is **Round {round}** of a live discussion. You can see what other personas said above.\n\n\
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
                "This is **Round {round}** — a targeted exchange. Address this specific point concisely (50-150 words):\n\n\
                 > {challenge_text}"
            )
        }
        DebatePhase::Final => "This is the **final round**. Summarize your final position in 2-3 sentences, accounting for the full discussion above.".to_string(),
    };

    format!(
        r#"{base_system}
{blackboard_context}

{phase_instructions}

Respond to: {user_message}"#
    )
}

/// Build the synthesis prompt from persona responses and learned weights.
pub fn build_synthesis_prompt(
    user_message: &str,
    persona_responses: &[PersonaResponse],
    learned_weights: &[LearnedWeight],
    output_mode: OutputMode,
) -> String {
    let mut voices = String::new();
    for resp in persona_responses {
        let weight_note = learned_weights
            .iter()
            .find(|w| w.persona_id == resp.persona_id)
            .map(|w| format!(" (weight: {:.2})", w.blended_weight))
            .unwrap_or_default();
        voices.push_str(&format!(
            "**{name}** ({role}){weight_note}:\n{content}\n\n",
            name = resp.persona_name,
            role = resp.persona_role,
            content = resp.content,
        ));
    }

    let mode_instruction = match output_mode {
        OutputMode::Synthesized => {
            "Integrate all perspectives into a single coherent response. \
             Resolve contradictions by explaining trade-offs. \
             Provide a clear, actionable recommendation. \
             Credit specific personas when their insight is particularly valuable. \
             Respond directly to the user — do not describe the synthesis process."
        }
        OutputMode::StructuredPerPersona => {
            "Preserve each persona's perspective under a ## {Name} — {Role} header. \
             Summarize their key points, noting where they agree or disagree with others. \
             End with a brief synthesis section that highlights the strongest points."
        }
    };

    format!(
        r#"You received the following perspectives from multiple expert personas analyzing a user's message.

User's message: {user_message}

Persona perspectives:
{voices}

{mode_instruction}"#
    )
}

/// Call the LLM judge and return the new-type JudgeDecision plus token usage.
async fn call_judge_v2(
    provider: &DynProvider,
    responses: &[PersonaResponse],
    params: &ChatParams,
    user_message: &str,
    round: u8,
    persona_ids: &[String],
) -> (JudgeDecision, u64) {
    let mut response_text = String::new();
    for resp in responses {
        let truncated = common::truncate_chars(&resp.content, 500, "");
        response_text.push_str(&format!("**{}**: {truncated}\n\n", resp.persona_name));
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
  "challenges": {{<persona_id: "specific question or challenge for this persona">}}
}}

Rules:
- speaking_order: put the persona who was most challenged or had the weakest argument first
- challenges: give each persona a specific, pointed question that advances the debate
- decision: use "continue" unless personas are clearly converging (70+) or stuck repeating themselves

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
            let tokens = u64::from(result.usage.total_tokens);
            let text = result.content.unwrap_or_default();
            (parse_judge_json_v2(&text), tokens)
        }
        Err(e) => {
            tracing::warn!("LLM judge call failed: {e}");
            (
                JudgeDecision {
                    consensus_score: 0.0,
                    decision: "continue".to_string(),
                    speaking_order: persona_ids.to_vec(),
                    challenges: HashMap::new(),
                },
                0,
            )
        }
    }
}

/// Run parallel persona calls for Opening/Final phases.
///
/// Returns `PersonaResponse` entries and total tokens used.
#[allow(clippy::too_many_arguments)]
async fn debate_fan_out(
    provider: &DynProvider,
    personas: &[PersonaRow],
    system_prompts: &HashMap<String, String>,
    params: &ChatParams,
    blackboard: &[BlackboardEntry],
    user_message: &str,
    round: u8,
    phase: DebatePhase,
    event_tx: &mpsc::Sender<DebateEvent>,
) -> (Vec<PersonaResponse>, u64) {
    let futures: Vec<_> = personas
        .iter()
        .map(|persona| {
            let provider = provider.clone();
            let params = params.clone();
            let base_system = system_prompts.get(&persona.id).cloned().unwrap_or_default();
            let system = build_debate_phase_prompt(
                &base_system,
                user_message,
                blackboard,
                round,
                phase,
                None,
            );
            let user_msg = user_message.to_string();
            let persona_id = persona.id.clone();
            let persona_name = persona.name.clone();
            let persona_icon = persona.icon.clone();
            let persona_role = persona.role.clone();
            let tx = event_tx.clone();

            async move {
                let messages = vec![
                    Message::System { content: system },
                    Message::User {
                        content: UserContent::Text(user_msg),
                    },
                ];
                let (text, tokens) = match provider.chat(&messages, None, &params).await {
                    Ok(r) => (
                        r.content.unwrap_or_default(),
                        u64::from(r.usage.total_tokens),
                    ),
                    Err(e) => {
                        tracing::warn!(persona = %persona_name, round, "LLM call failed: {e}");
                        (String::new(), 0u64)
                    }
                };

                let _ = tx
                    .send(DebateEvent::PersonaPerspective {
                        persona_id: persona_id.clone(),
                        name: persona_name.clone(),
                        icon: persona_icon.clone(),
                        role: persona_role.clone(),
                        content: text.clone(),
                        round,
                        challenge: None,
                    })
                    .await;

                let response = PersonaResponse {
                    persona_id,
                    persona_name,
                    persona_icon,
                    persona_role,
                    content: text,
                    round,
                    phase,
                    confidence: 0.8,
                    effective_confidence: 0.8,
                    consensus_alignment: None,
                };
                (response, tokens)
            }
        })
        .collect();

    let results = futures_util::future::join_all(futures).await;
    let total_tokens: u64 = results.iter().map(|(_, t)| t).sum();
    let responses: Vec<PersonaResponse> = results.into_iter().map(|(r, _)| r).collect();
    (responses, total_tokens)
}

/// Run sequential persona calls for Discussion/Targeted phases.
///
/// Each persona reads the blackboard before speaking. Returns `PersonaResponse`
/// entries and total tokens used.
#[allow(clippy::too_many_arguments)]
async fn debate_sequential_round(
    provider: &DynProvider,
    personas: &[PersonaRow],
    system_prompts: &HashMap<String, String>,
    params: &ChatParams,
    blackboard_repo: &BlackboardRepo,
    session_key: &str,
    squad_id: &str,
    user_message: &str,
    round: u8,
    phase: DebatePhase,
    speaking_order: &[String],
    challenges: &HashMap<String, String>,
    event_tx: &mpsc::Sender<DebateEvent>,
) -> (Vec<PersonaResponse>, u64) {
    let mut results = Vec::new();
    let mut total_tokens: u64 = 0;

    // Read blackboard once at the start; we'll append each speaker's entry locally.
    let mut blackboard = blackboard_repo
        .list_for_session(session_key)
        .await
        .unwrap_or_default();

    for persona_id in speaking_order {
        let Some(persona) = personas.iter().find(|p| p.id == *persona_id) else {
            continue;
        };

        let challenge = challenges.get(persona_id.as_str()).map(|s| s.as_str());

        let base_system = system_prompts.get(persona_id).cloned().unwrap_or_default();
        let system = build_debate_phase_prompt(
            &base_system,
            user_message,
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

        let (text, tokens) = match provider.chat(&messages, None, params).await {
            Ok(r) => (
                r.content.unwrap_or_default(),
                u64::from(r.usage.total_tokens),
            ),
            Err(e) => {
                tracing::warn!(persona = %persona.name, round, "LLM call failed, skipping: {e}");
                let _ = event_tx
                    .send(DebateEvent::PersonaPerspective {
                        persona_id: persona.id.clone(),
                        name: persona.name.clone(),
                        icon: persona.icon.clone(),
                        role: persona.role.clone(),
                        content: "[Persona unavailable this round]".to_string(),
                        round,
                        challenge: challenge.map(|s| s.to_string()),
                    })
                    .await;
                continue;
            }
        };

        total_tokens += tokens;

        // Emit event immediately (user sees responses appear one by one)
        let _ = event_tx
            .send(DebateEvent::PersonaPerspective {
                persona_id: persona.id.clone(),
                name: persona.name.clone(),
                icon: persona.icon.clone(),
                role: persona.role.clone(),
                content: text.clone(),
                round,
                challenge: challenge.map(|s| s.to_string()),
            })
            .await;

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

        // Append to local blackboard vec so the next speaker sees this without a DB read.
        blackboard.push(BlackboardEntry {
            id: String::new(),
            session_key: session_key.to_string(),
            squad_id: squad_id.to_string(),
            round: round as i64,
            persona_id: persona.id.clone(),
            persona_name: persona.name.clone(),
            entry_type: phase.as_str().to_string(),
            content: text.clone(),
            confidence: 0.8,
            references_entry_id: None,
            created_at: String::new(),
        });

        results.push(PersonaResponse {
            persona_id: persona.id.clone(),
            persona_name: persona.name.clone(),
            persona_icon: persona.icon.clone(),
            persona_role: persona.role.clone(),
            content: text,
            round,
            phase,
            confidence: 0.8,
            effective_confidence: 0.8,
            consensus_alignment: None,
        });
    }

    (results, total_tokens)
}

/// Score consensus alignment for each persona via a separate LLM call.
///
/// Returns a map of persona_id -> alignment (0.0-1.0).
async fn score_consensus_alignment(
    provider: &DynProvider,
    synthesis: &str,
    persona_responses: &[PersonaResponse],
    params: &ChatParams,
) -> (HashMap<String, f64>, u64) {
    let mut response_summaries = String::new();
    for resp in persona_responses {
        let truncated = common::truncate_chars(&resp.content, 300, "");
        response_summaries.push_str(&format!(
            "- {id} ({name}): {truncated}\n",
            id = resp.persona_id,
            name = resp.persona_name,
        ));
    }

    let prompt = format!(
        r#"Given this synthesis and the individual persona positions, rate how much each persona's position was reflected in the final synthesis.

Synthesis:
{synthesis}

Persona positions:
{response_summaries}

Return a JSON object mapping persona_id to a score from 0.0 to 1.0:
{{ "persona_id_1": 0.85, "persona_id_2": 0.6 }}

Output ONLY the JSON object."#
    );

    let messages = vec![Message::User {
        content: UserContent::Text(prompt),
    }];

    let scoring_params = ChatParams {
        max_tokens: Some(300),
        temperature: Some(0.0),
        ..params.clone()
    };

    match provider.chat(&messages, None, &scoring_params).await {
        Ok(result) => {
            let tokens = u64::from(result.usage.total_tokens);
            let text = result.content.unwrap_or_default();
            let json_str = common::helpers::extract_json_object(&text).unwrap_or(&text);
            let scores: HashMap<String, f64> = serde_json::from_str(json_str).unwrap_or_default();
            (scores, tokens)
        }
        Err(e) => {
            tracing::warn!("Consensus alignment scoring failed: {e}");
            (HashMap::new(), 0)
        }
    }
}

// FSRS rating mapping delegated to `cognitive::alignment_to_fsrs_rating`.

// ── New unified debate engine ──────────────────────────────────────────────

/// Run a multi-round debate with persona learning, budget checks, and structured output.
///
/// This is the unified entry point for all debate execution. It replaces the legacy
/// `run_room_debate` function with full support for:
/// - Budget pre-checks and approval flow
/// - Learned persona weights from accuracy history
/// - Structured `DebateResult` with token tracking
/// - Event streaming via `DebateEvent`
/// - Post-synthesis consensus scoring and FSRS accuracy outcomes
/// - Timeout handling with partial results
#[allow(clippy::too_many_arguments)]
pub async fn run_debate(
    config: DebateConfig,
    context: DebateContext,
    squad: ResolvedSquad,
    provider: &DynProvider,
    blackboard_repo: &BlackboardRepo,
    accuracy_repo: &PersonaAccuracyRepo,
    event_tx: mpsc::Sender<DebateEvent>,
    approval_rx: Option<oneshot::Receiver<bool>>,
    cancel: CancellationToken,
) -> common::Result<DebateResult> {
    let personas = &squad.personas;
    let squad_id = &squad.squad.id;
    let max_rounds = config.max_rounds;
    let primary_domain = context.domains.first().cloned().unwrap_or("general".into());

    // Build ChatParams
    let params = ChatParams {
        model: provider.default_model().to_string(),
        temperature: config.temperature_override,
        max_tokens: None,
        response_format: None,
    };

    let mut effective_max_rounds = max_rounds;

    // ── Step 1: Budget pre-check ───────────────────────────────────────────
    let estimated_cost =
        personas.len() as u64 * u64::from(max_rounds) * AVG_TOKENS_PER_PERSONA_ROUND;

    if let Some(ref budget) = config.token_budget {
        let action = if estimated_cost > budget.remaining_monthly {
            BudgetAction::RequiresApproval
        } else if let Some(daily_cap) = budget.daily_squad_cap {
            if estimated_cost > daily_cap {
                // Reduce rounds to fit within daily cap
                let affordable_rounds = (daily_cap
                    / (personas.len() as u64 * AVG_TOKENS_PER_PERSONA_ROUND))
                    .max(2) as u8;
                if affordable_rounds < max_rounds {
                    BudgetAction::ReducedRounds {
                        from: max_rounds,
                        to: affordable_rounds,
                    }
                } else {
                    BudgetAction::Proceed
                }
            } else {
                BudgetAction::Proceed
            }
        } else {
            BudgetAction::Proceed
        };

        let _ = event_tx
            .send(DebateEvent::BudgetCheck {
                estimated_cost,
                remaining: budget.remaining_monthly,
                action,
            })
            .await;

        if action == BudgetAction::RequiresApproval {
            if let Some(rx) = approval_rx {
                match rx.await {
                    Ok(true) => { /* approved, continue */ }
                    _ => return Ok(DebateResult::empty_cancelled()),
                }
            } else {
                // No approval channel provided, cancel
                return Ok(DebateResult::empty_cancelled());
            }
        }

        if let BudgetAction::ReducedRounds { to, .. } = action {
            effective_max_rounds = to;
        }
    }

    // ── Step 2: Load learned weights ───────────────────────────────────────
    let mut learned_weights = Vec::new();
    let persona_ids: Vec<String> = personas.iter().map(|p| p.id.clone()).collect();

    for persona in personas {
        let accuracy = accuracy_repo
            .get(&persona.id, squad_id, &primary_domain)
            .await
            .unwrap_or(None);

        let (accuracy_score, total_debates) = match &accuracy {
            Some(a) => (a.accuracy_rate(), a.total_debates as u32),
            None => (0.5, 0), // neutral default
        };

        let confidence_in_score = (total_debates as f64 / 20.0).min(1.0);
        let effective_blend = config.accuracy_blend as f64 * confidence_in_score;
        let blended_weight =
            (1.0 - effective_blend) * persona.relevance_score + effective_blend * accuracy_score;

        learned_weights.push(LearnedWeight {
            persona_id: persona.id.clone(),
            base_weight: persona.relevance_score,
            accuracy_weight: accuracy_score,
            blended_weight,
            domain: primary_domain.clone(),
            debates_used: total_debates,
        });
    }

    // ── Step 3: Build persona system prompts ───────────────────────────────
    let mut system_prompts: HashMap<String, String> = HashMap::new();
    for persona in personas {
        let prompt = build_persona_system_prompt(
            &context.skill_prompt,
            persona,
            context.cognitive_context.as_deref(),
        );
        system_prompts.insert(persona.id.clone(), prompt);
    }

    // ── Step 4: Debate loop ────────────────────────────────────────────────
    let debate_session_key = format!("debate:{}:{}", squad_id, uuid::Uuid::new_v4());
    let mut token_usage = TokenUsage::default();
    let mut all_rounds: Vec<DebateRound> = Vec::new();
    let mut all_responses: Vec<PersonaResponse> = Vec::new();
    let mut last_judge: Option<JudgeDecision> = None;
    let mut consensus_reached = false;
    let mut final_consensus_score = 0.0;
    let mut partial_reason: Option<PartialReason> = None;

    let debate_result = tokio::time::timeout(
        std::time::Duration::from_secs(config.timeout_seconds),
        async {
            let mut round: u8 = 0;

            loop {
                round += 1;

                // Check cancellation
                if cancel.is_cancelled() {
                    tracing::warn!("Debate cancelled at round {round}");
                    partial_reason = Some(PartialReason::Cancelled);
                    break;
                }

                // Determine phase
                let phase = if round == 1 {
                    DebatePhase::Opening
                } else if let Some(ref judge) = last_judge {
                    if judge.decision == "final_round" || round >= effective_max_rounds {
                        DebatePhase::Final
                    } else if round == 2 {
                        DebatePhase::Discussion
                    } else {
                        DebatePhase::Targeted
                    }
                } else {
                    DebatePhase::Discussion
                };

                // Determine speaking order
                let speaking_order = last_judge
                    .as_ref()
                    .map(|j| j.speaking_order.clone())
                    .unwrap_or_else(|| persona_ids.clone());

                let order_reason = if round == 1 {
                    "Initial order".to_string()
                } else {
                    last_judge
                        .as_ref()
                        .map(|_| "Judge-determined order".to_string())
                        .unwrap_or_else(|| "Default order".to_string())
                };

                // Emit round started
                let _ = event_tx
                    .send(DebateEvent::RoundStarted {
                        round,
                        phase,
                        persona_order: speaking_order.clone(),
                        order_reason: order_reason.clone(),
                    })
                    .await;

                // Execute round
                let (round_responses, round_tokens) = match phase {
                    DebatePhase::Opening | DebatePhase::Final => {
                        // Round 1 blackboard is guaranteed empty (session just created).
                        let blackboard = if round > 1 {
                            blackboard_repo
                                .list_for_session(&debate_session_key)
                                .await
                                .unwrap_or_default()
                        } else {
                            Vec::new()
                        };

                        let (responses, tokens) = debate_fan_out(
                            provider,
                            personas,
                            &system_prompts,
                            &params,
                            &blackboard,
                            &context.user_message,
                            round,
                            phase,
                            &event_tx,
                        )
                        .await;

                        // Write parallel results to blackboard
                        for resp in &responses {
                            if !resp.content.is_empty() {
                                let _ = blackboard_repo
                                    .insert(&NewBlackboardEntry {
                                        session_key: &debate_session_key,
                                        squad_id,
                                        round: round as i64,
                                        persona_id: &resp.persona_id,
                                        persona_name: &resp.persona_name,
                                        entry_type: phase.as_str(),
                                        content: &resp.content,
                                        confidence: 0.8,
                                        references_entry_id: None,
                                    })
                                    .await;
                            }
                        }
                        (responses, tokens)
                    }
                    DebatePhase::Discussion | DebatePhase::Targeted => {
                        let challenges = last_judge
                            .as_ref()
                            .map(|j| j.challenges.clone())
                            .unwrap_or_default();

                        debate_sequential_round(
                            provider,
                            personas,
                            &system_prompts,
                            &params,
                            blackboard_repo,
                            &debate_session_key,
                            squad_id,
                            &context.user_message,
                            round,
                            phase,
                            &speaking_order,
                            &challenges,
                            &event_tx,
                        )
                        .await
                    }
                };

                token_usage.add_persona(round_tokens);

                // Final phase — no judge needed
                if phase == DebatePhase::Final {
                    let _ = event_tx
                        .send(DebateEvent::RoundCompleted { round, phase })
                        .await;
                    let _ = event_tx
                        .send(DebateEvent::ConsensusReached {
                            round,
                            score: 100.0,
                        })
                        .await;

                    all_rounds.push(DebateRound {
                        round,
                        phase,
                        responses: round_responses.clone(),
                        judge_decision: None,
                        persona_order: speaking_order,
                        order_reason,
                    });
                    all_responses.extend(round_responses);
                    consensus_reached = true;
                    final_consensus_score = 100.0;
                    break;
                }

                // Call judge
                let (judge, judge_tokens) = call_judge_v2(
                    provider,
                    &round_responses,
                    &params,
                    &context.user_message,
                    round,
                    &persona_ids,
                )
                .await;
                token_usage.add_judge(judge_tokens);

                // Emit judge decision
                let _ = event_tx
                    .send(DebateEvent::JudgeDecision {
                        round,
                        consensus_score: judge.consensus_score,
                        speaking_order: judge.speaking_order.clone(),
                        decision: judge.decision.clone(),
                    })
                    .await;

                // Store judge on blackboard
                let judge_json = serde_json::to_string(&judge).unwrap_or_default();
                let _ = blackboard_repo
                    .insert(&NewBlackboardEntry {
                        session_key: &debate_session_key,
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

                let _ = event_tx
                    .send(DebateEvent::RoundCompleted { round, phase })
                    .await;

                all_rounds.push(DebateRound {
                    round,
                    phase,
                    responses: round_responses.clone(),
                    judge_decision: Some(judge.clone()),
                    persona_order: speaking_order,
                    order_reason,
                });
                all_responses.extend(round_responses);

                // Check for early consensus
                if judge.decision == "stop" || judge.consensus_score >= config.consensus_threshold {
                    consensus_reached = true;
                    final_consensus_score = judge.consensus_score;
                    let _ = event_tx
                        .send(DebateEvent::ConsensusReached {
                            round,
                            score: judge.consensus_score,
                        })
                        .await;

                    partial_reason = Some(PartialReason::ConsensusEarly { at_round: round });
                    break;
                }

                last_judge = Some(judge);
            }
        },
    )
    .await;

    // Handle timeout
    if debate_result.is_err() {
        tracing::warn!("Debate timed out after {}s", config.timeout_seconds);
        partial_reason = Some(PartialReason::Timeout {
            elapsed_seconds: config.timeout_seconds,
        });
        let _ = event_tx
            .send(DebateEvent::DebatePartial {
                reason: PartialReason::Timeout {
                    elapsed_seconds: config.timeout_seconds,
                },
                completed_rounds: all_rounds.len() as u8,
            })
            .await;
    }

    // ── Step 5: Synthesis ──────────────────────────────────────────────────
    let _ = event_tx.send(DebateEvent::SynthesisStarted).await;

    // Collect the final round's responses for synthesis (use all if short debate)
    let final_responses: Vec<&PersonaResponse> = if all_responses.is_empty() {
        Vec::new()
    } else {
        // Use the last round's responses, or all if only 1 round
        let last_round = all_rounds.last().map(|r| r.round).unwrap_or(1);
        let from_last: Vec<&PersonaResponse> = all_responses
            .iter()
            .filter(|r| r.round == last_round)
            .collect();
        if from_last.is_empty() {
            all_responses.iter().collect()
        } else {
            from_last
        }
    };

    let synthesis = if final_responses.is_empty() {
        String::new()
    } else {
        let synthesis_input: Vec<PersonaResponse> =
            final_responses.iter().map(|r| (*r).clone()).collect();
        let synthesis_prompt = build_synthesis_prompt(
            &context.user_message,
            &synthesis_input,
            &learned_weights,
            config.output_mode,
        );

        let synth_messages = vec![Message::User {
            content: UserContent::Text(synthesis_prompt),
        }];

        let synth_params = ChatParams {
            temperature: Some(0.3),
            ..params.clone()
        };

        match provider.chat(&synth_messages, None, &synth_params).await {
            Ok(result) => {
                let synth_tokens = u64::from(result.usage.total_tokens);
                token_usage.add_synthesis(synth_tokens);
                let content = result.content.unwrap_or_default();
                let _ = event_tx
                    .send(DebateEvent::SynthesisChunk {
                        content: content.clone(),
                    })
                    .await;
                content
            }
            Err(e) => {
                tracing::warn!("Synthesis LLM call failed: {e}");
                // Fallback: concatenate persona responses
                final_responses
                    .iter()
                    .map(|r| {
                        format!(
                            "## {} — {}\n\n{}",
                            r.persona_name, r.persona_role, r.content
                        )
                    })
                    .collect::<Vec<_>>()
                    .join("\n\n---\n\n")
            }
        }
    };

    // ── Step 6: Post-synthesis consensus scoring ───────────────────────────
    let mut accuracy_outcomes = Vec::new();

    if !synthesis.is_empty() && !all_responses.is_empty() {
        // Deduplicate: take each persona's last response
        let mut last_per_persona: HashMap<String, &PersonaResponse> = HashMap::new();
        for resp in &all_responses {
            last_per_persona.insert(resp.persona_id.clone(), resp);
        }
        let unique_responses: Vec<PersonaResponse> =
            last_per_persona.values().map(|r| (*r).clone()).collect();

        let (alignment_scores, scoring_tokens) =
            score_consensus_alignment(provider, &synthesis, &unique_responses, &params).await;
        token_usage.add_judge(scoring_tokens);

        for resp in &unique_responses {
            let alignment = alignment_scores
                .get(&resp.persona_id)
                .copied()
                .unwrap_or(0.5);
            let fsrs_rating = alignment_to_fsrs_rating(alignment);

            // Record outcome in the accuracy repo
            let _ = accuracy_repo
                .record_outcome(&resp.persona_id, squad_id, &primary_domain, alignment)
                .await;

            accuracy_outcomes.push(AccuracyOutcome {
                persona_id: resp.persona_id.clone(),
                squad_id: squad_id.to_string(),
                domain: primary_domain.clone(),
                consensus_alignment: alignment,
                fsrs_rating,
            });
        }

        // Update consensus_alignment on persona responses in all_responses
        for resp in &mut all_responses {
            if let Some(&alignment) = alignment_scores.get(&resp.persona_id) {
                resp.consensus_alignment = Some(alignment);
            }
        }
    }

    // ── Step 7: Blackboard cleanup ─────────────────────────────────────────
    let _ = blackboard_repo.clear_session(&debate_session_key).await;

    let _ = event_tx.send(DebateEvent::Complete).await;

    let rounds_completed = all_rounds.len() as u8;

    Ok(DebateResult {
        persona_responses: all_responses,
        synthesis,
        rounds_completed,
        total_rounds_planned: effective_max_rounds,
        consensus_reached,
        final_consensus_score,
        partial_reason,
        token_usage,
        learned_weights_applied: learned_weights,
        accuracy_outcomes,
        rounds: all_rounds,
    })
}

// ── Legacy API (kept for backward compatibility) ───────────────────────────

/// Build a persona prompt appropriate for the current debate phase.
///
/// Legacy version — used by `_run_room_debate_legacy`.
#[allow(clippy::too_many_arguments)]
pub fn build_phase_prompt(
    orchestrator_context: &str,
    user_message: &str,
    persona_name: &str,
    persona_role: &str,
    persona_perspective: &str,
    persona_tone: &str,
    blackboard: &[BlackboardEntry],
    current_round: u32,
    phase: LegacyDebatePhase,
    challenge: Option<&str>,
) -> String {
    let blackboard_context = BlackboardRepo::format_for_prompt(blackboard);

    let phase_instructions = match phase {
        LegacyDebatePhase::Opening => format!(
            "This is **Round {current_round}** — the opening round. Share your initial position on the topic. Be clear about your stance and reasoning."
        ),
        LegacyDebatePhase::Discussion => format!(
            "This is **Round {current_round}** of a live discussion. You can see what other personas said above.\n\n\
             Rules:\n\
             - Respond directly to specific points others have made\n\
             - If you agree with someone, say so and build on their point\n\
             - If you disagree, explain why with evidence\n\
             - If your position has changed, acknowledge the shift\n\
             - Be direct and specific"
        ),
        LegacyDebatePhase::Targeted => {
            let challenge_text = challenge.unwrap_or("Continue the discussion.");
            format!(
                "This is **Round {current_round}** — a targeted exchange. Address this specific point concisely (50-150 words):\n\n\
                 > {challenge_text}"
            )
        }
        LegacyDebatePhase::Final => "This is the **final round**. Summarize your final position in 2-3 sentences, accounting for the full discussion above.".to_string(),
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
///
/// Legacy version — used by `_run_room_debate_legacy`.
pub async fn call_judge(
    provider: &DynProvider,
    responses: &[(String, String)],
    params: &ChatParams,
    user_message: &str,
    round: u32,
    persona_ids: &[String],
) -> LegacyJudgeDecision {
    let mut response_text = String::new();
    for (name, content) in responses {
        let truncated = common::truncate_chars(content, 500, "");
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
            LegacyJudgeDecision {
                speaking_order: persona_ids.to_vec(),
                ..LegacyJudgeDecision::default()
            }
        }
    }
}

/// Phase 1 (Opening) and Phase Final (Closing): parallel fan-out.
///
/// Legacy version — used by `_run_room_debate_legacy`.
#[allow(clippy::too_many_arguments)]
async fn fan_out_parallel(
    provider: &DynProvider,
    orchestrator_context: &str,
    user_message: &str,
    personas: &[PersonaRow],
    params: &ChatParams,
    blackboard: &[BlackboardEntry],
    round: u32,
    phase: LegacyDebatePhase,
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
/// Legacy version — used by `_run_room_debate_legacy`.
#[allow(clippy::too_many_arguments)]
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
    phase: LegacyDebatePhase,
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
///
/// **Legacy** — kept temporarily while callers migrate to `run_debate`.
#[allow(clippy::too_many_arguments)]
pub async fn _run_room_debate_legacy(
    provider: &DynProvider,
    orchestrator_context: &str,
    user_message: &str,
    personas: &[PersonaRow],
    params: &ChatParams,
    blackboard_repo: &BlackboardRepo,
    session_key: &str,
    squad_id: &str,
    event_tx: Option<&tokio::sync::mpsc::Sender<crate::AgentEvent>>,
    cancel_token: Option<&tokio_util::sync::CancellationToken>,
) -> DebateRounds {
    let persona_ids: Vec<String> = personas.iter().map(|p| p.id.clone()).collect();
    let mut all_rounds: DebateRounds = Vec::new();
    let mut last_judge: Option<LegacyJudgeDecision> = None;
    let mut round: u32 = 0;

    loop {
        round += 1;

        // Check cancellation before each round
        if let Some(token) = cancel_token {
            if token.is_cancelled() {
                tracing::warn!("Squad debate cancelled at round {round}");
                break;
            }
        }

        // Determine phase
        let phase = if round == 1 {
            LegacyDebatePhase::Opening
        } else if let Some(ref judge) = last_judge {
            if judge.decision == "final_round" || round >= MAX_ROUNDS {
                LegacyDebatePhase::Final
            } else if round == 2 {
                LegacyDebatePhase::Discussion
            } else {
                LegacyDebatePhase::Targeted
            }
        } else {
            LegacyDebatePhase::Discussion
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
            LegacyDebatePhase::Opening | LegacyDebatePhase::Final => {
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
            LegacyDebatePhase::Discussion | LegacyDebatePhase::Targeted => {
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
        if phase == LegacyDebatePhase::Final {
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
            LegacyDebatePhase::Opening,
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
            LegacyDebatePhase::Targeted,
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
            LegacyDebatePhase::Final,
            None,
        );
        assert!(prompt.contains("final round"));
        assert!(prompt.contains("2-3 sentences"));
    }

    #[test]
    fn test_debate_phase_as_str() {
        assert_eq!(LegacyDebatePhase::Opening.as_str(), "opening");
        assert_eq!(LegacyDebatePhase::Discussion.as_str(), "discussion");
        assert_eq!(LegacyDebatePhase::Targeted.as_str(), "targeted");
        assert_eq!(LegacyDebatePhase::Final.as_str(), "final");
    }

    #[test]
    fn test_build_persona_system_prompt() {
        let persona = PersonaRow {
            id: "p1".into(),
            name: "Skeptic".into(),
            role: "Critical analyst".into(),
            expertise: "Risk assessment".into(),
            perspective: "Questions claims".into(),
            tone: "direct".into(),
            icon: "🔍".into(),
            source: "builtin".into(),
            domains: "[]".into(),
            is_active: 1,
            relevance_score: 0.8,
            skill_path: None,
            questioning_style: "Socratic".into(),
            cognitive_bias: "confirmation bias awareness".into(),
            analysis_frameworks: "[\"first principles\"]".into(),
            created_at: String::new(),
            updated_at: String::new(),
        };

        let prompt = build_persona_system_prompt("You are a helpful assistant.", &persona, None);
        assert!(prompt.contains("Skeptic"));
        assert!(prompt.contains("Critical analyst"));
        assert!(prompt.contains("Socratic"));
        assert!(prompt.contains("confirmation bias"));
        assert!(!prompt.contains("cognitive_context"));

        let prompt_with_ctx = build_persona_system_prompt(
            "You are a helpful assistant.",
            &persona,
            Some("User prefers conservative investments."),
        );
        assert!(prompt_with_ctx.contains("cognitive_context"));
        assert!(prompt_with_ctx.contains("conservative investments"));
    }

    #[test]
    fn test_build_synthesis_prompt_synthesized() {
        let responses = vec![PersonaResponse {
            persona_id: "p1".into(),
            persona_name: "Analyst".into(),
            persona_icon: "📊".into(),
            persona_role: "Financial expert".into(),
            content: "Index funds are the best.".into(),
            round: 1,
            phase: DebatePhase::Final,
            confidence: 0.9,
            effective_confidence: 0.9,
            consensus_alignment: None,
        }];

        let weights = vec![LearnedWeight {
            persona_id: "p1".into(),
            base_weight: 0.8,
            accuracy_weight: 0.9,
            blended_weight: 0.85,
            domain: "finance".into(),
            debates_used: 10,
        }];

        let prompt = build_synthesis_prompt(
            "Should I invest?",
            &responses,
            &weights,
            OutputMode::Synthesized,
        );
        assert!(prompt.contains("Should I invest?"));
        assert!(prompt.contains("Analyst"));
        assert!(prompt.contains("0.85"));
        assert!(prompt.contains("Integrate all perspectives"));
    }

    #[test]
    fn test_build_synthesis_prompt_structured() {
        let responses = vec![PersonaResponse {
            persona_id: "p1".into(),
            persona_name: "Analyst".into(),
            persona_icon: "📊".into(),
            persona_role: "Financial expert".into(),
            content: "Index funds are the best.".into(),
            round: 1,
            phase: DebatePhase::Final,
            confidence: 0.9,
            effective_confidence: 0.9,
            consensus_alignment: None,
        }];

        let prompt = build_synthesis_prompt(
            "Should I invest?",
            &responses,
            &[],
            OutputMode::StructuredPerPersona,
        );
        assert!(prompt.contains("Preserve each persona's perspective"));
        assert!(prompt.contains("## {Name}"));
    }

    #[test]
    fn test_alignment_to_fsrs_rating() {
        use cognitive::alignment_to_fsrs_rating;
        assert_eq!(alignment_to_fsrs_rating(0.9), 4);
        assert_eq!(alignment_to_fsrs_rating(0.8), 4);
        assert_eq!(alignment_to_fsrs_rating(0.7), 3);
        assert_eq!(alignment_to_fsrs_rating(0.5), 3);
        assert_eq!(alignment_to_fsrs_rating(0.4), 2);
        assert_eq!(alignment_to_fsrs_rating(0.3), 2);
        assert_eq!(alignment_to_fsrs_rating(0.2), 1);
        assert_eq!(alignment_to_fsrs_rating(0.0), 1);
    }

    #[test]
    fn test_empty_cancelled() {
        let result = DebateResult::empty_cancelled();
        assert_eq!(result.rounds_completed, 0);
        assert!(result.synthesis.is_empty());
        assert!(!result.consensus_reached);
        assert!(matches!(
            result.partial_reason,
            Some(PartialReason::Cancelled)
        ));
    }

    #[test]
    fn test_parse_judge_json_v2() {
        let json = r#"{"consensus_score":75,"decision":"continue","speaking_order":["p1","p2"],"challenges":{"p1":"Explain more"}}"#;
        let decision = parse_judge_json_v2(json);
        assert_eq!(decision.consensus_score, 75.0);
        assert_eq!(decision.decision, "continue");
        assert_eq!(decision.speaking_order, vec!["p1", "p2"]);
        assert_eq!(decision.challenges.get("p1").unwrap(), "Explain more");
    }
}
