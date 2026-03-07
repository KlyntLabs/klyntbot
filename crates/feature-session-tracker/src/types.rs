use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

// --- Claude Code JSONL types (deserialization) ---

/// Raw JSONL line from Claude Code session files.
#[derive(Debug, Clone, Deserialize)]
#[serde(tag = "type")]
pub enum RawSessionLine {
    #[serde(rename = "user")]
    User {
        uuid: String,
        #[serde(rename = "sessionId")]
        session_id: String,
        message: RawMessage,
        #[serde(rename = "isMeta", default)]
        is_meta: bool,
        #[serde(rename = "gitBranch")]
        git_branch: Option<String>,
        timestamp: String,
        cwd: Option<String>,
    },
    #[serde(rename = "assistant")]
    Assistant {
        uuid: String,
        #[serde(rename = "sessionId")]
        session_id: Option<String>,
        message: RawMessage,
        timestamp: String,
    },
    #[serde(rename = "system")]
    System {
        uuid: String,
        subtype: Option<String>,
        content: String,
        level: Option<String>,
        timestamp: String,
    },
    #[serde(rename = "progress")]
    Progress {
        uuid: String,
        data: serde_json::Value,
        timestamp: String,
        #[serde(rename = "toolUseID")]
        tool_use_id: Option<String>,
    },
    #[serde(rename = "queue-operation")]
    QueueOperation {
        operation: String,
        content: Option<String>,
        timestamp: String,
        #[serde(rename = "sessionId")]
        session_id: Option<String>,
    },
    #[serde(rename = "file-history-snapshot")]
    FileHistorySnapshot {
        #[serde(rename = "messageId")]
        message_id: String,
    },
    #[serde(rename = "last-prompt")]
    LastPrompt {
        #[serde(rename = "lastPrompt")]
        last_prompt: Option<String>,
        #[serde(rename = "sessionId")]
        session_id: Option<String>,
    },
}

#[derive(Debug, Clone, Deserialize)]
pub struct RawMessage {
    pub role: Option<String>,
    pub content: RawContent,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(untagged)]
pub enum RawContent {
    Text(String),
    Blocks(Vec<ContentBlock>),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum ContentBlock {
    #[serde(rename = "text")]
    Text { text: String },
    #[serde(rename = "tool_use")]
    ToolUse {
        id: String,
        name: String,
        input: serde_json::Value,
    },
    #[serde(rename = "tool_result")]
    ToolResult {
        tool_use_id: String,
        content: serde_json::Value,
        #[serde(default)]
        is_error: bool,
    },
}

// --- Application types (serialized to frontend) ---

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", tag = "type")]
pub enum SessionMessage {
    User {
        uuid: String,
        text: String,
        timestamp: DateTime<Utc>,
        is_meta: bool,
    },
    Assistant {
        uuid: String,
        content: Vec<ContentBlock>,
        timestamp: DateTime<Utc>,
    },
    System {
        uuid: String,
        subtype: Option<String>,
        content: String,
        timestamp: DateTime<Utc>,
    },
    Progress {
        uuid: String,
        data: serde_json::Value,
        tool_use_id: Option<String>,
        timestamp: DateTime<Utc>,
    },
    QueueOperation {
        operation: String,
        content: Option<String>,
        timestamp: DateTime<Utc>,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub enum SessionStatus {
    Active,
    Idle,
    Completed,
}

impl SessionStatus {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Active => "active",
            Self::Idle => "idle",
            Self::Completed => "completed",
        }
    }

    pub fn from_db(s: &str) -> Self {
        match s {
            "active" => Self::Active,
            "idle" => Self::Idle,
            _ => Self::Completed,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TrackedSession {
    pub session_id: String,
    pub project_path: String,
    pub project_name: String,
    pub jsonl_path: String,
    pub status: SessionStatus,
    pub first_message_preview: Option<String>,
    pub message_count: i64,
    pub git_branch: Option<String>,
    pub last_activity: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PinnedMessage {
    pub id: i64,
    pub session_id: String,
    pub message_uuid: String,
    pub message_content: String,
    pub message_role: String,
    pub pin_order: i64,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BrainstormConversation {
    pub id: String,
    pub session_id: String,
    pub title: Option<String>,
    pub mode: BrainstormMode,
    pub model_key: Option<String>,
    pub agent_profile: Option<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub enum BrainstormMode {
    DirectModel,
    Agent,
}

impl BrainstormMode {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::DirectModel => "direct_model",
            Self::Agent => "agent",
        }
    }

    pub fn from_db(s: &str) -> Self {
        match s {
            "agent" => Self::Agent,
            _ => Self::DirectModel,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BrainstormMessage {
    pub id: String,
    pub conversation_id: String,
    pub role: String,
    pub content: String,
    pub is_result_block: bool,
    pub edited_content: Option<String>,
    pub sent_to_cc: bool,
    pub created_at: DateTime<Utc>,
}

// --- History JSONL types ---

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HistoryEntry {
    pub display: String,
    pub timestamp: i64,
    pub project: String,
    pub session_id: String,
}

// --- Tauri event payloads ---

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SessionMessagePayload {
    pub session_id: String,
    pub message: SessionMessage,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SessionStatusPayload {
    pub session_id: String,
    pub status: SessionStatus,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BrainstormTokenPayload {
    pub conversation_id: String,
    pub token: String,
}

// --- Context builder types ---

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SessionContext {
    pub rolling_summary: String,
    pub pinned_messages: Vec<PinnedMessage>,
    pub recent_messages: Vec<SessionMessage>,
    pub total_messages: i64,
    pub estimated_tokens: usize,
}

#[derive(Debug, Clone)]
pub struct ChunkSummary {
    pub session_id: String,
    pub chunk_start: i64,
    pub chunk_end: i64,
    pub summary: String,
    pub files_touched: Vec<String>,
    pub key_decisions: Vec<String>,
    pub rolling_summary: String,
}
