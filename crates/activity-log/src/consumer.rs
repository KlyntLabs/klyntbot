//! NormalizerSignalConsumer — converts `AiSignal` into `ActivityLogEntry`.
//!
//! Replaces the per-variant `DomainEvent` match in `normalizers.rs` with
//! field-driven construction from the signal itself. Registered with
//! `SignalRouter` in `app-core::init`.

use std::sync::Arc;

use ai_core::{AiSignal, SignalConsumer};
use async_trait::async_trait;

use crate::normalizers::content_hash;
use crate::service::ActivityIngestionService;
use crate::types::{ActivityActor, ActivityLogEntry, ActivitySource};

pub struct NormalizerSignalConsumer {
    service: Arc<ActivityIngestionService>,
}

impl NormalizerSignalConsumer {
    pub fn new(service: Arc<ActivityIngestionService>) -> Self {
        Self { service }
    }
}

#[async_trait]
impl SignalConsumer for NormalizerSignalConsumer {
    fn name(&self) -> &'static str {
        "activity-log-normalizer"
    }

    async fn consume(&self, signal: &AiSignal) -> common::Result<()> {
        let entry = signal_to_entry(signal);
        self.service.ingest(entry).await?;
        Ok(())
    }
}

/// Build an `ActivityLogEntry` from an `AiSignal`.
pub fn signal_to_entry(signal: &AiSignal) -> ActivityLogEntry {
    let preview = if signal.content.is_empty() {
        None
    } else {
        Some(signal.content.clone())
    };

    let content_hash = preview.as_ref().map(|c| content_hash(c));

    ActivityLogEntry {
        id: crate::normalizers::new_ulid(),
        timestamp: signal.timestamp,
        source: source_from_domain(signal.domain),
        actor: actor_from_kind(signal.event_kind),
        resource_type: signal.entity.as_ref().map(|e| e.entity_type.to_string()),
        resource_id: signal.entity.as_ref().map(|e| e.id.clone()),
        resource_name: None,
        action: action_from_kind(signal.event_kind).to_string(),
        content_preview: preview,
        content_hash,
        metadata: classify_metadata(signal),
        app_name: None,
        project_id: None,
        work_context_id: None,
        embedding_id: None,
        duration_secs: None,
        session_key: None,
        is_sensitive: false,
    }
}

fn source_from_domain(domain: ai_core::RecallDomain) -> ActivitySource {
    use ai_core::RecallDomain;
    match domain {
        RecallDomain::Tasks => ActivitySource::Task,
        RecallDomain::Notes => ActivitySource::Note,
        RecallDomain::Productivity => ActivitySource::FocusSession,
        RecallDomain::General => ActivitySource::DomainEvent,
        _ => ActivitySource::DomainEvent,
    }
}

fn actor_from_kind(kind: &str) -> ActivityActor {
    match kind {
        "ProductivityScoreComputed"
        | "ActivitySessionCompleted"
        | "BudgetAlert"
        | "AutotunerDecision"
        | "AtomRetentionDecayed"
        | "NotificationDeliveryFailed"
        | "MissedAlarms" => ActivityActor::System,
        _ => ActivityActor::User,
    }
}

fn action_from_kind(kind: &str) -> &'static str {
    // Exact matches first — these would otherwise be caught by suffix rules.
    if kind == "ChatTurnCompleted" {
        return "prompt";
    }
    if kind == "ToolCallExecuted" {
        return "execute";
    }
    if kind == "UserStatedFact" || kind == "UserCorrectedAI" {
        return "state";
    }
    if kind == "CoachingFeedback" || kind == "CoachingPatternDetected" {
        return "coach";
    }
    if kind == "CrossDomainDotReady" {
        return "ready";
    }

    if kind.ends_with("Created") || kind.ends_with("Extracted") {
        "create"
    } else if kind.ends_with("Completed") || kind.ends_with("Recorded") {
        "complete"
    } else if kind.ends_with("Updated") || kind.ends_with("Edited") {
        "edit"
    } else if kind.ends_with("Deleted") || kind.ends_with("Archived") {
        "delete"
    } else if kind.ends_with("Started") || kind.ends_with("Ready") {
        "start"
    } else if kind.ends_with("Ended") || kind.ends_with("Closed") {
        "end"
    } else if kind.ends_with("Detected") || kind.ends_with("Discovered") {
        "detect"
    } else if kind.ends_with("Scored") || kind.ends_with("Computed") {
        "compute"
    } else if kind.ends_with("Deferred") || kind.ends_with("Snoozed") {
        "defer"
    } else if kind.ends_with("Fired") || kind.ends_with("Triggered") {
        "trigger"
    } else if kind.ends_with("Cancelled") {
        "cancel"
    } else {
        "action"
    }
}

