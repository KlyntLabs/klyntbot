//! Cognitive handler implementations — heuristic and LLM-backed.
//!
//! Heuristic handlers provide simple rule-based extraction and consolidation
//! without requiring LLM calls. LLM handlers use structured JSON output
//! for higher-quality results, falling back to heuristics on failure.

use async_trait::async_trait;
use serde_json::json;

use cognitive::extraction::ExtractedFact;
use cognitive::reflection::{ReflectionHandler, ReflectionInput, ReflectionOutput};
use cognitive::types::{MemoryOp, Observation, ProceduralRule, SemanticFact, DEFAULT_MEMORY_TYPE};
use cognitive::{ConsolidationHandler, ExtractionHandler};
use feature_coaching::reasoner::{
    CoachingDecision, CoachingReasonerHandler, InterventionType, ReasonerInput,
};
use providers::{ChatParams, DynProvider, Message, ResponseFormat};

// ── Heuristic handlers ──────────────────────────────────────────────────────

/// Heuristic fact extraction — parses observations into SPO triples
/// using pattern matching rather than LLM calls.
pub struct HeuristicExtractionHandler;

impl HeuristicExtractionHandler {
    /// Single-observation extraction logic (used by both heuristic and LLM fallback).
    pub(crate) fn extract_single(&self, observation: &Observation) -> Vec<ExtractedFact> {
        let fact = |domain: &str, predicate: &str, confidence: f64, source: &str| ExtractedFact {
            domain: domain.into(),
            subject: "user".into(),
            predicate: predicate.into(),
            object: observation.content.clone(),
            confidence,
            source: source.into(),
        };
        let od = observation.domain.as_str();

        match observation.source_event.as_str() {
            "UserStatedFact" => vec![fact(od, "stated", 1.0, "user_stated")],
            "UserCorrectedAI" => vec![fact(od, "corrected", 1.0, "user_stated")],
            "ChatTurnCompleted" => {
                let content = observation.content.trim();
                // Skip questions and very short messages — no facts to extract
                if content.ends_with('?') || content.len() < 10 {
                    vec![]
                } else {
                    // Use a content-derived predicate to avoid supersession
                    let predicate = format!("chat_stated_{:x}", {
                        use std::hash::{Hash, Hasher};
                        let mut h = std::collections::hash_map::DefaultHasher::new();
                        content.hash(&mut h);
                        h.finish() & 0xFFFF
                    });
                    vec![fact(od, &predicate, 0.8, "user_stated")]
                }
            }
            "BudgetAlert" => vec![fact("finance", "budget_pressure", 0.9, "observed")],
            "CoachingFeedback" => vec![fact("coaching", "coaching_response", 0.9, "observed")],
            source if source.starts_with("accumulated:") => {
                vec![fact(od, "pattern", 0.7, "inferred")]
            }
            _ if observation.importance >= 0.7 => {
                vec![fact(
                    od,
                    "observation",
                    observation.importance * 0.8,
                    "observed",
                )]
            }
            _ => vec![],
        }
    }
}

impl HeuristicExtractionHandler {
    /// Synchronous batch extraction (no async needed for heuristic).
    /// Used directly by `LlmExtractionHandler::fallback_all`.
    pub(crate) fn extract_facts_batch_sync(
        &self,
        observations: &[Observation],
    ) -> cognitive::BatchExtractionResult {
        let extractions = observations
            .iter()
            .enumerate()
            .map(|(i, obs)| cognitive::BatchExtraction {
                observation_index: i,
                facts: self.extract_single(obs),
            })
            .collect();
        cognitive::BatchExtractionResult {
            extractions,
            fallback_indices: Vec::new(), // Heuristic IS the fallback
        }
    }
}

#[async_trait]
impl ExtractionHandler for HeuristicExtractionHandler {
    async fn extract_facts_batch(
        &self,
        observations: &[Observation],
    ) -> common::Result<cognitive::BatchExtractionResult> {
        Ok(self.extract_facts_batch_sync(observations))
    }
}

/// Heuristic consolidation — decides ADD/UPDATE/DELETE/NOOP using
/// simple text matching on subject+predicate pairs.
pub struct HeuristicConsolidationHandler;

impl HeuristicConsolidationHandler {
    /// Single-candidate consolidation logic (used by both heuristic and LLM fallback).
    pub(crate) fn decide_single(
        &self,
        candidate: &cognitive::types::SemanticFact,
        existing: &[cognitive::types::SemanticFact],
    ) -> MemoryOp {
        let mut update_from: Option<&cognitive::types::SemanticFact> = None;
        for fact in existing {
            if fact.predicate == candidate.predicate {
                if fact.object == candidate.object {
                    return MemoryOp::Noop;
                }
                if update_from.is_none() {
                    update_from = Some(fact);
                }
            }
        }

        if let Some(old) = update_from {
            return MemoryOp::Update {
                id: candidate.id.clone(),
                old_id: old.id.clone(),
            };
        }

        MemoryOp::Add {
            id: candidate.id.clone(),
        }
    }
}

#[async_trait]
impl ConsolidationHandler for HeuristicConsolidationHandler {
    async fn decide_batch(
        &self,
        candidates: &[cognitive::ConsolidationCandidate],
    ) -> common::Result<Vec<MemoryOp>> {
        let mut ops = Vec::with_capacity(candidates.len());
        for entry in candidates {
            let op = self.decide_single(&entry.candidate, &entry.existing);
            ops.push(op);
        }
        Ok(ops)
    }
}

