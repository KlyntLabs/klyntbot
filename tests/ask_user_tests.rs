//! Integration tests for ask_user tool and InteractionRequest flow
//!
//! These tests validate the complete ask_user workflow including:
//! - Tool execution with InteractionRequest/FormResponse
//! - Semantic response formatting
//! - Multi-question forms
//! - Cancellation handling
//! - Non-TTY fallback behavior

use klyntbot::tools::ask_user::AskUserTool;
use klyntbot::tools::{InteractionBundle, RoutingContext, Tool};
use klyntbot::{Answer, AnswerValue, FormResponse};
use serde_json::json;
use tokio::sync::mpsc;

/// Helper to create a non-interactive routing context (no interaction_tx)
fn non_interactive_ctx() -> RoutingContext {
    RoutingContext::new(
        klyntbot::common::ChannelName::new("test"),
        klyntbot::common::ChatId::new("test-chat"),
    )
}

/// Helper to create an interactive routing context with interaction channel
fn interactive_ctx() -> (RoutingContext, mpsc::Receiver<InteractionBundle>) {
    let (interaction_tx, interaction_rx) = mpsc::channel(4);
    let ctx = RoutingContext::with_interaction(
        klyntbot::common::ChannelName::new("cli"),
        klyntbot::common::ChatId::new("test-session"),
        interaction_tx,
    );
    (ctx, interaction_rx)
}

#[tokio::test]
async fn test_ask_user_single_select_question() {
    let tool = AskUserTool;
    let (ctx, mut interaction_rx) = interactive_ctx();

    let args = json!({
        "title": "Choose authentication",
        "questions": [{
            "id": "auth_method",
            "title": "Auth",
            "text": "Which authentication method?",
            "answer_type": {
                "type": "single_select",
                "options": [
                    {"value": "oauth2", "label": "OAuth 2.0"},
                    {"value": "jwt", "label": "JWT"}
                ]
            }
        }]
    });

    // Execute ask_user in the background
    let execute_handle = tokio::spawn({
        let ctx = ctx.clone();
        async move { tool.execute(args, &ctx).await }
    });

    // Receive the interaction request
    let bundle = interaction_rx.recv().await.expect("Should receive bundle");
    assert_eq!(bundle.request.title, "Choose authentication");
    assert_eq!(bundle.request.questions.len(), 1);
    assert_eq!(bundle.request.questions[0].id, "auth_method");

    // Simulate user selecting OAuth 2.0
    let response = FormResponse::Completed(vec![Answer {
        question_id: "auth_method".to_string(),
        value: AnswerValue::Selected {
            value: "oauth2".to_string(),
        },
    }]);

    bundle
        .response_tx
        .send(response)
        .expect("Should send response");

    // Verify semantic response format
    let result = execute_handle.await.unwrap().unwrap();
    assert!(result.contains("User answered your questions"));
    assert!(result.contains("auth_method: oauth2"));
}

#[tokio::test]
async fn test_ask_user_multi_question_form() {
    let tool = AskUserTool;
    let (ctx, mut interaction_rx) = interactive_ctx();

    let args = json!({
        "title": "Task Configuration",
        "questions": [
            {
                "id": "priority",
                "title": "Priority",
                "text": "What priority level?",
                "answer_type": {
                    "type": "single_select",
                    "options": [
                        {"value": "high", "label": "High", "description": "Critical tasks"},
                        {"value": "low", "label": "Low"}
                    ]
                }
            },
            {
                "id": "features",
                "title": "Features",
                "text": "Which features?",
                "answer_type": {
                    "type": "multi_select",
                    "options": [
                        {"value": "auth", "label": "Authentication"},
                        {"value": "logging", "label": "Logging"}
                    ]
                }
            },
            {
                "id": "confirm",
                "title": "Confirm",
                "text": "Proceed?",
                "answer_type": {
                    "type": "yes_no",
                    "default": true
                }
            }
        ]
    });

    let execute_handle = tokio::spawn({
        let ctx = ctx.clone();
        async move { tool.execute(args, &ctx).await }
    });

    let bundle = interaction_rx.recv().await.expect("Should receive bundle");
    assert_eq!(bundle.request.questions.len(), 3);

    // Simulate user answering all three questions
    let response = FormResponse::Completed(vec![
        Answer {
            question_id: "priority".to_string(),
            value: AnswerValue::Selected {
                value: "high".to_string(),
            },
        },
        Answer {
            question_id: "features".to_string(),
            value: AnswerValue::MultiSelected {
                values: vec!["auth".to_string(), "logging".to_string()],
            },
        },
        Answer {
            question_id: "confirm".to_string(),
            value: AnswerValue::YesNo { answer: true },
        },
    ]);

    bundle.response_tx.send(response).unwrap();

    let result = execute_handle.await.unwrap().unwrap();
    assert!(result.contains("priority: high"));
    assert!(result.contains("features: auth, logging"));
    assert!(result.contains("confirm: Yes"));
}

