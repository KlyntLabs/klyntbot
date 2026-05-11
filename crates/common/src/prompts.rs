//! Interactive question types for the ask_user tool.
//!
//! These types live in `common` (Layer 0) so both `tools` (Layer 3, ask_user tool)
//! and `cli` (Layer 6, renderer) can use them without circular deps.

use serde::{Deserialize, Serialize};

/// A request to present structured questions to the user.
///
/// Created by the `ask_user` tool from LLM-provided JSON arguments.
/// Rendered by the CLI (tabbed UI) or channel-specific renderers.
#[derive(Debug, Clone, Serialize, Deserialize, specta::Type)]
pub struct InteractionRequest {
    /// Overall title for the interaction (displayed above the tabbed UI).
    pub title: String,
    /// 1–4 questions to present simultaneously.
    pub questions: Vec<Question>,
}

/// A single question with typed answer options.
#[derive(Debug, Clone, Serialize, Deserialize, specta::Type)]
pub struct Question {
    /// Machine-readable ID, used to correlate with Answer.question_id.
    /// Example: "auth_method", "priority"
    pub id: String,
    /// Short label for the tab header (≤12 chars). Example: "Auth method"
    pub title: String,
    /// Full question text displayed when this tab is active.
    pub text: String,
    /// The type of answer expected.
    pub answer_type: AnswerType,
}

/// The kind of answer a question expects.
#[derive(Debug, Clone, Serialize, Deserialize, specta::Type)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum AnswerType {
    /// User selects exactly one option.
    SingleSelect { options: Vec<AnswerOption> },
    /// User selects one or more options.
    MultiSelect { options: Vec<AnswerOption> },
    /// Yes/No toggle with optional default.
    YesNo { default: Option<bool> },
    /// Free-form text input with optional placeholder.
    FreeText { placeholder: Option<String> },
}

/// A selectable option for SingleSelect and MultiSelect questions.
#[derive(Debug, Clone, Serialize, Deserialize, specta::Type)]
pub struct AnswerOption {
    /// Machine-readable value returned in the answer. Example: "oauth2"
    pub value: String,
    /// Human-readable display label. Example: "OAuth 2.0 (Recommended)"
    pub label: String,
    /// Optional description shown below the label.
    pub description: Option<String>,
}

/// A single answer to a question.
#[derive(Debug, Clone, Serialize, Deserialize, specta::Type)]
pub struct Answer {
    /// Matches Question.id.
    pub question_id: String,
    /// The user's answer.
    pub value: AnswerValue,
}

/// The concrete answer value.
#[derive(Debug, Clone, Serialize, Deserialize, specta::Type)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum AnswerValue {
    /// User chose one option (value field from AnswerOption).
    Selected { value: String },
    /// User chose multiple options.
    MultiSelected { values: Vec<String> },
    /// User answered yes or no.
    YesNo { answer: bool },
    /// User typed free text.
    Text { content: String },
    /// User skipped this question.
    Skipped,
}

