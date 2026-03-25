//! LLM-backed NarrativeHandler implementation for the Mirror self-reflection layer.
//!
//! Dependency inversion: the `NarrativeHandler` trait is defined in `cognitive`
//! (L3-L4) but implemented here in `agent` (L5) where LLM providers are available.

use async_trait::async_trait;

use cognitive::mirror::{GeneratedNarrative, NarrativeContext, NarrativeHandler, UserFeedback};
use common::Result;
use providers::{DynProvider, Message};

// ---------------------------------------------------------------------------
// System prompts
// ---------------------------------------------------------------------------

const NARRATIVE_SYSTEM_PROMPT: &str = "\
You are writing a weekly reflection for a personal AI assistant's \"Mirror\" — a self-awareness journal.\n\n\
Write in first person (\"I noticed...\", \"I improved...\"). Be warm, specific, honest.\n\
No jargon — translate metrics into human terms.\n\n\
Structure:\n\
1. Opening: One sentence capturing the week's vibe\n\
2. Routing: How message understanding changed\n\
3. Looking ahead: One concrete thing to watch next week\n\n\
Respond with JSON in this exact format:\n\
{\"full_narrative\": \"...\", \"routing_summary\": \"...\", \"improvement_highlights\": [\"...\"]}";

const MIRROR_RESPONSE_SYSTEM_PROMPT: &str = "\
You are the Mirror — the self-aware part of a personal AI assistant. \
Answer in first person. Be warm, honest, specific. Keep concise (2-4 paragraphs).";

// ---------------------------------------------------------------------------
// LlmNarrativeHandler
// ---------------------------------------------------------------------------

/// LLM-backed implementation of [`NarrativeHandler`].
///
/// Uses the cognitive LLM provider to generate weekly trend narratives and
/// answer freeform mirror queries.
pub struct LlmNarrativeHandler {
    provider: DynProvider,
    model: String,
}

impl LlmNarrativeHandler {
    pub fn new(provider: DynProvider, model: String) -> Self {
        Self { provider, model }
    }
}

#[async_trait]
impl NarrativeHandler for LlmNarrativeHandler {
    async fn generate_narrative(&self, ctx: NarrativeContext) -> Result<GeneratedNarrative> {
        let user_msg = format_narrative_context(&ctx);
        let system_prompt = build_narrative_system_prompt(&ctx);

        let messages = vec![
            Message::system(system_prompt),
            Message::user(user_msg),
        ];

        let params = providers::ChatParams::new(&self.model)
            .with_response_format(providers::ResponseFormat::JsonObject);

        let response = self.provider.chat(&messages, None, &params).await?;
        let content = response.content.unwrap_or_default();

        serde_json::from_str::<GeneratedNarrative>(&content).map_err(|e| {
            common::KlyntbotError::Provider(common::ProviderError::InvalidResponse(format!(
                "Mirror narrative JSON parse error: {e}"
            )))
        })
    }

    async fn generate_mirror_response(&self, query: &str, ctx: NarrativeContext) -> Result<String> {
        let context_summary = format_narrative_context(&ctx);
        let user_msg = format!("## Context\n{context_summary}\n\n## User Question\n{query}");

        let messages = vec![
            Message::system(MIRROR_RESPONSE_SYSTEM_PROMPT),
            Message::user(user_msg),
        ];

        let params = providers::ChatParams::new(&self.model);

        let response = self.provider.chat(&messages, None, &params).await?;
        Ok(response.content.unwrap_or_default())
    }
}

// ---------------------------------------------------------------------------
// Context formatting
// ---------------------------------------------------------------------------

fn build_narrative_system_prompt(ctx: &NarrativeContext) -> String {
    let mut prompt = NARRATIVE_SYSTEM_PROMPT.to_string();

    if !ctx.past_narrative_feedback.is_empty() {
        let not_helpful_count = ctx
            .past_narrative_feedback
            .iter()
            .filter(|f| matches!(f, UserFeedback::NotHelpful))
            .count();
        let helpful_count = ctx
            .past_narrative_feedback
            .iter()
            .filter(|f| matches!(f, UserFeedback::Helpful))
            .count();

        let tone_instruction = if not_helpful_count > helpful_count {
            "\n\nTone guidance (based on past feedback): Be more concise and direct. \
Reduce flourishes. Focus on actionable insights."
        } else if helpful_count > 0 && not_helpful_count == 0 {
            "\n\nTone guidance (based on past feedback): Maintain your current warm, \
detailed style — the user appreciates it."
        } else if helpful_count > 0 {
            "\n\nTone guidance (based on past feedback): Balance conciseness with warmth. \
Lead with the most important insight."
        } else {
            ""
        };

        prompt.push_str(tone_instruction);
    }

    prompt
}

fn format_narrative_context(ctx: &NarrativeContext) -> String {
    let (start, end) = ctx.period;
    let mut out = format!("Period: {} to {}\n\n", start, end);

    // Top skills
    if !ctx.top_skills_by_usage.is_empty() {
        out.push_str("## Top Skills by Usage\n");
        for (skill, pct) in &ctx.top_skills_by_usage {
            out.push_str(&format!("- {skill}: {pct:.1}%\n"));
        }
        out.push('\n');
    }

    // Routing snapshot summary
    out.push_str(&format!(
        "## Routing Snapshots: {}\n",
        ctx.routing_snapshots.len()
    ));
    for snap in &ctx.routing_snapshots {
        out.push_str(&format!(
            "- {} | {} messages | fallback={:.1}% | avg_confidence={:.2}\n",
            snap.captured_at.format("%Y-%m-%d %H:%M"),
            snap.total_messages,
            snap.fallback_rate * 100.0,
            snap.avg_routing_confidence,
        ));
    }

    // Corrections
    if ctx.correction_count > 0 {
        out.push_str(&format!(
            "\nUser corrections this period: {}\n",
            ctx.correction_count
        ));
    }

    // Past feedback
    if !ctx.past_narrative_feedback.is_empty() {
        out.push_str(&format!(
            "\nPast narrative feedback entries: {}\n",
            ctx.past_narrative_feedback.len()
        ));
    }

    out
}
