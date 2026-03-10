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

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WorkContext {
    pub id: String,
    pub title: String,
    pub description: Option<String>,
    pub status: WorkContextStatus,
    pub context_type: WorkContextType,
    pub embedding_id: Option<String>,
    pub linked_project_id: Option<String>,
    pub color: Option<String>,
    pub tags: Vec<String>,
    pub confidence: f64,
    pub first_seen_at: DateTime<Utc>,
    pub last_active_at: DateTime<Utc>,
    pub total_duration_secs: i64,
    pub event_count: i64,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum WorkContextStatus {
    Active,
    Paused,
    Completed,
    Archived,
}

impl WorkContextStatus {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Active => "active",
            Self::Paused => "paused",
            Self::Completed => "completed",
            Self::Archived => "archived",
        }
    }

    pub fn parse(s: &str) -> Option<Self> {
        match s {
            "active" => Some(Self::Active),
            "paused" => Some(Self::Paused),
            "completed" => Some(Self::Completed),
            "archived" => Some(Self::Archived),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum WorkContextType {
    Coding,
    Research,
    Communication,
    Planning,
    Review,
    Meeting,
    Learning,
    General,
}

impl WorkContextType {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Coding => "coding",
            Self::Research => "research",
            Self::Communication => "communication",
            Self::Planning => "planning",
            Self::Review => "review",
            Self::Meeting => "meeting",
            Self::Learning => "learning",
            Self::General => "general",
        }
    }

    pub fn parse(s: &str) -> Option<Self> {
        match s {
            "coding" => Some(Self::Coding),
            "research" => Some(Self::Research),
            "communication" => Some(Self::Communication),
            "planning" => Some(Self::Planning),
            "review" => Some(Self::Review),
            "meeting" => Some(Self::Meeting),
            "learning" => Some(Self::Learning),
            "general" => Some(Self::General),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WorkResource {
    pub id: String,
    pub resource_type: String,
    pub resource_name: String,
    pub resource_path: Option<String>,
    pub resource_uri: Option<String>,
    pub first_seen_at: DateTime<Utc>,
    pub last_seen_at: DateTime<Utc>,
    pub access_count: i64,
    pub embedding_id: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ResourceEdge {
    pub source_id: String,
    pub target_id: String,
    pub edge_type: String,
    pub weight: f64,
    pub first_seen_at: DateTime<Utc>,
    pub last_seen_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ContextAssignment {
    pub event_id: String,
    pub context_id: String,
    pub is_new_context: bool,
    pub similarity_score: f64,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_work_context_status_roundtrip() {
        let s = WorkContextStatus::Active;
        let json = serde_json::to_string(&s).unwrap();
        assert_eq!(json, "\"active\"");
        let parsed: WorkContextStatus = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed, WorkContextStatus::Active);
    }

    #[test]
    fn test_work_context_type_roundtrip() {
        let t = WorkContextType::Coding;
        let json = serde_json::to_string(&t).unwrap();
        assert_eq!(json, "\"coding\"");
        let parsed: WorkContextType = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed, WorkContextType::Coding);
    }

    #[test]
    fn test_work_context_status_as_str() {
        assert_eq!(WorkContextStatus::Active.as_str(), "active");
        assert_eq!(WorkContextStatus::Archived.as_str(), "archived");
    }

    #[test]
    fn test_work_context_status_parse() {
        assert_eq!(
            WorkContextStatus::parse("active"),
            Some(WorkContextStatus::Active)
        );
        assert_eq!(WorkContextStatus::parse("invalid"), None);
    }

    #[test]
    fn test_work_context_type_as_str() {
        assert_eq!(WorkContextType::Coding.as_str(), "coding");
        assert_eq!(WorkContextType::General.as_str(), "general");
    }

    #[test]
    fn test_work_context_type_parse() {
        assert_eq!(
            WorkContextType::parse("coding"),
            Some(WorkContextType::Coding)
        );
        assert_eq!(WorkContextType::parse("invalid"), None);
    }
}
