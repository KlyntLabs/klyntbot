//! AskUser Tool - Interactive clarification system
//!
//! Allows the agent to ask structured questions to the user when it needs
//! clarification, preferences, or decisions. Supports single-select, multi-select,
//! yes/no, and free-text questions.

use async_trait::async_trait;
use serde_json::Value;
use tracing::warn;

use super::{InteractionBundle, RoutingContext, Tool};
use common::{
    Answer, AnswerOption, AnswerType, AnswerValue, FormResponse, InteractionRequest, Question,
    Result, ToolError,
};

/// Tool for asking structured questions to the user.
pub struct AskUserTool;

#[async_trait]
impl Tool for AskUserTool {
    fn name(&self) -> &str {
        "ask_user"
    }

    fn description(&self) -> &str {
        "Ask the user structured questions when you need clarification, \
         preferences, or decisions. Supports single-select, multi-select, \
         yes/no, and free-text questions. Group related questions (1-4) \
         into a single call. IMPORTANT: Never call ask_user alongside other \
         tools in the same turn - it blocks execution until the user responds."
    }

    fn parameters(&self) -> Value {
        // Flat schema — no oneOf, no nested answer_type objects.
        // The question `type` field determines which extra fields are used.
        serde_json::json!({
            "type": "object",
            "properties": {
                "title": {
                    "type": "string",
                    "description": "Overall title for the interaction"
                },
                "questions": {
                    "type": "array",
                    "minItems": 1,
                    "maxItems": 4,
                    "description": "1-4 questions to present to the user",
                    "items": {
                        "type": "object",
                        "properties": {
                            "id": {
                                "type": "string",
                                "description": "Machine-readable question ID (e.g., 'priority', 'tags')"
                            },
                            "title": {
                                "type": "string",
                                "description": "Short tab label (e.g., 'Priority')"
                            },
                            "text": {
                                "type": "string",
                                "description": "Full question text shown to the user"
                            },
                            "type": {
                                "type": "string",
                                "enum": ["single_select", "multi_select", "yes_no", "free_text"],
                                "description": "Question type"
                            },
                            "options": {
                                "type": "array",
                                "description": "Options for single_select or multi_select questions",
                                "items": {
                                    "type": "object",
                                    "properties": {
                                        "value": { "type": "string" },
                                        "label": { "type": "string" },
                                        "description": { "type": "string" }
                                    },
                                    "required": ["value", "label"]
                                }
                            },
                            "default": {
                                "type": "boolean",
                                "description": "Default value for yes_no questions"
                            },
                            "placeholder": {
                                "type": "string",
                                "description": "Placeholder text for free_text questions"
                            }
                        },
                        "required": ["id", "title", "text", "type"]
                    }
                }
            },
            "required": ["title", "questions"]
        })
    }

    async fn execute(&self, args: Value, ctx: &RoutingContext) -> Result<String> {
        // 1. Parse JSON args into InteractionRequest (lenient parsing)
        let request = match parse_interaction_request(&args) {
            Ok(r) => r,
            Err(e) => {
                // Log the error visibly so it can be diagnosed
                warn!("ask_user parse error: {} | args: {}", e, args);
                eprintln!("[ask_user] parse error: {}", e);
                return Err(e);
            }
        };

        // Path 1: CLI/dashboard interactive mode (oneshot channel)
        if let Some(interaction_tx) = &ctx.interaction_tx {
            return self
                .execute_via_cli(interaction_tx, request)
                .await;
        }

        // Path 2: Platform-native channel interaction (Telegram buttons, Discord selects, etc.)
        if let Some(channel) = &ctx.interaction_channel {
            if channel.supports_interaction() {
                let request_for_response = request.clone();
                let response = channel
                    .send_interaction(ctx.chat_id.as_str(), &request)
                    .await?;
                return Ok(format_semantic_response(&response, &request_for_response));
            }
        }

        // Path 3: Text fallback (non-TTY, no native channel support)
        Ok(format_text_fallback(&request))
    }
}

