//! Cognitive handler implementations — heuristic and LLM-backed.
//!
//! Heuristic handlers provide simple rule-based extraction and consolidation
//! without requiring LLM calls. LLM handlers use structured JSON output
//! for higher-quality results, falling back to heuristics on failure.

use async_trait::async_trait;

use cognitive::extraction::ExtractedFact;
use cognitive::services::extraction_critic::ExtractionCriticHandler;
use cognitive::services::extraction_critic_types::*;
use cognitive::services::micro_reforge::MicroReforgeHandler;
use cognitive::services::micro_reforge_types::{MicroReforgeInput, MicroReforgeOutput};
use cognitive::types::{MemoryOp, Observation};
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
            bus::DomainEvent::KIND_USER_STATED_FACT => vec![fact(od, "stated", 1.0, "user_stated")],
            bus::DomainEvent::KIND_USER_CORRECTED_AI => {
                vec![fact(od, "corrected", 1.0, "user_stated")]
            }
            bus::DomainEvent::KIND_CHAT_TURN_COMPLETED => {
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
                    // Truncate to first sentence or 150 chars — don't store entire messages as facts
                    let object = match content.find(['.', '!']) {
                        Some(i) if i < 150 => content[..=i].to_string(),
                        _ => common::helpers::truncate_at_boundary(content, 150).to_string(),
                    };
                    vec![ExtractedFact {
                        domain: od.into(),
                        subject: "user".into(),
                        predicate,
                        object,
                        confidence: 0.8,
                        source: "user_stated".into(),
                    }]
                }
            }
            bus::DomainEvent::KIND_BUDGET_ALERT => {
                vec![fact("finance", "budget_pressure", 0.9, "observed")]
            }
            bus::DomainEvent::KIND_COACHING_FEEDBACK => {
                vec![fact("coaching", "coaching_response", 0.9, "observed")]
            }
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
            entities: Vec::new(),
            relationships: Vec::new(),
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
Additionally, extract named entities (people, organizations, projects, technologies, places) \
and relationships between them. Only extract entities that are explicitly mentioned — do not infer.\n\n\
Respond with JSON in this exact format:\n\
{\"facts\": [{\"domain\": \"identity\", \"subject\": \"user\", \"predicate\": \"name\", \"object\": \"Jayden\", \"confidence\": 1.0, \"source\": \"user_stated\"}], \
\"entities\": [{\"name\": \"Klynt\", \"type\": \"project\", \"description\": \"AI assistant project\"}], \
\"relationships\": [{\"source\": \"Jayden\", \"target\": \"Klynt\", \"type\": \"works_on\"}]}";

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
    #[serde(default)]
    entities: Vec<ExtractedEntityJson>,
    #[serde(default)]
    relationships: Vec<ExtractedRelationshipJson>,
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

#[derive(serde::Deserialize)]
struct ExtractedEntityJson {
    name: String,
    #[serde(rename = "type")]
    entity_type: String,
    #[serde(default)]
    description: Option<String>,
}

#[derive(serde::Deserialize)]
struct ExtractedRelationshipJson {
    source: String,
    target: String,
    #[serde(rename = "type")]
    relationship_type: String,
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
                entities: Vec::new(),
                relationships: Vec::new(),
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
                        let mut entities = Vec::new();
                        let mut relationships = Vec::new();
                        let extractions = result
                            .results
                            .into_iter()
                            .filter(|r| r.observation_index > 0) // skip invalid 0 indices (prompt is 1-based)
                            .map(|r| {
                                // Aggregate entities and relationships from each observation
                                for e in &r.entities {
                                    entities.push(cognitive::ExtractedEntity {
                                        name: e.name.clone(),
                                        entity_type: e.entity_type.clone(),
                                        description: e.description.clone(),
                                    });
                                }
                                for rel in &r.relationships {
                                    relationships.push(cognitive::ExtractedRelationship {
                                        source_name: rel.source.clone(),
                                        target_name: rel.target.clone(),
                                        relationship_type: rel.relationship_type.clone(),
                                    });
                                }
                                cognitive::BatchExtraction {
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
                                }
                            })
                            .collect();
                        Ok(cognitive::BatchExtractionResult {
                            extractions,
                            fallback_indices: Vec::new(),
                            entities,
                            relationships,
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
The candidate may include SUBJECT NEIGHBORHOOD, OBJECT NEIGHBORHOOD, or CROSS-ENTITY FACTS.\n\
Use this graph context to detect:\n\
- Better existing matches you might miss looking only at exact subject+predicate (return update with target_id pointing to the better match if a CROSS-ENTITY FACT contradicts the new candidate).\n\
- Redundant facts already implied by the neighborhood (return noop).\n\
- Facts that introduce a new entity reference — extract relationships are handled separately, not here. Do NOT invent merges.\n\
Stay conservative: prefer noop over update when uncertain.\n\n\
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

pub(crate) fn build_consolidation_user_message(
    candidates: &[cognitive::ConsolidationCandidate],
) -> String {
    use std::fmt::Write;
    let mut out = String::new();
    out.push_str("CANDIDATES:\n");
    for (i, c) in candidates.iter().enumerate() {
        let _ = writeln!(
            out,
            "[{}] {} -- {} -- {} (confidence: {:.2})",
            i + 1,
            c.candidate.subject,
            c.candidate.predicate,
            c.candidate.object,
            c.candidate.confidence
        );

        if !c.existing.is_empty() {
            out.push_str("  existing for same subject+predicate:\n");
            for ex in &c.existing {
                let _ = writeln!(
                    out,
                    "    - id={} object={} confidence={:.2}",
                    ex.id, ex.object, ex.confidence
                );
            }
        }
        if !c.subject_neighborhood.is_empty() {
            out.push_str("  SUBJECT NEIGHBORHOOD (1-hop):\n");
            for (name, rel) in &c.subject_neighborhood {
                let _ = writeln!(out, "    - {} -> {}", rel, name);
            }
        }
        if !c.object_neighborhood.is_empty() {
            out.push_str("  OBJECT NEIGHBORHOOD (1-hop):\n");
            for (name, rel) in &c.object_neighborhood {
                let _ = writeln!(out, "    - {} -> {}", rel, name);
            }
        }
        if !c.cross_entity_facts.is_empty() {
            out.push_str("  CROSS-ENTITY FACTS (different subject/predicate, same entities):\n");
            for f in &c.cross_entity_facts {
                let _ = writeln!(
                    out,
                    "    - {} -- {} -- {}",
                    f.subject, f.predicate, f.object
                );
            }
        }
    }
    out.push_str(
        "\nDecide for ALL candidates. Return JSON:\n\
         {\"decisions\": [{\"index\": 1, \"action\": \"add|update|delete|noop\", \"target_id\": null}]}\n",
    );
    out
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
        let llm_candidates: Vec<cognitive::ConsolidationCandidate> =
            llm_indices.iter().map(|&i| candidates[i].clone()).collect();
        let user_msg = build_consolidation_user_message(&llm_candidates);

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

// ── Graph Linker ──────────────────────────────────────────────────────

pub(crate) const GRAPH_LINK_SYSTEM_PROMPT: &str = r#"You are a per-turn knowledge graph linker for a personal AI assistant.

You receive a single newly-written fact, the 1-hop neighborhood of its subject and object entities, and up to 5 candidate facts that share an entity with the new fact. Decide:

(1) MERGES — Do any pair of entities in the neighborhood refer to the same real-world thing? Only emit a merge when the names are clearly aliases (case differences, common short forms, exact synonyms). When in doubt, do not merge. Output the canonical name and a one-sentence reason.

(2) DISCOVERED RELATIONSHIPS — Does the new fact reveal a relationship between its entities and any neighbor that isn't already represented? Output as (source, target, relationship_type, edge_type, strength, evidence). edge_type ∈ {"causal", "correlational", "temporal", "structural"}.
  - causal: A causes B (e.g., "deadline approaching" causes "stress increases")
  - correlational: A and B co-occur but no causation claimed (default for most facts)
  - temporal: A precedes B in time (e.g., "graduated" precedes "started job")
  - structural: A is part of / contains B (e.g., "Anthropic contains Claude team")
  Only emit a relationship if (a) both endpoints already exist in the neighborhood or in the new fact, AND (b) the relationship is supported by either the new fact's text or the candidate facts. Do NOT invent edges.

(3) SUPERSEDED — Does the new fact directly contradict or replace any candidate fact? Mark the candidate's id, the valid_until timestamp (use the new fact's valid_at), and a short reason. Conservative: only supersede on direct contradiction, not refinement.

Output strict JSON exactly matching:
{"merges": [...], "discovered_relationships": [...], "superseded": [...]}

If nothing applies, output {"merges": [], "discovered_relationships": [], "superseded": []}. Never invent IDs or names not present in the input."#;

/// LLM-backed graph linker handler.
pub struct LlmGraphLinkHandler {
    provider: DynProvider,
    params: ChatParams,
}

impl LlmGraphLinkHandler {
    pub fn new(provider: DynProvider, params: ChatParams) -> Self {
        Self {
            provider,
            params: params.with_response_format(ResponseFormat::JsonObject),
        }
    }
}

#[async_trait]
impl cognitive::services::graph_linker::GraphLinkHandler for LlmGraphLinkHandler {
    async fn link(
        &self,
        input: cognitive::services::graph_linker_types::GraphLinkInput,
    ) -> common::Result<cognitive::services::graph_linker_types::GraphLinkOutput> {
        if !cognitive::services::graph_linker_types::should_invoke_linker(&input) {
            return Ok(cognitive::services::graph_linker_types::GraphLinkOutput::default());
        }

        let user_msg = build_graph_link_user_message(&input);

        let messages = vec![
            Message::system(GRAPH_LINK_SYSTEM_PROMPT.to_string()),
            Message::user(user_msg),
        ];

        let resp = self
            .provider
            .chat(&messages, None, &self.params)
            .await
            .map_err(|e| crate::provider_err("graph_link", e))?;

        let text = resp.content.unwrap_or_default();
        match serde_json::from_str::<cognitive::services::graph_linker_types::GraphLinkOutput>(
            &text,
        ) {
            Ok(out) => Ok(out),
            Err(e) => {
                tracing::warn!(error = %e, raw = %text, "graph_link: failed to parse LLM JSON; returning empty output");
                Ok(cognitive::services::graph_linker_types::GraphLinkOutput::default())
            }
        }
    }
}

fn build_graph_link_user_message(
    input: &cognitive::services::graph_linker_types::GraphLinkInput,
) -> String {
    use std::fmt::Write;
    let mut s = String::new();
    let nf = &input.new_fact;
    let _ = writeln!(
        s,
        "NEW FACT (id={}): {} -- {} -- {} (confidence: {:.2}, valid_at: {})",
        nf.fact_id, nf.subject, nf.predicate, nf.object, nf.confidence, nf.valid_at
    );

    if let Some(t) = &input.recent_user_text {
        let _ = writeln!(s, "\nRECENT USER TEXT: {}", t);
    }

    let _ = writeln!(s, "\nSUBJECT NEIGHBORHOOD:");
    for n in &input.subject_neighborhood {
        let _ = writeln!(
            s,
            "  - id={} name={} via={} (strength {:.2})",
            n.entity_id, n.name, n.relationship_type, n.strength
        );
    }
    if input.subject_neighborhood.is_empty() {
        let _ = writeln!(s, "  (none)");
    }

    let _ = writeln!(s, "\nOBJECT NEIGHBORHOOD:");
    for n in &input.object_neighborhood {
        let _ = writeln!(
            s,
            "  - id={} name={} via={} (strength {:.2})",
            n.entity_id, n.name, n.relationship_type, n.strength
        );
    }
    if input.object_neighborhood.is_empty() {
        let _ = writeln!(s, "  (none)");
    }

    let _ = writeln!(s, "\nCANDIDATE FACTS:");
    for c in &input.candidate_facts {
        let _ = writeln!(
            s,
            "  - id={} {} -- {} -- {} (valid {} -> {:?})",
            c.fact_id, c.subject, c.predicate, c.object, c.valid_at, c.valid_until
        );
    }
    if input.candidate_facts.is_empty() {
        let _ = writeln!(s, "  (none)");
    }

    s
}

// ── Deep Consolidation ────────────────────────────────────────────────

const DEEP_CONSOLIDATION_PROMPT: &str = r#"You are a knowledge consolidation engine for a personal AI assistant. You receive clusters of signals from different sources about a user. Each cluster groups signals about the same topic.

For each cluster, decide ONE action:
- "fact": Extract a semantic fact as a subject-predicate-object triple
- "rule": Extract a behavioral rule/guideline (only if signals include coaching patterns)
- "episode": Store as an episodic memory event
- "skip": Not significant enough to store

Guidelines:
- Prefer "fact" when confidence >= 0.6 or convergence >= 0.4
- Use "rule" only when coaching pattern signals are present and confidence >= 0.7
- Use "episode" for moderate-confidence observations (>= 0.5) that don't fit fact/rule
- Use "skip" for low-confidence noise
- Facts must have clear subject, predicate, and object
- Rules should be prescriptive behavioral guidelines
- Episodes should be concise event summaries

Respond as JSON:
{
  "decisions": [
    {
      "cluster_index": 0,
      "action": "fact",
      "subject": "Jayden",
      "predicate": "is learning",
      "object": "Rust programming",
      "confidence": 0.85
    }
  ]
}

For "rule" actions, include "rule_text" and "confidence".
For "episode" actions, include "content", "summary", and "importance" (0.0-1.0).
For "skip" actions, just include "cluster_index" and "action"."#;

/// LLM-backed deep consolidation handler for the unified cognitive pipeline.
/// Receives knowledge clusters and uses an LLM to decide promotion operations.
pub struct LlmDeepConsolidationHandler {
    provider: DynProvider,
    params: ChatParams,
}

impl LlmDeepConsolidationHandler {
    pub fn new(provider: DynProvider, params: ChatParams) -> Self {
        Self {
            provider,
            params: params.with_response_format(ResponseFormat::JsonObject),
        }
    }
}

#[async_trait]
impl cognitive::pipeline::DeepConsolidationHandler for LlmDeepConsolidationHandler {
    async fn consolidate_deep(
        &self,
        clusters: &[cognitive::pipeline::KnowledgeCluster],
    ) -> common::Result<Vec<cognitive::pipeline::PromotionOp>> {
        if clusters.is_empty() {
            return Ok(Vec::new());
        }

        // Format clusters for LLM
        let mut user_msg = String::from("Analyze these knowledge clusters:\n\n");
        for (i, cluster) in clusters.iter().enumerate() {
            let sources: Vec<String> = cluster
                .signals
                .iter()
                .map(|s| format!("{:?}", s.source))
                .collect();
            user_msg.push_str(&format!(
                "[Cluster {i}] subject=\"{}\", domain=\"{}\", convergence={:.2}, \
                 confidence={:.2}, sources=[{}]\n",
                cluster.merged_subject,
                cluster.domain,
                cluster.convergence_score,
                cluster.max_confidence,
                sources.join(", ")
            ));
            for obs in &cluster.combined_observations {
                let truncated = if obs.len() > 200 { &obs[..200] } else { obs };
                user_msg.push_str(&format!("  - \"{truncated}\"\n"));
            }
            user_msg.push('\n');
        }

        let messages = vec![
            Message::system(DEEP_CONSOLIDATION_PROMPT),
            Message::user(user_msg),
        ];

        let response = self.provider.chat(&messages, None, &self.params).await?;
        let content = response.content.unwrap_or_default();

        let parsed: DeepConsolidationResponse = serde_json::from_str(&content)
            .map_err(|e| crate::provider_err("Deep consolidation JSON parse", e))?;

        let mut ops = Vec::new();
        for decision in parsed.decisions {
            let idx = decision.cluster_index;
            if idx >= clusters.len() {
                continue;
            }
            let cluster = &clusters[idx];

            match decision.action.as_str() {
                "fact" => {
                    if let (Some(subject), Some(predicate), Some(object)) =
                        (decision.subject, decision.predicate, decision.object)
                    {
                        ops.push(cognitive::pipeline::PromotionOp::CreateFact {
                            subject,
                            predicate,
                            object,
                            domain: cluster.domain,
                            confidence: decision.confidence.unwrap_or(cluster.max_confidence),
                            convergence: cluster.convergence_score,
                            source: format!(
                                "deep+{}",
                                cognitive::pipeline::promotion_source(&cluster.signals)
                            ),
                        });
                    }
                }
                "rule" => {
                    if let Some(rule_text) = decision.rule_text {
                        ops.push(cognitive::pipeline::PromotionOp::CreateRule {
                            rule_text,
                            domain: cluster.domain,
                            confidence: decision.confidence.unwrap_or(cluster.max_confidence),
                        });
                    }
                }
                "episode" => {
                    let content = decision.content.unwrap_or(cluster.merged_subject.clone());
                    let summary = decision.summary.unwrap_or_else(|| {
                        if content.len() > 120 {
                            content[..120].to_string()
                        } else {
                            content.clone()
                        }
                    });
                    ops.push(cognitive::pipeline::PromotionOp::CreateEpisode {
                        content,
                        summary,
                        domain: cluster.domain,
                        importance: decision.importance.unwrap_or(0.5),
                    });
                }
                _ => {} // "skip" or unknown
            }
        }

        Ok(ops)
    }
}

#[derive(serde::Deserialize)]
struct DeepConsolidationResponse {
    decisions: Vec<DeepDecision>,
}

#[derive(serde::Deserialize)]
struct DeepDecision {
    cluster_index: usize,
    action: String,
    subject: Option<String>,
    predicate: Option<String>,
    object: Option<String>,
    rule_text: Option<String>,
    content: Option<String>,
    summary: Option<String>,
    confidence: Option<f64>,
    importance: Option<f64>,
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

// ── Extraction critic handler (KCA Track 5) ────────────────────────────────

pub(crate) const EXTRACTION_CRITIC_SYSTEM_PROMPT: &str = r#"You are a fact-extraction critic.

You receive a turn of conversation and a list of facts that another system extracted from it. For each fact, decide:
- "grounded": the fact is directly stated or strongly implied in the turn text.
- "hallucinated": the fact contains information not present in the turn text. Includes invented entity names, fabricated numbers, or claims the user did not make.
- "ambiguous": evidence is partial or could be read either way; do not penalize.

Also report any FACTS that the extractor MISSED — but only those clearly stated in the turn (≤3 missed facts).

Output JSON:
{"verdicts": [{"fact_id": "...", "verdict": "grounded|hallucinated|ambiguous", "reason": "..."}], "missed_facts": [{"subject": "...", "predicate": "...", "object": "...", "confidence": 0.0-1.0}]}

Be strict on hallucinated but conservative on missed_facts — only include the obvious ones."#;

pub struct LlmExtractionCriticHandler {
    provider: DynProvider,
    params: ChatParams,
}

impl LlmExtractionCriticHandler {
    pub fn new(provider: DynProvider, params: ChatParams) -> Self {
        Self { provider, params }
    }
}

#[async_trait]
impl ExtractionCriticHandler for LlmExtractionCriticHandler {
    async fn judge(&self, input: ExtractionCriticInput) -> common::Result<ExtractionCriticOutput> {
        if input.extracted_facts.is_empty() {
            return Ok(Default::default());
        }
        let user = serde_json::to_string(&input)
            .map_err(|e| crate::provider_err("critic serialize", e))?;
        let messages = vec![
            Message::System {
                content: EXTRACTION_CRITIC_SYSTEM_PROMPT.to_string(),
            },
            Message::User {
                content: providers::UserContent::Text(user),
            },
        ];
        let resp = self
            .provider
            .chat(&messages, None, &self.params)
            .await
            .map_err(|e| crate::provider_err("critic", e))?;
        match serde_json::from_str::<ExtractionCriticOutput>(&resp.content.unwrap_or_default()) {
            Ok(o) => Ok(o),
            Err(e) => {
                tracing::warn!(error = %e, "critic: parse failed");
                Ok(Default::default())
            }
        }
    }
}

// ── Community membership handler (KCA Track 11) ─────────────────────────────

pub(crate) const COMMUNITY_MEMBERSHIP_SYSTEM_PROMPT: &str = r#"You are a knowledge-graph cluster membership confirmer.

You are given a candidate entity, a community summary, and a co-activation score. Decide:
- confirm = true: this entity belongs in the community.
- confirm = false: it does not, the heuristic was misleading.

Output JSON: {"confirm": true|false, "reason": "short reason"}.

Be conservative: confirm only when the entity clearly fits the community theme."#;

#[derive(Debug, Clone)]
pub struct CommunityMembershipDecision {
    pub confirm: bool,
    pub reason: String,
}

pub struct LlmCommunityMembershipHandler {
    provider: DynProvider,
    params: ChatParams,
}

impl std::fmt::Debug for LlmCommunityMembershipHandler {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("LlmCommunityMembershipHandler")
            .field("params", &self.params)
            .finish_non_exhaustive()
    }
}

impl LlmCommunityMembershipHandler {
    pub fn new(provider: DynProvider, params: ChatParams) -> Self {
        Self { provider, params }
    }

    pub async fn confirm_membership(
        &self,
        entity_name: &str,
        entity_type: &str,
        community_name: &str,
        community_summary: &str,
        score: f64,
    ) -> common::Result<CommunityMembershipDecision> {
        let user = format!(
            "Entity: {} (type: {})\nCommunity: {}\nSummary: {}\nCo-activation score: {:.2}\n\nConfirm membership?",
            entity_name, entity_type, community_name, community_summary, score
        );
        let messages = vec![
            Message::System {
                content: COMMUNITY_MEMBERSHIP_SYSTEM_PROMPT.to_string(),
            },
            Message::User {
                content: providers::UserContent::Text(user),
            },
        ];
        let resp = self
            .provider
            .chat(&messages, None, &self.params)
            .await
            .map_err(|e| crate::provider_err("community_membership", e))?;

        #[derive(serde::Deserialize)]
        struct R {
            confirm: bool,
            reason: String,
        }
        match serde_json::from_str::<R>(&resp.content.unwrap_or_default()) {
            Ok(r) => Ok(CommunityMembershipDecision {
                confirm: r.confirm,
                reason: r.reason,
            }),
            Err(_) => Ok(CommunityMembershipDecision {
                confirm: false,
                reason: "parse failed".into(),
            }),
        }
    }
}

#[async_trait::async_trait]
impl cognitive::services::community_membership_online::AsyncConfirmFn
    for LlmCommunityMembershipHandler
{
    async fn confirm(
        &self,
        entity_name: &str,
        entity_type: &str,
        community_name: &str,
        summary: &str,
        score: f64,
    ) -> bool {
        match self
            .confirm_membership(entity_name, entity_type, community_name, summary, score)
            .await
        {
            Ok(decision) => decision.confirm,
            Err(e) => {
                tracing::warn!(error = %e, "community_membership confirm failed");
                false
            }
        }
    }
}

// ── Micro-Reforge handler (KCA Track 4) ────────────────────────────────────

pub(crate) const MICRO_REFORGE_SYSTEM_PROMPT: &str = r#"You are a procedural-rule synthesizer running mid-session.

You are given recent episodic memories, recent low-level observations, and a summary of existing procedural rules. Your job: identify NEW stable behavioral patterns worth promoting to a rule. Rules are short imperative statements ("When X, do Y").

Conservative criteria — only propose a rule when ALL of the following hold:
1. The pattern appears in ≥3 recent episodic memories.
2. The pattern is not already represented in existing_rules_summary (paraphrasing counts as duplicate).
3. The rule is actionable for an AI assistant (not a description; not a fact about the user).
4. Confidence ≥ 0.6.

Output JSON exactly:
{"proposed_rules": [{"domain": "...", "rule_text": "...", "confidence": 0.0-1.0, "signal_count": N, "evidence_episodic_ids": ["..."]}], "notes": "optional"}

If nothing meets the bar, return {"proposed_rules": [], "notes": "no stable patterns this cycle"}. Never invent episodic ids; reference only those provided in the input."#;

pub struct LlmMicroReforgeHandler {
    provider: DynProvider,
    params: ChatParams,
}

impl LlmMicroReforgeHandler {
    pub fn new(provider: DynProvider, params: ChatParams) -> Self {
        Self { provider, params }
    }
}

#[async_trait]
impl MicroReforgeHandler for LlmMicroReforgeHandler {
    async fn synthesize(&self, input: MicroReforgeInput) -> common::Result<MicroReforgeOutput> {
        let user = serde_json::to_string(&input)
            .map_err(|e| crate::provider_err("micro_reforge serialize", e))?;
        let messages = vec![
            Message::System {
                content: MICRO_REFORGE_SYSTEM_PROMPT.to_string(),
            },
            Message::User {
                content: providers::UserContent::Text(user),
            },
        ];
        let resp = self
            .provider
            .chat(&messages, None, &self.params)
            .await
            .map_err(|e| crate::provider_err("micro_reforge", e))?;
        let text = resp.content.unwrap_or_default();
        match serde_json::from_str::<MicroReforgeOutput>(&text) {
            Ok(out) => Ok(out),
            Err(e) => {
                tracing::warn!(error = %e, raw = %text, "micro_reforge: parse failed; returning empty");
                Ok(MicroReforgeOutput::default())
            }
        }
    }
}

// ── Query Predictor (KCA Track 7) ───────────────────────────────────────────

pub(crate) const QUERY_PREDICTOR_SYSTEM_PROMPT: &str = r#"You are a query predictor.

Given the just-completed conversation turn, predict the user's next 1-3 most likely follow-up questions.

Output strict JSON:
{"predictions": ["...", "...", "..."]}

Heuristics:
- If user asked "what is X?", a likely follow-up is "how do I use X?" or "what's an alternative to X?"
- If user asked "how do I do Y?", likely follow-ups are "is there a faster way?" or "what about edge case Z?"
- Keep predictions short (≤80 chars), specific, and grounded in the actual turn content.
- Don't invent topics not present in the conversation."#;

pub struct LlmQueryPredictorHandler {
    provider: providers::DynProvider,
    params: providers::ChatParams,
}

impl LlmQueryPredictorHandler {
    pub fn new(provider: providers::DynProvider, params: providers::ChatParams) -> Self {
        Self { provider, params }
    }

    pub async fn predict_next(&self, recent_turn: &str, n: u32) -> common::Result<Vec<String>> {
        let user = format!("RECENT TURN:\n{}\n\nPredict {n} follow-up questions.", recent_turn);
        let messages = vec![
            providers::Message::system(QUERY_PREDICTOR_SYSTEM_PROMPT),
            providers::Message::user(user),
        ];
        let resp = self.provider.chat(&messages, None, &self.params).await
            .map_err(|e| crate::provider_err("query_predictor", e))?;
        #[derive(serde::Deserialize)]
        struct R { predictions: Vec<String> }
        let parsed: R = serde_json::from_str(&resp.content.unwrap_or_default())
            .map_err(|e| common::KlyntbotError::Json(e))?;
        Ok(parsed.predictions.into_iter().take(n as usize).collect())
    }
}

// ── Hierarchical Summarizer (KCA Track 8) ───────────────────────────────────

pub(crate) const HIERARCHICAL_HOURLY_PROMPT: &str = r#"You are an episodic memory summarizer.

Summarize the following hour of activity into a single 2-3 sentence paragraph that:
- Names the dominant topic(s) of the hour.
- Lists key decisions or discoveries.
- Preserves quantifiable details (numbers, names, durations).

Output plain text, no JSON, no headings."#;

pub(crate) const HIERARCHICAL_DAILY_PROMPT: &str = r#"You are an episodic memory summarizer.

Summarize the following day of hourly summaries into a 4-6 sentence narrative paragraph that:
- Names the day's primary themes.
- Captures arc (morning vs evening, build-up vs resolution).
- Preserves any breakthroughs, blockers, or open questions.

Output plain text, no JSON, no headings."#;

pub(crate) const HIERARCHICAL_WEEKLY_PROMPT: &str = r#"You are an episodic memory summarizer.

Summarize the week of daily summaries into a structured narrative with:
- 1 paragraph: dominant theme of the week.
- 1 paragraph: notable wins.
- 1 paragraph: notable open threads.

Output plain text with paragraph breaks, no JSON, no headings."#;

pub struct LlmHierarchicalSummarizer {
    provider: providers::DynProvider,
    params: providers::ChatParams,
}

impl LlmHierarchicalSummarizer {
    pub fn new(provider: providers::DynProvider, params: providers::ChatParams) -> Self {
        Self { provider, params }
    }
}

#[async_trait::async_trait]
impl cognitive::services::hierarchical_compressor::HierarchicalSummarizer for LlmHierarchicalSummarizer {
    async fn summarize(&self, items: &[cognitive::types::EpisodicMemory], tier: cognitive::services::hierarchical_compressor::Tier) -> common::Result<String> {
        if items.is_empty() { return Ok(String::new()); }
        let system = match tier {
            cognitive::services::hierarchical_compressor::Tier::Raw => "Return content unchanged.",
            cognitive::services::hierarchical_compressor::Tier::Hourly => HIERARCHICAL_HOURLY_PROMPT,
            cognitive::services::hierarchical_compressor::Tier::Daily => HIERARCHICAL_DAILY_PROMPT,
            cognitive::services::hierarchical_compressor::Tier::Weekly => HIERARCHICAL_WEEKLY_PROMPT,
        };
        let user = items.iter()
            .map(|e| format!("[{}] {}", e.recorded_at, e.summary.as_deref().unwrap_or(&e.content)))
            .collect::<Vec<_>>()
            .join("\n");
        let messages = vec![
            providers::Message::system(system),
            providers::Message::user(user),
        ];
        let resp = self.provider.chat(&messages, None, &self.params).await
            .map_err(|e| crate::provider_err("hier_summarize", e))?;
        Ok(resp.content.unwrap_or_default())
    }
}

// ── Temporal Pruner (KCA Track 13) ──────────────────────────────────────────

pub(crate) const TEMPORAL_PRUNE_SYSTEM_PROMPT: &str = r#"You are a temporal pruner for retrieved facts.

You receive a list of facts (subject, predicate, object, valid_at, valid_until) and a query_time. Your job: identify facts that are CLEARLY superseded by newer facts in the same batch about the SAME (subject, predicate) pair. Drop the older one.

Rules:
- Only drop a fact if there's another fact with the SAME (subject, predicate) and a more recent valid_at.
- Do NOT drop facts about different predicates ("works_at" vs "manages" — keep both even if one is older).
- Do NOT drop facts that have explicit `valid_until` — those represent historical truths the user may still want to know.
- If unsure, keep the fact.

Output JSON: {"keep": ["fact_id1", ...], "drop": [{"fact_id": "...", "reason": "..."}]}"#;

pub struct LlmTemporalPrunerHandler {
    provider: providers::DynProvider,
    params: providers::ChatParams,
}

impl LlmTemporalPrunerHandler {
    pub fn new(provider: providers::DynProvider, params: providers::ChatParams) -> Self {
        Self { provider, params }
    }
}

#[async_trait::async_trait]
impl cognitive::services::temporal_pruner::TemporalPrunerHandler for LlmTemporalPrunerHandler {
    async fn prune(&self, input: cognitive::services::temporal_pruner::PruneInput) -> common::Result<cognitive::services::temporal_pruner::PruneOutput> {
        if input.facts.is_empty() { return Ok(Default::default()); }
        let user = serde_json::to_string(&input)
            .map_err(|e| common::KlyntbotError::Json(e))?;
        let messages = vec![
            providers::Message::system(TEMPORAL_PRUNE_SYSTEM_PROMPT),
            providers::Message::user(user),
        ];
        let resp = self.provider.chat(&messages, None, &self.params).await
            .map_err(|e| crate::provider_err("temporal_prune", e))?;
        match serde_json::from_str::<cognitive::services::temporal_pruner::PruneOutput>(&resp.content.unwrap_or_default()) {
            Ok(o) => Ok(o),
            Err(_) => Ok(cognitive::services::temporal_pruner::PruneOutput {
                keep: input.facts.iter().map(|f| f.fact_id.clone()).collect(),
                drop: vec![],
            }),
        }
    }
}

// ── Tests ───────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;

    use cognitive::situation::UserSituation;
    use cognitive::types::{SemanticFact, DEFAULT_MEMORY_TYPE};
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
            timestamp: jiff::Timestamp::now(),
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
            convergence_score: 0.0,
            project_id: None,
            memory_type: DEFAULT_MEMORY_TYPE.to_string(),
            scope_type: "system".to_string(),
            scope_id: None,
            scope_repo_id: None,
            metadata: None,
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
                subject_neighborhood: Vec::new(),
                object_neighborhood: Vec::new(),
                cross_entity_facts: Vec::new(),
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
                subject_neighborhood: Vec::new(),
                object_neighborhood: Vec::new(),
                cross_entity_facts: Vec::new(),
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
                subject_neighborhood: Vec::new(),
                object_neighborhood: Vec::new(),
                cross_entity_facts: Vec::new(),
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
                subject_neighborhood: Vec::new(),
                object_neighborhood: Vec::new(),
                cross_entity_facts: Vec::new(),
            }])
            .await
            .unwrap();
        let result = ops.into_iter().next().unwrap();
        assert!(matches!(result, MemoryOp::Add { .. }));
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
            source_event: bus::DomainEvent::KIND_PRODUCTIVITY_SCORE_COMPUTED.into(),
            timestamp: jiff::Timestamp::now(),
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
            source_event: bus::DomainEvent::KIND_USER_STATED_FACT.into(),
            timestamp: jiff::Timestamp::now(),
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
                subject_neighborhood: Vec::new(),
                object_neighborhood: Vec::new(),
                cross_entity_facts: Vec::new(),
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
                subject_neighborhood: Vec::new(),
                object_neighborhood: Vec::new(),
                cross_entity_facts: Vec::new(),
            }])
            .await
            .unwrap();
        let op = ops.into_iter().next().unwrap();
        assert_eq!(op, MemoryOp::Noop);
    }

    // ── LLM graph linker tests ──

    #[tokio::test]
    async fn llm_graph_link_handler_parses_well_formed_response() {
        use cognitive::services::graph_linker::GraphLinkHandler;
        use cognitive::services::graph_linker_types::*;

        let json_response = r#"{
            "merges": [],
            "discovered_relationships": [
                {"source_entity_name": "Alice", "target_entity_name": "Rust",
                 "relationship_type": "uses", "edge_type": "correlational",
                 "strength": 0.7, "evidence": "Alice prefers Rust per recent fact"}
            ],
            "superseded": []
        }"#;

        let mock = Arc::new(MockProvider::new(mock_response(json_response)));
        let params = ChatParams::new("test-model");
        let handler = LlmGraphLinkHandler::new(mock, params);

        let input = GraphLinkInput {
            new_fact: NewFactRef {
                fact_id: "f1".into(),
                subject: "Alice".into(),
                subject_entity_id: Some("e_alice".into()),
                predicate: "prefers".into(),
                object: "Rust".into(),
                object_entity_id: None,
                confidence: 0.8,
                valid_at: "2026-04-29T00:00:00Z".into(),
            },
            subject_neighborhood: vec![NeighborRef {
                entity_id: "e_bob".into(),
                name: "Bob".into(),
                relationship_type: "knows".into(),
                strength: 0.8,
            }],
            object_neighborhood: vec![],
            candidate_facts: vec![],
            recent_user_text: None,
        };

        let out = handler.link(input).await.unwrap();
        assert_eq!(out.discovered_relationships.len(), 1);
        assert_eq!(out.discovered_relationships[0].edge_type, "correlational");
    }

    #[tokio::test]
    async fn llm_graph_link_handler_returns_empty_on_malformed_response() {
        use cognitive::services::graph_linker::GraphLinkHandler;
        use cognitive::services::graph_linker_types::*;

        let mock = Arc::new(MockProvider::new(mock_response("not json")));
        let params = ChatParams::new("test-model");
        let handler = LlmGraphLinkHandler::new(mock, params);

        let input = GraphLinkInput {
            new_fact: NewFactRef {
                fact_id: "f1".into(),
                subject: "A".into(),
                subject_entity_id: None,
                predicate: "p".into(),
                object: "B".into(),
                object_entity_id: None,
                confidence: 0.5,
                valid_at: "2026-04-29T00:00:00Z".into(),
            },
            subject_neighborhood: vec![NeighborRef {
                entity_id: "e".into(),
                name: "X".into(),
                relationship_type: "r".into(),
                strength: 0.5,
            }],
            object_neighborhood: vec![],
            candidate_facts: vec![],
            recent_user_text: None,
        };

        let out = handler.link(input).await.unwrap();
        assert!(out.discovered_relationships.is_empty());
        assert!(out.merges.is_empty());
        assert!(out.superseded.is_empty());
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

    #[test]
    fn build_consolidation_user_message_includes_neighborhood_section_when_present() {
        use cognitive::types::SemanticFact;
        let cand = cognitive::ConsolidationCandidate {
            candidate: SemanticFact {
                id: "c1".into(),
                domain: "test".into(),
                subject: "Alice".into(),
                predicate: "prefers".into(),
                object: "Rust".into(),
                confidence: 0.7,
                source: "t".into(),
                valid_from: "2026-04-29".into(),
                valid_until: None,
                recorded_at: "2026-04-29".into(),
                superseded_at: None,
                superseded_by: None,
                stability: 1.0,
                last_accessed: None,
                access_count: 0,
                convergence_score: 0.0,
                project_id: None,
                memory_type: "fact".into(),
                scope_type: "system".into(),
                scope_id: None,
                scope_repo_id: None,
                metadata: None,
            },
            existing: vec![],
            subject_neighborhood: vec![("Bob".into(), "knows".into())],
            object_neighborhood: vec![],
            cross_entity_facts: vec![SemanticFact {
                id: "f2".into(),
                domain: "test".into(),
                subject: "Bob".into(),
                predicate: "works_at".into(),
                object: "Anthropic".into(),
                confidence: 0.9,
                source: "t".into(),
                valid_from: "2026-04-29".into(),
                valid_until: None,
                recorded_at: "2026-04-29".into(),
                superseded_at: None,
                superseded_by: None,
                stability: 1.0,
                last_accessed: None,
                access_count: 0,
                convergence_score: 0.0,
                project_id: None,
                memory_type: "fact".into(),
                scope_type: "system".into(),
                scope_id: None,
                scope_repo_id: None,
                metadata: None,
            }],
        };

        let msg = build_consolidation_user_message(&[cand]);

        assert!(
            msg.contains("SUBJECT NEIGHBORHOOD"),
            "must include neighborhood section, got:\n{msg}"
        );
        assert!(msg.contains("knows -> Bob"));
        assert!(msg.contains("CROSS-ENTITY FACTS"));
        assert!(msg.contains("works_at"));
    }

    #[tokio::test]
    async fn llm_micro_reforge_handler_extracts_high_confidence_rules() {
        use cognitive::services::micro_reforge::MicroReforgeHandler;
        use cognitive::services::micro_reforge_types::*;

        let json = r#"{
            "proposed_rules": [
                {"domain": "coding", "rule_text": "When user runs cargo nextest, also run clippy.",
                 "confidence": 0.85, "signal_count": 4,
                 "evidence_episodic_ids": ["ep1", "ep2"]}
            ],
            "notes": null
        }"#;
        let provider = MockProvider::new(LlmResponse {
            content: Some(json.to_string()),
            tool_calls: vec![],
            finish_reason: "stop".to_string(),
            usage: Usage {
                prompt_tokens: 10,
                completion_tokens: 5,
                total_tokens: 15,
                cache_read_tokens: 0,
                cache_write_tokens: 0,
            },
            reasoning_content: None,
        });
        let handler = LlmMicroReforgeHandler::new(
            Arc::new(provider),
            ChatParams::new("m").with_max_tokens(2048),
        );

        let input = MicroReforgeInput {
            recent_episodics: vec![EpisodicRef {
                id: "ep1".into(),
                domain: "coding".into(),
                summary: "user ran cargo nextest".into(),
                importance: 0.7,
                recorded_at: "2026-04-29T00:00:00Z".into(),
            }],
            recent_observations: vec![],
            existing_rules_summary: vec![],
            session_count: 3,
            turn_count_since_last_run: 10,
        };

        let out = handler.synthesize(input).await.unwrap();
        assert_eq!(out.proposed_rules.len(), 1);
        assert!(out.proposed_rules[0].confidence > 0.8);
    }

    #[tokio::test]
    async fn llm_community_membership_confirms_match() {
        let json = r#"{"confirm": true, "reason": "C aligns with alpha by topic"}"#;
        let provider = MockProvider::new(LlmResponse {
            content: Some(json.to_string()),
            tool_calls: vec![],
            finish_reason: "stop".to_string(),
            usage: Usage {
                prompt_tokens: 10,
                completion_tokens: 5,
                total_tokens: 15,
                cache_read_tokens: 0,
                cache_write_tokens: 0,
            },
            reasoning_content: None,
        });
        let handler = LlmCommunityMembershipHandler::new(
            Arc::new(provider),
            ChatParams::new("m").with_max_tokens(256),
        );

        let result = handler
            .confirm_membership("C", "concept", "alpha", "topic about α-particles", 0.85)
            .await
            .unwrap();
        assert!(result.confirm);
    }

    #[tokio::test]
    async fn llm_extraction_critic_marks_hallucinated_facts() {
        use cognitive::services::extraction_critic::ExtractionCriticHandler;
        use cognitive::services::extraction_critic_types::*;

        let json = r#"{
            "verdicts": [
                {"fact_id": "f1", "verdict": "grounded", "reason": "directly stated"},
                {"fact_id": "f2", "verdict": "hallucinated", "reason": "not in turn text"}
            ],
            "missed_facts": []
        }"#;
        let provider = MockProvider::new(LlmResponse {
            content: Some(json.to_string()),
            tool_calls: vec![],
            finish_reason: "stop".to_string(),
            usage: Usage {
                prompt_tokens: 10,
                completion_tokens: 5,
                total_tokens: 15,
                cache_read_tokens: 0,
                cache_write_tokens: 0,
            },
            reasoning_content: None,
        });
        let handler = LlmExtractionCriticHandler::new(
            Arc::new(provider),
            ChatParams::new("m").with_max_tokens(1024),
        );

        let input = ExtractionCriticInput {
            turn_text: "I love Rust.".into(),
            extracted_facts: vec![
                FactRef {
                    fact_id: "f1".into(),
                    subject: "user".into(),
                    predicate: "loves".into(),
                    object: "Rust".into(),
                },
                FactRef {
                    fact_id: "f2".into(),
                    subject: "user".into(),
                    predicate: "hates".into(),
                    object: "Java".into(),
                },
            ],
        };

        let out = handler.judge(input).await.unwrap();
        assert_eq!(out.verdicts.len(), 2);
        assert_eq!(out.verdicts[1].verdict, "hallucinated");
    }

    #[tokio::test]
    async fn llm_query_predictor_returns_n_predictions() {
        let json = r#"{"predictions": ["how do I install rust?", "what about cargo?", "memory safety details?"]}"#;
        let provider = MockProvider::new(LlmResponse {
            content: Some(json.to_string()),
            tool_calls: vec![],
            finish_reason: "stop".to_string(),
            usage: Usage::default(),
            reasoning_content: None,
        });
        let handler = LlmQueryPredictorHandler::new(
            Arc::new(provider),
            providers::ChatParams::new("m").with_max_tokens(256).with_response_format(providers::ResponseFormat::JsonObject),
        );

        let preds = handler.predict_next("user just asked about Rust", 3).await.unwrap();
        assert_eq!(preds.len(), 3);
    }

    #[tokio::test]
    async fn llm_temporal_pruner_drops_facts_with_explicit_supersede() {
        use cognitive::services::temporal_pruner::*;

        let json = r#"{"keep": ["f2"], "drop": [{"fact_id": "f1", "reason": "f2 supersedes by date"}]}"#;
        let provider = MockProvider::new(LlmResponse {
            content: Some(json.to_string()),
            tool_calls: vec![],
            finish_reason: "stop".to_string(),
            usage: Usage::default(),
            reasoning_content: None,
        });
        let handler = LlmTemporalPrunerHandler::new(
            Arc::new(provider),
            providers::ChatParams::new("m").with_max_tokens(512).with_response_format(providers::ResponseFormat::JsonObject),
        );

        let input = PruneInput {
            facts: vec![
                PruneFactRef { fact_id: "f1".into(), subject: "Alice".into(), predicate: "works_at".into(), object: "Google".into(), valid_at: "2023-01-01T00:00:00Z".into(), valid_until: None },
                PruneFactRef { fact_id: "f2".into(), subject: "Alice".into(), predicate: "works_at".into(), object: "Anthropic".into(), valid_at: "2025-06-01T00:00:00Z".into(), valid_until: None },
            ],
            query_time: "2026-04-29T00:00:00Z".into(),
        };
        let out = handler.prune(input).await.unwrap();
        assert_eq!(out.drop.len(), 1);
        assert_eq!(out.drop[0].fact_id, "f1");
    }
}
