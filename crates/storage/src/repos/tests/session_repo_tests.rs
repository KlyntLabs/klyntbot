//! Unit tests for SessionRepo.

#[cfg(test)]
mod tests {
    // TODO: use super::super::fixtures::*;

    #[tokio::test]
    async fn create_and_get() {
        // Given: empty DB
        // When: create session with key="test-session", metadata={}
        // Then: get("test-session") returns session
    }

    #[tokio::test]
    async fn add_message_to_session() {
        // Given: existing session
        // When: add message(role="user", content="hello")
        // Then: message row persisted with session_key FK
    }

    #[tokio::test]
    async fn get_messages_in_order() {
        // Given: session with 5 messages added at t=1..5
        // When: get_messages(session_key)
        // Then: messages returned sorted by timestamp ASC
    }

    #[tokio::test]
    async fn delete_cascades_messages() {
        // Given: session with 3 messages
        // When: delete(session_key)
        // Then: session gone, all messages gone (CASCADE)
    }

    #[tokio::test]
    async fn list_sessions() {
        // Given: 3 sessions
        // When: list()
        // Then: returns 3 entries with keys and metadata
    }

    #[tokio::test]
    async fn compact_keeps_recent_messages() {
        // Given: session with 1000 messages
        // When: compact(session_key, keep=500)
        // Then: 500 oldest deleted, 500 newest retained
        // SQL: DELETE FROM session_messages WHERE id NOT IN
        //      (SELECT id ... ORDER BY timestamp DESC LIMIT 500)
    }

    #[tokio::test]
    async fn message_ordering_ignores_insert_order() {
        // Given: messages inserted out of timestamp order
        // When: get_messages()
        // Then: returned sorted by timestamp, not insert order
    }

    #[tokio::test]
    async fn concurrent_message_writes() {
        // Given: session
        // When: 10 tokio tasks concurrently add_message
        // Then: all 10 messages persisted (Postgres handles serialization)
        // TODO: use tokio::spawn + JoinSet
    }
}