/// Heuristic reflection — generates a statistical summary without LLM.
/// Returns empty fact/rule updates but provides a useful summary.
pub struct HeuristicReflectionHandler;

#[async_trait]
impl ReflectionHandler for HeuristicReflectionHandler {
    async fn reflect(&self, input: &ReflectionInput) -> common::Result<ReflectionOutput> {
        let mem_count = input.episodic_memories.len();
        let rule_count = input.procedural_rules.len();
        let domain_count = input.user_model.non_empty_domain_count();

        let summary = format!(
            "Heuristic reflection ({} to {}): {} episodic memories, {} active rules, {} domains tracked. \
             No LLM available for cross-domain synthesis.",
            input.period_start, input.period_end, mem_count, rule_count, domain_count
        );

        Ok(ReflectionOutput {
            fact_updates: vec![],
            rule_updates: vec![],
            summary,
        })
    }
}

/// Heuristic coaching reasoner — wraps the standalone `heuristic_reason` function.
pub struct HeuristicCoachingReasonerHandler;

#[async_trait]
impl CoachingReasonerHandler for HeuristicCoachingReasonerHandler {
    async fn reason(&self, input: &ReasonerInput) -> common::Result<CoachingDecision> {
        Ok(feature_coaching::reasoner::heuristic_reason(input))
    }
}

// ── LLM-backed handlers ────────────────────────────────────────────────────

// ── Extraction ──

const EXTRACTION_SYSTEM_PROMPT: &str = "\
You are a semantic memory extraction agent. Given an observation about a user, \
extract structured facts as subject-predicate-object triples.\n\n\
Domains: identity, energy, work, finance, learning, preferences, general\n\
Subjects: usually \"user\", or \"project:<name>\", \"task:<id>\"\n\
Predicates: descriptive relationship (e.g., \"name\", \"favorite_language\", \"peak_hours\", \"occupation\")\n\
Object: the value (e.g., \"Jayden\", \"Rust\", \"10am-12pm\", \"software developer\")\n\n\
Rules:\n\
- Extract EVERY distinct fact from the observation as a separate triple\n\
- Set confidence based on certainty (user-stated = 1.0, inferred = 0.5-0.8)\n\
- Use source \"user_stated\" for explicit statements, \"observed\" for behavioral data, \"inferred\" for patterns\n\
- Return empty facts array if nothing meaningful can be extracted\n\
- Questions (e.g., 'What is my name?') are NOT facts — return empty array for questions\n\
- Be specific in predicates — use snake_case names like \"name\", \"occupation\", \"favorite_language\"\n\
- Be concise in objects — just the value, not the full sentence\n\n\
Respond with JSON in this exact format:\n\
{\"facts\": [{\"domain\": \"identity\", \"subject\": \"user\", \"predicate\": \"name\", \"object\": \"Jayden\", \"confidence\": 1.0, \"source\": \"user_stated\"}]}";

/// LLM-backed fact extraction with heuristic fallback.
pub struct LlmExtractionHandler {
    provider: DynProvider,
    params: ChatParams,
    fallback: HeuristicExtractionHandler,
}

impl LlmExtractionHandler {
    pub fn new(provider: DynProvider, params: ChatParams) -> Self {
        Self {
            provider,
            params: params.with_response_format(ResponseFormat::JsonObject),
            fallback: HeuristicExtractionHandler,
        }
    }
}

#[derive(serde::Deserialize)]
struct BatchExtractionLlmResult {
    results: Vec<ObservationExtraction>,
}

#[derive(serde::Deserialize)]
struct ObservationExtraction {
    observation_index: usize,
    facts: Vec<ExtractedFactJson>,
}

#[derive(serde::Deserialize)]
struct ExtractedFactJson {
    domain: String,
    subject: String,
    predicate: String,
    object: String,
    confidence: f64,
    source: String,
}

impl LlmExtractionHandler {
    fn fallback_all(&self, observations: &[Observation]) -> cognitive::BatchExtractionResult {
        // Delegate to the heuristic handler (same logic), then mark all as fallback
        let mut result = self.fallback.extract_facts_batch_sync(observations);
        result.fallback_indices = (0..observations.len()).collect();
        result
    }
}

