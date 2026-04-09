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

Respond with JSON:
{\"skill_edits\":[{\"skill_name\":\"...\",\"file_path\":\"...\",\"edit_type\":\"frontmatter\"|\"body_replace\"|\"body_insert\"|\"body_remove\",\"field\":null,\"new_value\":null,\"old_text\":null,\"new_text\":null,\"section\":null,\"reason\":\"...\"}],\"routing_insights\":[\"...\"],\"context_priority_suggestions\":[{\"source\":\"...\",\"suggestion\":\"...\",\"reason\":\"...\"}],\"trial_suggestions\":[{\"hypothesis\":\"...\",\"pace\":\"conservative\"|\"balanced\"|\"bold\",\"param_overrides\":{\"param_name\":0.0}}]}";

const NARRATE_PROMPT: &str = "\
Summarize tonight's Reforge cycle for the user in 2-3 concise paragraphs.
Include: what was learned, what changed (facts, rules, skills), and any notable patterns.
Be conversational, not clinical. Address the user directly.";

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