#[tokio::test]
async fn test_ask_user_cancellation() {
    let tool = AskUserTool;
    let (ctx, mut interaction_rx) = interactive_ctx();

    let args = json!({
        "title": "Test Question",
        "questions": [{
            "id": "test",
            "title": "Test",
            "text": "A test question",
            "answer_type": {"type": "yes_no"}
        }]
    });

    let execute_handle = tokio::spawn({
        let ctx = ctx.clone();
        async move { tool.execute(args, &ctx).await }
    });

    let bundle = interaction_rx.recv().await.unwrap();

    // User cancels (presses Esc or Ctrl+C)
    bundle.response_tx.send(FormResponse::Cancelled).unwrap();

    let result = execute_handle.await.unwrap().unwrap();
    assert_eq!(result, "User cancelled the interaction.");
}

#[tokio::test]
async fn test_ask_user_non_tty_fallback() {
    let tool = AskUserTool;
    let ctx = non_interactive_ctx(); // No interaction_tx

    let args = json!({
        "title": "Authentication Setup",
        "questions": [{
            "id": "auth_method",
            "title": "Auth",
            "text": "Which authentication method?",
            "answer_type": {
                "type": "single_select",
                "options": [
                    {"value": "oauth2", "label": "OAuth 2.0", "description": "Industry standard"},
                    {"value": "jwt", "label": "JWT"}
                ]
            }
        }]
    });

    let result = tool.execute(args, &ctx).await.unwrap();

    // Verify non-TTY fallback format
    assert!(result.contains("Interactive UI not available"));
    assert!(result.contains("Authentication Setup"));
    assert!(result.contains("Which authentication method?"));
    assert!(result.contains("a) OAuth 2.0 — Industry standard"));
    assert!(result.contains("b) JWT"));
    assert!(result.contains("Present these questions to the user"));
}

#[tokio::test]
async fn test_ask_user_free_text_question() {
    let tool = AskUserTool;
    let (ctx, mut interaction_rx) = interactive_ctx();

    let args = json!({
        "title": "Provide details",
        "questions": [{
            "id": "description",
            "title": "Description",
            "text": "Describe the issue",
            "answer_type": {
                "type": "free_text",
                "placeholder": "Enter details..."
            }
        }]
    });

    let execute_handle = tokio::spawn({
        let ctx = ctx.clone();
        async move { tool.execute(args, &ctx).await }
    });

    let bundle = interaction_rx.recv().await.unwrap();
    let response = FormResponse::Completed(vec![Answer {
        question_id: "description".to_string(),
        value: AnswerValue::Text {
            content: "The login button is not responding".to_string(),
        },
    }]);

    bundle.response_tx.send(response).unwrap();

    let result = execute_handle.await.unwrap().unwrap();
    assert!(result.contains("description: \"The login button is not responding\""));
}

#[tokio::test]
async fn test_ask_user_skipped_answers() {
    let tool = AskUserTool;
    let (ctx, mut interaction_rx) = interactive_ctx();

    let args = json!({
        "title": "Optional questions",
        "questions": [{
            "id": "optional",
            "title": "Optional",
            "text": "Optional question",
            "answer_type": {"type": "yes_no"}
        }]
    });

    let execute_handle = tokio::spawn({
        let ctx = ctx.clone();
        async move { tool.execute(args, &ctx).await }
    });

    let bundle = interaction_rx.recv().await.unwrap();
    let response = FormResponse::Completed(vec![Answer {
        question_id: "optional".to_string(),
        value: AnswerValue::Skipped,
    }]);

    bundle.response_tx.send(response).unwrap();

    let result = execute_handle.await.unwrap().unwrap();
    assert!(result.contains("optional: (skipped)"));
}

#[tokio::test]
async fn test_ask_user_empty_multi_select() {
    let tool = AskUserTool;
    let (ctx, mut interaction_rx) = interactive_ctx();

    let args = json!({
        "title": "Select features",
        "questions": [{
            "id": "features",
            "title": "Features",
            "text": "Which features?",
            "answer_type": {
                "type": "multi_select",
                "options": [
                    {"value": "a", "label": "Feature A"},
                    {"value": "b", "label": "Feature B"}
                ]
            }
        }]
    });

    let execute_handle = tokio::spawn({
        let ctx = ctx.clone();
        async move { tool.execute(args, &ctx).await }
    });

    let bundle = interaction_rx.recv().await.unwrap();
    // User selects nothing
    let response = FormResponse::Completed(vec![Answer {
        question_id: "features".to_string(),
        value: AnswerValue::MultiSelected { values: vec![] },
    }]);

    bundle.response_tx.send(response).unwrap();

    let result = execute_handle.await.unwrap().unwrap();
    assert!(result.contains("features: (none selected)"));
}