#[async_trait]
impl ExtractionHandler for LlmExtractionHandler {
    async fn extract_facts_batch(
        &self,
        observations: &[Observation],
    ) -> common::Result<cognitive::BatchExtractionResult> {
        if observations.is_empty() {
            return Ok(cognitive::BatchExtractionResult {
                extractions: Vec::new(),
                fallback_indices: Vec::new(),
            });
        }

        // Build numbered observation list for the prompt
        let mut user_msg = String::new();
        for (i, obs) in observations.iter().enumerate() {
            use std::fmt::Write;
            writeln!(
                &mut user_msg,
                "Observation {}:\nDomain: {}\nSource: {}\nImportance: {:.1}\nContent: {}\n",
                i + 1,
                obs.domain,
                obs.source_event,
                obs.importance,
                obs.content
            )
            .unwrap();
        }
        user_msg.push_str(
            "Extract facts from ALL observations. Return JSON:\n\
             {\"results\": [{\"observation_index\": 1, \"facts\": [{\"domain\": \"...\", \"subject\": \"...\", \"predicate\": \"...\", \"object\": \"...\", \"confidence\": 0.0, \"source\": \"...\"}]}]}",
        );

        let messages = vec![
            Message::system(EXTRACTION_SYSTEM_PROMPT),
            Message::user(user_msg),
        ];

        match self.provider.chat(&messages, None, &self.params).await {
            Ok(response) => {
                let content = response.content.unwrap_or_default();
                match serde_json::from_str::<BatchExtractionLlmResult>(&content) {
                    Ok(result) => {
                        let extractions = result
                            .results
                            .into_iter()
                            .filter(|r| r.observation_index > 0) // skip invalid 0 indices (prompt is 1-based)
                            .map(|r| cognitive::BatchExtraction {
                                observation_index: r.observation_index - 1, // 1-based to 0-based
                                facts: r
                                    .facts
                                    .into_iter()
                                    .map(|f| ExtractedFact {
                                        domain: f.domain,
                                        subject: f.subject,
                                        predicate: f.predicate,
                                        object: f.object,
                                        confidence: f.confidence,
                                        source: f.source,
                                    })
                                    .collect(),
                            })
                            .collect();
                        Ok(cognitive::BatchExtractionResult {
                            extractions,
                            fallback_indices: Vec::new(),
                        })
                    }
                    Err(e) => {
                        tracing::warn!(
                            "LLM batch extraction JSON parse failed: {e}, falling back to heuristic for all"
                        );
                        Ok(self.fallback_all(observations))
                    }
                }
            }
            Err(e) => {
                tracing::warn!(
                    "LLM batch extraction call failed: {e}, falling back to heuristic for all"
                );
                Ok(self.fallback_all(observations))
            }
        }
    }
}

// ── Consolidation ──

const CONSOLIDATION_SYSTEM_PROMPT: &str = "\
You are a semantic memory consolidation agent. Given candidate facts and their existing \
similar facts, decide the correct operation for each:\n\n\
- add: The candidate is genuinely new information, no existing fact covers it.\n\
- update: The candidate refines or corrects an existing fact. Provide the target_id of the fact to supersede.\n\
- delete: The candidate contradicts an existing fact and the existing fact should be marked superseded. Provide the target_id to delete.\n\
- noop: The candidate is already known.\n\n\
Always prefer noop over add if the information is essentially the same.\n\
Always prefer update over delete+add when the meaning is similar but the value changed.\n\n\
Respond with JSON containing a decisions array, one entry per candidate:\n\
{\"decisions\": [{\"index\": 1, \"action\": \"add|update|delete|noop\", \"target_id\": null}]}";

/// LLM-backed consolidation with heuristic fallback.
pub struct LlmConsolidationHandler {
    provider: DynProvider,
    params: ChatParams,
    fallback: HeuristicConsolidationHandler,
}

impl LlmConsolidationHandler {
    pub fn new(provider: DynProvider, params: ChatParams) -> Self {
        Self {
            provider,
            params: params.with_response_format(ResponseFormat::JsonObject),
            fallback: HeuristicConsolidationHandler,
        }
    }
}

#[derive(serde::Deserialize)]
struct BatchConsolidationLlmResult {
    decisions: Vec<ConsolidationDecisionIndexed>,
}

#[derive(serde::Deserialize)]
struct ConsolidationDecisionIndexed {
    index: usize,
    action: String,
    target_id: Option<String>,
}

impl LlmConsolidationHandler {
    fn decision_to_op(
        &self,
        action: &str,
        target_id: Option<&str>,
        candidate: &cognitive::types::SemanticFact,
        existing: &[cognitive::types::SemanticFact],
    ) -> MemoryOp {
        match action {
            "add" => MemoryOp::Add {
                id: candidate.id.clone(),
            },
            "update" => {
                let old_id = target_id
                    .map(String::from)
                    .or_else(|| existing.first().map(|f| f.id.clone()))
                    .unwrap_or_default();
                MemoryOp::Update {
                    id: candidate.id.clone(),
                    old_id,
                }
            }
            "delete" => {
                let target = target_id
                    .map(String::from)
                    .or_else(|| existing.first().map(|f| f.id.clone()))
                    .unwrap_or_default();
                MemoryOp::Delete {
                    id: target,
                    superseded_by: candidate.id.clone(),
                }
            }
            _ => MemoryOp::Noop,
        }
    }
}

