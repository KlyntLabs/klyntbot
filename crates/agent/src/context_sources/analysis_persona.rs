//! Injects DB-based persona perspectives when the user asks for analysis.
//!
//! Detects analysis keywords in the current message and fetches personas from
//! `PersonaRepo` to provide structured perspective context in the system prompt.

use async_trait::async_trait;
use cognitive::repos::PersonaRepo;
use context_engine::source::{ContextSource, SourceContext};

/// Analysis keywords that trigger persona injection.
const ANALYSIS_KEYWORDS: &[&str] = &[
    "analyze",
    "analysis",
    "trade-off",
    "tradeoff",
    "compare",
    "advice",
    "perspective",
    "opinions",
    "weigh",
    "pros and cons",
    "evaluate",
    "assess",
    "review",
    "critique",
    "should i",
    "what do you think",
];

pub struct AnalysisPersonaContextSource {
    persona_repo: PersonaRepo,
}

impl AnalysisPersonaContextSource {
    pub fn new(persona_repo: PersonaRepo) -> Self {
        Self { persona_repo }
    }

    fn is_analysis_query(message: &str) -> bool {
        let lower = message.to_lowercase();
        ANALYSIS_KEYWORDS.iter().any(|kw| lower.contains(kw))
    }
}

#[async_trait]
impl ContextSource for AnalysisPersonaContextSource {
    fn name(&self) -> &str {
        "analysis_personas"
    }

    fn priority(&self) -> u8 {
        94
    }

    async fn provide(&self, ctx: &SourceContext) -> Option<String> {
        let message = ctx.message.as_deref()?;
        if !Self::is_analysis_query(message) {
            return None;
        }

        let personas = self.persona_repo.list_active().await.ok()?;
        if personas.is_empty() {
            return None;
        }

        let selected: Vec<_> = personas.into_iter().take(4).collect();

        let mut output = String::from("# Analysis Perspectives\n\n");
        output.push_str(
            "The user is asking for analysis. Consider these expert perspectives:\n\n",
        );

        for p in &selected {
            output.push_str(&format!(
                "**{} ({}):** {}\n",
                p.name, p.role, p.perspective
            ));
        }

        output.push_str(
            "\nFor each perspective: 2-3 sentences of focused analysis, then synthesize a recommendation.\n",
        );

        Some(output)
    }

    fn estimated_tokens(&self) -> usize {
        300
    }
}
