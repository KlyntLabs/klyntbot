//! LLM-powered proactive suggestion handler.
//!
//! Evaluates tasks against triggers (overdue, stale, WIP exceeded, etc.)
//! and generates suggestion candidates using LLM reasoning.

use std::sync::Arc;

use async_trait::async_trait;
use chrono::Utc;
use common::Result;
use providers::{ChatParams, DynProvider, Message, ResponseFormat};
use tracing::{debug, warn};

use feature_tasks::types::{
    SuggestionAction, SuggestionCandidate, SuggestionScope, SuggestionTrigger, SuggestionType, Task,
};
use feature_tasks::ProactiveHandler;
use storage::TaskRepo;

static PROMPT_TEMPLATE: &str = include_str!("../templates/proactive_suggestions.md");

pub struct LlmProactiveHandler {
    provider: DynProvider,
    model: String,
    repo: TaskRepo,
    domain_bus: Option<Arc<bus::DomainEventBus>>,
}

impl LlmProactiveHandler {
    pub fn new(
        provider: DynProvider,
        model: String,
        repo: TaskRepo,
        domain_bus: Option<Arc<bus::DomainEventBus>>,
    ) -> Self {
        Self {
            provider,
            model,
            repo,
            domain_bus,
        }
    }
}

fn build_prompt(task: &Task, trigger: &SuggestionTrigger, context: &str) -> String {
    PROMPT_TEMPLATE
        .replace("{{ trigger }}", &format!("{trigger:?}"))
        .replace("{{ task_id }}", &task.id)
        .replace("{{ title }}", &task.title)
        .replace("{{ status }}", &task.status)
        .replace(
            "{{ priority }}",
            &task.priority.map_or("none".into(), |p| p.to_string()),
        )
        .replace(
            "{{ energy_level }}",
            task.energy_level
                .as_ref()
                .map(|e| e.to_string())
                .as_deref()
                .unwrap_or("none"),
        )
        .replace(
            "{{ estimated_minutes }}",
            &task
                .estimated_minutes
                .map_or("none".into(), |m| m.to_string()),
        )
        .replace(
            "{{ due_date }}",
            &task.due_date.map_or("none".into(), |d| d.to_rfc3339()),
        )
        .replace("{{ tags }}", &task.tags.join(", "))
        .replace("{{ created_at }}", &task.created_at.to_rfc3339())
        .replace(
            "{{ description }}",
            task.description.as_deref().unwrap_or(""),
        )
        .replace("{{ context }}", context)
}

fn parse_suggestions(
    response: &str,
    task_id: &str,
    trigger: &SuggestionTrigger,
) -> Vec<SuggestionCandidate> {
    let json_str = common::helpers::strip_llm_fences(response);

    let parsed: serde_json::Value = match serde_json::from_str(json_str) {
        Ok(v) => v,
        Err(e) => {
            warn!("Failed to parse proactive suggestions JSON: {e}");
            return vec![];
        }
    };

    let suggestions = parsed
        .get("suggestions")
        .and_then(|s| s.as_array())
        .cloned()
        .unwrap_or_default();

    suggestions
        .into_iter()
        .filter_map(|s| {
            let suggestion_type_str = s.get("suggestion_type")?.as_str()?;
            let suggestion_type: SuggestionType =
                serde_json::from_value(serde_json::Value::String(suggestion_type_str.to_string()))
                    .ok()?;
            let action: Option<SuggestionAction> = s
                .get("action")
                .and_then(|a| serde_json::from_value(a.clone()).ok());

            Some(SuggestionCandidate {
                task_id: Some(task_id.to_string()),
                suggestion_type,
                title: s.get("title")?.as_str()?.to_string(),
                description: s
                    .get("description")
                    .and_then(|d| d.as_str())
                    .map(String::from),
                confidence: s.get("confidence")?.as_f64()?,
                action,
                trigger: Some(trigger.clone()),
            })
        })
        .collect()
}

