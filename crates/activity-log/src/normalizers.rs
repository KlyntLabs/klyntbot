use jiff::Timestamp;
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

/// Parse an RFC3339 timestamp string, falling back to `Timestamp::now()` on failure.
pub fn parse_rfc3339(s: &str) -> Timestamp {
    s.parse::<Timestamp>().unwrap_or_else(|_| Timestamp::now())
}

/// Compute SHA-256 hex digest for content dedup.
pub fn content_hash(content: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(content.as_bytes());
    hasher.finalize().iter().map(|b| format!("{:02x}", b)).collect()
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
            timestamp: Timestamp::now(),
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
            timestamp: Timestamp::now(),
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
    pub started_at: Timestamp,
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
            evt.started_at
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
            started_at: Timestamp::now(),
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
            started_at: Timestamp::now(),
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
            started_at: Timestamp::now(),
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
