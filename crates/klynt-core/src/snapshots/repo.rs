use storage::StoragePool;
use common::Result;
use sqlx::Row;

pub struct Snapshot {
    pub id: i64,
    pub session_key: String,
    pub message_id: Option<String>,
    pub file_path: String,
    pub content_before: Vec<u8>,
    pub file_existed: bool,
    pub content_hash: String,
    pub created_at: i64,
}

#[derive(Clone)]
pub struct SnapshotRepo { pool: StoragePool }

impl SnapshotRepo {
    pub fn new(pool: StoragePool) -> Self { Self { pool } }

    #[tracing::instrument(skip(self, content), err)]
    pub async fn record(&self, session_key: &str, message_id: Option<&str>,
                        file_path: &str, content: &[u8], existed: bool) -> Result<i64> {
        let hash = blake3::hash(content).to_hex().to_string();
        let res = sqlx::query(
            "INSERT INTO coding_snapshots \
             (session_key, message_id, file_path, content_before, file_existed, content_hash) \
             VALUES (?, ?, ?, ?, ?, ?)",
        )
        .bind(session_key).bind(message_id).bind(file_path)
        .bind(content).bind(existed as i64).bind(&hash)
        .execute(self.pool.inner()).await
        .map_err(common::KlyntbotError::from)?;
        Ok(res.last_insert_rowid())
    }

    pub async fn get(&self, id: i64) -> Result<Option<Snapshot>> {
        let row = sqlx::query("SELECT * FROM coding_snapshots WHERE id = ?")
            .bind(id).fetch_optional(self.pool.inner()).await
            .map_err(common::KlyntbotError::from)?;
        Ok(row.map(row_to_snapshot))
    }

    pub async fn list_for_session(&self, session_key: &str) -> Result<Vec<Snapshot>> {
        let rows = sqlx::query("SELECT * FROM coding_snapshots WHERE session_key = ? ORDER BY id DESC")
            .bind(session_key).fetch_all(self.pool.inner()).await
            .map_err(common::KlyntbotError::from)?;
        Ok(rows.into_iter().map(row_to_snapshot).collect())
    }

    pub async fn list_after_message(&self, session_key: &str, message_id: &str) -> Result<Vec<Snapshot>> {
        let rows = sqlx::query(
            "SELECT s.* FROM coding_snapshots s \
             WHERE s.session_key = ? AND s.id > COALESCE( \
               (SELECT MAX(id) FROM coding_snapshots WHERE session_key = ? AND message_id = ?), 0 \
             ) ORDER BY s.id ASC",
        ).bind(session_key).bind(session_key).bind(message_id)
        .fetch_all(self.pool.inner()).await
        .map_err(common::KlyntbotError::from)?;
        Ok(rows.into_iter().map(row_to_snapshot).collect())
    }
}

fn row_to_snapshot(row: sqlx::sqlite::SqliteRow) -> Snapshot {
    Snapshot {
        id: row.get("id"), session_key: row.get("session_key"),
        message_id: row.get("message_id"), file_path: row.get("file_path"),
        content_before: row.get("content_before"),
        file_existed: row.get::<i64, _>("file_existed") != 0,
        content_hash: row.get("content_hash"), created_at: row.get("created_at"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use storage::StoragePool;

    #[tokio::test]
    async fn snapshot_round_trip() {
        let pool = StoragePool::connect_in_memory().await.unwrap();
        let repo = SnapshotRepo::new(pool.clone());
        let id = repo.record("sess1", Some("msg1"), "/tmp/foo.txt", b"old", true).await.unwrap();
        let snap = repo.get(id).await.unwrap().expect("exists");
        assert_eq!(snap.content_before, b"old");
        assert!(snap.file_existed);
    }

    #[tokio::test]
    async fn list_after_returns_descending() {
        let pool = StoragePool::connect_in_memory().await.unwrap();
        let repo = SnapshotRepo::new(pool.clone());
        let _a = repo.record("s", None, "/a", b"1", true).await.unwrap();
        let b = repo.record("s", None, "/b", b"2", true).await.unwrap();
        let snaps = repo.list_for_session("s").await.unwrap();
        assert_eq!(snaps.len(), 2);
        assert_eq!(snaps[0].id, b, "newest first");
    }
}
