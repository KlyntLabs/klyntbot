pub mod repo;
pub use repo::{Snapshot, SnapshotRepo};

use common::Result;

/// Trait abstraction over snapshot storage (enables mocking in tests).
#[async_trait::async_trait]
pub trait SnapshotService: Send + Sync {
    async fn record(
        &self,
        session_key: &str,
        message_id: Option<&str>,
        file_path: &str,
        content: &[u8],
        existed: bool,
    ) -> Result<i64>;

    async fn get(&self, id: i64) -> Result<Option<Snapshot>>;

    async fn list_for_session(&self, session_key: &str) -> Result<Vec<Snapshot>>;

    async fn list_after_message(&self, session_key: &str, message_id: &str) -> Result<Vec<Snapshot>>;
}

#[async_trait::async_trait]
impl SnapshotService for SnapshotRepo {
    async fn record(
        &self,
        session_key: &str,
        message_id: Option<&str>,
        file_path: &str,
        content: &[u8],
        existed: bool,
    ) -> Result<i64> {
        self.record(session_key, message_id, file_path, content, existed).await
    }

    async fn get(&self, id: i64) -> Result<Option<Snapshot>> {
        self.get(id).await
    }

    async fn list_for_session(&self, session_key: &str) -> Result<Vec<Snapshot>> {
        self.list_for_session(session_key).await
    }

    async fn list_after_message(&self, session_key: &str, message_id: &str) -> Result<Vec<Snapshot>> {
        self.list_after_message(session_key, message_id).await
    }
}
