use chrono::Utc;
use sha2::{Digest, Sha256};
use ulid::Ulid;

use crate::types::{ActivityActor, ActivityLogEntry, ActivitySource, MAX_PREVIEW_LEN};

/// Trait for converting domain-specific events into unified ActivityLogEntry.
pub trait ActivityNormalizer: Send + Sync {
    fn normalize(&self, input: &dyn std::any::Any) -> Option<ActivityLogEntry>;
}

/// Helper to generate a new ULID string.
pub fn new_ulid() -> String {
    Ulid::new().to_string()
}

/// Parse an RFC3339 timestamp string, falling back to `Utc::now()` on failure.
pub fn parse_rfc3339(s: &str) -> chrono::DateTime<Utc> {
    chrono::DateTime::parse_from_rfc3339(s)
        .map(|dt| dt.with_timezone(&Utc))
        .unwrap_or_else(|_| Utc::now())
}

/// Compute SHA-256 hex digest for content dedup.
pub fn content_hash(content: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(content.as_bytes());
    format!("{:x}", hasher.finalize())
}

// ── DomainEvent normalizer ──────────────────────────────────────

pub struct DomainEventNormalizer;

impl ActivityNormalizer for DomainEventNormalizer {
    fn normalize(&self, input: &dyn std::any::Any) -> Option<ActivityLogEntry> {
        let event = input.downcast_ref::<bus::DomainEvent>()?;
        Some(normalize_domain_event(event))
    }
}

