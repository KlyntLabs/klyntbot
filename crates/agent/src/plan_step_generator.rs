//! Plan step generator — LLM-driven decomposition of a task description into
//! actionable plan steps.
//!
//! Public API:
//! - [`PlanStepDraft`] — lightweight draft before DB persistence
//! - [`generate_plan_steps`] — call an LLM and parse the result
//! - [`drafts_to_plan_steps`] — convert drafts to full [`plan::PlanStep`] records

use common::Result;
use plan::{PlanStep, StepStatus, DEFAULT_MAX_STEP_ATTEMPTS};
use providers::{ChatParams, DynProvider, Message};
use tracing::warn;
use uuid::Uuid;

/// Lightweight representation of a plan step before it is assigned an ID and
/// persisted to the database.
#[derive(Debug, Clone)]
pub struct PlanStepDraft {
    pub description: String,
    pub reasoning: String,
    pub expected_tools: Vec<String>,
}

/// Ask the LLM to decompose `description` into 3-8 actionable plan steps.
///
/// `context` carries any prior conversation messages to include for grounding
/// (pass an empty slice when calling from a fresh context).
/// `available_tools` is a hint list shown to the LLM so it can name real tools.
///
/// Returns a `Vec<PlanStepDraft>`, capped at 8 entries.  On any error (LLM
/// failure, bad JSON, empty response) a warning is logged and an empty `Vec`
/// is returned so callers can degrade gracefully.
pub async fn generate_plan_steps(
    provider: &DynProvider,
    model: &str,
    description: &str,
    context: &[Message],
    available_tools: &[&str],
) -> Result<Vec<PlanStepDraft>> {
    let tools_hint = if available_tools.is_empty() {
        "Use whatever tools are appropriate.".to_string()
    } else {
        format!("Available tools: {}", available_tools.join(", "))
    };

    let prompt = format!(
        "Break this task into 3-8 concrete, actionable steps.\n\
         Task: {description}\n\
         {tools_hint}\n\
         \n\
         IMPORTANT: Each step description must specify the EXACT tool action and parameters to use.\n\
         BAD: \"Add due dates to tasks\"\n\
         GOOD: \"Call todo with action=update, id=<task_id>, due_date=<date> for each task missing a due date\"\n\
         BAD: \"Review the tasks\"\n\
         GOOD: \"Call todo with action=list to retrieve all current tasks\"\n\
         \n\
         Respond ONLY with a valid JSON array (no markdown fences, no extra text):\n\
         [{{\"description\": \"...\", \"reasoning\": \"...\", \"expectedTools\": [\"tool_name\"]}}]"
    );

    let mut messages: Vec<Message> = vec![Message::system(
        "You are a planning assistant. Decompose tasks into clear, sequential steps. \
         Each step must specify the exact tool action and arguments to use. \
         Respond ONLY with a valid JSON array. No markdown, no explanations.",
    )];
    messages.extend_from_slice(context);
    messages.push(Message::user(prompt));

    let params = ChatParams::new(model)
        .with_max_tokens(1024)
        .with_temperature(0.0);

    let response = match provider.chat(&messages, None, &params).await {
        Ok(r) => r,
        Err(e) => {
            warn!("generate_plan_steps: LLM call failed: {}", e);
            return Err(e);
        }
    };

    let content = response.content.unwrap_or_default();
    if content.trim().is_empty() {
        warn!("generate_plan_steps: LLM returned empty content");
        return Ok(Vec::new());
    }

    Ok(parse_step_drafts(&content))
}

/// Convert a slice of [`PlanStepDraft`] values into [`PlanStep`] records,
/// assigning sequential indices starting at `start_index`.
pub fn drafts_to_plan_steps(drafts: &[PlanStepDraft], start_index: usize) -> Vec<PlanStep> {
    drafts
        .iter()
        .enumerate()
        .map(|(i, d)| PlanStep {
            id: Uuid::new_v4(),
            index: start_index + i,
            description: d.description.clone(),
            reasoning: d.reasoning.clone(),
            expected_tools: d.expected_tools.clone(),
            status: StepStatus::Pending,
            attempt_count: 0,
            max_attempts: DEFAULT_MAX_STEP_ATTEMPTS,
            result: None,
            started_at: None,
            completed_at: None,
        })
        .collect()
}

// ── Private helpers ──────────────────────────────────────────────────────────

/// Parse a JSON array of step objects from raw LLM output.
pub fn parse_step_drafts(content: &str) -> Vec<PlanStepDraft> {
    let trimmed = common::utils::extract_json_array(content);
    let raw: Vec<serde_json::Value> = match serde_json::from_str(trimmed) {
        Ok(v) => v,
        Err(e) => {
            warn!(
                "generate_plan_steps: failed to parse JSON: {} | raw: {}",
                e, content
            );
            return Vec::new();
        }
    };

    raw.into_iter()
        .filter_map(|v| {
            let description = v
                .get("description")
                .and_then(|d| d.as_str())
                .unwrap_or("")
                .to_string();
            if description.is_empty() {
                return None;
            }
            let reasoning = v
                .get("reasoning")
                .and_then(|r| r.as_str())
                .unwrap_or("")
                .to_string();
            let expected_tools = v
                .get("expectedTools")
                .and_then(|t| t.as_array())
                .map(|arr| {
                    arr.iter()
                        .filter_map(|t| t.as_str().map(String::from))
                        .collect()
                })
                .unwrap_or_default();
            Some(PlanStepDraft {
                description,
                reasoning,
                expected_tools,
            })
        })
        .take(8)
        .collect()
}


