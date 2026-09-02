//! Language practice tool — start/end practice sessions and get feedback.

use async_trait::async_trait;
use serde_json::Value;

use common::{Result, ToolError};
use tools_core::{approval_class::ApprovalClass, RoutingContext, Tool};

#[derive(Default)]
pub struct LanguagePracticeTool;

impl LanguagePracticeTool {
    pub fn new() -> Self {
        Self
    }
}

#[async_trait]
impl Tool for LanguagePracticeTool {
    fn name(&self) -> &str {
        "language_practice"
    }

    fn exposure_policy(&self) -> tools_core::ExposurePolicy {
        tools_core::ExposurePolicy {
            mcp: tools_core::McpExposure::Default,
            ..Default::default()
        }
    }

    fn description(&self) -> &str {
        "Language learning practice tool. Actions: start_session (begin a pronunciation practice session), end_session (finish and get summary), get_feedback (retrieve pronunciation feedback for the current session), get_weak_phonemes (list phonemes that need more practice), log_exam (record an exam attempt score)."
    }

    fn parameters(&self) -> Value {
        serde_json::json!({
            "type": "object",
            "properties": {
                "action": {
                    "type": "string",
                    "enum": ["start_session", "end_session", "get_feedback", "get_weak_phonemes", "log_exam"],
                    "description": "Action to perform"
                },
                "language": {
                    "type": "string",
                    "description": "Target language code (e.g., 'en', 'zh')"
                },
                "exam_type": {
                    "type": "string",
                    "description": "Exam type for log_exam (e.g., 'IELTS', 'HSK')"
                },
                "score": {
                    "type": "number",
                    "description": "Exam score for log_exam"
                },
                "max_score": {
                    "type": "number",
                    "description": "Maximum possible score for log_exam"
                }
            },
            "required": ["action"]
        })
    }

    async fn execute(&self, params: Value, _ctx: &RoutingContext) -> Result<String> {
        let action = params["action"]
            .as_str()
            .ok_or_else(|| ToolError::InvalidParams("missing 'action'".to_string()))?;

        let result = match action {
            "start_session" => {
                let lang = params
                    .get("language")
                    .and_then(|v| v.as_str())
                    .unwrap_or("en");
                serde_json::json!({
                    "status": "session_started",
                    "language": lang,
                    "message": format!("Pronunciation practice session started for {lang}. Speak naturally and I'll provide feedback after each turn.")
                })
            }
            "end_session" => serde_json::json!({
                "status": "session_ended",
                "message": "Practice session ended. Great work!"
            }),
            "get_feedback" => {
                // TODO: Query pronunciation_logs for the current session
                serde_json::json!({
                    "status": "no_feedback",
                    "message": "No pronunciation data available yet. Start speaking to receive feedback."
                })
            }
            "get_weak_phonemes" => {
                // TODO: Query phoneme_mastery for low-stability phonemes
                serde_json::json!({
                    "weak_phonemes": [],
                    "message": "No weak phonemes tracked yet. Keep practicing!"
                })
            }
            "log_exam" => {
                let exam_type = params["exam_type"]
                    .as_str()
                    .ok_or_else(|| ToolError::InvalidParams("missing 'exam_type'".to_string()))?;
                let score = params.get("score").and_then(|v| v.as_f64()).unwrap_or(0.0);
                serde_json::json!({
                    "status": "exam_logged",
                    "exam_type": exam_type,
                    "score": score,
                    "message": format!("Recorded {exam_type} score: {score}")
                })
            }
            other => serde_json::json!({
                "error": format!("Unknown action: {other}")
            }),
        };

        Ok(serde_json::to_string(&result)?)
    }

    fn approval_class(&self, args: &Value) -> ApprovalClass {
        match args.get("action").and_then(|v| v.as_str()) {
            Some("start_session" | "end_session" | "log_exam") => ApprovalClass::Sensitive,
            _ => ApprovalClass::Safe,
        }
    }
}