/// Normalize a DomainEvent directly (avoids Any trait for Send/Sync contexts).
pub fn normalize_domain_event(event: &bus::DomainEvent) -> ActivityLogEntry {
    let now = Utc::now();
    let (source, actor, action, resource_type, resource_id, resource_name, preview, metadata) =
        match event {
            // Tasks
            bus::DomainEvent::TaskCreated {
                task_id, project, ..
            } => (
                ActivitySource::Task,
                ActivityActor::User,
                "create",
                Some("task"),
                Some(task_id.as_str()),
                None,
                Some(format!("Task created: {task_id}")),
                project.as_ref().map(|p| serde_json::json!({"project": p})),
            ),
            bus::DomainEvent::TaskCompleted { task_id, .. } => (
                ActivitySource::Task,
                ActivityActor::User,
                "complete",
                Some("task"),
                Some(task_id.as_str()),
                None,
                Some(format!("Task completed: {task_id}")),
                None,
            ),
            bus::DomainEvent::TaskDeferred { task_id, .. } => (
                ActivitySource::Task,
                ActivityActor::User,
                "defer",
                Some("task"),
                Some(task_id.as_str()),
                None,
                Some(format!("Task deferred: {task_id}")),
                None,
            ),

            // Focus sessions
            bus::DomainEvent::FocusSessionStarted {
                session_type,
                target_mins,
            } => (
                ActivitySource::FocusSession,
                ActivityActor::User,
                "start",
                Some("focus_session"),
                None,
                Some(session_type.as_str()),
                Some(format!(
                    "Focus session started: {session_type} ({target_mins}m)"
                )),
                Some(serde_json::json!({"target_mins": target_mins})),
            ),
            bus::DomainEvent::FocusSessionEnded {
                duration_secs,
                quality,
                interruptions,
            } => (
                ActivitySource::FocusSession,
                ActivityActor::User,
                "end",
                Some("focus_session"),
                None,
                None,
                Some(format!(
                    "Focus session ended: {duration_secs}s, quality={quality:.1}"
                )),
                Some(serde_json::json!({"quality": quality, "interruptions": interruptions})),
            ),

            // Chat
            bus::DomainEvent::ChatTurnCompleted {
                user_message,
                session_key,
            } => (
                ActivitySource::Chat,
                ActivityActor::User,
                "prompt",
                Some("conversation"),
                None,
                None,
                Some(truncate(user_message, MAX_PREVIEW_LEN)),
                Some(serde_json::json!({"session_key": session_key})),
            ),

            // Notes
            bus::DomainEvent::NoteCreated { note_id, title } => (
                ActivitySource::Note,
                ActivityActor::User,
                "create",
                Some("note"),
                Some(note_id.as_str()),
                Some(title.as_str()),
                Some(format!("Note created: {title}")),
                None,
            ),
            bus::DomainEvent::NoteUpdated { note_id, title } => (
                ActivitySource::Note,
                ActivityActor::User,
                "edit",
                Some("note"),
                Some(note_id.as_str()),
                Some(title.as_str()),
                Some(format!("Note updated: {title}")),
                None,
            ),

            // Productivity
            bus::DomainEvent::ProductivityScoreComputed { date, score } => (
                ActivitySource::DomainEvent,
                ActivityActor::System,
                "compute",
                None,
                None,
                None,
                Some(format!("Productivity score: {score:.0} for {date}")),
                Some(serde_json::json!({"date": date, "score": score})),
            ),
            bus::DomainEvent::DistractionDetected {
                app,
                duration_secs,
                context,
            } => (
                ActivitySource::DomainEvent,
                ActivityActor::System,
                "detect",
                Some("app"),
                None,
                Some(app.as_str()),
                Some(format!("Distraction: {app} ({context})")),
                Some(serde_json::json!({"duration_secs": duration_secs})),
            ),
            bus::DomainEvent::ActivitySessionCompleted { date, .. } => (
                ActivitySource::DomainEvent,
                ActivityActor::System,
                "complete",
                None,
                None,
                None,
                Some(format!("Activity session completed: {date}")),
                Some(serde_json::to_value(event).ok()).flatten(),
            ),

            // Finance
            bus::DomainEvent::TransactionRecorded {
                category, amount, ..
            } => (
                ActivitySource::DomainEvent,
                ActivityActor::User,
                "record",
                Some("transaction"),
                None,
                Some(category.as_str()),
                Some(format!("Transaction: {category} ${amount:.2}")),
                None,
            ),
            bus::DomainEvent::BudgetAlert { category, .. } => (
                ActivitySource::DomainEvent,
                ActivityActor::System,
                "alert",
                None,
                None,
                Some(category.as_str()),
                Some(format!("Budget alert: {category}")),
                None,
            ),

            // Goals
            bus::DomainEvent::GoalProgress {
                objective_id,
                progress,
                target,
            } => (
                ActivitySource::Task,
                ActivityActor::User,
                "progress",
                Some("objective"),
                Some(objective_id.as_str()),
                None,
                Some(format!("Goal progress: {progress:.0}/{target:.0}")),
                None,
            ),

            // Tool calls
            bus::DomainEvent::ToolCallExecuted {
                tool_name,
                args_preview,
                session_key,
                duration_ms,
            } => {
                let preview = format!(
                    "Tool call: {}{}",
                    tool_name,
                    args_preview
                        .as_ref()
                        .map(|a| format!(" — {}", truncate(a, MAX_PREVIEW_LEN - 100)))
                        .unwrap_or_default()
                );
                return ActivityLogEntry {
                    id: new_ulid(),
                    timestamp: now,
                    source: ActivitySource::ToolCall,
                    actor: ActivityActor::AiAgent,
                    resource_type: Some("command".to_string()),
                    resource_id: None,
                    resource_name: Some(tool_name.clone()),
                    action: "run".to_string(),
                    content_preview: Some(truncate(&preview, MAX_PREVIEW_LEN)),
                    content_hash: Some(content_hash(&preview)),
                    metadata: None,
                    app_name: None,
                    project_id: None,
                    work_context_id: None,
                    embedding_id: None,
                    duration_secs: duration_ms.map(|ms| ms / 1000),
                    session_key: session_key.clone(),
                    is_sensitive: false,
                };
            }

            // Knowledge Atoms
            bus::DomainEvent::KnowledgeAtomCreated {
                atom_type, domain, ..
            } => (
                ActivitySource::DomainEvent,
                ActivityActor::System,
                "atom_created",
                Some("atom"),
                None,
                None,
                Some(format!("Atom created: {atom_type} in {domain}")),
                serde_json::to_value(event).ok(),
            ),
            bus::DomainEvent::KnowledgeAtomAccepted { atom_id, .. } => (
                ActivitySource::DomainEvent,
                ActivityActor::User,
                "atom_accepted",
                Some("atom"),
                Some(atom_id.as_str()),
                None,
                Some(format!("Atom accepted: {atom_id}")),
                None,
            ),
            bus::DomainEvent::KnowledgeAtomArchived { atom_id, reason } => (
                ActivitySource::DomainEvent,
                ActivityActor::User,
                "atom_archived",
                Some("atom"),
                Some(atom_id.as_str()),
                None,
                Some(format!("Atom archived: {reason}")),
                None,
            ),
            bus::DomainEvent::AtomFlashcardReviewed {
                atom_id,
                quality,
                new_retention_pct,
                ..
            } => (
                ActivitySource::DomainEvent,
                ActivityActor::User,
                "flashcard_reviewed",
                Some("atom"),
                Some(atom_id.as_str()),
                None,
                Some(format!(
                    "Flashcard reviewed: q={quality} retention={new_retention_pct:.0}%"
                )),
                serde_json::to_value(event).ok(),
            ),
            bus::DomainEvent::TranslationCompleted {
                note_id,
                target_lang,
                word_count,
                ..
            } => (
                ActivitySource::DomainEvent,
                ActivityActor::User,
                "translation_completed",
                Some("note"),
                Some(note_id.as_str()),
                None,
                Some(format!(
                    "Translation to {target_lang}: {word_count} words"
                )),
                serde_json::to_value(event).ok(),
            ),
            bus::DomainEvent::NoteStudied {
                note_id,
                duration_secs,
                atoms_reviewed,
                ..
            } => (
                ActivitySource::DomainEvent,
                ActivityActor::User,
                "note_studied",
                Some("note"),
                Some(note_id.as_str()),
                None,
                Some(format!(
                    "Note studied: {duration_secs}s, {atoms_reviewed} atoms"
                )),
                serde_json::to_value(event).ok(),
            ),

            // Catch-all for remaining variants
            _ => (
                ActivitySource::DomainEvent,
                ActivityActor::System,
                "event",
                None,
                None,
                None,
                Some(format!("{event:?}")),
                serde_json::to_value(event).ok(),
            ),
        };

    let preview_str = preview.map(|s| truncate(&s, MAX_PREVIEW_LEN));
    let hash = preview_str.as_ref().map(|p| content_hash(p));

    ActivityLogEntry {
        id: new_ulid(),
        timestamp: now,
        source,
        actor,
        resource_type: resource_type.map(String::from),
        resource_id: resource_id.map(String::from),
        resource_name: resource_name.map(String::from),
        action: action.to_string(),
        content_preview: preview_str,
        content_hash: hash,
        metadata,
        app_name: None,
        project_id: None,
        work_context_id: None,
        embedding_id: None,
        duration_secs: match event {
            bus::DomainEvent::FocusSessionEnded { duration_secs, .. } => Some(*duration_secs),
            _ => None,
        },
        session_key: match event {
            bus::DomainEvent::ChatTurnCompleted { session_key, .. } => Some(session_key.clone()),
            _ => None,
        },
        is_sensitive: false,
    }
}

