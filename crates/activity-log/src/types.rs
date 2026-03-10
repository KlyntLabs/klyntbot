use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

pub const MAX_PREVIEW_LEN: usize = 500;

/// A normalized activity event in the unified activity log.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ActivityLogEntry {
    pub id: String,
    pub timestamp: DateTime<Utc>,
    pub source: ActivitySource,
    pub actor: ActivityActor,
    pub resource_type: Option<String>,
    pub resource_id: Option<String>,
    pub resource_name: Option<String>,
    pub action: String,
    pub content_preview: Option<String>,
    pub content_hash: Option<String>,
    pub metadata: Option<serde_json::Value>,
    pub app_name: Option<String>,
    pub project_id: Option<String>,
    pub work_context_id: Option<String>,
    pub embedding_id: Option<String>,
    pub duration_secs: Option<i64>,
    pub session_key: Option<String>,
    pub is_sensitive: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ActivitySource {
    OsWindow,
    Browser,
    Terminal,
    Chat,
    ToolCall,
    FileSystem,
    Calendar,
    Ide,
    Note,
    DomainEvent,
    Task,
    FocusSession,
}

impl ActivitySource {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::OsWindow => "os_window",
            Self::Browser => "browser",
            Self::Terminal => "terminal",
            Self::Chat => "chat",
            Self::ToolCall => "tool_call",
            Self::FileSystem => "file_system",
            Self::Calendar => "calendar",
            Self::Ide => "ide",
            Self::Note => "note",
            Self::DomainEvent => "domain_event",
            Self::Task => "task",
            Self::FocusSession => "focus_session",
        }
    }

    pub fn parse(s: &str) -> Option<Self> {
        match s {
            "os_window" => Some(Self::OsWindow),
            "browser" => Some(Self::Browser),
            "terminal" => Some(Self::Terminal),
            "chat" => Some(Self::Chat),
            "tool_call" => Some(Self::ToolCall),
            "file_system" => Some(Self::FileSystem),
            "calendar" => Some(Self::Calendar),
            "ide" => Some(Self::Ide),
            "note" => Some(Self::Note),
            "domain_event" => Some(Self::DomainEvent),
            "task" => Some(Self::Task),
            "focus_session" => Some(Self::FocusSession),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ActivityActor {
    User,
    System,
    AiAgent,
}

impl ActivityActor {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::User => "user",
            Self::System => "system",
            Self::AiAgent => "ai_agent",
        }
    }

    pub fn parse(s: &str) -> Option<Self> {
        match s {
            "user" => Some(Self::User),
            "system" => Some(Self::System),
            "ai_agent" => Some(Self::AiAgent),
            _ => None,
        }
    }
}

impl ActivityLogEntry {
    /// Truncate content_preview to [`MAX_PREVIEW_LEN`] chars max.
    pub fn truncate_preview(&mut self) {
        if let Some(ref mut preview) = self.content_preview {
            if preview.len() > MAX_PREVIEW_LEN {
                *preview = preview.chars().take(MAX_PREVIEW_LEN).collect();
            }
        }
    }
}