#[async_trait]
impl ConsolidationHandler for LlmConsolidationHandler {
    async fn decide_batch(
        &self,
        candidates: &[cognitive::ConsolidationCandidate],
    ) -> common::Result<Vec<MemoryOp>> {
        if candidates.is_empty() {
            return Ok(Vec::new());
        }

        // Candidates with no existing matches -> direct ADD (skip LLM)
        let mut ops = vec![None; candidates.len()];
        let mut llm_indices = Vec::new();

        for (i, entry) in candidates.iter().enumerate() {
            if entry.existing.is_empty() {
                ops[i] = Some(MemoryOp::Add {
                    id: entry.candidate.id.clone(),
                });
            } else {
                llm_indices.push(i);
            }
        }

        if llm_indices.is_empty() {
            return Ok(ops.into_iter().map(|o| o.unwrap()).collect());
        }

        // Build prompt for candidates that need LLM decisions
        let mut user_msg = String::new();
        for (prompt_idx, &cand_idx) in llm_indices.iter().enumerate() {
            let entry = &candidates[cand_idx];
            let existing_json: Vec<serde_json::Value> = entry
                .existing
                .iter()
                .map(|f| {
                    json!({
                        "id": f.id,
                        "subject": f.subject,
                        "predicate": f.predicate,
                        "object": f.object,
                        "confidence": f.confidence,
                    })
                })
                .collect();

            use std::fmt::Write;
            writeln!(
                &mut user_msg,
                "Decision {}:\nCandidate: {}.{} = {} (confidence: {})\nExisting: {}\n",
                prompt_idx + 1,
                entry.candidate.subject,
                entry.candidate.predicate,
                entry.candidate.object,
                entry.candidate.confidence,
                serde_json::to_string(&existing_json).unwrap_or_default()
            )
            .unwrap();
        }
        user_msg.push_str(
            "Decide for ALL candidates. Return JSON:\n\
             {\"decisions\": [{\"index\": 1, \"action\": \"add|update|delete|noop\", \"target_id\": null}]}",
        );

        let messages = vec![
            Message::system(CONSOLIDATION_SYSTEM_PROMPT),
            Message::user(user_msg),
        ];

        match self.provider.chat(&messages, None, &self.params).await {
            Ok(response) => {
                let content = response.content.unwrap_or_default();
                match serde_json::from_str::<BatchConsolidationLlmResult>(&content) {
                    Ok(result) => {
                        for decision in result.decisions {
                            let prompt_idx = decision.index.saturating_sub(1); // 1-based to 0-based
                            if let Some(&cand_idx) = llm_indices.get(prompt_idx) {
                                let candidate = &candidates[cand_idx].candidate;
                                let existing = &candidates[cand_idx].existing;
                                ops[cand_idx] = Some(self.decision_to_op(
                                    &decision.action,
                                    decision.target_id.as_deref(),
                                    candidate,
                                    existing,
                                ));
                            }
                        }
                    }
                    Err(e) => {
                        tracing::warn!(
                            "LLM batch consolidation JSON parse failed: {e}, falling back"
                        );
                        for &cand_idx in &llm_indices {
                            let entry = &candidates[cand_idx];
                            ops[cand_idx] = Some(
                                self.fallback
                                    .decide_single(&entry.candidate, &entry.existing),
                            );
                        }
                    }
                }
            }
            Err(e) => {
                tracing::warn!("LLM batch consolidation call failed: {e}, falling back");
                for &cand_idx in &llm_indices {
                    let entry = &candidates[cand_idx];
                    ops[cand_idx] = Some(
                        self.fallback
                            .decide_single(&entry.candidate, &entry.existing),
                    );
                }
            }
        }

        // Fill any remaining None entries with Noop (missing from LLM response)
        Ok(ops
            .into_iter()
            .map(|o| o.unwrap_or(MemoryOp::Noop))
            .collect())
    }
}

// ── Reflection ──

const REFLECTION_SYSTEM_PROMPT: &str = "\
You are a cognitive reflection agent performing weekly self-review. Analyze the user's \
episodic memories, current model, and procedural rules to identify:\n\n\
1. Cross-domain patterns (e.g., exercise correlates with productivity)\n\
2. Facts that should be updated based on new evidence\n\
3. New procedural rules based on validated patterns (minimum 5 signals across 3+ days)\n\
4. Facts that may be outdated and should be revisited\n\n\
Output:\n\
- fact_updates: New or updated semantic facts. Use source \"reflected\".\n\
- rule_updates: New or updated procedural rules.\n\
- summary: 2-3 sentence synthesis of the week's patterns.\n\n\
Be conservative. Only propose changes with strong evidence.\n\n\
Respond with JSON in this exact format:\n\
{\"fact_updates\": [{\"domain\": \"energy\", \"subject\": \"user\", \"predicate\": \"pattern\", \"object\": \"value\", \"confidence\": 0.8, \"source\": \"reflected\"}], \"rule_updates\": [{\"domain\": \"productivity\", \"rule_text\": \"description\", \"confidence\": 0.7}], \"summary\": \"Brief synthesis\"}";

/// LLM-backed weekly reflection.
pub struct LlmReflectionHandler {
    provider: DynProvider,
    params: ChatParams,
}

impl LlmReflectionHandler {
    pub fn new(provider: DynProvider, params: ChatParams) -> Self {
        Self {
            provider,
            params: params.with_response_format(ResponseFormat::JsonObject),
        }
    }
}

#[derive(serde::Deserialize)]
struct ReflectionResultJson {
    fact_updates: Vec<ReflectionFactJson>,
    rule_updates: Vec<ReflectionRuleJson>,
    summary: String,
}

#[derive(serde::Deserialize)]
struct ReflectionFactJson {
    domain: String,
    subject: String,
    predicate: String,
    object: String,
    confidence: f64,
    source: String,
}

#[derive(serde::Deserialize)]
struct ReflectionRuleJson {
    domain: String,
    rule_text: String,
    confidence: f64,
}