fn classify_metadata(signal: &AiSignal) -> Option<serde_json::Value> {
    let mut map = serde_json::Map::new();
    if let Some(ref app) = signal.metrics.app {
        map.insert("app".into(), app.clone().into());
    }
    if let Some(amount) = signal.metrics.amount {
        map.insert("amount".into(), amount.into());
    }
    if let Some(ref category) = signal.metrics.category {
        map.insert("category".into(), category.clone().into());
    }
    if !map.is_empty() {
        Some(serde_json::Value::Object(map))
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ai_core::{RecallDomain, SalienceVerdict};
    use jiff::Timestamp;

    fn test_signal(kind: &'static str, domain: RecallDomain, content: &str) -> AiSignal {
        AiSignal {
            domain,
            event_kind: kind,
            importance: 0.5,
            salience: SalienceVerdict::Accumulate,
            content: content.into(),
            entity: None,
            timestamp: Timestamp::now(),
            raw_event: None,
            metrics: ai_core::AiMetrics::default(),
            coaching_signal: false,
            coaching_rule: None,
            metric_samples: Vec::new(),
        }
    }

    fn test_signal_with_entity(
        kind: &'static str,
        domain: RecallDomain,
        content: &str,
        entity_type: &'static str,
        id: &str,
    ) -> AiSignal {
        AiSignal {
            domain,
            event_kind: kind,
            importance: 0.5,
            salience: SalienceVerdict::Accumulate,
            content: content.into(),
            entity: Some(ai_core::EntityRef {
                entity_type,
                id: id.into(),
                name: String::new(),
            }),
            timestamp: Timestamp::now(),
            raw_event: None,
            metrics: ai_core::AiMetrics::default(),
            coaching_signal: false,
            coaching_rule: None,
            metric_samples: Vec::new(),
        }
    }

    #[test]
    fn task_created_maps_correctly() {
        let sig = test_signal_with_entity(
            "TaskCreated",
            RecallDomain::Tasks,
            "Task created",
            "task",
            "t-123",
        );
        let entry = signal_to_entry(&sig);
        assert_eq!(entry.source, ActivitySource::Task);
        assert_eq!(entry.action, "create");
        assert_eq!(entry.resource_id.as_deref(), Some("t-123"));
        assert_eq!(entry.actor, ActivityActor::User);
    }

    #[test]
    fn focus_session_started_maps_correctly() {
        let sig = test_signal_with_entity(
            "FocusSessionStarted",
            RecallDomain::Productivity,
            "Focus started",
            "focus_session",
            "fs-1",
        );
        let entry = signal_to_entry(&sig);
        assert_eq!(entry.source, ActivitySource::FocusSession);
        assert_eq!(entry.action, "start");
    }

    #[test]
    fn note_created_maps_correctly() {
        let sig = test_signal_with_entity(
            "NoteCreated",
            RecallDomain::Notes,
            "Note created",
            "note",
            "n-1",
        );
        let entry = signal_to_entry(&sig);
        assert_eq!(entry.source, ActivitySource::Note);
        assert_eq!(entry.action, "create");
        assert_eq!(entry.resource_id.as_deref(), Some("n-1"));
    }

    #[test]
    fn system_events_use_system_actor() {
        let sig = test_signal(
            "ProductivityScoreComputed",
            RecallDomain::Productivity,
            "Score computed",
        );
        let entry = signal_to_entry(&sig);
        assert_eq!(entry.actor, ActivityActor::System);
        assert_eq!(entry.action, "compute");
    }

    #[test]
    fn action_suffixes_are_derived() {
        assert_eq!(action_from_kind("TaskCompleted"), "complete");
        assert_eq!(action_from_kind("NoteUpdated"), "edit");
        assert_eq!(action_from_kind("NoteDeleted"), "delete");
        assert_eq!(action_from_kind("FocusSessionStarted"), "start");
        assert_eq!(action_from_kind("FocusSessionEnded"), "end");
        assert_eq!(action_from_kind("DistractionDetected"), "detect");
        assert_eq!(action_from_kind("AlarmFired"), "trigger");
        assert_eq!(action_from_kind("ChatTurnCompleted"), "prompt");
        assert_eq!(action_from_kind("TaskCreated"), "create");
    }

    #[test]
    fn content_hash_is_populated() {
        let sig = test_signal("TaskCreated", RecallDomain::Tasks, "some content");
        let entry = signal_to_entry(&sig);
        assert!(entry.content_hash.is_some());
    }

    #[test]
    fn empty_content_has_no_preview() {
        let sig = test_signal("TaskCreated", RecallDomain::Tasks, "");
        let entry = signal_to_entry(&sig);
        assert!(entry.content_preview.is_none());
        assert!(entry.content_hash.is_none());
    }
}
