use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use crate::events::{MessageSegment, TransparencyData};

// ── Chat ────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ChatThreadResponse {
    pub session_key: String,
    pub title: String,
    pub message_count: i64,
    pub updated_at: DateTime<Utc>,
    // Context fields from session_context join
    pub context_type: Option<String>,
    pub entity_kind: Option<String>,
    pub entity_id: Option<String>,
    pub area_id: Option<String>,
    pub area_name: Option<String>,
    pub project_id: Option<String>,
    pub project_name: Option<String>,
    // Squad fields (resolved from session.squad_id)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub squad_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub squad_name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub squad_icon: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ChatMessageResponse {
    pub id: String,
    pub role: String,
    pub content: String,
    pub timestamp: DateTime<Utc>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub segments: Option<Vec<MessageSegment>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub transparency: Option<TransparencyData>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub persona_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub persona_name: Option<String>,
}

/// Detailed session response for `chat_get_session`.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ChatSessionResponse {
    pub session_key: String,
    pub title: String,
    pub message_count: i64,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub project_id: Option<String>,
    pub conversation_type: Option<String>,
    pub pinned: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub squad_id: Option<String>,
}

/// Optional session context sent from the frontend alongside a chat message.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SessionContextInput {
    pub entity_kind: Option<String>,
    pub entity_id: Option<String>,
    pub context_type: Option<String>,
    pub is_ephemeral: Option<bool>,
    pub squad_id: Option<String>,
}
