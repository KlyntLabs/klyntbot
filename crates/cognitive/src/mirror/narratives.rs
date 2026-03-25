//! NarrativeHandler trait and alert-to-snippet conversion templates.

use async_trait::async_trait;
use chrono::Utc;
use uuid::Uuid;

use common::Result;

use crate::mirror::{
    MirrorAlert, MirrorAlertType, NarrativeContext, NarrativeSnippet, SuggestedAction,
};

/// Generated components of a narrative output from an LLM call.
pub use crate::mirror::GeneratedNarrative;

// ---------------------------------------------------------------------------
// NarrativeHandler trait
// ---------------------------------------------------------------------------

/// Handles LLM-based narrative generation for the Mirror self-reflection layer.
#[async_trait]
pub trait NarrativeHandler: Send + Sync {
    /// Generate a full trend narrative from accumulated routing context.
    async fn generate_narrative(&self, ctx: NarrativeContext) -> Result<GeneratedNarrative>;

    /// Answer a user's direct mirror query with focused context.
    async fn generate_mirror_response(&self, query: &str, ctx: NarrativeContext) -> Result<String>;
}

// ---------------------------------------------------------------------------
// snippet_from_alert — deterministic template conversion
// ---------------------------------------------------------------------------

/// Convert a [`MirrorAlert`] into a user-facing [`NarrativeSnippet`] card.
///
/// These snippets are displayed in the Mirror UI without requiring an LLM call.
/// Each alert variant maps to a specific headline template and suggested action.
pub fn snippet_from_alert(alert: &MirrorAlert) -> NarrativeSnippet {
    match alert {
        MirrorAlert::RoutingDrift {
            skill,
            delta,
            suggestion,
        } => {
            let headline = format!("I'm routing more to {skill} lately");
            let body = format!(
                "Your usage of the '{skill}' skill has shifted by {delta:.1}pp compared to your \
                 7-day average. {suggestion}. This may reflect a change in your focus or needs \
                 — you can adjust skill preferences in settings."
            );
            NarrativeSnippet {
                id: Uuid::new_v4(),
                created_at: Utc::now(),
                alert_type: MirrorAlertType::RoutingDrift,
                headline,
                body,
                suggested_action: Some(SuggestedAction::BoostSkill {
                    skill: skill.clone(),
                }),
                user_feedback: None,
                dismissed_at: None,
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_snippet_from_routing_drift() {
        let alert = MirrorAlert::RoutingDrift {
            skill: "finance-management".to_string(),
            delta: 18.5,
            suggestion: "strengthen finance routing".to_string(),
        };
        let snippet = snippet_from_alert(&alert);
        assert!(snippet.headline.contains("finance-management"));
        assert_eq!(snippet.alert_type, MirrorAlertType::RoutingDrift);
        assert!(snippet.suggested_action.is_some());
        // Verify the suggested action is BoostSkill for the right skill
        match snippet.suggested_action.unwrap() {
            SuggestedAction::BoostSkill { skill } => {
                assert_eq!(skill, "finance-management");
            }
            other => panic!("Expected BoostSkill, got {other:?}"),
        }
    }

    #[test]
    fn test_snippet_body_contains_delta() {
        let alert = MirrorAlert::RoutingDrift {
            skill: "general".to_string(),
            delta: 22.3,
            suggestion: "review routing config".to_string(),
        };
        let snippet = snippet_from_alert(&alert);
        assert!(snippet.body.contains("22.3pp"));
        assert!(snippet.body.contains("general"));
    }
}
