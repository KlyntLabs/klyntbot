//! EnrichmentEngine — orchestrator that implements EnrichmentHandler.

use async_trait::async_trait;
use common::Result;
use config::TodoEnrichmentConfig;
use feature_tasks::{EnrichmentHandler, EnrichmentResult, EnrichmentSuggestion, Task};
use providers::{ChatParams, DynProvider, Message};
use serde::Deserialize;
use tracing::warn;

/// JSON structure returned by the LLM for task enrichment.
#[derive(Deserialize)]
struct LlmEnrichmentResponse {
    priority: Option<i16>,
    priority_confidence: Option<f64>,
    priority_reasoning: Option<String>,
    due_date: Option<String>,
    due_date_confidence: Option<f64>,
    due_date_reasoning: Option<String>,
}

/// Central enrichment engine that coordinates priority, duration, and scheduling modules.
pub struct EnrichmentEngine {
    config: TodoEnrichmentConfig,
    provider: Option<DynProvider>,
    model: Option<String>,
}

impl EnrichmentEngine {
    pub fn new(config: TodoEnrichmentConfig) -> Self {
        Self {
            config,
            provider: None,
            model: None,
        }
    }

    /// Attach an LLM provider for use when `config.use_llm` is true.
    pub fn with_provider(mut self, provider: DynProvider, model: String) -> Self {
        self.provider = Some(provider);
        self.model = Some(model);
        self
    }

    /// Try to enrich a task using the LLM. Returns `None` on any error so callers
    /// can fall back to keyword enrichment.
    async fn enrich_with_llm(&self, task: &Task) -> Option<EnrichmentResult> {
        let provider = self.provider.as_ref()?;
        let model = self.model.as_deref().unwrap_or("gpt-4o-mini");

        // Build a compact task context for the prompt
        let mut parts = vec![format!("Title: {}", task.title)];
        if let Some(ref desc) = task.description {
            parts.push(format!("Description: {}", desc));
        }
        if !task.tags.is_empty() {
            parts.push(format!("Tags: {}", task.tags.join(", ")));
        }
        let task_context = parts.join("\n");

        let system_prompt = r#"You are a task enrichment assistant. Analyze the given task and return enrichment suggestions as a JSON object.

Return exactly this JSON structure (use null for uncertain fields):
{
  "priority": 1-4 or null,
  "priority_confidence": 0.0-1.0,
  "priority_reasoning": "brief explanation",
  "estimated_minutes": integer or null,
  "duration_confidence": 0.0-1.0,
  "duration_reasoning": "brief explanation",
  "due_date": "ISO 8601 datetime" or null,
  "due_date_confidence": 0.0-1.0,
  "due_date_reasoning": "brief explanation"
}

Priority scale: 1=urgent/critical, 2=important/bug, 3=feature/improvement, 4=nice-to-have/chore
Duration guide: 15=typo/tweak, 30=small fix/patch, 60=feature/implement, 120=refactor/rewrite

Return only valid JSON, no markdown fences."#;

        let messages = [Message::system(system_prompt), Message::user(task_context)];

        let params = ChatParams::new(model)
            .with_temperature(0.0)
            .with_max_tokens(512);

        match provider.chat(&messages, None, &params).await {
            Ok(response) => match response.content {
                Some(content) => parse_llm_enrichment_response(&content, task),
                None => {
                    warn!("LLM enrichment returned empty content, falling back to keywords");
                    None
                }
            },
            Err(e) => {
                warn!("LLM enrichment failed: {}, falling back to keywords", e);
                None
            }
        }
    }
}

/// Parse the LLM JSON response into an `EnrichmentResult`.
/// Only fills fields that are not already set on the task.
fn parse_llm_enrichment_response(content: &str, task: &Task) -> Option<EnrichmentResult> {
    let json_str = common::helpers::strip_llm_fences(content);
    let parsed: LlmEnrichmentResponse = match serde_json::from_str(json_str) {
        Ok(r) => r,
        Err(e) => {
            warn!("Failed to parse LLM enrichment JSON: {}", e);
            return None;
        }
    };

    let mut result = EnrichmentResult::default();

    if task.priority.is_none() {
        if let Some(priority) = parsed.priority {
            if (1..=4).contains(&priority) {
                result.priority = Some(EnrichmentSuggestion {
                    value: priority,
                    confidence: parsed.priority_confidence.unwrap_or(0.7),
                    reasoning: parsed.priority_reasoning.unwrap_or_default(),
                });
            }
        }
    }

    if task.due_date.is_none() {
        if let Some(ref date_str) = parsed.due_date {
            if chrono::DateTime::parse_from_rfc3339(date_str).is_ok() {
                result.due_date = Some(EnrichmentSuggestion {
                    value: date_str.clone(),
                    confidence: parsed.due_date_confidence.unwrap_or(0.6),
                    reasoning: parsed.due_date_reasoning.unwrap_or_default(),
                });
            }
        }
    }

    Some(result)
}