impl AskUserTool {
    /// Execute via CLI/dashboard oneshot channel (Path 1).
    async fn execute_via_cli(
        &self,
        interaction_tx: &tokio::sync::mpsc::Sender<InteractionBundle>,
        request: InteractionRequest,
    ) -> Result<String> {
        let (response_tx, response_rx) = tokio::sync::oneshot::channel();
        let request_for_response = request.clone();

        if let Err(e) = interaction_tx
            .send(InteractionBundle {
                request,
                response_tx,
            })
            .await
        {
            let msg = format!("Interaction channel closed (CLI may have exited): {}", e);
            warn!("{}", msg);
            eprintln!("[ask_user] {}", msg);
            return Err(ToolError::ExecutionFailed(msg).into());
        }

        let response = match response_rx.await {
            Ok(r) => r,
            Err(e) => {
                let msg = format!(
                    "User interaction cancelled (response channel dropped): {}",
                    e
                );
                warn!("{}", msg);
                eprintln!("[ask_user] {}", msg);
                return Err(ToolError::ExecutionFailed(msg).into());
            }
        };

        Ok(format_semantic_response(&response, &request_for_response))
    }
}

/// Parse JSON arguments into an InteractionRequest.
///
/// Uses a **flat schema** for LLM compatibility:
/// ```json
/// {
///     "title": "Task Details",
///     "questions": [{
///         "id": "priority",
///         "title": "Priority",
///         "text": "What priority?",
///         "type": "single_select",
///         "options": [{"value": "high", "label": "High"}, ...]
///     }]
/// }
/// ```
fn parse_interaction_request(args: &Value) -> Result<InteractionRequest> {
    let title = args
        .get("title")
        .and_then(|v| v.as_str())
        .unwrap_or("User Input")
        .to_string();

    let questions_array = args
        .get("questions")
        .and_then(|v| v.as_array())
        .ok_or_else(|| ToolError::InvalidParams("missing 'questions' array".into()))?;

    if questions_array.is_empty() {
        return Err(ToolError::InvalidParams("questions array cannot be empty".into()).into());
    }

    // Silently cap at 4 instead of erroring
    let questions_to_parse = if questions_array.len() > 4 {
        &questions_array[..4]
    } else {
        questions_array
    };

    let mut questions = Vec::new();
    for (idx, q_val) in questions_to_parse.iter().enumerate() {
        let q_obj = q_val.as_object().ok_or_else(|| {
            ToolError::InvalidParams(format!("question {} is not an object", idx))
        })?;

        let id = q_obj
            .get("id")
            .and_then(|v| v.as_str())
            .unwrap_or("q")
            .to_string();

        // Auto-generate id if it's just "q"
        let id = if id == "q" {
            format!("q{}", idx + 1)
        } else {
            id
        };

        // Get title, truncate if too long for tab display (no error, just truncate)
        let raw_title = q_obj.get("title").and_then(|v| v.as_str()).unwrap_or(&id);
        let question_title = if raw_title.len() > 20 {
            format!("{}…", &raw_title[..19])
        } else {
            raw_title.to_string()
        };

        let text = q_obj
            .get("text")
            .and_then(|v| v.as_str())
            .ok_or_else(|| ToolError::InvalidParams(format!("question {} missing 'text'", idx)))?
            .to_string();

        // Parse answer type from the flat schema
        let answer_type = parse_flat_answer_type(q_obj, idx)?;

        questions.push(Question {
            id,
            title: question_title,
            text,
            answer_type,
        });
    }

    Ok(InteractionRequest { title, questions })
}

/// Parse the answer type from a flat question object.
///
/// Expects `"type"` field at the question level.
fn parse_flat_answer_type(
    q_obj: &serde_json::Map<String, Value>,
    question_idx: usize,
) -> Result<AnswerType> {
    let type_str = q_obj
        .get("type")
        .and_then(|v| v.as_str())
        .ok_or_else(|| {
            ToolError::InvalidParams(format!(
                "question {} missing 'type' field (expected: single_select, multi_select, yes_no, free_text)",
                question_idx
            ))
        })?;

    let options_val = q_obj.get("options");

    match type_str {
        "single_select" => {
            let options = parse_options(options_val, question_idx)?;
            Ok(AnswerType::SingleSelect { options })
        }
        "multi_select" => {
            let options = parse_options(options_val, question_idx)?;
            Ok(AnswerType::MultiSelect { options })
        }
        "yes_no" => {
            // Try flat default, then nested
            let default = q_obj
                .get("default")
                .and_then(|v| v.as_bool())
                .or_else(|| {
                    q_obj
                        .get("answer_type")
                        .and_then(|at| at.get("default"))
                        .and_then(|v| v.as_bool())
                });
            Ok(AnswerType::YesNo { default })
        }
        "free_text" => {
            let placeholder = q_obj
                .get("placeholder")
                .and_then(|v| v.as_str())
                .or_else(|| {
                    q_obj
                        .get("answer_type")
                        .and_then(|at| at.get("placeholder"))
                        .and_then(|v| v.as_str())
                })
                .map(String::from);
            Ok(AnswerType::FreeText { placeholder })
        }
        _ => Err(ToolError::InvalidParams(format!(
            "question {} unknown type '{}' (expected: single_select, multi_select, yes_no, free_text)",
            question_idx, type_str
        ))
        .into()),
    }
}