#[tokio::test]
async fn test_ask_user_parameter_validation_missing_title() {
    let tool = AskUserTool;
    let ctx = non_interactive_ctx();

    let args = json!({
        "questions": [{
            "id": "q1",
            "title": "Q1",
            "text": "Question",
            "answer_type": {"type": "yes_no"}
        }]
    });

    // Missing title defaults to "User Input" (lenient parsing)
    let result = tool.execute(args, &ctx).await.unwrap();
    assert!(result.contains("User Input"));
}

#[tokio::test]
async fn test_ask_user_parameter_validation_empty_questions() {
    let tool = AskUserTool;
    let ctx = non_interactive_ctx();

    let args = json!({
        "title": "Test",
        "questions": []
    });

    let result = tool.execute(args, &ctx).await;
    assert!(result.is_err());
    assert!(result
        .unwrap_err()
        .to_string()
        .contains("questions array cannot be empty"));
}

#[tokio::test]
async fn test_ask_user_parameter_validation_too_many_questions() {
    let tool = AskUserTool;
    let ctx = non_interactive_ctx();

    let args = json!({
        "title": "Test",
        "questions": [
            {"id": "q1", "title": "Q1", "text": "Q", "answer_type": {"type": "yes_no"}},
            {"id": "q2", "title": "Q2", "text": "Q", "answer_type": {"type": "yes_no"}},
            {"id": "q3", "title": "Q3", "text": "Q", "answer_type": {"type": "yes_no"}},
            {"id": "q4", "title": "Q4", "text": "Q", "answer_type": {"type": "yes_no"}},
            {"id": "q5", "title": "Q5", "text": "Q", "answer_type": {"type": "yes_no"}}
        ]
    });

    // >4 questions silently capped at 4 (lenient parsing)
    let result = tool.execute(args, &ctx).await.unwrap();
    assert!(result.contains("Test"));
    // Only first 4 questions should be present
    assert!(result.contains("q1"));
    assert!(result.contains("q4"));
    assert!(!result.contains("q5"));
}

#[tokio::test]
async fn test_ask_user_parameter_validation_title_too_long() {
    let tool = AskUserTool;
    let ctx = non_interactive_ctx();

    let args = json!({
        "title": "Test",
        "questions": [{
            "id": "q1",
            "title": "This title is too long for tab",
            "text": "Question",
            "answer_type": {"type": "yes_no"}
        }]
    });

    // Long titles are truncated, not rejected (lenient parsing)
    let result = tool.execute(args, &ctx).await.unwrap();
    assert!(result.contains("Test"));
    assert!(result.contains("Question"));
}

#[tokio::test]
async fn test_ask_user_parameter_validation_options_min_count() {
    let tool = AskUserTool;
    let ctx = non_interactive_ctx();

    let args = json!({
        "title": "Test",
        "questions": [{
            "id": "q1",
            "title": "Q1",
            "text": "Question",
            "answer_type": {
                "type": "single_select",
                "options": [{"value": "only_one", "label": "Only one"}]
            }
        }]
    });

    let result = tool.execute(args, &ctx).await;
    assert!(result.is_err());
    assert!(result
        .unwrap_err()
        .to_string()
        .contains("must have at least 2 items"));
}

#[tokio::test]
async fn test_ask_user_response_channel_closed() {
    let tool = AskUserTool;
    let (ctx, mut interaction_rx) = interactive_ctx();

    let args = json!({
        "title": "Test",
        "questions": [{
            "id": "test",
            "title": "Test",
            "text": "Test question",
            "answer_type": {"type": "yes_no"}
        }]
    });

    let execute_handle = tokio::spawn({
        let ctx = ctx.clone();
        async move { tool.execute(args, &ctx).await }
    });

    let bundle = interaction_rx.recv().await.unwrap();
    // Drop response_tx without sending (simulates user closing UI)
    drop(bundle.response_tx);

    let result = execute_handle.await.unwrap();
    assert!(result.is_err());
    assert!(result
        .unwrap_err()
        .to_string()
        .contains("response channel dropped"));
}

#[tokio::test]
async fn test_ask_user_interaction_channel_closed() {
    let tool = AskUserTool;
    let (ctx, interaction_rx) = interactive_ctx();

    // Drop the receiver immediately
    drop(interaction_rx);

    let args = json!({
        "title": "Test",
        "questions": [{
            "id": "test",
            "title": "Test",
            "text": "Test question",
            "answer_type": {"type": "yes_no"}
        }]
    });

    let result = tool.execute(args, &ctx).await;
    assert!(result.is_err());
    assert!(result
        .unwrap_err()
        .to_string()
        .contains("Interaction channel closed"));
}