#[async_trait]
impl EnrichmentHandler for EnrichmentEngine {
    async fn enrich(&self, task: &Task) -> Result<Option<EnrichmentResult>> {
        if !self.config.enabled {
            return Ok(None);
        }

        // Try LLM enrichment first if opt-in flag is set
        if self.config.use_llm {
            if let Some(llm_result) = self.enrich_with_llm(task).await {
                return Ok(if llm_result.is_empty() {
                    None
                } else {
                    Some(llm_result)
                });
            }
            // LLM unavailable or failed — fall through to keyword enrichment
        }

        let mut result = EnrichmentResult::default();

        // Infer priority (only if not already set)
        if task.priority.is_none() {
            result.priority = super::priority::infer_priority(task);
        }

        // Suggest due date (only if not already set)
        if task.due_date.is_none() {
            if let Some(due) = super::scheduling::suggest_due_date(task) {
                result.due_date = Some(EnrichmentSuggestion {
                    value: due.value.to_rfc3339(),
                    confidence: due.confidence,
                    reasoning: due.reasoning,
                });
            }
        }

        if result.is_empty() {
            Ok(None)
        } else {
            Ok(Some(result))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use feature_tasks::Task;
    use serde_json::Value;
    use std::sync::Arc;

    // ── Minimal mock provider for unit tests ────────────────────────────────

    struct MockProvider {
        response: String,
    }

    #[async_trait]
    impl providers::LlmProvider for MockProvider {
        async fn chat(
            &self,
            _messages: &[Message],
            _tools: Option<&[Value]>,
            _params: &ChatParams,
        ) -> Result<providers::LlmResponse> {
            Ok(providers::LlmResponse {
                content: Some(self.response.clone()),
                tool_calls: vec![],
                finish_reason: "stop".to_string(),
                usage: providers::Usage::default(),
                reasoning_content: None,
            })
        }

        fn default_model(&self) -> &str {
            "mock"
        }

        fn name(&self) -> &str {
            "mock"
        }
    }

    struct ErrorProvider;

    #[async_trait]
    impl providers::LlmProvider for ErrorProvider {
        async fn chat(
            &self,
            _messages: &[Message],
            _tools: Option<&[Value]>,
            _params: &ChatParams,
        ) -> Result<providers::LlmResponse> {
            Err(common::KlyntbotError::Provider(
                common::ProviderError::InvalidResponse("mock LLM error".to_string()),
            ))
        }

        fn default_model(&self) -> &str {
            "error"
        }

        fn name(&self) -> &str {
            "error"
        }
    }

    // ── Existing tests (unchanged) ──────────────────────────────────────────

    #[tokio::test]
    async fn test_enrichment_engine_disabled() {
        let config = TodoEnrichmentConfig {
            enabled: false,
            ..Default::default()
        };
        let engine = EnrichmentEngine::new(config);
        let task = Task::default_instance();

        let result = engine.enrich(&task).await.unwrap();
        assert!(result.is_none());
    }

    #[tokio::test]
    async fn test_enrichment_engine_skips_existing_fields() {
        let config = TodoEnrichmentConfig::default();
        let engine = EnrichmentEngine::new(config);

        let mut task = Task::default_instance();
        task.priority = Some(1);
        // due_date is still None — should still suggest

        let result = engine.enrich(&task).await.unwrap();
        // Priority should be skipped (already set)
        if let Some(ref enrichment) = result {
            assert!(enrichment.priority.is_none());
        }
    }

    #[tokio::test]
    async fn test_enrichment_engine_enriches_blank_task() {
        let config = TodoEnrichmentConfig::default();
        let engine = EnrichmentEngine::new(config);

        let mut task = Task::default_instance();
        task.title = "Fix urgent production bug in auth".to_string();

        let result = engine.enrich(&task).await.unwrap();
        assert!(result.is_some());

        let enrichment = result.unwrap();
        // Priority should be inferred from "urgent" keyword
        assert!(enrichment.priority.is_some());
    }

    // ========================================================================
    // Additional integration tests (added by QA)
    // ========================================================================

    #[tokio::test]
    async fn test_enrichment_returns_none_when_all_fields_set() {
        let config = TodoEnrichmentConfig::default();
        let engine = EnrichmentEngine::new(config);

        let mut task = Task::default_instance();
        task.title = "URGENT: Fix bug".to_string();
        task.priority = Some(1);
        task.due_date = Some(chrono::Utc::now() + chrono::Duration::days(1));

        let result = engine.enrich(&task).await.unwrap();
        // All fields already set, should return None
        assert!(
            result.is_none(),
            "Should return None when all fields are already set"
        );
    }

    #[tokio::test]
    async fn test_enrichment_partial_fields() {
        let config = TodoEnrichmentConfig::default();
        let engine = EnrichmentEngine::new(config);

        let mut task = Task::default_instance();
        task.title = "Fix typo in docs".to_string();
        task.priority = Some(4); // Already set
                                 // due_date is None

        let result = engine.enrich(&task).await.unwrap();
        // Priority already set, should skip; due_date may or may not be suggested
        if let Some(ref enrichment) = result {
            assert!(
                enrichment.priority.is_none(),
                "Priority already set, should skip"
            );
        }
    }

    #[tokio::test]
    async fn test_enrichment_with_unicode_text() {
        let config = TodoEnrichmentConfig::default();
        let engine = EnrichmentEngine::new(config);

        let mut task = Task::default_instance();
        task.title = "修复紧急bug (fix urgent bug)".to_string();

        let result = engine.enrich(&task).await.unwrap();
        assert!(result.is_some());

        let enrichment = result.unwrap();
        // Should still detect "urgent" and "bug" keywords despite Unicode
        assert!(
            enrichment.priority.is_some(),
            "Should handle Unicode and detect keywords"
        );
    }

    #[tokio::test]
    async fn test_enrichment_empty_vs_whitespace_title() {
        let config = TodoEnrichmentConfig::default();
        let engine = EnrichmentEngine::new(config);

        // Test 1: Completely empty title
        let mut task1 = Task::default_instance();
        task1.title = "".to_string();

        let result1 = engine.enrich(&task1).await.unwrap();
        assert!(result1.is_some());
        let enrich1 = result1.unwrap();
        assert!(enrich1.priority.is_some());
        assert_eq!(
            enrich1.priority.as_ref().unwrap().value,
            3,
            "Empty title should default to medium priority"
        );

        // Test 2: Whitespace-only title
        let mut task2 = Task::default_instance();
        task2.title = "   \t\n  ".to_string();

        let result2 = engine.enrich(&task2).await.unwrap();
        assert!(result2.is_some());
        let enrich2 = result2.unwrap();
        assert_eq!(
            enrich2.priority.as_ref().unwrap().value,
            3,
            "Whitespace title should default to medium priority"
        );
    }

    #[tokio::test]
    async fn test_enrichment_confidence_values() {
        let config = TodoEnrichmentConfig::default();
        let engine = EnrichmentEngine::new(config);

        let mut task = Task::default_instance();
        task.title = "URGENT critical production issue".to_string();

        let result = engine.enrich(&task).await.unwrap();
        assert!(result.is_some());

        let enrichment = result.unwrap();
        if let Some(priority_sug) = enrichment.priority {
            assert!(
                priority_sug.confidence >= 0.80,
                "High-priority keywords should have high confidence: {}",
                priority_sug.confidence
            );
            assert!(
                !priority_sug.reasoning.is_empty(),
                "Should provide reasoning"
            );
        }
    }

    #[tokio::test]
    async fn test_enrichment_with_only_tags() {
        let config = TodoEnrichmentConfig::default();
        let engine = EnrichmentEngine::new(config);

        let mut task = Task::default_instance();
        task.title = "Do something".to_string(); // Generic title
        task.tags = vec!["urgent".to_string(), "critical".to_string()];

        let result = engine.enrich(&task).await.unwrap();
        assert!(result.is_some());

        let enrichment = result.unwrap();
        assert!(
            enrichment.priority.is_some(),
            "Should detect keywords in tags"
        );
        assert_eq!(
            enrichment.priority.unwrap().value,
            1,
            "Should infer high priority from tags"
        );
    }

    // ── LLM enrichment tests ────────────────────────────────────────────────

    #[tokio::test]
    async fn test_enrichment_with_llm_provider() {
        let json_response = r#"{"priority":1,"priority_confidence":0.95,"priority_reasoning":"urgent keyword","estimated_minutes":30,"duration_confidence":0.80,"duration_reasoning":"small fix","due_date":null,"due_date_confidence":0.0,"due_date_reasoning":"no deadline"}"#;

        let config = TodoEnrichmentConfig {
            use_llm: true,
            ..Default::default()
        };
        let engine = EnrichmentEngine::new(config).with_provider(
            Arc::new(MockProvider {
                response: json_response.to_string(),
            }),
            "mock".to_string(),
        );

        let mut task = Task::default_instance();
        task.title = "Fix urgent auth bug".to_string();

        let result = engine.enrich(&task).await.unwrap();
        assert!(result.is_some(), "LLM enrichment should return a result");

        let enrichment = result.unwrap();
        assert!(enrichment.priority.is_some(), "Priority should be set");
        assert_eq!(
            enrichment.priority.as_ref().unwrap().value,
            1,
            "LLM-inferred priority should be 1"
        );
    }

    #[tokio::test]
    async fn test_enrichment_falls_back_on_llm_error() {
        let config = TodoEnrichmentConfig {
            use_llm: true,
            ..Default::default()
        };
        let engine = EnrichmentEngine::new(config)
            .with_provider(Arc::new(ErrorProvider), "error".to_string());

        let mut task = Task::default_instance();
        task.title = "Fix urgent auth bug".to_string();

        // LLM errors → keyword enrichment should still produce results
        let result = engine.enrich(&task).await.unwrap();
        assert!(
            result.is_some(),
            "Should fall back to keyword enrichment on LLM error"
        );

        let enrichment = result.unwrap();
        // Keyword enrichment detects "urgent" → priority 1
        assert!(enrichment.priority.is_some());
        assert_eq!(
            enrichment.priority.unwrap().value,
            1,
            "Keyword fallback should detect 'urgent' → priority 1"
        );
    }

    #[tokio::test]
    async fn test_enrichment_llm_skips_already_set_fields() {
        // LLM returns priority=2 but task already has priority=1
        // The engine must NOT overwrite it
        let json_response = r#"{"priority":2,"priority_confidence":0.8,"priority_reasoning":"bug keyword","estimated_minutes":30,"duration_confidence":0.75,"duration_reasoning":"small task","due_date":null,"due_date_confidence":0.0,"due_date_reasoning":""}"#;

        let config = TodoEnrichmentConfig {
            use_llm: true,
            ..Default::default()
        };
        let engine = EnrichmentEngine::new(config).with_provider(
            Arc::new(MockProvider {
                response: json_response.to_string(),
            }),
            "mock".to_string(),
        );

        let mut task = Task::default_instance();
        task.title = "Fix bug in auth".to_string();
        task.priority = Some(1); // Already set

        let result = engine.enrich(&task).await.unwrap();
        // May be Some (with due_date set) or None — priority must be absent
        if let Some(enrichment) = result {
            assert!(
                enrichment.priority.is_none(),
                "Should not overwrite already-set priority"
            );
        }
    }

    #[tokio::test]
    async fn test_enrichment_llm_use_llm_false_skips_provider() {
        // Even if a provider is attached, use_llm=false should use keywords only
        let json_response = r#"{"priority":1,"priority_confidence":0.99,"priority_reasoning":"from LLM","estimated_minutes":15,"duration_confidence":0.9,"duration_reasoning":"quick","due_date":null,"due_date_confidence":0.0,"due_date_reasoning":""}"#;

        let config = TodoEnrichmentConfig {
            use_llm: false, // explicitly off
            ..Default::default()
        };
        // Attach provider — but it should not be called
        let engine = EnrichmentEngine::new(config).with_provider(
            Arc::new(MockProvider {
                response: json_response.to_string(),
            }),
            "mock".to_string(),
        );

        let mut task = Task::default_instance();
        task.title = "Rename variable".to_string(); // keyword → P3/quick

        let result = engine.enrich(&task).await.unwrap();
        assert!(result.is_some());
        // Keyword engine sees no high-priority keywords → medium priority (3)
        // If LLM had run it would return priority=1, keywords return priority=3.
        let enrichment = result.unwrap();
        if let Some(priority) = enrichment.priority {
            assert_ne!(
                priority.value, 1,
                "use_llm=false should not produce LLM-only priority=1 for 'Rename variable'"
            );
        }
    }
}