// ── Chat message normalizer ─────────────────────────────────────

/// Input for ChatMessageNormalizer: (session_key, role, content)
pub struct ChatMessageInput {
    pub session_key: String,
    pub role: String,
    pub content: String,
}

pub struct ChatMessageNormalizer;

impl ActivityNormalizer for ChatMessageNormalizer {
    fn normalize(&self, input: &dyn std::any::Any) -> Option<ActivityLogEntry> {
        let msg = input.downcast_ref::<ChatMessageInput>()?;

        let (actor, action) = if msg.role == "user" {
            (ActivityActor::User, "prompt")
        } else {
            (ActivityActor::AiAgent, "reply")
        };

        let preview = truncate(&msg.content, MAX_PREVIEW_LEN);
        let hash = content_hash(&preview);

        Some(ActivityLogEntry {
            id: new_ulid(),
            timestamp: Utc::now(),
            source: ActivitySource::Chat,
            actor,
            resource_type: Some("conversation".to_string()),
            resource_id: None,
            resource_name: None,
            action: action.to_string(),
            content_preview: Some(preview),
            content_hash: Some(hash),
            metadata: None,
            app_name: None,
            project_id: None,
            work_context_id: None,
            embedding_id: None,
            duration_secs: None,
            session_key: Some(msg.session_key.clone()),
            is_sensitive: false,
        })
    }
}

// ── Tool call normalizer ────────────────────────────────────────

pub struct ToolCallInput {
    pub tool_name: String,
    pub args_preview: Option<String>,
    pub session_key: Option<String>,
    pub duration_ms: Option<i64>,
}

pub struct ToolCallNormalizer;

impl ActivityNormalizer for ToolCallNormalizer {
    fn normalize(&self, input: &dyn std::any::Any) -> Option<ActivityLogEntry> {
        let call = input.downcast_ref::<ToolCallInput>()?;

        let preview = format!(
            "Tool call: {}{}",
            call.tool_name,
            call.args_preview
                .as_ref()
                .map(|a| format!(" — {}", truncate(a, MAX_PREVIEW_LEN - 100)))
                .unwrap_or_default()
        );
        let hash = content_hash(&preview);
        let duration_secs = call.duration_ms.map(|ms| ms / 1000);

        Some(ActivityLogEntry {
            id: new_ulid(),
            timestamp: Utc::now(),
            source: ActivitySource::ToolCall,
            actor: ActivityActor::AiAgent,
            resource_type: Some("command".to_string()),
            resource_id: None,
            resource_name: Some(call.tool_name.clone()),
            action: "run".to_string(),
            content_preview: Some(preview),
            content_hash: Some(hash),
            metadata: None,
            app_name: None,
            project_id: None,
            work_context_id: None,
            embedding_id: None,
            duration_secs,
            session_key: call.session_key.clone(),
            is_sensitive: false,
        })
    }
}