// ── Tests ────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use async_trait::async_trait;
    use providers::{ChatParams, LlmProvider, LlmResponse, Usage};
    use serde_json::Value;
    use std::sync::Arc;

    struct MockJsonProvider {
        response: String,
    }

    #[async_trait]
    impl LlmProvider for MockJsonProvider {
        async fn chat(
            &self,
            _messages: &[Message],
            _tools: Option<&[Value]>,
            _params: &ChatParams,
        ) -> common::Result<LlmResponse> {
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
    }

    #[tokio::test]
    async fn test_generate_plan_steps_returns_valid_steps() {
        let json = r#"[
            {"description": "Set up project structure", "reasoning": "Foundation needed", "expectedTools": ["shell"]},
            {"description": "Implement authentication", "reasoning": "Core feature", "expectedTools": ["file_write"]},
            {"description": "Add tests", "reasoning": "Quality assurance", "expectedTools": ["shell"]}
        ]"#;
        let provider: providers::DynProvider = Arc::new(MockJsonProvider {
            response: json.to_string(),
        });

        let steps = generate_plan_steps(
            &provider,
            "mock",
            "Build a REST API with user authentication",
            &[],
            &["shell", "file_write"],
        )
        .await
        .unwrap();

        assert!(!steps.is_empty());
        assert!(steps.len() <= 8);
        assert_eq!(steps[0].description, "Set up project structure");
        assert_eq!(steps[1].expected_tools, vec!["file_write"]);
    }

    #[tokio::test]
    async fn test_generate_plan_steps_empty_on_bad_json() {
        let provider: providers::DynProvider = Arc::new(MockJsonProvider {
            response: "I cannot generate steps for this task.".to_string(),
        });
        let steps = generate_plan_steps(&provider, "mock", "some task", &[], &[])
            .await
            .unwrap();
        assert!(steps.is_empty());
    }

    #[tokio::test]
    async fn test_generate_plan_steps_strips_markdown_fences() {
        let json = r#"```json
        [{"description": "Step one", "reasoning": "first", "expectedTools": []}]
        ```"#;
        let provider: providers::DynProvider = Arc::new(MockJsonProvider {
            response: json.to_string(),
        });
        let steps = generate_plan_steps(&provider, "mock", "task", &[], &[])
            .await
            .unwrap();
        // extract_json_array should find the array inside the fences
        assert_eq!(steps.len(), 1);
        assert_eq!(steps[0].description, "Step one");
    }

    #[test]
    fn test_drafts_to_plan_steps_correct_indices() {
        let drafts = vec![
            PlanStepDraft {
                description: "Step A".into(),
                reasoning: "r1".into(),
                expected_tools: vec![],
            },
            PlanStepDraft {
                description: "Step B".into(),
                reasoning: "r2".into(),
                expected_tools: vec!["shell".into()],
            },
        ];
        let steps = drafts_to_plan_steps(&drafts, 0);
        assert_eq!(steps.len(), 2);
        assert_eq!(steps[0].index, 0);
        assert_eq!(steps[1].index, 1);
        assert_eq!(steps[0].description, "Step A");
        assert_eq!(steps[1].expected_tools, vec!["shell"]);
    }

    #[test]
    fn test_drafts_to_plan_steps_with_offset() {
        let drafts = vec![PlanStepDraft {
            description: "Retry step".into(),
            reasoning: "fallback".into(),
            expected_tools: vec![],
        }];
        let steps = drafts_to_plan_steps(&drafts, 3);
        assert_eq!(steps[0].index, 3);
    }

    #[test]
    fn test_drafts_to_plan_steps_all_pending() {
        let drafts = vec![PlanStepDraft {
            description: "Do something".into(),
            reasoning: "because".into(),
            expected_tools: vec![],
        }];
        let steps = drafts_to_plan_steps(&drafts, 0);
        assert_eq!(steps[0].status, StepStatus::Pending);
        assert_eq!(steps[0].attempt_count, 0);
        assert_eq!(steps[0].max_attempts, 3);
    }

    #[test]
    fn test_parse_step_drafts_caps_at_8() {
        let items: Vec<serde_json::Value> = (0..12)
            .map(|i| {
                serde_json::json!({
                    "description": format!("Step {}", i),
                    "reasoning": "r",
                    "expectedTools": []
                })
            })
            .collect();
        let json = serde_json::to_string(&items).unwrap();
        let drafts = parse_step_drafts(&json);
        assert_eq!(drafts.len(), 8);
    }
}