#[async_trait]
impl ReflectionHandler for LlmReflectionHandler {
    async fn reflect(&self, input: &ReflectionInput) -> common::Result<ReflectionOutput> {
        let memories_text: Vec<String> = input
            .episodic_memories
            .iter()
            .map(|m| format!("[{}] {}: {}", m.occurred_at, m.domain, m.content))
            .collect();

        let rules_text: Vec<String> = input
            .procedural_rules
            .iter()
            .map(|r| {
                format!(
                    "[{}] {} (confidence: {:.0}%)",
                    r.domain,
                    r.rule_text,
                    r.confidence * 100.0
                )
            })
            .collect();

        let model_text = serde_json::to_string_pretty(&input.user_model).unwrap_or_default();

        let user_msg = format!(
            "Period: {} to {}\n\n## Episodic Memories ({})\n{}\n\n## Current User Model\n{}\n\n## Active Procedural Rules ({})\n{}",
            input.period_start,
            input.period_end,
            memories_text.len(),
            memories_text.join("\n"),
            model_text,
            rules_text.len(),
            rules_text.join("\n"),
        );

        let messages = vec![
            Message::system(REFLECTION_SYSTEM_PROMPT),
            Message::user(user_msg),
        ];

        let response = self.provider.chat(&messages, None, &self.params).await?;
        let content = response.content.unwrap_or_default();
        let result: ReflectionResultJson = serde_json::from_str(&content).map_err(|e| {
            common::KlyntbotError::Provider(common::ProviderError::InvalidResponse(format!(
                "Reflection JSON parse error: {e}"
            )))
        })?;

        let now = chrono::Utc::now().format("%Y-%m-%dT%H:%M:%S").to_string();
        let fact_updates = result
            .fact_updates
            .into_iter()
            .map(|f| SemanticFact {
                id: uuid::Uuid::new_v4().to_string(),
                domain: f.domain,
                subject: f.subject,
                predicate: f.predicate,
                object: f.object,
                confidence: f.confidence,
                source: f.source,
                valid_from: now.clone(),
                valid_until: None,
                recorded_at: now.clone(),
                superseded_at: None,
                superseded_by: None,
                stability: 1.0,
                last_accessed: None,
                access_count: 0,
                project_id: None,
                memory_type: DEFAULT_MEMORY_TYPE.to_string(),
            })
            .collect();

        let rule_updates = result
            .rule_updates
            .into_iter()
            .map(|r| ProceduralRule {
                id: uuid::Uuid::new_v4().to_string(),
                domain: r.domain,
                rule_text: r.rule_text,
                confidence: r.confidence,
                source: "reflected".into(),
                signal_count: 0,
                created_at: now.clone(),
                updated_at: now.clone(),
                active: true,
                project_id: None,
            })
            .collect();

        Ok(ReflectionOutput {
            fact_updates,
            rule_updates,
            summary: result.summary,
        })
    }
}

// ── Coaching Reasoner ──

const COACHING_SYSTEM_PROMPT: &str = "\
You are a proactive coaching agent. Given the user's current situation, a triggered \
condition, detected patterns, and relevant memories, decide whether and how to intervene.\n\n\
Principles:\n\
- Be helpful, not annoying. Respect the user's flow.\n\
- Don't interrupt deep focus for low-priority nudges.\n\
- Consider coaching_receptivity: below 0.3 means the user doesn't engage with nudges.\n\
- Personalize the message based on patterns and memories.\n\
- Keep messages concise (1-2 sentences).\n\n\
Intervention types (from least to most intrusive):\n\
- dashboard_card: Subtle, shown on dashboard\n\
- chat_message: Sent as a chat message\n\
- notification: System notification\n\
- overlay: Full-screen overlay (only for critical situations)\n\
- none: No intervention\n\n\
Set should_intervene to false if unsure or if the user would likely dismiss it.\n\n\
Respond with JSON in this exact format:\n\
{\"should_intervene\": false, \"confidence\": 0.5, \"message\": null, \"intervention_type\": \"none\", \"reasoning\": \"explanation\", \"observations\": [\"obs1\"]}";

/// LLM-backed coaching reasoner with heuristic fallback.
pub struct LlmCoachingReasonerHandler {
    provider: DynProvider,
    params: ChatParams,
}

impl LlmCoachingReasonerHandler {
    pub fn new(provider: DynProvider, params: ChatParams) -> Self {
        Self {
            provider,
            params: params.with_response_format(ResponseFormat::JsonObject),
        }
    }
}

#[derive(serde::Deserialize)]
struct CoachingDecisionJson {
    should_intervene: bool,
    confidence: f64,
    message: Option<String>,
    intervention_type: String,
    reasoning: String,
    observations: Vec<String>,
}

