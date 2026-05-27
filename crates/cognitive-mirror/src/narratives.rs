//! NarrativeHandler trait and alert-to-snippet conversion templates.

use async_trait::async_trait;
use jiff::Timestamp;
use uuid::Uuid;

use common::{Result, MIRROR_ALERT_COST_THRESHOLD_CROSSED};

use crate::{
    MetaRule, MirrorAlert, MirrorAlertSeverity, MirrorAlertType, NarrativeContext,
    NarrativeSnippet, SuggestedAction,
};

/// Generated components of a narrative output from an LLM call.
pub use crate::GeneratedNarrative;

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
// MetaRuleProposer trait
// ---------------------------------------------------------------------------

/// Context provided to a [`MetaRuleProposer`] for proposing new meta-rules.
#[derive(Debug, Clone)]
pub struct MetaRuleProposalContext {
    pub correction_history: Vec<String>,
    pub affected_skill: Option<String>,
    pub pattern_description: String,
}

/// Proposes new meta-rules based on observed correction and routing patterns.
#[async_trait]
pub trait MetaRuleProposer: Send + Sync {
    /// Analyse the provided context and optionally propose a new [`MetaRule`].
    ///
    /// Returns `Ok(None)` if no rule is warranted by the current signals.
    async fn propose_meta_rule(&self, context: MetaRuleProposalContext)
        -> Result<Option<MetaRule>>;
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
                created_at: Timestamp::now(),
                alert_type: MirrorAlertType::RoutingDrift,
                headline,
                body,
                suggested_action: Some(SuggestedAction::BoostSkill {
                    skill: skill.clone(),
                }),
                user_feedback: None,
                dismissed_at: None,
                coding_alert_kind: None,
                coding_alert_severity: None,
            }
        }
        MirrorAlert::TrialUnpromising { trial_id, reason } => NarrativeSnippet {
            id: Uuid::new_v4(),
            created_at: Timestamp::now(),
            alert_type: MirrorAlertType::TrialUnpromising,
            headline: "An experiment isn't looking great".to_string(),
            body: format!(
                "After 4 hours, this experiment is {}. Want to kill it early or let it finish?",
                reason
            ),
            suggested_action: Some(SuggestedAction::KillTrial {
                trial_id: trial_id.clone(),
            }),
            user_feedback: None,
            dismissed_at: None,
            coding_alert_kind: None,
            coding_alert_severity: None,
        },
        MirrorAlert::MetaRuleProposed {
            rule_id,
            rule_text,
            source: _,
        } => NarrativeSnippet {
            id: Uuid::new_v4(),
            created_at: Timestamp::now(),
            alert_type: MirrorAlertType::MetaRuleProposed,
            headline: "I learned something about how I think".to_string(),
            body: format!(
                "Based on recent patterns, I think I should: \"{}\". Does this sound right?",
                rule_text
            ),
            suggested_action: Some(SuggestedAction::ApproveMetaRule { rule_id: *rule_id }),
            user_feedback: None,
            dismissed_at: None,
            coding_alert_kind: None,
            coding_alert_severity: None,
        },
        MirrorAlert::Coding {
            kind,
            severity,
            payload,
        } => NarrativeSnippet {
            id: Uuid::new_v4(),
            created_at: Timestamp::now(),
            alert_type: MirrorAlertType::Coding,
            headline: format!("Coding alert: {kind}"),
            body: payload.to_string(),
            suggested_action: Some(SuggestedAction::ViewDetails),
            user_feedback: None,
            dismissed_at: None,
            coding_alert_kind: Some(kind.clone()),
            coding_alert_severity: Some(severity.clone()),
        },
        MirrorAlert::CostThresholdCrossed {
            session_key,
            spend_usd,
            ceiling_usd,
            percent,
        } => NarrativeSnippet {
            id: Uuid::new_v4(),
            created_at: Timestamp::now(),
            alert_type: MirrorAlertType::Coding,
            headline: format!("Cost alert: ${spend_usd:.2} / ${ceiling_usd:.2}"),
            body: format!(
                "Session {session_key} has reached {percent:.0}% of its cost ceiling (${ceiling_usd:.2})."
            ),
            suggested_action: Some(SuggestedAction::ViewDetails),
            user_feedback: None,
            dismissed_at: None,
            coding_alert_kind: Some(MIRROR_ALERT_COST_THRESHOLD_CROSSED.into()),
            coding_alert_severity: Some(MirrorAlertSeverity::Medium),
        },
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::MetaRuleSource;

    #[test]
    fn test_snippet_from_routing_drift() {
        let alert = MirrorAlert::RoutingDrift {
            skill: "task-management".to_string(),
            delta: 18.5,
            suggestion: "strengthen task routing".to_string(),
        };
        let snippet = snippet_from_alert(&alert);
        assert!(snippet.headline.contains("task-management"));
        assert_eq!(snippet.alert_type, MirrorAlertType::RoutingDrift);
        assert!(snippet.suggested_action.is_some());
        // Verify the suggested action is BoostSkill for the right skill
        match snippet.suggested_action.unwrap() {
            SuggestedAction::BoostSkill { skill } => {
                assert_eq!(skill, "task-management");
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

    #[test]
    fn test_snippet_from_meta_rule_proposed() {
        let rule_id = Uuid::new_v4();
        let alert = MirrorAlert::MetaRuleProposed {
            rule_id,
            rule_text: "When user corrects me twice about tasks, ask for clarification".to_string(),
            source: MetaRuleSource::CorrectionDerived,
        };
        let snippet = snippet_from_alert(&alert);
        assert_eq!(snippet.alert_type, MirrorAlertType::MetaRuleProposed);
        assert!(snippet.headline.contains("learned something"));
        assert!(snippet.body.contains("tasks"));
        assert!(snippet.suggested_action.is_some());
    }
}