// ── Window event normalizer ─────────────────────────────────────

/// Generic window event input — avoids depending on feature-productivity types.
pub struct WindowEventInput {
    pub app_name: String,
    pub window_title: Option<String>,
    pub url: Option<String>,
    pub started_at: chrono::DateTime<Utc>,
    pub duration_secs: Option<i64>,
    pub project_id: Option<String>,
    pub is_idle: bool,
}

pub struct WindowEventNormalizer;

impl ActivityNormalizer for WindowEventNormalizer {
    fn normalize(&self, input: &dyn std::any::Any) -> Option<ActivityLogEntry> {
        let evt = input.downcast_ref::<WindowEventInput>()?;

        if evt.is_idle {
            return None; // Skip idle events
        }

        let action = if evt.url.is_some() { "browse" } else { "view" };
        let preview = format!(
            "{}: {}",
            evt.app_name,
            evt.window_title.as_deref().unwrap_or("(untitled)")
        );
        let hash = content_hash(&format!(
            "{}:{}:{}",
            evt.app_name,
            evt.window_title.as_deref().unwrap_or(""),
            evt.started_at.to_rfc3339()
        ));

        let source = if evt.url.is_some() {
            ActivitySource::Browser
        } else {
            ActivitySource::OsWindow
        };

        Some(ActivityLogEntry {
            id: new_ulid(),
            timestamp: evt.started_at,
            source,
            actor: ActivityActor::User,
            resource_type: evt.url.as_ref().map(|_| "url".to_string()),
            resource_id: evt.url.clone(),
            resource_name: evt.window_title.clone(),
            action: action.to_string(),
            content_preview: Some(truncate(&preview, MAX_PREVIEW_LEN)),
            content_hash: Some(hash),
            metadata: None,
            app_name: Some(evt.app_name.clone()),
            project_id: evt.project_id.clone(),
            work_context_id: None,
            embedding_id: None,
            duration_secs: evt.duration_secs,
            session_key: None,
            is_sensitive: false,
        })
    }
}

