//! Confidence evaluator: parse, assess, decide.

use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::Arc;

use chrono::Utc;
use tracing::debug;

use super::types::{
    AssessmentPhase, ConfidenceAssessment, ConfidenceDimensions, DecisionAction, RawConfidenceBlock,
};

/// Evaluates LLM confidence and decides whether to proceed, clarify, or skip.
///
/// The threshold is stored as `Arc<AtomicU32>` (f32 bits) so the background
/// `LearningService` can update it without locks on the hot path.
pub struct ConfidenceEvaluator {
    threshold: Arc<AtomicU32>,
}

impl ConfidenceEvaluator {
    /// Create a new evaluator with the given threshold.
    pub fn new(threshold: f32) -> Self {
        Self {
            threshold: Arc::new(AtomicU32::new(threshold.clamp(0.0, 1.0).to_bits())),
        }
    }

    /// Get the current threshold (lock-free read).
    pub fn threshold(&self) -> f32 {
        f32::from_bits(self.threshold.load(Ordering::SeqCst))
    }

    /// Get a handle to the atomic threshold for external updates
    /// (used by `LearningService` to adjust the threshold).
    pub fn threshold_handle(&self) -> Arc<AtomicU32> {
        Arc::clone(&self.threshold)
    }

    /// Parse a `<confidence>` JSON block from LLM response content.
    ///
    /// Returns `None` if no block is found or parsing fails.
    pub fn parse_assessment(&self, content: &str) -> Option<ConfidenceAssessment> {
        let block = extract_confidence_block(content)?;
        let raw: RawConfidenceBlock = serde_json::from_str(&block).ok()?;

        let mut assessment = ConfidenceAssessment {
            score: raw.score.clamp(0.0, 1.0),
            phase: AssessmentPhase::PreTool,
            reasoning: raw.reasoning,
            dimensions: ConfidenceDimensions {
                intent_clarity: raw.intent_clarity.clamp(0.0, 1.0),
                tool_fit: raw.tool_fit.clamp(0.0, 1.0),
                info_sufficiency: raw.info_sufficiency.clamp(0.0, 1.0),
            },
            action: DecisionAction::default(),
            assessed_at: Utc::now(),
        };

        assessment.action = self.decide(&assessment);
        debug!(
            score = assessment.score,
            threshold = self.threshold(),
            "Parsed confidence assessment"
        );
        Some(assessment)
    }

    /// Decide action based on assessment score and threshold.
    pub fn decide(&self, assessment: &ConfidenceAssessment) -> DecisionAction {
        let threshold = self.threshold();
        if assessment.score >= threshold {
            DecisionAction::Proceed
        } else {
            // Build clarification questions from low-scoring dimensions
            let mut questions = Vec::new();

            if assessment.dimensions.intent_clarity < threshold {
                questions.push(
                    "Could you clarify what you'd like me to do? I want to make sure I understand your intent correctly.".to_string(),
                );
            }

            if assessment.dimensions.info_sufficiency < threshold {
                questions.push(
                    "I may be missing some information needed to complete this. Could you provide more details?".to_string(),
                );
            }

            if assessment.dimensions.tool_fit < threshold {
                questions.push(
                    "I'm not fully confident this is the best approach. Would you like me to try a different method?".to_string(),
                );
            }

            if questions.is_empty() {
                questions.push(
                    "I'm not fully confident about this request. Could you provide more context?"
                        .to_string(),
                );
            }

            DecisionAction::Clarify { questions }
        }
    }
}

/// Extract the JSON content between `<confidence>` tags.
fn extract_confidence_block(content: &str) -> Option<String> {
    let start_tag = "<confidence>";
    let end_tag = "</confidence>";

    let start = content.find(start_tag)? + start_tag.len();
    let end = content[start..].find(end_tag)? + start;

    let block = content[start..end].trim();
    if block.is_empty() {
        return None;
    }

    Some(block.to_string())
}

