//! Multi-query stage — LLM generates 3 query variants for fan-out retrieval.
//!
//! Only active when `budget.max_llm_calls > 0` (Deep+ modes). On Normal mode
//! or when no provider is configured, the bundle passes through unchanged.
//! On LLM error or timeout the stage degrades gracefully and returns the input.

use std::sync::Arc;
use std::time::Duration;

use async_trait::async_trait;
use context_engine::enhancement::{EnhancementBudget, QueryBundle, QuerySource, QueryStage};
use context_engine::rewriter::RetrievalContext;
use tracing::debug;

// ---------------------------------------------------------------------------
// MultiQueryStage
// ---------------------------------------------------------------------------

/// Generates alternative query variants via an LLM call and appends them to
/// the bundle's `variants` list, enabling fan-out retrieval.
pub struct MultiQueryStage {
    provider: Option<providers::DynProvider>,
    model: Option<String>,
    max_variants: usize,
    champion_overrides: Option<Arc<std::sync::RwLock<Option<common::TrialParams>>>>,
}

impl MultiQueryStage {
    pub fn new(
        provider: Option<providers::DynProvider>,
        model: Option<String>,
        max_variants: usize,
    ) -> Self {
        Self {
            provider,
            model,
            max_variants,
            champion_overrides: None,
        }
    }

    /// Attach an autotuner champion-override sink. When present,
    /// `TrialParams.multi_query_max_variants` overrides the configured
    /// variant count per call.
    pub fn with_champion_overrides(
        mut self,
        sink: Arc<std::sync::RwLock<Option<common::TrialParams>>>,
    ) -> Self {
        self.champion_overrides = Some(sink);
        self
    }

    fn resolved_max_variants(&self) -> usize {
        if let Some(sink) = &self.champion_overrides {
            if let Ok(guard) = sink.read() {
                if let Some(params) = guard.as_ref() {
                    if let Some(v) = params.multi_query_max_variants {
                        return v;
                    }
                }
            }
        }
        self.max_variants
    }
}

// ---------------------------------------------------------------------------
// Prompt builder
// ---------------------------------------------------------------------------

fn build_prompt(bundle: &QueryBundle, context: &RetrievalContext, n: usize) -> String {
    let mut prompt = format!(
        "Generate {n} different search queries to find information relevant to: \"{primary}\"\n\
         Each query should approach from a different angle.\n\
         Return one query per line, nothing else.",
        primary = bundle.primary,
    );

    if let Some(ref skill) = context.active_skill {
        if skill != "general" {
            prompt.push_str(&format!("\nContext: user is in the {skill} domain."));
        }
    }

    if let Some(ref task) = context.active_task {
        prompt.push_str(&format!("\nCurrent task: {}", task.title));
    }

    prompt
}

// ---------------------------------------------------------------------------
// QueryStage impl
// ---------------------------------------------------------------------------

#[async_trait]
impl QueryStage for MultiQueryStage {
    fn name(&self) -> QuerySource {
        QuerySource::MultiQuery
    }

    async fn transform(
        &self,
        input: QueryBundle,
        context: &RetrievalContext,
        budget: &EnhancementBudget,
    ) -> common::Result<QueryBundle> {
        // Skip in Normal mode (no LLM calls allowed).
        if budget.max_llm_calls == 0 {
            debug!("MultiQueryStage: skipping — budget.max_llm_calls == 0");
            return Ok(input);
        }

        let provider = match &self.provider {
            Some(p) => p,
            None => {
                debug!("MultiQueryStage: skipping — no provider configured");
                return Ok(input);
            }
        };

        let max_variants = self.resolved_max_variants();
        if max_variants == 0 {
            debug!("MultiQueryStage: skipping — resolved max_variants == 0");
            return Ok(input);
        }

        let prompt = build_prompt(&input, context, max_variants);
        let messages = vec![providers::Message::user(prompt)];
        let model_name = self
            .model
            .clone()
            .unwrap_or_else(|| provider.default_model().to_string());
        // 20 tokens/line is a generous ceiling for short search queries.
        let max_tokens = ((max_variants * 20) as u32).max(40);
        let params = providers::ChatParams::new(model_name)
            .with_max_tokens(max_tokens)
            .with_temperature(0.7);

        let timeout_ms = budget.max_latency_ms.min(500);
        let timeout_dur = Duration::from_millis(timeout_ms);

        let result =
            tokio::time::timeout(timeout_dur, provider.chat(&messages, None, &params, &[])).await;

        match result {
            Ok(Ok(response)) => {
                let text = response.content.as_deref().unwrap_or("").trim().to_string();

                let variants: Vec<String> = text
                    .lines()
                    .map(|line| {
                        // Strip leading bullets, numbers, dashes (e.g. "1.", "-", "*", "•")
                        let trimmed = line.trim();
                        let stripped = trimmed
                            .trim_start_matches(|c: char| {
                                c == '-' || c == '*' || c == '•' || c == '·'
                            })
                            .trim_start_matches(|c: char| c.is_ascii_digit())
                            .trim_start_matches('.')
                            .trim_start_matches(')')
                            .trim();
                        stripped.to_string()
                    })
                    .filter(|s| s.len() >= 5 && s.len() <= 200)
                    .take(max_variants)
                    .collect();

                if variants.is_empty() {
                    debug!(
                        primary = input.primary.as_str(),
                        "MultiQueryStage: LLM returned no usable variants"
                    );
                    return Ok(input);
                }

                debug!(
                    primary = input.primary.as_str(),
                    count = variants.len(),
                    "MultiQueryStage: generated query variants"
                );

                let mut bundle = input;
                bundle.variants.extend(variants);
                if !bundle.sources.contains(&QuerySource::MultiQuery) {
                    bundle.sources.push(QuerySource::MultiQuery);
                }
                Ok(bundle)
            }
            Ok(Err(e)) => {
                debug!(
                    primary = input.primary.as_str(),
                    error = %e,
                    "MultiQueryStage: LLM call failed — returning input unchanged"
                );
                Ok(input)
            }
            Err(_) => {
                debug!(
                    primary = input.primary.as_str(),
                    timeout_ms, "MultiQueryStage: LLM timed out — returning input unchanged"
                );
                Ok(input)
            }
        }
    }
}
