//! Coaching message templates for learning patterns.
//!
//! 70% celebration / 30% gentle correction tone.

use crate::reasoner::InterventionType;

pub struct LearningMessage {
    pub message: String,
    pub intervention_type: InterventionType,
}

/// Generate a coaching message for a learning pattern.
pub fn learning_message(pattern_name: &str, description: &str) -> LearningMessage {
    match pattern_name {
        // Celebrations (70%)
        p if p.starts_with("study_streak_") && !p.contains("risk") => LearningMessage {
            message: format!("\u{1f525} {description} You're building real knowledge depth."),
            intervention_type: InterventionType::DashboardCard,
        },
        "learning_momentum_review_strong" => LearningMessage {
            message: format!("\u{1f4aa} {description} Your retention is rock-solid."),
            intervention_type: InterventionType::DashboardCard,
        },
        "knowledge_transfer_detected" => LearningMessage {
            message: format!("\u{1f9e0} {description}"),
            intervention_type: InterventionType::ChatMessage,
        },

        // Gentle nudges (30%)
        "study_streak_at_risk" => LearningMessage {
            message: format!("\u{1f4da} {description}"),
            intervention_type: InterventionType::ChatMessage,
        },
        "high_importance_retention_decay" => LearningMessage {
            message: format!("\u{1f514} {description}"),
            intervention_type: InterventionType::ChatMessage,
        },
        "learning_momentum_create_heavy" => LearningMessage {
            message: format!("\u{1f4d6} {description}"),
            intervention_type: InterventionType::DashboardCard,
        },
        "domain_retention_gap" => LearningMessage {
            message: format!("\u{1f4c9} {description}"),
            intervention_type: InterventionType::DashboardCard,
        },

        // Fallback
        _ => LearningMessage {
            message: description.to_string(),
            intervention_type: InterventionType::DashboardCard,
        },
    }
}