/// Parse options array for SingleSelect/MultiSelect.
fn parse_options(value: Option<&Value>, question_idx: usize) -> Result<Vec<AnswerOption>> {
    let options_array = value.and_then(|v| v.as_array()).ok_or_else(|| {
        ToolError::InvalidParams(format!(
            "question {} requires 'options' array for select questions",
            question_idx
        ))
    })?;

    if options_array.len() < 2 {
        return Err(ToolError::InvalidParams(format!(
            "question {} options must have at least 2 items",
            question_idx
        ))
        .into());
    }

    let mut options = Vec::new();
    for (opt_idx, opt_val) in options_array.iter().enumerate() {
        let opt_obj = opt_val.as_object().ok_or_else(|| {
            ToolError::InvalidParams(format!(
                "question {} option {} is not an object",
                question_idx, opt_idx
            ))
        })?;

        let value = opt_obj
            .get("value")
            .and_then(|v| v.as_str())
            .ok_or_else(|| {
                ToolError::InvalidParams(format!(
                    "question {} option {} missing 'value'",
                    question_idx, opt_idx
                ))
            })?
            .to_string();

        let label = opt_obj
            .get("label")
            .and_then(|v| v.as_str())
            .ok_or_else(|| {
                ToolError::InvalidParams(format!(
                    "question {} option {} missing 'label'",
                    question_idx, opt_idx
                ))
            })?
            .to_string();

        let description = opt_obj
            .get("description")
            .and_then(|v| v.as_str())
            .map(String::from);

        options.push(AnswerOption {
            value,
            label,
            description,
        });
    }

    Ok(options)
}

/// Format the FormResponse into a rich semantic string for the LLM.
///
/// Includes full question context (text, descriptions, other options) so the
/// LLM has maximum information to continue the conversation meaningfully.
fn format_semantic_response(response: &FormResponse, request: &InteractionRequest) -> String {
    match response {
        FormResponse::Cancelled => format_cancelled_response(request),
        FormResponse::Completed(answers) => {
            if answers.is_empty() {
                return "User submitted the form with no answers.".to_string();
            }
            format_completed_response(answers, request)
        }
    }
}

/// Format a completed form response with full question context.
fn format_completed_response(answers: &[Answer], request: &InteractionRequest) -> String {
    use std::collections::HashMap;

    let mut output = format!("User completed \"{}\":\n", request.title);

    // Build answer lookup by question_id
    let answer_map: HashMap<&str, &Answer> = answers
        .iter()
        .map(|a| (a.question_id.as_str(), a))
        .collect();

    for (idx, question) in request.questions.iter().enumerate() {
        output.push_str(&format!(
            "\n{}. {} — \"{}\"\n",
            idx + 1,
            question.id,
            question.text
        ));

        if let Some(answer) = answer_map.get(question.id.as_str()) {
            format_rich_answer(&mut output, &answer.value, &question.answer_type);
        } else {
            output.push_str("   Answer: (not answered)\n");
        }
    }

    // Explicit directive to prevent the LLM from re-asking the same question
    output.push_str(
        "\n---\nThe user has responded. Do NOT call ask_user again for this same question. \
         Proceed with the action using the answers above.\n",
    );

    output
}

