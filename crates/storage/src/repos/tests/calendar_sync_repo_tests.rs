//! Unit tests for CalendarSyncRepo.

#[cfg(test)]
mod tests {
    // TODO: use super::super::fixtures::*;

    #[tokio::test]
    async fn save_and_load_state() {
        // Given: empty DB
        // When: save_state(provider_id="apple", sync_token="abc123", last_sync_at=now)
        // Then: load_state("apple") returns matching state
    }

    #[tokio::test]
    async fn update_sync_token() {
        // Given: existing sync state with token "abc"
        // When: save_state with updated token "def"
        // Then: load_state returns token "def" (UPSERT behavior)
    }

    #[tokio::test]
    async fn load_nonexistent_returns_none() {
        // Given: empty DB
        // When: load_state("nonexistent")
        // Then: returns None
    }
}
