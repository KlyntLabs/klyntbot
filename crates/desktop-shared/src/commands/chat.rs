use serde::{Deserialize, Serialize};

use crate::events::{MessageSegment, TransparencyData};

// ── Chat ────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct ChatThreadResponse {
    pub session_key: String,
    pub title: String,
    pub message_count: i64,
    #[specta(type = crate::specta_helpers::Timestamp)]
    pub updated_at: jiff::Timestamp,
    // Context fields from session_context join
    pub context_type: Option<String>,
    pub entity_kind: Option<String>,
    pub entity_id: Option<String>,
    pub area_id: Option<String>,
    pub area_name: Option<String>,
    pub project_id: Option<String>,
    pub project_name: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct ChatMessageResponse {
    pub id: String,
    pub role: String,
    pub content: String,
    #[specta(type = crate::specta_helpers::Timestamp)]
    pub timestamp: jiff::Timestamp,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub segments: Option<Vec<MessageSegment>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub transparency: Option<TransparencyData>,
}

/// Detailed session response for `chat_get_session`.
#[derive(Debug, Clone, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct ChatSessionResponse {
    pub session_key: String,
    pub title: String,
    pub message_count: i64,
    #[specta(type = crate::specta_helpers::Timestamp)]
    pub created_at: jiff::Timestamp,
    #[specta(type = crate::specta_helpers::Timestamp)]
    pub updated_at: jiff::Timestamp,
    pub project_id: Option<String>,
    pub conversation_type: Option<String>,
    pub pinned: bool,
}

/// Optional session context sent from the frontend alongside a chat message.
#[derive(Debug, Clone, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct SessionContextInput {
    pub entity_kind: Option<String>,
    pub entity_id: Option<String>,
    pub context_type: Option<String>,
    pub is_ephemeral: Option<bool>,
}
