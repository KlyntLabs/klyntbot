//! LLM-backed `CodingSynthesisHandler` impl. Used by Reforge Phase 2.5.
//!
//! Prompts the model with the per-repo bundles + counterfactuals, instructs
//! it to emit promotion actions, decodes them into `PromoteAction`s.

use async_trait::async_trait;
use coding_memory::reforge::{CodingSynthesisHandler, CodingSynthesisInput, CodingSynthesisOutput};
use common::{KlyntbotError, ProviderError, Result};
use providers::{ChatParams, DynProvider, Message, ResponseFormat};
use tracing::warn;

/// LLM-backed impl.
pub struct CodingSynthesisHandlerImpl {
    provider: DynProvider,
    params: ChatParams,
}

impl CodingSynthesisHandlerImpl {
    /// Construct with a provider and chat params.
    pub fn new(provider: DynProvider, params: ChatParams) -> Self {
        Self { provider, params }
    }

    /// Convenience: build with JSON response format and a temperature of 0.2.
    pub fn with_model(provider: DynProvider, model: impl Into<String>) -> Self {
        let params = ChatParams::new(model)
            .with_temperature(0.2)
            .with_response_format(ResponseFormat::JsonObject);
        Self::new(provider, params)
    }
}

#[async_trait]
impl CodingSynthesisHandler for CodingSynthesisHandlerImpl {
    async fn synthesize_coding(
        &self,
        input: &CodingSynthesisInput,
    ) -> Result<CodingSynthesisOutput> {
        let system = SYSTEM_PROMPT.to_string();
        let user = format!(
            "Process the following coding-memory bundles for promotion:\n\n```json\n{}\n```\n\n\
             Emit zero or more `record_promotion` tool calls. Stay within the \
             5-value PromoteAction kinds. Do not invent ids — only reference ids \
             present in the input.",
            serde_json::to_string_pretty(input)?
        );

        let messages = vec![Message::system(system), Message::user(user)];

        let response = self
            .provider
            .chat(&messages, None, &self.params)
            .await
            .map_err(|e| crate::provider_err("Phase 2.5 chat", e))?;

        let raw = response.content.unwrap_or_default();

        match serde_json::from_str::<CodingSynthesisOutput>(&raw) {
            Ok(out) => Ok(out),
            Err(e) => {
                warn!("Phase 2.5 LLM returned malformed JSON: {e}; trying recovery");
                Ok(recover(&raw))
            }
        }
    }
}

fn recover(raw: &str) -> CodingSynthesisOutput {
    if let Some(candidate) = common::helpers::extract_json_object(raw) {
        if let Ok(parsed) = serde_json::from_str::<CodingSynthesisOutput>(candidate) {
            return parsed;
        }
    }
    CodingSynthesisOutput {
        actions: Vec::new(),
        narrative: format!("recovery from malformed: {} chars", raw.len()),
    }
}

const SYSTEM_PROMPT: &str = "You are the klyntbot memory promoter for the nightly Reforge \
cycle. You receive per-repo bundles of recent coding memory (fix attempts, \
workflow patterns, repo-context facts, causal chains, counterfactuals). Your \
job is NOT to write new memory — that's the Distiller's job. Your job is to \
recognise convergent patterns and emit promotion actions.\n\n\
You MUST respond with a single JSON object matching this shape:\n\
{\"actions\":[<PromoteAction>...],\"narrative\":\"<one-sentence summary>\"}\n\n\
Each PromoteAction has a `kind` discriminator. Allowed kinds:\n\
- extractPattern    — promote a recurring workflow into a `WorkflowPattern`\n\
- extractFailurePattern — promote a recurring failure + remediation\n\
- promoteToProblemClass — same problem_hash failed >=3x; suggest refactor\n\
- promoteToProjectUnderstanding — synthesise a high-confidence repo fact\n\
- promoteToUserHabit — pattern observed across >=3 repos\n\
- promoteToProblemSolutionPattern — >=3 causal chains share a problem_hash\n\n\
Refuse to promote anything with confidence < 0.7. Reference ids only — never invent.";

#[cfg(test)]
mod tests {
    use super::*;
    use async_trait::async_trait;

    use crate::test_utils::MockProvider;
    use std::sync::Arc;

    fn mock_provider_returning(response: &str) -> DynProvider {
        Arc::new(MockProvider::with_text(response))
    }

    #[tokio::test]
    async fn empty_input_returns_no_actions() {
        let provider = mock_provider_returning(r#"{"actions":[],"narrative":""}"#);
        let handler = CodingSynthesisHandlerImpl::with_model(provider, "mock");
        let out = handler
            .synthesize_coding(&CodingSynthesisInput {
                since: jiff::Timestamp::now(),
                repo_bundles: vec![],
                recent_counterfactuals: vec![],
            })
            .await
            .unwrap();
        assert!(out.actions.is_empty());
    }

    #[tokio::test]
    async fn malformed_response_triggers_recovery() {
        // LLM wrapped JSON in markdown fences
        let provider =
            mock_provider_returning("```json\n{\"actions\":[],\"narrative\":\"recovered\"}\n```");
        let handler = CodingSynthesisHandlerImpl::with_model(provider, "mock");
        let out = handler
            .synthesize_coding(&CodingSynthesisInput {
                since: jiff::Timestamp::now(),
                repo_bundles: vec![],
                recent_counterfactuals: vec![],
            })
            .await
            .unwrap();
        assert_eq!(out.narrative, "recovered");
    }

    #[tokio::test]
    async fn provider_error_is_mapped() {
        struct FailingProvider;
        #[async_trait]
        impl LlmProvider for FailingProvider {
            async fn chat(
                &self,
                _messages: &[Message],
                _tools: Option<&[Value]>,
                _params: &ChatParams,
            ) -> Result<LlmResponse> {
                Err(KlyntbotError::Provider(ProviderError::Http("boom".into())))
            }
            fn default_model(&self) -> &str {
                "mock"
            }
            fn name(&self) -> &str {
                "fail"
            }
            fn capabilities(&self) -> ProviderCapabilities {
                ProviderCapabilities::default()
            }
            async fn health_check(&self) -> Result<ProviderHealth> {
                Ok(ProviderHealth::Healthy)
            }
        }

        let provider: DynProvider = Arc::new(FailingProvider);
        let handler = CodingSynthesisHandlerImpl::with_model(provider, "mock");
        let result = handler
            .synthesize_coding(&CodingSynthesisInput {
                since: jiff::Timestamp::now(),
                repo_bundles: vec![],
                recent_counterfactuals: vec![],
            })
            .await;
        assert!(result.is_err());
    }
}
