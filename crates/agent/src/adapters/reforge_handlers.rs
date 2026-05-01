//! LLM-backed and no-op ReforgeHandler implementations.
//!
//! Dependency inversion: the `ReforgeHandler` trait is defined in `cognitive`
//! (L4) but implemented here in `agent` (L5) where LLM providers are available.

use async_trait::async_trait;

use cognitive::services::reforge::types::{
    NarrateInput, ReviewInput, ReviewOutput, SkillContent, SynthesizeInput, SynthesizeOutput,
};
use cognitive::services::reforge::ReforgeHandler;
use providers::{ChatParams, DynProvider, Message, ResponseFormat};

// ---------------------------------------------------------------------------
// Prompt constants
// ---------------------------------------------------------------------------

const SYNTHESIZE_PROMPT: &str = "\
You are a knowledge consolidation engine for a personal AI assistant.
Analyze the user's recent sessions and episodic memories against their existing knowledge base.

For each session, look for:
1. New facts about the user (preferences, habits, skills, context)
2. Changes to existing beliefs (contradictions, updates, refinements)
3. Cross-session patterns (behaviors that appear across multiple sessions)
4. Stale facts no longer supported by recent evidence

Respond with JSON:
{\"fact_updates\":[{\"action\":\"add\"|\"update\"|\"remove\",\"subject\":\"...\",\"predicate\":\"...\",\"object\":\"...\",\"domain\":\"...\",\"confidence\":0.0-1.0,\"reason\":\"...\"}],\"rule_updates\":[{\"action\":\"add\"|\"update\"|\"reinforce\",\"rule_text\":\"...\",\"domain\":\"...\",\"reason\":\"...\"}],\"stale_facts\":[{\"fact_id\":\"...\",\"reason\":\"...\"}],\"cross_session_patterns\":[{\"pattern\":\"...\",\"confidence\":0.0-1.0}],\"extraction_quality_flag\":null}";

const REVIEW_PROMPT: &str = "\
You are a skill improvement engine for a personal AI assistant.
Analyze correction patterns and routing data to propose targeted skill edits.

For each skill, consider:
1. Does whenToUse cover the actual trigger phrases users employ?
2. Are there correction patterns that indicate bad instructions in the skill body?
3. Do reference files need updates based on learned patterns?

Only propose edits with clear evidence. Do not speculate.

If autotuner context is provided, also suggest up to 3 parameter experiments.
Each experiment should have a hypothesis, a pace (conservative/balanced/bold), and param_overrides (only params you want to change — the rest inherit from champion).
Build on past experiment results: don't repeat configurations that failed. Focus on parameters related to the issues you identified.
Only suggest experiments when you have evidence (routing problems, retrieval issues, correction patterns). If everything is working well, return an empty trial_suggestions array.

When tool failure data is provided, analyze error patterns and propose skill edits that fix the root cause.
For example, if a tool consistently receives wrong parameter types, update the skill's Critical Rules to clarify the correct parameter usage.

When correction data is provided, identify which skill instructions led to the mistake and propose targeted fixes.

When previous suggestions are provided, evaluate whether they should still be acted upon or have become stale.

Respond with JSON:
{\"skill_edits\":[{\"skill_name\":\"...\",\"file_path\":\"...\",\"edit_type\":\"frontmatter\"|\"body_replace\"|\"body_insert\"|\"body_remove\",\"field\":null,\"new_value\":null,\"old_text\":null,\"new_text\":null,\"section\":null,\"reason\":\"...\"}],\"routing_insights\":[\"...\"],\"context_priority_suggestions\":[{\"source\":\"...\",\"suggestion\":\"...\",\"reason\":\"...\"}],\"trial_suggestions\":[{\"hypothesis\":\"...\",\"pace\":\"conservative\"|\"balanced\"|\"bold\",\"param_overrides\":{\"param_name\":0.0}}]}";

const NARRATE_PROMPT: &str = "\
Summarize tonight's Reforge cycle for the user in 2-3 concise paragraphs.
Include: what was learned, what changed (facts, rules, skills), and any notable patterns.
Be conversational, not clinical. Address the user directly.";

const GRAPH_ENRICHMENT_PROMPT: &str = "\
You are a knowledge graph maintenance agent. Given conversation excerpts and \
candidate duplicate entity pairs, make decisions about entity merging and \
discover relationships.\n\n\
For duplicate candidates, decide if two entities refer to the same real-world thing. \
Consider: name similarity, type match, context clues.\n\n\
For conversation excerpts, extract any relationships between mentioned entities.\n\n\
Respond with JSON:\n\
{\"merge_decisions\": [{\"entity_a_id\": \"...\", \"entity_b_id\": \"...\", \
\"should_merge\": true, \"canonical_name\": \"Preferred Name\", \
\"reason\": \"Same entity, different casing\"}], \
\"relationships\": [{\"source\": \"Entity A\", \"target\": \"Entity B\", \
\"type\": \"works_on\", \"strength\": 0.7}]}";

// ---------------------------------------------------------------------------
// Input formatters
// ---------------------------------------------------------------------------

fn format_synthesize_input(input: &SynthesizeInput) -> String {
    use std::fmt::Write;
    let mut out = String::new();

    // Sessions
    writeln!(&mut out, "## Recent Sessions ({})", input.sessions.len()).unwrap();
    for session in &input.sessions {
        writeln!(
            &mut out,
            "### Session: {} | Turns: {} | Updated: {}",
            session.session_key, session.turn_count, session.updated_at
        )
        .unwrap();
        if !session.scratchpad.is_empty() {
            writeln!(&mut out, "Scratchpad:\n{}\n", session.scratchpad).unwrap();
        }
    }

    // Episodic memories
    writeln!(
        &mut out,
        "\n## Episodic Memories ({})",
        input.episodic_memories.len()
    )
    .unwrap();
    for mem in &input.episodic_memories {
        writeln!(
            &mut out,
            "- [{}] {}: {}",
            mem.occurred_at, mem.domain, mem.summary
        )
        .unwrap();
    }

    // User model
    if !input.user_model_summary.is_empty() {
        writeln!(
            &mut out,
            "\n## Current User Model\n{}",
            input.user_model_summary
        )
        .unwrap();
    }

    // Rules
    if !input.rules_summary.is_empty() {
        writeln!(&mut out, "\n## Active Rules\n{}", input.rules_summary).unwrap();
    }

    // Retrieval precision hint
    if let Some(precision) = input.retrieval_precision {
        writeln!(
            &mut out,
            "\n## Retrieval Precision: {:.1}%",
            precision * 100.0
        )
        .unwrap();
    }

    out
}

fn format_review_input(input: &ReviewInput) -> String {
    use std::fmt::Write;
    let mut out = String::new();

    // Pending meta rules
    if !input.pending_meta_rules.is_empty() {
        writeln!(
            &mut out,
            "## Pending Meta Rules ({})",
            input.pending_meta_rules.len()
        )
        .unwrap();
        for rule in &input.pending_meta_rules {
            writeln!(&mut out, "- {rule}").unwrap();
        }
        out.push('\n');
    }

    // Routing summaries
    writeln!(
        &mut out,
        "## Routing Summaries ({})",
        input.routing_summaries.len()
    )
    .unwrap();
    for r in &input.routing_summaries {
        writeln!(
            &mut out,
            "- {}: {} messages | avg_confidence={:.2} | fallback_rate={:.1}%",
            r.skill_name,
            r.message_count,
            r.avg_confidence,
            r.fallback_rate * 100.0,
        )
        .unwrap();
    }

    // New facts summary
    if !input.new_facts_summary.is_empty() {
        writeln!(
            &mut out,
            "\n## New Facts from Synthesis\n{}",
            input.new_facts_summary
        )
        .unwrap();
    }

    // Retrieval precision hint
    if let Some(precision) = input.retrieval_precision {
        writeln!(
            &mut out,
            "\n## Retrieval Precision: {:.1}%",
            precision * 100.0
        )
        .unwrap();
    }

    // Tool failures
    if let Some(ref summary) = input.tool_failure_summary {
        writeln!(&mut out, "\n## Tool Health (since last cycle)\n{summary}").unwrap();
    }

    // Correction patterns per skill
    if let Some(ref summary) = input.correction_summary {
        writeln!(&mut out, "\n## Correction Patterns\n{summary}").unwrap();
    }

    // Previous suggestions
    if let Some(ref summary) = input.previous_suggestions_summary {
        writeln!(
            &mut out,
            "\n## Previous Cycle Suggestions (unacted)\n{summary}"
        )
        .unwrap();
    }

    // Autotuner context
    if let Some(ref ctx) = input.autotuner_context {
        writeln!(&mut out, "\n## Autotuner Context\n{ctx}").unwrap();
    }

    // Skill contents
    if !input.skill_contents.is_empty() {
        writeln!(
            &mut out,
            "\n## Skill Files ({})",
            input.skill_contents.len()
        )
        .unwrap();
        for SkillContent {
            skill_name,
            file_path,
            content,
        } in &input.skill_contents
        {
            writeln!(&mut out, "### {skill_name} ({file_path})").unwrap();
            writeln!(&mut out, "```\n{content}\n```\n").unwrap();
        }
    }

    out
}

// ---------------------------------------------------------------------------
// LlmReforgeHandler
// ---------------------------------------------------------------------------

/// LLM-backed implementation of [`ReforgeHandler`].
///
/// Uses a DynProvider to drive the three LLM phases of the Reforge cycle:
/// Synthesize, Review, and Narrate.
pub struct LlmReforgeHandler {
    provider: DynProvider,
    params: ChatParams,
}

impl LlmReforgeHandler {
    pub fn new(provider: DynProvider, params: ChatParams) -> Self {
        Self {
            provider,
            params: params.with_response_format(ResponseFormat::JsonObject),
        }
    }
}

#[async_trait]
impl ReforgeHandler for LlmReforgeHandler {
    async fn synthesize(&self, input: &SynthesizeInput) -> common::Result<SynthesizeOutput> {
        let user_msg = format_synthesize_input(input);
        let messages = vec![Message::system(SYNTHESIZE_PROMPT), Message::user(user_msg)];

        let response = self.provider.chat(&messages, None, &self.params).await?;
        let content = response.content.unwrap_or_default();

        serde_json::from_str::<SynthesizeOutput>(&content).map_err(|e| {
            tracing::warn!("Reforge synthesize JSON parse failed: {e}");
            common::KlyntbotError::Provider(common::ProviderError::InvalidResponse(format!(
                "Reforge synthesize JSON parse error: {e}"
            )))
        })
    }

    async fn review(&self, input: &ReviewInput) -> common::Result<ReviewOutput> {
        let user_msg = format_review_input(input);
        let messages = vec![Message::system(REVIEW_PROMPT), Message::user(user_msg)];

        let response = self.provider.chat(&messages, None, &self.params).await?;
        let content = response.content.unwrap_or_default();

        serde_json::from_str::<ReviewOutput>(&content).map_err(|e| {
            tracing::warn!("Reforge review JSON parse failed: {e}");
            common::KlyntbotError::Provider(common::ProviderError::InvalidResponse(format!(
                "Reforge review JSON parse error: {e}"
            )))
        })
    }

    async fn narrate(&self, input: &NarrateInput) -> common::Result<String> {
        let user_msg = format!(
            "## Synthesis Summary\n{}\n\n## Review Summary\n{}\n\n## Routing Summary\n{}",
            input.synthesize_summary, input.review_summary, input.routing_summary
        );
        let messages = vec![Message::system(NARRATE_PROMPT), Message::user(user_msg)];

        // Use plain text format for the narrative — override the stored JsonObject params
        let text_params = self
            .params
            .clone()
            .with_response_format(ResponseFormat::Text);

        let response = self.provider.chat(&messages, None, &text_params).await?;
        Ok(response.content.unwrap_or_default())
    }
}

// ---------------------------------------------------------------------------
// LlmGraphEnrichmentHandler — JSON types
// ---------------------------------------------------------------------------

#[derive(serde::Deserialize)]
struct EnrichmentResponse {
    #[serde(default)]
    merge_decisions: Vec<MergeDecisionJson>,
    #[serde(default)]
    relationships: Vec<DiscoveredRelationshipJson>,
}

#[derive(serde::Deserialize)]
struct MergeDecisionJson {
    entity_a_id: String,
    entity_b_id: String,
    should_merge: bool,
    canonical_name: Option<String>,
    reason: String,
}

#[derive(serde::Deserialize)]
struct DiscoveredRelationshipJson {
    source: String,
    target: String,
    #[serde(rename = "type")]
    relationship_type: String,
    #[serde(default = "default_rel_strength")]
    strength: f64,
}

fn default_rel_strength() -> f64 {
    0.5
}

// ---------------------------------------------------------------------------
// LlmGraphEnrichmentHandler
// ---------------------------------------------------------------------------

/// LLM-backed graph enrichment for Reforge Phase 6.5.
pub struct LlmGraphEnrichmentHandler {
    provider: DynProvider,
    params: ChatParams,
}

impl LlmGraphEnrichmentHandler {
    pub fn new(provider: DynProvider, params: ChatParams) -> Self {
        Self {
            provider,
            params: params.with_response_format(ResponseFormat::JsonObject),
        }
    }
}

#[async_trait]
impl cognitive::services::reforge::GraphEnrichmentHandler for LlmGraphEnrichmentHandler {
    async fn enrich_graph(
        &self,
        input: &cognitive::services::graph_enrichment::GraphEnrichmentInput,
    ) -> common::Result<cognitive::services::graph_enrichment::GraphEnrichmentOutput> {
        use cognitive::services::graph_enrichment::*;

        // Build the user message with context
        let mut user_msg = String::new();
        if !input.duplicate_candidates.is_empty() {
            user_msg.push_str("## Duplicate Candidates\n");
            for c in &input.duplicate_candidates {
                user_msg.push_str(&format!(
                    "- ({}) \"{}\" vs ({}) \"{}\"\n",
                    c.entity_a_id, c.entity_a_name, c.entity_b_id, c.entity_b_name
                ));
            }
        }
        if !input.turn_previews.is_empty() {
            user_msg.push_str("\n## Conversation Excerpts\n");
            for (i, preview) in input.turn_previews.iter().enumerate() {
                user_msg.push_str(&format!("{}: {}\n", i + 1, preview));
            }
        }

        if user_msg.is_empty() {
            return Ok(GraphEnrichmentOutput::default());
        }

        let messages = vec![
            Message::system(GRAPH_ENRICHMENT_PROMPT),
            Message::user(user_msg),
        ];

        let response = self.provider.chat(&messages, None, &self.params).await?;
        let content = response.content.unwrap_or_default();
        let text = content.trim();

        // Parse response JSON
        let parsed: EnrichmentResponse = serde_json::from_str(text).unwrap_or(EnrichmentResponse {
            merge_decisions: Vec::new(),
            relationships: Vec::new(),
        });

        Ok(GraphEnrichmentOutput {
            merge_decisions: parsed
                .merge_decisions
                .into_iter()
                .map(|d| MergeDecision {
                    entity_a_id: d.entity_a_id,
                    entity_b_id: d.entity_b_id,
                    should_merge: d.should_merge,
                    canonical_name: d.canonical_name,
                    reason: d.reason,
                })
                .collect(),
            discovered_relationships: parsed
                .relationships
                .into_iter()
                .map(|r| DiscoveredRelationship {
                    source_entity_name: r.source,
                    target_entity_name: r.target,
                    relationship_type: r.relationship_type,
                    strength: r.strength,
                })
                .collect(),
        })
    }
}

// ---------------------------------------------------------------------------
// CommunityIntelligenceHandler — JSON types
// ---------------------------------------------------------------------------

const COMMUNITY_INTELLIGENCE_PROMPT: &str = "\
You are a knowledge graph curator. Review these communities and:\n\
1. Generate a short (2-4 word) noun-phrase label for each\n\
2. Identify pairs that should MERGE (same topic, fragmented)\n\
3. Identify any that should SPLIT (multiple unrelated topics)\n\n\
Rules:\n\
- Labels must be unique, human-readable noun phrases\n\
- Only merge when communities clearly overlap (>50% entity similarity)\n\
- Only split when a community has 2+ clearly distinct domains\n\
- Prefer stability: don't merge/split unless evidence is strong\n\
- Skip merge/split for communities younger than 3 days\n\n\
Respond with JSON:\n\
{\"names\": [{\"id\": \"...\", \"label\": \"...\"}], \
\"merges\": [{\"absorb\": \"...\", \"into\": \"...\", \"reason\": \"...\"}], \
\"splits\": [{\"id\": \"...\", \"reason\": \"...\"}]}";

#[derive(serde::Deserialize)]
struct CommunityIntelligenceResponse {
    #[serde(default)]
    names: Vec<CommunityNameJson>,
    #[serde(default)]
    merges: Vec<CommunityMergeJson>,
    #[serde(default)]
    splits: Vec<CommunitySplitJson>,
}

#[derive(serde::Deserialize)]
struct CommunityNameJson {
    id: String,
    label: String,
}

#[derive(serde::Deserialize)]
struct CommunityMergeJson {
    absorb: String,
    into: String,
    reason: String,
}

#[derive(serde::Deserialize)]
struct CommunitySplitJson {
    id: String,
    reason: String,
}

#[async_trait]
impl cognitive::services::reforge::CommunityIntelligenceHandler for LlmGraphEnrichmentHandler {
    async fn analyze_communities(
        &self,
        input: &cognitive::services::community_intelligence::CommunityIntelligenceInput,
    ) -> common::Result<cognitive::services::community_intelligence::CommunityIntelligenceOutput>
    {
        use cognitive::services::community_intelligence::*;

        if input.communities.is_empty() {
            return Ok(CommunityIntelligenceOutput::default());
        }

        // Build compact community list for the prompt
        let mut user_msg = String::from("Communities:\n");
        for c in &input.communities {
            user_msg.push_str(&format!(
                "- id={}, name=\"{}\", entities=[{}], members={}, age={}d\n",
                c.id,
                c.current_name,
                c.entities.join(", "),
                c.member_count,
                c.age_days,
            ));
        }

        let messages = vec![
            Message::system(COMMUNITY_INTELLIGENCE_PROMPT),
            Message::user(user_msg),
        ];

        let response = self.provider.chat(&messages, None, &self.params).await?;
        let content = response.content.unwrap_or_default();
        let text = content.trim();

        let parsed: CommunityIntelligenceResponse =
            serde_json::from_str(text).unwrap_or(CommunityIntelligenceResponse {
                names: Vec::new(),
                merges: Vec::new(),
                splits: Vec::new(),
            });

        Ok(CommunityIntelligenceOutput {
            names: parsed
                .names
                .into_iter()
                .map(|n| CommunityRename {
                    community_id: n.id,
                    label: n.label,
                })
                .collect(),
            merges: parsed
                .merges
                .into_iter()
                .map(|m| CommunityMerge {
                    absorb_id: m.absorb,
                    into_id: m.into,
                    reason: m.reason,
                })
                .collect(),
            splits: parsed
                .splits
                .into_iter()
                .map(|s| CommunitySplit {
                    community_id: s.id,
                    reason: s.reason,
                })
                .collect(),
        })
    }
}

// ---------------------------------------------------------------------------
// LlmCrossCliSynthesisHandler — KCA Track 10
// ---------------------------------------------------------------------------

const CROSS_CLI_SYNTHESIS_PROMPT: &str = r#"You confirm cross-CLI procedural-rule transfers.

For each transferable candidate, decide whether the rule should be promoted from "observed in ONE CLI" to "applicable across all CLIs". Approve when:
- support_strength ≥ 3 distinct supporting episodes from a non-observed source.
- The rule is generic (does not reference CLI-specific tooling like "claude code" or "codex").
- Confidence ≥ 0.85 (already filtered upstream; double-check).

Reject when:
- The rule contains CLI-specific keywords ("claude_code", "codex", "kimi-cli", "opencode" by name).
- Evidence is repetitive (same action many times in one short window).
- The rule contradicts an existing source-agnostic rule.

Output JSON: {"approved_rule_ids": ["..."], "rejected": [{"rule_id": "...", "reason": "..."}], "notes": "..."}"#;

#[derive(serde::Deserialize, Default)]
pub struct CrossCliSynthesisOutput {
    pub approved_rule_ids: Vec<String>,
    pub rejected: Vec<RejectedRule>,
    pub notes: Option<String>,
}

#[derive(serde::Deserialize)]
pub struct RejectedRule {
    pub rule_id: String,
    pub reason: String,
}

/// LLM-backed handler that confirms whether transferable rules should be promoted.
pub struct LlmCrossCliSynthesisHandler {
    provider: DynProvider,
    params: ChatParams,
}

impl LlmCrossCliSynthesisHandler {
    pub fn new(provider: DynProvider, model: impl Into<String>, max_tokens: u32) -> Self {
        Self {
            provider,
            params: ChatParams::new(model)
                .with_temperature(0.1)
                .with_max_tokens(max_tokens)
                .with_response_format(ResponseFormat::JsonObject),
        }
    }

    pub async fn confirm_promotions(
        &self,
        candidates: Vec<coding_memory::reforge::cross_cli_synthesis::TransferableCandidate>,
    ) -> common::Result<CrossCliSynthesisOutput> {
        if candidates.is_empty() {
            return Ok(Default::default());
        }
        let user = serde_json::to_string(&candidates)?;
        let messages = vec![
            Message::system(CROSS_CLI_SYNTHESIS_PROMPT),
            Message::user(user),
        ];
        let resp = self.provider.chat(&messages, None, &self.params).await?;
        let text = resp.content.unwrap_or_default();
        Ok(serde_json::from_str(&text).unwrap_or_default())
    }
}

// ---------------------------------------------------------------------------
// LlmSkillDiscoveryHandler — KCA Track 12
// ---------------------------------------------------------------------------

const SKILL_DISCOVERY_PROMPT: &str = r###"You are a Klynt skill author.

You are given clusters of related procedural rules (each cluster is a coherent behavioral pattern).
For each cluster, produce a candidate Klynt orchestrator skill following the Agent Skills format:

- name: short, lowercase-with-dashes, ≤30 chars.
- description: 1-2 sentences explaining when this skill should activate.
- yaml_frontmatter: a complete YAML frontmatter block (--- … ---) with `name`, `description`, optional `mcp_tools` list.
- body_markdown: the skill body. Should include:
  - "# {Title}" header
  - "## When to invoke" with concrete user-message patterns
  - "## Steps" with numbered procedure
  - "## References" with links if applicable

Output JSON exactly:
{"skills": [{"name": "...", "description": "...", "yaml_frontmatter": "...", "body_markdown": "...", "source_rule_ids": ["..."], "avg_confidence": 0.0-1.0}], "notes": "..."}

Rules:
- Skip clusters that don't form a coherent skill (e.g., random unrelated rules with low Jaccard overlap).
- Never invent rule_ids; reference only those provided.
- Never name a skill the same as an existing skill (avoid: task-management, finance-management, automation, learning, notebook).
- Keep skills simple. If unsure, don't propose."###;

#[derive(serde::Deserialize, Default, Debug)]
pub struct SkillDiscoveryOutput {
    pub skills: Vec<ProposedSkill>,
    pub notes: Option<String>,
}

#[derive(serde::Deserialize, Debug)]
pub struct ProposedSkill {
    pub name: String,
    pub description: String,
    pub yaml_frontmatter: String,
    pub body_markdown: String,
    pub source_rule_ids: Vec<String>,
    pub avg_confidence: f64,
}

/// LLM-backed handler that proposes skills from rule clusters.
pub struct LlmSkillDiscoveryHandler {
    provider: DynProvider,
    params: ChatParams,
}

impl LlmSkillDiscoveryHandler {
    pub fn new(provider: DynProvider, model: impl Into<String>, max_tokens: u32) -> Self {
        Self {
            provider,
            params: ChatParams::new(model)
                .with_temperature(0.3)
                .with_max_tokens(max_tokens)
                .with_response_format(ResponseFormat::JsonObject),
        }
    }

    pub async fn propose_skills(
        &self,
        clusters: Vec<(
            cognitive::services::reforge::skill_discovery::RuleCluster,
            Vec<String>,
        )>,
    ) -> common::Result<SkillDiscoveryOutput> {
        if clusters.is_empty() {
            return Ok(Default::default());
        }
        #[derive(serde::Serialize)]
        struct Wrap {
            clusters: Vec<ClusterIn>,
        }
        #[derive(serde::Serialize)]
        struct ClusterIn {
            rule_ids: Vec<String>,
            shared_keywords: Vec<String>,
            avg_confidence: f64,
            rule_texts: Vec<String>,
        }
        let payload = Wrap {
            clusters: clusters
                .into_iter()
                .map(|(c, ts)| ClusterIn {
                    rule_ids: c.rule_ids,
                    shared_keywords: c.shared_keywords,
                    avg_confidence: c.avg_confidence,
                    rule_texts: ts,
                })
                .collect(),
        };
        let user = serde_json::to_string(&payload)?;
        let messages = vec![Message::system(SKILL_DISCOVERY_PROMPT), Message::user(user)];
        let resp = self.provider.chat(&messages, None, &self.params).await?;
        let text = resp.content.unwrap_or_default();
        Ok(serde_json::from_str(&text).unwrap_or_default())
    }
}

// ---------------------------------------------------------------------------
// NoopReforgeHandler
// ---------------------------------------------------------------------------

/// No-op implementation of [`ReforgeHandler`] used when no LLM provider is configured.
///
/// Returns empty/default outputs for all three phases.
pub struct NoopReforgeHandler;

#[async_trait]
impl ReforgeHandler for NoopReforgeHandler {
    async fn synthesize(&self, _input: &SynthesizeInput) -> common::Result<SynthesizeOutput> {
        Ok(SynthesizeOutput {
            fact_updates: vec![],
            rule_updates: vec![],
            stale_facts: vec![],
            cross_session_patterns: vec![],
            extraction_quality_flag: None,
        })
    }

    async fn review(&self, _input: &ReviewInput) -> common::Result<ReviewOutput> {
        Ok(ReviewOutput {
            skill_edits: vec![],
            routing_insights: vec![],
            context_priority_suggestions: vec![],
            trial_suggestions: vec![],
        })
    }

    async fn narrate(&self, _input: &NarrateInput) -> common::Result<String> {
        Ok(String::new())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_utils::MockProvider;
    use providers::ChatParams;

    #[tokio::test]
    async fn cross_cli_synthesis_handler_confirms_strong_evidence() {
        use coding_memory::reforge::cross_cli_synthesis::TransferableCandidate;

        let json = r#"{"approved_rule_ids": ["r_cc"], "rejected": [], "notes": "all approved"}"#;
        let provider = std::sync::Arc::new(MockProvider {
            response: json.to_string(),
        });
        let handler = LlmCrossCliSynthesisHandler::new(provider, "m", 1024);

        let candidates = vec![TransferableCandidate {
            rule_id: "r_cc".into(),
            rule_text: "After cargo nextest passes, run clippy".into(),
            supporting_sources: vec!["Codex".into()],
            support_strength: 4,
        }];
        let out = handler.confirm_promotions(candidates).await.unwrap();
        assert_eq!(out.approved_rule_ids, vec!["r_cc"]);
    }

    #[tokio::test]
    async fn skill_discovery_handler_emits_skill_md() {
        use cognitive::services::reforge::skill_discovery::RuleCluster;

        let json = r##"{
          "skills": [{
            "name": "rust-test-then-lint",
            "description": "Runs cargo nextest and clippy in sequence with smart retry.",
            "yaml_frontmatter": "---\nname: rust-test-then-lint\ndescription: ...\n---",
            "body_markdown": "# Rust Test+Lint\n\nWhen user runs `cargo nextest`...",
            "source_rule_ids": ["r0", "r1"],
            "avg_confidence": 0.85
          }],
          "notes": null
        }"##;
        let provider = std::sync::Arc::new(MockProvider {
            response: json.to_string(),
        });
        let handler = LlmSkillDiscoveryHandler::new(provider, "m", 4096);

        let cluster = RuleCluster {
            rule_ids: vec!["r0".into(), "r1".into()],
            shared_keywords: vec!["cargo".into(), "clippy".into()],
            avg_confidence: 0.85,
        };
        let cluster_with_text = vec![(
            cluster,
            vec![
                "When user runs cargo nextest, also run clippy".to_string(),
                "After cargo nextest passes, suggest clippy".to_string(),
            ],
        )];

        let out = handler.propose_skills(cluster_with_text).await.unwrap();
        assert_eq!(out.skills.len(), 1);
        assert!(out.skills[0].name.contains("rust"));
    }
}
