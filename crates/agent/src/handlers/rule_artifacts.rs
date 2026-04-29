//! LLM-backed `RuleArtifactsHandler` impl.

use async_trait::async_trait;
use coding_memory::reforge::{RuleArtifactInput, RuleArtifactOutput, RuleArtifactsHandler};
use coding_memory::reforge_phase::RuleArtifact;
use common::Result;
use providers::{ChatParams, DynProvider, Message, ResponseFormat};
use tracing::warn;

/// LLM-backed impl.
pub struct RuleArtifactsHandlerImpl {
    provider: DynProvider,
    params: ChatParams,
}

impl RuleArtifactsHandlerImpl {
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
impl RuleArtifactsHandler for RuleArtifactsHandlerImpl {
    async fn synthesize_artifact(&self, input: &RuleArtifactInput) -> Result<RuleArtifactOutput> {
        let system = artifact_system_prompt(&input.artifact);
        let user = format!(
            "Generate the {} artifact for repo `{}` ({} facts, {} rules).\n\n\
             ```json\n{}\n```",
            artifact_name(&input.artifact),
            input.plan.repo_id,
            input.plan.facts.len(),
            input.plan.rules.len(),
            serde_json::to_string_pretty(input)?
        );

        let messages = vec![Message::system(system), Message::user(user)];

        let response = self
            .provider
            .chat(&messages, None, &self.params)
            .await
            .map_err(|e| crate::provider_err("Phase 3.5 chat", e))?;

        let raw = response.content.unwrap_or_default();

        match serde_json::from_str::<RuleArtifactOutput>(&raw) {
            Ok(out) => Ok(out),
            Err(e) => {
                warn!("Phase 3.5 LLM returned malformed JSON: {e}; trying recovery");
                Ok(recover_artifact(&raw, &input.artifact))
            }
        }
    }
}

fn artifact_name(artifact: &RuleArtifact) -> &'static str {
    match artifact {
        RuleArtifact::ClaudeMd => "CLAUDE.md",
        RuleArtifact::AgentsMd => "AGENTS.md",
        RuleArtifact::CursorRules => ".cursorrules",
        RuleArtifact::ContinueRules => ".continue/rules/klyntbot.md",
    }
}

fn artifact_system_prompt(artifact: &RuleArtifact) -> String {
    let base = "You are the klyntbot rule-artifact writer. You receive a repo's \
consolidated coding memory (facts + procedural rules) and produce a concise, \
actionable instruction file for AI assistants.\n\n\
You MUST respond with a single JSON object:\n\
{\"body\":\"<markdown body>\",\"sectionLabels\":[\"<section>\",...]}\n\n\
Rules:\n\
- Only include facts with sensitivity != high/excluded.\n\
- Group related rules under section headers (## Section).\n\
- Use bullet lists, not paragraphs.\n\
- Never emit raw memory ids.\n\
- Keep total body under 400 lines.";

    match artifact {
        RuleArtifact::ClaudeMd => format!(
            "{base}\n\n\
            This artifact is CLAUDE.md — instructions for Claude Code / Claude Desktop. \
            Emphasise Rust-specific patterns, test conventions, and project structure."
        ),
        RuleArtifact::AgentsMd => format!(
            "{base}\n\n\
            This artifact is AGENTS.md — instructions for generic AI agents. \
            Emphasise build steps, test commands, and coding conventions."
        ),
        RuleArtifact::CursorRules => format!(
            "{base}\n\n\
            This artifact is .cursorrules — instructions for Cursor IDE. \
            Emphasise lint rules, formatting preferences, and file organisation."
        ),
        RuleArtifact::ContinueRules => format!(
            "{base}\n\n\
            This artifact is .continue/rules/klyntbot.md — instructions for Continue.dev. \
            Emphasise prompt patterns, context-window usage, and tool conventions."
        ),
    }
}

fn recover_artifact(raw: &str, artifact: &RuleArtifact) -> RuleArtifactOutput {
    if let Some(candidate) = common::helpers::extract_json_object(raw) {
        if let Ok(parsed) = serde_json::from_str::<RuleArtifactOutput>(candidate) {
            return parsed;
        }
    }
    RuleArtifactOutput {
        body: format!(
            "<!-- klyntbot:managed:start -->\n\
             ## Auto-generated {}\n\
             (recovery from malformed response: {} chars)\n\
             <!-- klyntbot:managed:end -->",
            artifact_name(artifact),
            raw.len()
        ),
        section_labels: vec!["Auto-generated".to_string()],
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use async_trait::async_trait;
    use coding_memory::reforge::types::RepoArtifactPlan;
    use providers::{LlmProvider, LlmResponse, ProviderCapabilities, ProviderHealth, Usage};
    use serde_json::Value;
    use std::sync::Arc;

    struct MockProvider {
        response: String,
    }

    #[async_trait]
    impl LlmProvider for MockProvider {
        async fn chat(
            &self,
            _messages: &[Message],
            _tools: Option<&[Value]>,
            _params: &ChatParams,
        ) -> Result<LlmResponse> {
            Ok(LlmResponse {
                content: Some(self.response.clone()),
                tool_calls: vec![],
                finish_reason: "stop".to_string(),
                usage: Usage::default(),
                reasoning_content: None,
            })
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
        async fn health_check(&self) -> Result<ProviderHealth> {
            Ok(ProviderHealth::Healthy)
        }
    }

    fn mock_provider_returning(response: &str) -> DynProvider {
        Arc::new(MockProvider {
            response: response.to_string(),
        })
    }

    #[tokio::test]
    async fn returns_body_with_section_labels() {
        let provider = mock_provider_returning(
            "{\"body\":\"## Architecture\\n- rust\\n\",\"sectionLabels\":[\"Architecture\"]}",
        );
        let handler = RuleArtifactsHandlerImpl::with_model(provider, "mock");
        let out = handler
            .synthesize_artifact(&RuleArtifactInput {
                plan: RepoArtifactPlan {
                    repo_id: "r1".into(),
                    root: std::path::PathBuf::from("/tmp/r1"),
                    enabled: vec![RuleArtifact::ClaudeMd],
                    facts: vec![],
                    rules: vec![],
                },
                artifact: RuleArtifact::ClaudeMd,
            })
            .await
            .unwrap();
        assert!(out.body.contains("Architecture"));
        assert_eq!(out.section_labels, vec!["Architecture".to_string()]);
    }

    #[tokio::test]
    async fn malformed_response_triggers_recovery() {
        let provider = mock_provider_returning(
            "```json\n{\"body\":\"## Recovered\\n\",\"sectionLabels\":[]}\n```",
        );
        let handler = RuleArtifactsHandlerImpl::with_model(provider, "mock");
        let out = handler
            .synthesize_artifact(&RuleArtifactInput {
                plan: RepoArtifactPlan {
                    repo_id: "r1".into(),
                    root: std::path::PathBuf::from("/tmp/r1"),
                    enabled: vec![RuleArtifact::ClaudeMd],
                    facts: vec![],
                    rules: vec![],
                },
                artifact: RuleArtifact::ClaudeMd,
            })
            .await
            .unwrap();
        assert!(out.body.contains("Recovered"));
    }
}