#[async_trait]
impl CoachingReasonerHandler for LlmCoachingReasonerHandler {
    async fn reason(&self, input: &ReasonerInput) -> common::Result<CoachingDecision> {
        let patterns_text: Vec<String> = input
            .patterns
            .iter()
            .map(|p| {
                format!(
                    "{}: {} (confidence: {:.0}%)",
                    p.name,
                    p.description,
                    p.confidence * 100.0
                )
            })
            .collect();

        let user_msg = format!(
            "## Current Situation\n\
             Energy: {:.0}%, Focus: {:.0}%, Deadline pressure: {:.0}%\n\
             Distraction risk: {:.0}%, Coaching receptivity: {:.0}%\n\
             Hours active: {:.1}h, Since break: {:.0}min, Context switches: {}\n\
             Task avoidance: {}\n\n\
             ## Trigger\n{}: {} (confidence: {:.0}%)\n\n\
             ## Detected Patterns ({})\n{}\n\n\
             ## Relevant Memories\n{}\n\n\
             ## Recent Interventions\n{}",
            input.situation.energy_level * 100.0,
            input.situation.focus_state * 100.0,
            input.situation.deadline_pressure * 100.0,
            input.situation.distraction_risk * 100.0,
            input.situation.coaching_receptivity * 100.0,
            input.situation.hours_active_today,
            input.situation.mins_since_break,
            input.situation.recent_context_switches,
            input.situation.task_avoidance_detected,
            input.trigger.condition_name,
            input.trigger.context,
            input.trigger.confidence * 100.0,
            patterns_text.len(),
            patterns_text.join("\n"),
            if input.relevant_memories.is_empty() {
                "None".into()
            } else {
                input.relevant_memories.join("\n")
            },
            if input.recent_interventions.is_empty() {
                "None".into()
            } else {
                input.recent_interventions.join("\n")
            },
        );

        let messages = vec![
            Message::system(COACHING_SYSTEM_PROMPT),
            Message::user(user_msg),
        ];

        match self.provider.chat(&messages, None, &self.params).await {
            Ok(response) => {
                let content = response.content.unwrap_or_default();
                match serde_json::from_str::<CoachingDecisionJson>(&content) {
                    Ok(d) => Ok(CoachingDecision {
                        should_intervene: d.should_intervene,
                        confidence: d.confidence,
                        message: d.message,
                        intervention_type: match d.intervention_type.as_str() {
                            "dashboard_card" => InterventionType::DashboardCard,
                            "chat_message" => InterventionType::ChatMessage,
                            "notification" => InterventionType::Notification,
                            "overlay" => InterventionType::Overlay,
                            _ => InterventionType::None,
                        },
                        reasoning: d.reasoning,
                        observations: d.observations,
                    }),
                    Err(e) => {
                        tracing::warn!("LLM coaching JSON parse failed: {e}, falling back");
                        Ok(feature_coaching::reasoner::heuristic_reason(input))
                    }
                }
            }
            Err(e) => {
                tracing::warn!("LLM coaching call failed: {e}, falling back");
                Ok(feature_coaching::reasoner::heuristic_reason(input))
            }
        }
    }
}