/// Strip all `<confidence>...</confidence>` blocks from content.
///
/// Returns cleaned content suitable for user display.
pub fn strip_confidence_blocks(content: &str) -> String {
    let mut result = content.to_string();
    while let (Some(start), Some(_)) = (result.find("<confidence>"), result.find("</confidence>")) {
        let end = result.find("</confidence>").unwrap() + "</confidence>".len();
        result.replace_range(start..end, "");
    }
    // Clean up leftover blank lines
    while result.contains("\n\n\n") {
        result = result.replace("\n\n\n", "\n\n");
    }
    result.trim().to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_extract_confidence_block() {
        let content = r#"I'll read that file for you.
<confidence>
{"score": 0.9, "intent_clarity": 0.95, "tool_fit": 0.85, "info_sufficiency": 0.9, "reasoning": "clear request"}
</confidence>"#;

        let block = extract_confidence_block(content).unwrap();
        assert!(block.contains("0.9"));
    }

    #[test]
    fn test_extract_no_block() {
        assert!(extract_confidence_block("no confidence here").is_none());
    }

    #[test]
    fn test_parse_assessment_high_confidence() {
        let evaluator = ConfidenceEvaluator::new(0.7);
        let content = r#"<confidence>
{"score": 0.85, "intent_clarity": 0.9, "tool_fit": 0.8, "info_sufficiency": 0.85, "reasoning": "User wants to read a file"}
</confidence>"#;

        let assessment = evaluator.parse_assessment(content).unwrap();
        assert!((assessment.score - 0.85).abs() < f32::EPSILON);
        assert!(matches!(assessment.action, DecisionAction::Proceed));
    }

    #[test]
    fn test_parse_assessment_low_confidence() {
        let evaluator = ConfidenceEvaluator::new(0.7);
        let content = r#"<confidence>
{"score": 0.3, "intent_clarity": 0.2, "tool_fit": 0.4, "info_sufficiency": 0.3, "reasoning": "Unclear request"}
</confidence>"#;

        let assessment = evaluator.parse_assessment(content).unwrap();
        assert!(matches!(assessment.action, DecisionAction::Clarify { .. }));
    }

    #[test]
    fn test_strip_confidence_blocks() {
        let content = "Hello\n<confidence>\n{\"score\":0.9}\n</confidence>\nWorld";
        let cleaned = strip_confidence_blocks(content);
        assert_eq!(cleaned, "Hello\n\nWorld");
        assert!(!cleaned.contains("confidence"));
    }

    #[test]
    fn test_scores_clamped() {
        let evaluator = ConfidenceEvaluator::new(0.7);
        let content = r#"<confidence>
{"score": 1.5, "intent_clarity": -0.1, "tool_fit": 2.0, "info_sufficiency": 0.5, "reasoning": "out of range"}
</confidence>"#;

        let assessment = evaluator.parse_assessment(content).unwrap();
        assert!((assessment.score - 1.0).abs() < f32::EPSILON);
        assert!((assessment.dimensions.intent_clarity - 0.0).abs() < f32::EPSILON);
        assert!((assessment.dimensions.tool_fit - 1.0).abs() < f32::EPSILON);
    }

    #[test]
    fn test_threshold_handle_allows_external_update() {
        let evaluator = ConfidenceEvaluator::new(0.65);
        let handle = evaluator.threshold_handle();
        handle.store(0.72_f32.to_bits(), std::sync::atomic::Ordering::SeqCst);
        assert!(
            (evaluator.threshold() - 0.72).abs() < f32::EPSILON,
            "evaluator must reflect the externally updated threshold"
        );
    }

    #[test]
    fn test_threshold_clamped_to_valid_range() {
        let below = ConfidenceEvaluator::new(-0.5);
        assert!((below.threshold() - 0.0).abs() < f32::EPSILON);
        let above = ConfidenceEvaluator::new(1.5);
        assert!((above.threshold() - 1.0).abs() < f32::EPSILON);
    }

    #[test]
    fn test_decide_threshold_boundary() {
        let evaluator = ConfidenceEvaluator::new(0.7);

        let high = ConfidenceAssessment {
            score: 0.7,
            phase: AssessmentPhase::PreTool,
            reasoning: String::new(),
            dimensions: ConfidenceDimensions {
                intent_clarity: 0.7,
                tool_fit: 0.7,
                info_sufficiency: 0.7,
            },
            action: DecisionAction::default(),
            assessed_at: Utc::now(),
        };
        assert!(matches!(evaluator.decide(&high), DecisionAction::Proceed));

        let low = ConfidenceAssessment {
            score: 0.69,
            ..high
        };
        assert!(matches!(
            evaluator.decide(&low),
            DecisionAction::Clarify { .. }
        ));
    }
}