/// Format a single answer with context from the question's answer type.
fn format_rich_answer(output: &mut String, value: &AnswerValue, answer_type: &AnswerType) {
    match (value, answer_type) {
        (AnswerValue::Selected { value }, AnswerType::SingleSelect { options }) => {
            // Look up the label and description for the selected option
            if let Some(opt) = options.iter().find(|o| o.value == *value) {
                output.push_str(&format!("   Answer: {}\n", opt.label));
                if let Some(desc) = &opt.description {
                    output.push_str(&format!("   Description: {}\n", desc));
                }
            } else {
                output.push_str(&format!("   Answer: {}\n", value));
            }

            // Show other available options for context
            let others: Vec<&AnswerOption> = options.iter().filter(|o| o.value != *value).collect();
            if !others.is_empty() {
                output.push_str("   Other options were:\n");
                for opt in others {
                    output.push_str(&format!("   - {}", opt.label));
                    if let Some(desc) = &opt.description {
                        output.push_str(&format!(" — {}", desc));
                    }
                    output.push('\n');
                }
            }
        }
        (AnswerValue::MultiSelected { values }, AnswerType::MultiSelect { options }) => {
            let labels: Vec<&str> = values
                .iter()
                .filter_map(|v| {
                    options
                        .iter()
                        .find(|o| &o.value == v)
                        .map(|o| o.label.as_str())
                })
                .collect();
            output.push_str(&format!("   Answer: {}\n", labels.join(", ")));

            // Show unselected options
            let unselected: Vec<&AnswerOption> = options
                .iter()
                .filter(|o| !values.contains(&o.value))
                .collect();
            if !unselected.is_empty() {
                output.push_str("   Not selected:\n");
                for opt in unselected {
                    output.push_str(&format!("   - {}", opt.label));
                    if let Some(desc) = &opt.description {
                        output.push_str(&format!(" — {}", desc));
                    }
                    output.push('\n');
                }
            }
        }
        (AnswerValue::YesNo { answer }, _) => {
            output.push_str(&format!(
                "   Answer: {}\n",
                if *answer { "Yes" } else { "No" }
            ));
        }
        (AnswerValue::Text { content }, _) => {
            output.push_str(&format!("   Answer: \"{}\"\n", content));
        }
        (AnswerValue::Skipped, _) => {
            output.push_str("   Answer: (skipped)\n");
        }
        // Fallback for mismatched value/type
        (AnswerValue::Selected { value }, _) => {
            output.push_str(&format!("   Answer: {}\n", value));
        }
        (AnswerValue::MultiSelected { values }, _) => {
            output.push_str(&format!("   Answer: {}\n", values.join(", ")));
        }
    }
}

/// Format a cancelled interaction response with question context.
fn format_cancelled_response(request: &InteractionRequest) -> String {
    let mut output = format!("User cancelled the interaction \"{}\".\n", request.title);

    if !request.questions.is_empty() {
        output.push_str("\nQuestions that were presented:\n");
        for (idx, question) in request.questions.iter().enumerate() {
            output.push_str(&format!(
                "  {}. {} — \"{}\"\n",
                idx + 1,
                question.id,
                question.text
            ));
        }
        output.push_str(
            "\nThe user chose not to answer. Ask if they'd like to try again or take a different approach.\n",
        );
    }

    output
}

/// Format a text-based fallback when interactive UI is not available.
///
/// Returns instructions for the LLM to present questions conversationally
/// and wait for the user's text reply.
fn format_text_fallback(request: &InteractionRequest) -> String {
    let mut output = format!(
        "Interactive UI not available (non-TTY environment).\n\n\
         Present these questions to the user in your response and wait for their reply:\n\n\
         {}\n\n",
        request.title
    );

    for (idx, question) in request.questions.iter().enumerate() {
        output.push_str(&format!(
            "{}. {} (id: {})\n",
            idx + 1,
            question.text,
            question.id
        ));

        match &question.answer_type {
            AnswerType::SingleSelect { options } | AnswerType::MultiSelect { options } => {
                output.push_str("   Options:\n");
                for (opt_idx, option) in options.iter().enumerate() {
                    let letter = char::from(b'a' + opt_idx as u8);
                    output.push_str(&format!("   {}) {}", letter, option.label));
                    if let Some(desc) = &option.description {
                        output.push_str(&format!(" — {}", desc));
                    }
                    output.push('\n');
                }
            }
            AnswerType::YesNo { default } => {
                let default_str = match default {
                    Some(true) => " (default: Yes)",
                    Some(false) => " (default: No)",
                    None => "",
                };
                output.push_str(&format!("   Yes/No{}\n", default_str));
            }
            AnswerType::FreeText { placeholder } => {
                if let Some(ph) = placeholder {
                    output.push_str(&format!("   Free text (placeholder: \"{}\")\n", ph));
                } else {
                    output.push_str("   Free text input\n");
                }
            }
        }

        output.push('\n');
    }

    output.push_str(
        "Ask the user to provide their answers in their next message, \
         then call ask_user again with their responses if needed, or \
         proceed with the information gathered conversationally.",
    );

    output
}

#[cfg(test)]
mod tests {
    use super::*;
    use common::Answer;
    use serde_json::json;