// ── Tests ───────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;

    use chrono::Utc;
    use cognitive::situation::UserSituation;
    use cognitive::types::UserModel;
    use feature_coaching::signal_accumulator::TriggerFired;
    use providers::{LlmResponse, LlmStream, ProviderCapabilities, ProviderHealth, Usage};

    // ── MockProvider ──

    struct MockProvider {
        response: Result<LlmResponse, String>,
    }

    impl MockProvider {
        fn new(response: LlmResponse) -> Self {
            Self {
                response: Ok(response),
            }
        }

        fn new_error(msg: &str) -> Self {
            Self {
                response: Err(msg.into()),
            }
        }
    }

    #[async_trait]
    impl providers::LlmProvider for MockProvider {
        async fn chat(
            &self,
            _messages: &[Message],
            _tools: Option<&[serde_json::Value]>,
            _params: &ChatParams,
        ) -> common::Result<LlmResponse> {
            match &self.response {
                Ok(r) => Ok(r.clone()),
                Err(e) => Err(common::KlyntbotError::Provider(
                    common::ProviderError::InvalidResponse(e.clone()),
                )),
            }
        }

        async fn chat_stream(
            &self,
            _messages: &[Message],
            _tools: Option<&[serde_json::Value]>,
            _params: &ChatParams,
        ) -> common::Result<LlmStream> {
            unimplemented!("mock doesn't support streaming")
        }

        fn supports_streaming(&self) -> bool {
            false
        }
        fn default_model(&self) -> &str {
            "mock"
        }
        fn name(&self) -> &str {
            "mock"
        }
        fn capabilities(&self) -> ProviderCapabilities {
            ProviderCapabilities::default()
        }
        fn context_window(&self) -> usize {
            128000
        }
        async fn health_check(&self) -> common::Result<ProviderHealth> {
            Ok(ProviderHealth::Healthy)
        }
    }

    fn mock_response(content: &str) -> LlmResponse {
        LlmResponse {
            content: Some(content.into()),
            tool_calls: vec![],
            finish_reason: "stop".into(),
            usage: Usage::default(),
            reasoning_content: None,
        }
    }

    // ── Test helpers ──

    fn test_observation(source: &str, content: &str, importance: f64) -> Observation {
        Observation {
            domain: "test".into(),
            content: content.into(),
            importance,
            source_event: source.into(),
            timestamp: Utc::now(),
        }
    }

    fn test_fact(id: &str, pred: &str, obj: &str) -> SemanticFact {
        SemanticFact {
            id: id.into(),
            domain: "test".into(),
            subject: "user".into(),
            predicate: pred.into(),
            object: obj.into(),
            confidence: 0.8,
            source: "observed".into(),
            valid_from: "2026-03-01".into(),
            valid_until: None,
            recorded_at: "2026-03-06".into(),
            superseded_at: None,
            superseded_by: None,
            stability: 1.0,
            last_accessed: None,
            access_count: 0,
            project_id: None,
            memory_type: DEFAULT_MEMORY_TYPE.to_string(),
        }
    }

    // ── Heuristic extraction tests ──

    #[tokio::test]
    async fn test_extraction_user_stated_fact() {
        let handler = HeuristicExtractionHandler;
        let obs = test_observation("UserStatedFact", "I prefer dark mode", 1.0);
        let result = handler.extract_facts_batch(&[obs]).await.unwrap();
        let facts: Vec<_> = result
            .extractions
            .into_iter()
            .flat_map(|e| e.facts)
            .collect();
        assert_eq!(facts.len(), 1);
        assert_eq!(facts[0].source, "user_stated");
        assert_eq!(facts[0].confidence, 1.0);
    }

    #[tokio::test]
    async fn test_extraction_low_importance_skipped() {
        let handler = HeuristicExtractionHandler;
        let obs = test_observation("ProductivityScoreComputed", "Score: 72", 0.5);
        let result = handler.extract_facts_batch(&[obs]).await.unwrap();
        let facts: Vec<_> = result
            .extractions
            .into_iter()
            .flat_map(|e| e.facts)
            .collect();
        assert!(facts.is_empty());
    }

    #[tokio::test]
    async fn test_extraction_high_importance_extracted() {
        let handler = HeuristicExtractionHandler;
        let obs = test_observation("TransactionRecorded", "Over budget!", 0.8);
        let result = handler.extract_facts_batch(&[obs]).await.unwrap();
        let facts: Vec<_> = result
            .extractions
            .into_iter()
            .flat_map(|e| e.facts)
            .collect();
        assert_eq!(facts.len(), 1);
    }

    // ── Heuristic consolidation tests ──

    #[tokio::test]
    async fn test_consolidation_add_when_empty() {
        let handler = HeuristicConsolidationHandler;
        let candidate = test_fact("c1", "peak_hours", "10am-12pm");
        let ops = handler
            .decide_batch(&[cognitive::ConsolidationCandidate {
                candidate,
                existing: vec![],
            }])
            .await
            .unwrap();
        let result = ops.into_iter().next().unwrap();
        assert!(matches!(result, MemoryOp::Add { .. }));
    }

    #[tokio::test]
    async fn test_consolidation_noop_on_duplicate() {
        let handler = HeuristicConsolidationHandler;
        let candidate = test_fact("c1", "peak_hours", "10am-12pm");
        let existing = vec![test_fact("e1", "peak_hours", "10am-12pm")];
        let ops = handler
            .decide_batch(&[cognitive::ConsolidationCandidate {
                candidate,
                existing,
            }])
            .await
            .unwrap();
        let result = ops.into_iter().next().unwrap();
        assert!(matches!(result, MemoryOp::Noop));
    }

    #[tokio::test]
    async fn test_consolidation_update_on_changed_value() {
        let handler = HeuristicConsolidationHandler;
        let candidate = test_fact("c1", "peak_hours", "2pm-4pm");
        let existing = vec![test_fact("e1", "peak_hours", "10am-12pm")];
        let ops = handler
            .decide_batch(&[cognitive::ConsolidationCandidate {
                candidate,
                existing,
            }])
            .await
            .unwrap();
        let result = ops.into_iter().next().unwrap();
        assert!(matches!(result, MemoryOp::Update { .. }));
    }

    #[tokio::test]
    async fn test_consolidation_add_different_predicate() {
        let handler = HeuristicConsolidationHandler;
        let candidate = test_fact("c1", "work_style", "deep focus");
        let existing = vec![test_fact("e1", "peak_hours", "10am-12pm")];
        let ops = handler
            .decide_batch(&[cognitive::ConsolidationCandidate {
                candidate,
                existing,
            }])
            .await
            .unwrap();
        let result = ops.into_iter().next().unwrap();
        assert!(matches!(result, MemoryOp::Add { .. }));
    }

    // ── Heuristic reflection test ──

    #[tokio::test]
    async fn test_heuristic_reflection_returns_summary() {
        let handler = HeuristicReflectionHandler;
        let input = ReflectionInput {
            episodic_memories: vec![cognitive::types::EpisodicMemory {
                id: "e1".into(),
                domain: "productivity".into(),
                content: "Had a productive morning".into(),
                summary: Some("Productive morning".into()),
                importance: 0.7,
                occurred_at: "2026-03-01T10:00:00".into(),
                recorded_at: "2026-03-01T10:00:00".into(),
                stability: 1.0,
                last_accessed: None,
                access_count: 0,
                project_id: None,
            }],
            user_model: UserModel::default(),
            procedural_rules: vec![],
            period_start: "2026-03-01T00:00:00".into(),
            period_end: "2026-03-07T23:59:59".into(),
        };

        let output = handler.reflect(&input).await.unwrap();
        assert!(!output.summary.is_empty());
        assert!(output.fact_updates.is_empty());
        assert!(output.rule_updates.is_empty());
    }

    // ── LLM extraction tests ──

    #[tokio::test]
    async fn test_llm_extraction_parses_json_response() {
        let mock = Arc::new(MockProvider::new(mock_response(
            r#"{"results":[{"observation_index":1,"facts":[{"domain":"energy","subject":"user","predicate":"peak_hours","object":"10am-12pm","confidence":0.85,"source":"observed"}]}]}"#,
        )));
        let params = ChatParams::new("test-model")
            .with_temperature(0.2)
            .with_max_tokens(1024);
        let handler = LlmExtractionHandler::new(mock, params);

        let obs = Observation {
            domain: "productivity".into(),
            content: "User is most productive between 10am and 12pm".into(),
            importance: 0.8,
            source_event: "ProductivityScoreComputed".into(),
            timestamp: Utc::now(),
        };
        let result = handler.extract_facts_batch(&[obs]).await.unwrap();
        let facts: Vec<_> = result
            .extractions
            .into_iter()
            .flat_map(|e| e.facts)
            .collect();
        assert_eq!(facts.len(), 1);
        assert_eq!(facts[0].domain, "energy");
        assert_eq!(facts[0].predicate, "peak_hours");
        assert_eq!(facts[0].object, "10am-12pm");
    }

    #[tokio::test]
    async fn test_llm_extraction_falls_back_on_error() {
        let mock = Arc::new(MockProvider::new_error("LLM unavailable"));
        let params = ChatParams::new("test-model");
        let handler = LlmExtractionHandler::new(mock, params);

        let obs = Observation {
            domain: "productivity".into(),
            content: "User stated: I like mornings".into(),
            importance: 1.0,
            source_event: "UserStatedFact".into(),
            timestamp: Utc::now(),
        };
        let result = handler.extract_facts_batch(&[obs]).await.unwrap();
        let facts: Vec<_> = result
            .extractions
            .into_iter()
            .flat_map(|e| e.facts)
            .collect();
        assert!(!facts.is_empty());
    }

    // ── LLM consolidation tests ──

    #[tokio::test]
    async fn test_llm_consolidation_parses_update() {
        let mock = Arc::new(MockProvider::new(mock_response(
            r#"{"decisions":[{"index":1,"action":"update","target_id":"old-1"}]}"#,
        )));
        let params = ChatParams::new("test-model");
        let handler = LlmConsolidationHandler::new(mock, params);

        let candidate = test_fact("new-1", "peak_hours", "9am-11am");
        let existing = vec![test_fact("old-1", "peak_hours", "10am-12pm")];
        let ops = handler
            .decide_batch(&[cognitive::ConsolidationCandidate {
                candidate,
                existing,
            }])
            .await
            .unwrap();
        let op = ops.into_iter().next().unwrap();
        assert!(
            matches!(op, MemoryOp::Update { ref id, ref old_id } if id == "new-1" && old_id == "old-1")
        );
    }

    #[tokio::test]
    async fn test_llm_consolidation_parses_noop() {
        let mock = Arc::new(MockProvider::new(mock_response(
            r#"{"decisions":[{"index":1,"action":"noop","target_id":null}]}"#,
        )));
        let params = ChatParams::new("test-model");
        let handler = LlmConsolidationHandler::new(mock, params);

        let candidate = test_fact("new-1", "peak_hours", "10am-12pm");
        let existing = vec![test_fact("old-1", "peak_hours", "10am-12pm")];
        let ops = handler
            .decide_batch(&[cognitive::ConsolidationCandidate {
                candidate,
                existing,
            }])
            .await
            .unwrap();
        let op = ops.into_iter().next().unwrap();
        assert_eq!(op, MemoryOp::Noop);
    }

    // ── LLM reflection tests ──

    #[tokio::test]
    async fn test_llm_reflection_parses_output() {
        let json_response = r#"{
            "fact_updates": [
                {"domain":"energy","subject":"user","predicate":"afternoon_dip","object":"energy drops after 3pm","confidence":0.8,"source":"reflected"}
            ],
            "rule_updates": [
                {"domain":"productivity","rule_text":"Suggest break at 3pm when energy declining","confidence":0.75}
            ],
            "summary":"User shows consistent afternoon energy decline. Exercise-productivity correlation observed."
        }"#;
        let mock = Arc::new(MockProvider::new(mock_response(json_response)));
        let params = ChatParams::new("test-model").with_max_tokens(2048);
        let handler = LlmReflectionHandler::new(mock, params);

        let input = ReflectionInput {
            episodic_memories: vec![],
            user_model: UserModel::default(),
            procedural_rules: vec![],
            period_start: "2026-03-01".into(),
            period_end: "2026-03-07".into(),
        };
        let output = handler.reflect(&input).await.unwrap();
        assert_eq!(output.fact_updates.len(), 1);
        assert_eq!(output.fact_updates[0].predicate, "afternoon_dip");
        assert_eq!(output.rule_updates.len(), 1);
        assert!(output.summary.contains("afternoon energy decline"));
    }

    // ── LLM coaching reasoner tests ──

    #[tokio::test]
    async fn test_llm_coaching_reasoner_parses_intervention() {
        let mock = Arc::new(MockProvider::new(mock_response(
            r#"{"should_intervene":true,"confidence":0.75,"message":"You've been distracted 3 times. A short walk might help.","intervention_type":"chat_message","reasoning":"Distraction pattern detected","observations":["Afternoon focus decline"]}"#,
        )));
        let params = ChatParams::new("test-model");
        let handler = LlmCoachingReasonerHandler::new(mock, params);

        let input = ReasonerInput {
            situation: UserSituation::default(),
            trigger: TriggerFired {
                condition_name: "distraction_streak".into(),
                confidence: 0.8,
                context: "3 distractions in 15min".into(),
            },
            patterns: vec![],
            relevant_memories: vec![],
            recent_interventions: vec![],
        };

        let decision = handler.reason(&input).await.unwrap();
        assert!(decision.should_intervene);
        assert!(decision.message.is_some());
        assert!((decision.confidence - 0.75).abs() < 0.01);
    }

    #[tokio::test]
    async fn test_llm_coaching_reasoner_falls_back() {
        let mock = Arc::new(MockProvider::new_error("LLM down"));
        let params = ChatParams::new("test-model");
        let handler = LlmCoachingReasonerHandler::new(mock, params);

        let input = ReasonerInput {
            situation: UserSituation {
                coaching_receptivity: 0.7,
                ..Default::default()
            },
            trigger: TriggerFired {
                condition_name: "distraction_streak".into(),
                confidence: 0.8,
                context: "test".into(),
            },
            patterns: vec![],
            relevant_memories: vec![],
            recent_interventions: vec![],
        };

        let decision = handler.reason(&input).await.unwrap();
        assert!(decision.should_intervene);
    }
}
