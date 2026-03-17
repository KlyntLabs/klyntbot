use std::sync::Arc;

use async_trait::async_trait;

use super::decomposer::QueryDecomposer;

/// Trait for LLM providers used by the decomposer.
/// Lives here (L3) to avoid depending on the `providers` crate directly.
#[async_trait]
pub trait DecomposerLlm: Send + Sync {
    /// Single-turn chat returning the response text.
    async fn generate(&self, prompt: &str) -> Result<String, String>;
}

/// LLM-backed query decomposer.
///
/// Asks a cheap model to break a query into sub-queries.
/// The caller wraps this in a timeout via `InsightForge::retrieve`.
pub struct LlmDecomposer {
    llm: Arc<dyn DecomposerLlm>,
}

impl LlmDecomposer {
    pub fn new(llm: Arc<dyn DecomposerLlm>) -> Self {
        Self { llm }
    }
}

#[async_trait]
impl QueryDecomposer for LlmDecomposer {
    async fn decompose(&self, query: &str, _context_hint: Option<&str>) -> Vec<String> {
        let prompt = format!(
            r#"Break the following user message into 3-5 distinct search queries that would help find all relevant information. Each query should focus on a different aspect (facts, relationships, timeline, context, risks).

User message: {query}

Respond with ONLY a JSON array of strings, no explanation:
["query 1", "query 2", "query 3"]"#
        );

        match self.llm.generate(&prompt).await {
            Ok(response) => {
                let trimmed = response.trim();
                if let Ok(queries) = serde_json::from_str::<Vec<String>>(trimmed) {
                    let mut result = vec![query.to_string()];
                    result.extend(queries);
                    result.truncate(5);
                    result
                } else {
                    vec![query.to_string()]
                }
            }
            Err(_) => vec![query.to_string()],
        }
    }
}