    #[test]
    fn test_parse_flat_single_select() {
        let args = json!({
            "title": "Choose authentication",
            "questions": [{
                "id": "auth_method",
                "title": "Auth method",
                "text": "Which authentication method should we use?",
                "type": "single_select",
                "options": [
                    {"value": "oauth2", "label": "OAuth 2.0", "description": "Industry standard"},
                    {"value": "jwt", "label": "JWT tokens"}
                ]
            }]
        });

        let request = parse_interaction_request(&args).unwrap();
        assert_eq!(request.title, "Choose authentication");
        assert_eq!(request.questions.len(), 1);
        assert_eq!(request.questions[0].id, "auth_method");
        assert_eq!(request.questions[0].title, "Auth method");

        match &request.questions[0].answer_type {
            AnswerType::SingleSelect { options } => {
                assert_eq!(options.len(), 2);
                assert_eq!(options[0].value, "oauth2");
                assert_eq!(options[0].label, "OAuth 2.0");
                assert_eq!(options[0].description, Some("Industry standard".into()));
            }
            _ => panic!("Expected SingleSelect"),
        }
    }

    #[test]
    fn test_parse_flat_multi_select() {
        let args = json!({
            "title": "Choose features",
            "questions": [{
                "id": "features",
                "title": "Features",
                "text": "Which features do you want?",
                "type": "multi_select",
                "options": [
                    {"value": "auth", "label": "Authentication"},
                    {"value": "logging", "label": "Logging"},
                    {"value": "caching", "label": "Caching"}
                ]
            }]
        });

        let request = parse_interaction_request(&args).unwrap();
        match &request.questions[0].answer_type {
            AnswerType::MultiSelect { options } => {
                assert_eq!(options.len(), 3);
                assert_eq!(options[1].value, "logging");
            }
            _ => panic!("Expected MultiSelect"),
        }
    }

    #[test]
    fn test_parse_flat_yes_no() {
        let args = json!({
            "title": "Confirm action",
            "questions": [{
                "id": "confirm",
                "title": "Confirm",
                "text": "Do you want to proceed?",
                "type": "yes_no",
                "default": true
            }]
        });

        let request = parse_interaction_request(&args).unwrap();
        match &request.questions[0].answer_type {
            AnswerType::YesNo { default } => {
                assert_eq!(*default, Some(true));
            }
            _ => panic!("Expected YesNo"),
        }
    }

    #[test]
    fn test_parse_flat_free_text() {
        let args = json!({
            "title": "Provide details",
            "questions": [{
                "id": "description",
                "title": "Description",
                "text": "Please describe the issue",
                "type": "free_text",
                "placeholder": "Enter details here..."
            }]
        });

        let request = parse_interaction_request(&args).unwrap();
        match &request.questions[0].answer_type {
            AnswerType::FreeText { placeholder } => {
                assert_eq!(placeholder.as_deref(), Some("Enter details here..."));
            }
            _ => panic!("Expected FreeText"),
        }
    }

    #[test]
    fn test_long_title_truncated_not_rejected() {
        let args = json!({
            "title": "Test",
            "questions": [{
                "id": "test",
                "title": "This title is way too long for a tab",
                "text": "Test question",
                "type": "yes_no"
            }]
        });

        // Should NOT error — just truncate
        let request = parse_interaction_request(&args).unwrap();
        assert!(request.questions[0].title.chars().count() <= 20);
    }

    #[test]
    fn test_more_than_4_questions_silently_capped() {
        let args = json!({
            "title": "Many questions",
            "questions": [
                {"id": "q1", "title": "Q1", "text": "Question 1", "type": "yes_no"},
                {"id": "q2", "title": "Q2", "text": "Question 2", "type": "yes_no"},
                {"id": "q3", "title": "Q3", "text": "Question 3", "type": "yes_no"},
                {"id": "q4", "title": "Q4", "text": "Question 4", "type": "yes_no"},
                {"id": "q5", "title": "Q5", "text": "Question 5", "type": "yes_no"}
            ]
        });

        // Should NOT error — just cap at 4
        let request = parse_interaction_request(&args).unwrap();
        assert_eq!(request.questions.len(), 4);
    }

    #[test]
    fn test_missing_title_uses_default() {
        let args = json!({
            "questions": [{
                "id": "test",
                "title": "Test",
                "text": "A question",
                "type": "yes_no"
            }]
        });

        let request = parse_interaction_request(&args).unwrap();
        assert_eq!(request.title, "User Input");
    }

