//! GraphQL types for chat messages and sessions.

use async_graphql::{Enum, Object, SimpleObject, ID};
use chrono::{DateTime, Utc};

/// Role of the chat message author.
#[derive(Enum, Clone, Copy, PartialEq, Eq, Debug)]
pub enum MessageRole {
    User,
    Assistant,
    System,
}

/// A chat message in a session.
#[derive(Clone)]
pub struct GqlChatMessage {
    pub id: String,
    pub role: MessageRole,
    pub content: String,
    pub session_id: String,
    pub created_at: DateTime<Utc>,
}

#[Object]
impl GqlChatMessage {
    async fn id(&self) -> ID {
        self.id.clone().into()
    }

    async fn role(&self) -> MessageRole {
        self.role
    }

    async fn content(&self) -> &str {
        &self.content
    }

    async fn session_id(&self) -> &str {
        &self.session_id
    }

    async fn created_at(&self) -> DateTime<Utc> {
        self.created_at
    }
}

/// Result of a calendar sync operation.
#[derive(SimpleObject, Clone)]
pub struct GqlCalendarSyncResult {
    pub success: bool,
    pub events_synced: i32,
    pub conflicts_found: i32,
    pub duration_ms: i64,
    pub message: String,
}