#[async_trait]
impl ProactiveHandler for LlmProactiveHandler {
    async fn suggest(&self, scope: &SuggestionScope) -> Result<Vec<SuggestionCandidate>> {
        let filter = storage::TaskFilter {
            status: Some("todo".into()),
            project_id: scope.project_id.clone(),
            area_id: scope.area_id.clone(),
            ..Default::default()
        };
        let tasks = self.repo.list(&filter).await?;

        let mut all_suggestions = Vec::new();
        let now = Utc::now();

        for row in &tasks {
            let task = Task::from(row.clone());

            if let Some(due) = &task.due_date {
                if *due < now {
                    let candidates = self
                        .evaluate_task(&task, &SuggestionTrigger::TaskOverdue)
                        .await?;
                    all_suggestions.extend(candidates);
                }
            }
        }

        // Deduplicate: keep highest confidence per (task_id, suggestion_type)
        all_suggestions.sort_by(|a, b| {
            b.confidence
                .partial_cmp(&a.confidence)
                .unwrap_or(std::cmp::Ordering::Equal)
        });
        let mut seen = std::collections::HashSet::new();
        all_suggestions.retain(|s| {
            let key = (s.task_id.clone(), format!("{:?}", s.suggestion_type));
            seen.insert(key)
        });

        Ok(all_suggestions)
    }

    async fn evaluate_task(
        &self,
        task: &Task,
        trigger: &SuggestionTrigger,
    ) -> Result<Vec<SuggestionCandidate>> {
        let context = format!("Trigger reason: {trigger:?}");
        let prompt = build_prompt(task, trigger, &context);

        debug!(task_id = %task.id, trigger = ?trigger, "Evaluating task for proactive suggestions");

        let params = ChatParams::new(&self.model)
            .with_temperature(0.2)
            .with_max_tokens(4096)
            .with_response_format(ResponseFormat::JsonObject);

        let messages = vec![
            Message::system(
                "You are a proactive task management assistant. Return only valid JSON.",
            ),
            Message::user(prompt),
        ];

        let response = self.provider.chat(&messages, None, &params).await?;
        let text = response.content.unwrap_or_default();
        let candidates = parse_suggestions(&text, &task.id, trigger);

        // Emit events for created suggestions
        if let Some(ref bus) = self.domain_bus {
            for c in &candidates {
                let _ = bus.publish(bus::DomainEvent::ProactiveSuggestionCreated {
                    suggestion_id: uuid::Uuid::new_v4().to_string(),
                    suggestion_type: format!("{:?}", c.suggestion_type),
                    task_id: c.task_id.clone(),
                    confidence: c.confidence,
                });
            }
        }

        Ok(candidates)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_build_prompt_includes_task_details() {
        let mut task = Task::default_instance();
        task.id = "t1".into();
        task.title = "Fix login bug".into();
        task.status = "todo".into();
        task.tags = vec!["rust".into(), "auth".into()];

        let prompt = build_prompt(&task, &SuggestionTrigger::TaskOverdue, "overdue by 3 days");

        assert!(prompt.contains("Fix login bug"));
        assert!(prompt.contains("TaskOverdue"));
        assert!(prompt.contains("rust, auth"));
    }

    #[test]
    fn test_parse_suggestions_valid_json() {
        let response = r#"{"suggestions": [{"suggestion_type": "reprioritize", "title": "Increase priority", "description": "Task is overdue", "confidence": 0.85, "action": {"type": "setpriority", "priority": 1}}]}"#;

        let candidates = parse_suggestions(response, "t1", &SuggestionTrigger::TaskOverdue);

        assert_eq!(candidates.len(), 1);
        assert_eq!(candidates[0].title, "Increase priority");
        assert!((candidates[0].confidence - 0.85).abs() < 0.01);
        assert!(candidates[0].action.is_some());
    }

    #[test]
    fn test_parse_suggestions_empty() {
        let response = r#"{"suggestions": []}"#;
        let candidates = parse_suggestions(response, "t1", &SuggestionTrigger::TaskOverdue);
        assert!(candidates.is_empty());
    }

    #[test]
    fn test_parse_suggestions_invalid_json() {
        let response = "I'm sorry, I can't help with that.";
        let candidates = parse_suggestions(response, "t1", &SuggestionTrigger::TaskOverdue);
        assert!(candidates.is_empty());
    }

    #[test]
    fn test_parse_suggestions_with_markdown_fences() {
        let response = "```json\n{\"suggestions\": [{\"suggestion_type\": \"decompose\", \"title\": \"Break down\", \"description\": \"Too complex\", \"confidence\": 0.9, \"action\": {\"type\": \"triggerdecomposition\"}}]}\n```";
        let candidates = parse_suggestions(response, "t1", &SuggestionTrigger::PeriodicScan);

        assert_eq!(candidates.len(), 1);
        assert_eq!(candidates[0].title, "Break down");
    }
}