    fn make_test_request() -> InteractionRequest {
        InteractionRequest {
            title: "Test Form".into(),
            questions: vec![
                Question {
                    id: "auth_method".into(),
                    title: "Auth".into(),
                    text: "Which authentication method?".into(),
                    answer_type: AnswerType::SingleSelect {
                        options: vec![
                            AnswerOption {
                                value: "oauth2".into(),
                                label: "OAuth 2.0".into(),
                                description: Some("Industry standard".into()),
                            },
                            AnswerOption {
                                value: "jwt".into(),
                                label: "JWT".into(),
                                description: None,
                            },
                        ],
                    },
                },
                Question {
                    id: "features".into(),
                    title: "Features".into(),
                    text: "Which features do you want?".into(),
                    answer_type: AnswerType::MultiSelect {
                        options: vec![
                            AnswerOption {
                                value: "auth".into(),
                                label: "Authentication".into(),
                                description: None,
                            },
                            AnswerOption {
                                value: "logging".into(),
                                label: "Logging".into(),
                                description: None,
                            },
                        ],
                    },
                },
                Question {
                    id: "confirm".into(),
                    title: "Confirm".into(),
                    text: "Do you want to proceed?".into(),
                    answer_type: AnswerType::YesNo { default: None },
                },
            ],
        }
    }

    #[test]
    fn test_format_semantic_response_selected() {
        let request = make_test_request();
        let response = FormResponse::Completed(vec![Answer {
            question_id: "auth_method".into(),
            value: AnswerValue::Selected {
                value: "oauth2".into(),
            },
        }]);

        let formatted = format_semantic_response(&response, &request);
        assert!(formatted.contains("User completed"));
        assert!(formatted.contains("Test Form"));
        assert!(formatted.contains("auth_method"));
        assert!(formatted.contains("OAuth 2.0"));
        assert!(formatted.contains("Industry standard"));
        // Should show other options
        assert!(formatted.contains("JWT"));
    }

    #[test]
    fn test_format_semantic_response_multi_selected() {
        let request = make_test_request();
        let response = FormResponse::Completed(vec![Answer {
            question_id: "features".into(),
            value: AnswerValue::MultiSelected {
                values: vec!["auth".into(), "logging".into()],
            },
        }]);

        let formatted = format_semantic_response(&response, &request);
        assert!(formatted.contains("Authentication, Logging"));
    }

    #[test]
    fn test_format_semantic_response_yes_no() {
        let request = make_test_request();
        let response = FormResponse::Completed(vec![Answer {
            question_id: "confirm".into(),
            value: AnswerValue::YesNo { answer: true },
        }]);

        let formatted = format_semantic_response(&response, &request);
        assert!(formatted.contains("Answer: Yes"));
    }

    #[test]
    fn test_format_completed_response_includes_no_reask_directive() {
        let request = make_test_request();
        let response = FormResponse::Completed(vec![Answer {
            question_id: "confirm".into(),
            value: AnswerValue::YesNo { answer: true },
        }]);

        let formatted = format_semantic_response(&response, &request);
        assert!(formatted.contains("Do NOT call ask_user again"));
        assert!(formatted.contains("Proceed with the action"));
    }

    #[test]
    fn test_format_semantic_response_cancelled() {
        let request = make_test_request();
        let response = FormResponse::Cancelled;
        let formatted = format_semantic_response(&response, &request);
        assert!(formatted.contains("cancelled"));
        assert!(formatted.contains("Test Form"));
        // Should list the questions that were presented
        assert!(formatted.contains("auth_method"));
        assert!(formatted.contains("features"));
        assert!(formatted.contains("confirm"));
    }

    #[test]
    fn test_format_text_fallback() {
        let request = InteractionRequest {
            title: "Test Interaction".into(),
            questions: vec![Question {
                id: "priority".into(),
                title: "Priority".into(),
                text: "What's the priority?".into(),
                answer_type: AnswerType::SingleSelect {
                    options: vec![
                        AnswerOption {
                            value: "high".into(),
                            label: "High".into(),
                            description: Some("Critical tasks".into()),
                        },
                        AnswerOption {
                            value: "low".into(),
                            label: "Low".into(),
                            description: None,
                        },
                    ],
                },
            }],
        };

        let fallback = format_text_fallback(&request);
        assert!(fallback.contains("Interactive UI not available"));
        assert!(fallback.contains("Test Interaction"));
        assert!(fallback.contains("1. What's the priority?"));
        assert!(fallback.contains("a) High — Critical tasks"));
        assert!(fallback.contains("b) Low"));
    }
}