fn truncate(s: &str, max: usize) -> String {
    if s.len() <= max {
        s.to_string()
    } else {
        s.chars().take(max).collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_domain_event_normalizer_task_created() {
        let normalizer = DomainEventNormalizer;
        let event = bus::DomainEvent::TaskCreated {
            task_id: "t-123".into(),
            project: Some("klyntbot".into()),
            estimate_mins: Some(30),
            task_type: "manual".into(),
        };
        let entry = normalizer.normalize(&event).unwrap();
        assert_eq!(entry.source, ActivitySource::Task);
        assert_eq!(entry.action, "create");
        assert_eq!(entry.resource_id.as_deref(), Some("t-123"));
        assert_eq!(entry.actor, ActivityActor::User);
    }

    #[test]
    fn test_domain_event_normalizer_chat_turn() {
        let normalizer = DomainEventNormalizer;
        let event = bus::DomainEvent::ChatTurnCompleted {
            user_message: "hello world".into(),
            session_key: "sk-1".into(),
        };
        let entry = normalizer.normalize(&event).unwrap();
        assert_eq!(entry.source, ActivitySource::Chat);
        assert_eq!(entry.action, "prompt");
        assert_eq!(entry.session_key.as_deref(), Some("sk-1"));
    }

    #[test]
    fn test_domain_event_normalizer_focus_session() {
        let normalizer = DomainEventNormalizer;
        let event = bus::DomainEvent::FocusSessionStarted {
            session_type: "deep_work".into(),
            target_mins: 25,
        };
        let entry = normalizer.normalize(&event).unwrap();
        assert_eq!(entry.source, ActivitySource::FocusSession);
        assert_eq!(entry.action, "start");
    }

    #[test]
    fn test_domain_event_normalizer_focus_session_ended() {
        let normalizer = DomainEventNormalizer;
        let event = bus::DomainEvent::FocusSessionEnded {
            duration_secs: 1500,
            quality: 0.85,
            interruptions: 2,
        };
        let entry = normalizer.normalize(&event).unwrap();
        assert_eq!(entry.source, ActivitySource::FocusSession);
        assert_eq!(entry.action, "end");
        assert_eq!(entry.duration_secs, Some(1500));
    }

    #[test]
    fn test_domain_event_normalizer_note_created() {
        let normalizer = DomainEventNormalizer;
        let event = bus::DomainEvent::NoteCreated {
            note_id: "n-1".into(),
            title: "My Note".into(),
        };
        let entry = normalizer.normalize(&event).unwrap();
        assert_eq!(entry.source, ActivitySource::Note);
        assert_eq!(entry.action, "create");
        assert_eq!(entry.resource_id.as_deref(), Some("n-1"));
    }

    #[test]
    fn test_chat_message_normalizer_user() {
        let normalizer = ChatMessageNormalizer;
        let input = ChatMessageInput {
            session_key: "sk-1".into(),
            role: "user".into(),
            content: "What is the weather?".into(),
        };
        let entry = normalizer.normalize(&input).unwrap();
        assert_eq!(entry.actor, ActivityActor::User);
        assert_eq!(entry.action, "prompt");
        assert_eq!(entry.session_key.as_deref(), Some("sk-1"));
    }

    #[test]
    fn test_chat_message_normalizer_assistant() {
        let normalizer = ChatMessageNormalizer;
        let input = ChatMessageInput {
            session_key: "sk-1".into(),
            role: "assistant".into(),
            content: "It's sunny today.".into(),
        };
        let entry = normalizer.normalize(&input).unwrap();
        assert_eq!(entry.actor, ActivityActor::AiAgent);
        assert_eq!(entry.action, "reply");
    }

    #[test]
    fn test_tool_call_normalizer() {
        let normalizer = ToolCallNormalizer;
        let input = ToolCallInput {
            tool_name: "task".into(),
            args_preview: Some(r#"{"action": "add"}"#.into()),
            session_key: Some("sk-1".into()),
            duration_ms: Some(150),
        };
        let entry = normalizer.normalize(&input).unwrap();
        assert_eq!(entry.source, ActivitySource::ToolCall);
        assert_eq!(entry.actor, ActivityActor::AiAgent);
        assert_eq!(entry.action, "run");
        assert_eq!(entry.resource_name.as_deref(), Some("task"));
        assert_eq!(entry.duration_secs, Some(0)); // 150ms / 1000 = 0
    }

    #[test]
    fn test_window_event_normalizer() {
        let normalizer = WindowEventNormalizer;
        let input = WindowEventInput {
            app_name: "Visual Studio Code".into(),
            window_title: Some("main.rs — klyntbot".into()),
            url: None,
            started_at: Utc::now(),
            duration_secs: Some(120),
            project_id: Some("proj-1".into()),
            is_idle: false,
        };
        let entry = normalizer.normalize(&input).unwrap();
        assert_eq!(entry.source, ActivitySource::OsWindow);
        assert_eq!(entry.actor, ActivityActor::User);
        assert_eq!(entry.action, "view");
        assert_eq!(entry.app_name.as_deref(), Some("Visual Studio Code"));
    }

    #[test]
    fn test_window_event_normalizer_browser() {
        let normalizer = WindowEventNormalizer;
        let input = WindowEventInput {
            app_name: "Google Chrome".into(),
            window_title: Some("GitHub".into()),
            url: Some("https://github.com".into()),
            started_at: Utc::now(),
            duration_secs: Some(60),
            project_id: None,
            is_idle: false,
        };
        let entry = normalizer.normalize(&input).unwrap();
        assert_eq!(entry.source, ActivitySource::Browser);
        assert_eq!(entry.action, "browse");
    }

    #[test]
    fn test_window_event_normalizer_idle_skipped() {
        let normalizer = WindowEventNormalizer;
        let input = WindowEventInput {
            app_name: "Finder".into(),
            window_title: None,
            url: None,
            started_at: Utc::now(),
            duration_secs: None,
            project_id: None,
            is_idle: true,
        };
        assert!(normalizer.normalize(&input).is_none());
    }

    #[test]
    fn test_content_hash_deterministic() {
        let h1 = content_hash("hello world");
        let h2 = content_hash("hello world");
        assert_eq!(h1, h2);
        assert_ne!(h1, content_hash("different"));
    }

    #[test]
    fn test_truncate() {
        assert_eq!(truncate("short", MAX_PREVIEW_LEN), "short");
        let long = "a".repeat(600);
        assert_eq!(truncate(&long, MAX_PREVIEW_LEN).len(), MAX_PREVIEW_LEN);
    }
}