/// The user's response to an entire InteractionRequest.
#[derive(Debug, Clone, Serialize, Deserialize, specta::Type)]
pub enum FormResponse {
    /// User completed the form with answers for each question.
    Completed(Vec<Answer>),
    /// User cancelled the entire interaction (Esc / Ctrl+C).
    Cancelled,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_answer_types_serde_roundtrip() {
        // SingleSelect
        let single = AnswerType::SingleSelect {
            options: vec![AnswerOption {
                value: "high".to_string(),
                label: "High Priority".to_string(),
                description: Some("Critical tasks".to_string()),
            }],
        };
        let json = serde_json::to_string(&single).unwrap();
        assert!(json.contains(r#""type":"single_select"#));
        let deserialized: AnswerType = serde_json::from_str(&json).unwrap();
        match deserialized {
            AnswerType::SingleSelect { options } => {
                assert_eq!(options.len(), 1);
                assert_eq!(options[0].value, "high");
            }
            _ => panic!("Wrong variant"),
        }

        // YesNo
        let yes_no = AnswerType::YesNo {
            default: Some(true),
        };
        let json = serde_json::to_string(&yes_no).unwrap();
        assert!(json.contains(r#""type":"yes_no"#));
        let deserialized: AnswerType = serde_json::from_str(&json).unwrap();
        match deserialized {
            AnswerType::YesNo { default } => assert_eq!(default, Some(true)),
            _ => panic!("Wrong variant"),
        }
    }

    #[test]
    fn test_answer_values_serde_roundtrip() {
        // Selected
        let selected = AnswerValue::Selected {
            value: "oauth2".to_string(),
        };
        let json = serde_json::to_string(&selected).unwrap();
        assert!(json.contains(r#""type":"selected"#));
        let deserialized: AnswerValue = serde_json::from_str(&json).unwrap();
        match &deserialized {
            AnswerValue::Selected { value } => assert_eq!(value, "oauth2"),
            _ => panic!("Wrong variant"),
        }

        // MultiSelected
        let multi = AnswerValue::MultiSelected {
            values: vec!["option1".to_string(), "option2".to_string()],
        };
        let json = serde_json::to_string(&multi).unwrap();
        assert!(json.contains(r#""type":"multi_selected"#));
        let deserialized: AnswerValue = serde_json::from_str(&json).unwrap();
        match &deserialized {
            AnswerValue::MultiSelected { values } => {
                assert_eq!(values.len(), 2);
                assert_eq!(values[0], "option1");
            }
            _ => panic!("Wrong variant"),
        }

        // FormResponse::Completed
        let response = FormResponse::Completed(vec![Answer {
            question_id: "priority".to_string(),
            value: AnswerValue::Selected {
                value: "high".to_string(),
            },
        }]);
        let json = serde_json::to_string(&response).unwrap();
        let deserialized: FormResponse = serde_json::from_str(&json).unwrap();
        match deserialized {
            FormResponse::Completed(answers) => {
                assert_eq!(answers.len(), 1);
                assert_eq!(answers[0].question_id, "priority");
            }
            _ => panic!("Wrong variant"),
        }

        // FormResponse::Cancelled
        let response = FormResponse::Cancelled;
        let json = serde_json::to_string(&response).unwrap();
        assert!(json.contains("Cancelled"));
        let deserialized: FormResponse = serde_json::from_str(&json).unwrap();
        assert!(matches!(deserialized, FormResponse::Cancelled));
    }

    #[test]
    fn test_interaction_request_serde_roundtrip() {
        let request = InteractionRequest {
            title: "Configure Authentication".to_string(),
            questions: vec![Question {
                id: "auth_method".to_string(),
                title: "Auth".to_string(),
                text: "Which authentication method?".to_string(),
                answer_type: AnswerType::SingleSelect {
                    options: vec![
                        AnswerOption {
                            value: "oauth2".to_string(),
                            label: "OAuth 2.0".to_string(),
                            description: Some("Industry standard".to_string()),
                        },
                        AnswerOption {
                            value: "jwt".to_string(),
                            label: "JWT".to_string(),
                            description: None,
                        },
                    ],
                },
            }],
        };

        let json = serde_json::to_string(&request).unwrap();
        let deserialized: InteractionRequest = serde_json::from_str(&json).unwrap();

        assert_eq!(deserialized.title, "Configure Authentication");
        assert_eq!(deserialized.questions.len(), 1);
        assert_eq!(deserialized.questions[0].id, "auth_method");
    }

    #[test]
    fn test_question_max_4_validation_in_docs() {
        // This test documents that we expect 1-4 questions
        // Actual validation happens in the ask_user tool
        let request = InteractionRequest {
            title: "Test".to_string(),
            questions: vec![
                Question {
                    id: "q1".to_string(),
                    title: "Q1".to_string(),
                    text: "Question 1".to_string(),
                    answer_type: AnswerType::YesNo { default: None },
                },
                Question {
                    id: "q2".to_string(),
                    title: "Q2".to_string(),
                    text: "Question 2".to_string(),
                    answer_type: AnswerType::YesNo { default: None },
                },
                Question {
                    id: "q3".to_string(),
                    title: "Q3".to_string(),
                    text: "Question 3".to_string(),
                    answer_type: AnswerType::YesNo { default: None },
                },
                Question {
                    id: "q4".to_string(),
                    title: "Q4".to_string(),
                    text: "Question 4".to_string(),
                    answer_type: AnswerType::YesNo { default: None },
                },
            ],
        };

        // Should serialize fine - validation is at tool level
        let json = serde_json::to_string(&request).unwrap();
        assert!(json.contains("q1"));
        assert!(json.contains("q4"));
    }
}
