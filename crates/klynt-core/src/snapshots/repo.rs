use common::Result;
use sqlx::Row;
use storage::StoragePool;

const GHOST_FILE_PATH: &str = "<ghost>";
const GHOST_CONTENT_HASH: &str = "ghost";

pub struct Snapshot {
    pub id: i64,
    pub session_key: String,
    pub message_id: Option<String>,
    pub file_path: String,
    pub content_before: Vec<u8>,
    pub file_existed: bool,
    pub content_hash: String,
    pub ghost_commit_sha: Option<String>,
    pub ghost_repo_root: Option<String>,
    pub ghost_preexisting_untracked_json: Option<String>,
    pub created_at: i64,
}

#[derive(Clone)]
pub struct SnapshotRepo {
    pool: StoragePool,
}

impl SnapshotRepo {
    pub fn new(pool: StoragePool) -> Self {
        Self { pool }
    }

    #[tracing::instrument(skip(self, content), err)]
    pub async fn record(
        &self,
        session_key: &str,
        message_id: Option<&str>,
        file_path: &str,
        content: &[u8],
        existed: bool,
    ) -> Result<i64> {
        let hash = blake3::hash(content).to_hex().to_string();
        let res = sqlx::query(
            "INSERT INTO coding_snapshots \
             (session_key, message_id, file_path, content_before, file_existed, content_hash) \
             VALUES (?, ?, ?, ?, ?, ?)",
        )
        .bind(session_key)
        .bind(message_id)
        .bind(file_path)
        .bind(content)
        .bind(existed as i64)
        .bind(&hash)
        .execute(self.pool.inner())
        .await
        .map_err(common::KlyntbotError::from)?;
        Ok(res.last_insert_rowid())
    }

    #[tracing::instrument(skip(self), err)]
    pub async fn record_ghost(
        &self,
        session_key: &str,
        message_id: Option<&str>,
        ghost_commit_sha: &str,
        ghost_repo_root: &str,
        ghost_preexisting_untracked_json: Option<&str>,
    ) -> Result<i64> {
        let res = sqlx::query(
            "INSERT INTO coding_snapshots \
             (session_key, message_id, file_path, content_before, file_existed, content_hash, ghost_commit_sha, ghost_repo_root, ghost_preexisting_untracked_json) \
             VALUES (?, ?, ?, X'', 1, ?, ?, ?, ?)",
        )
        .bind(session_key)
        .bind(message_id)
        .bind(GHOST_FILE_PATH)
        .bind(GHOST_CONTENT_HASH)
        .bind(ghost_commit_sha)
        .bind(ghost_repo_root)
        .bind(ghost_preexisting_untracked_json)
        .execute(self.pool.inner())
        .await
        .map_err(common::KlyntbotError::from)?;
        Ok(res.last_insert_rowid())
    }

    /// Record a snapshot using ghost-commit when the path lives in a git repo,
    /// falling back to BLOB storage otherwise. Best-effort: if ghost-commit
    /// creation fails (git missing, permission error, etc.) we silently
    /// fall back to BLOB rather than blocking the user's edit.
    pub async fn try_record_with_ghost(
        &self,
        session_key: &str,
        message_id: Option<&str>,
        file_path: &str,
        content: &[u8],
        existed: bool,
    ) -> Result<i64> {
        use std::path::Path;
        let path = Path::new(file_path);
        let parent = path.parent().unwrap_or(path);

        let maybe_ghost = async {
            let root = klynt_git_utils::get_git_repo_root(parent).await.ok()?;
            let cfg = klynt_git_utils::GhostSnapshotConfig::default();
            let ghost = klynt_git_utils::create_ghost_commit(&root, &cfg)
                .await
                .ok()?;
            let preexisting_json = serde_json::to_string(ghost.preexisting_untracked_files()).ok();
            Some((
                ghost.id().to_string(),
                root.to_string_lossy().into_owned(),
                preexisting_json,
            ))
        }
        .await;

        if let Some((sha, root, preexisting_json)) = maybe_ghost {
            return self
                .record_ghost(
                    session_key,
                    message_id,
                    &sha,
                    &root,
                    preexisting_json.as_deref(),
                )
                .await;
        }
        self.record(session_key, message_id, file_path, content, existed)
            .await
    }

    pub async fn get(&self, id: i64) -> Result<Option<Snapshot>> {
        let row = sqlx::query("SELECT * FROM coding_snapshots WHERE id = ?")
            .bind(id)
            .fetch_optional(self.pool.inner())
            .await
            .map_err(common::KlyntbotError::from)?;
        Ok(row.map(row_to_snapshot))
    }

    pub async fn list_for_session(&self, session_key: &str) -> Result<Vec<Snapshot>> {
        let rows =
            sqlx::query("SELECT * FROM coding_snapshots WHERE session_key = ? ORDER BY id DESC")
                .bind(session_key)
                .fetch_all(self.pool.inner())
                .await
                .map_err(common::KlyntbotError::from)?;
        Ok(rows.into_iter().map(row_to_snapshot).collect())
    }

    pub async fn list_after_message(
        &self,
        session_key: &str,
        message_id: &str,
    ) -> Result<Vec<Snapshot>> {
        let rows = sqlx::query(
            "SELECT s.* FROM coding_snapshots s \
             WHERE s.session_key = ? AND s.id > COALESCE( \
               (SELECT MAX(id) FROM coding_snapshots WHERE session_key = ? AND message_id = ?), 0 \
             ) ORDER BY s.id ASC",
        )
        .bind(session_key)
        .bind(session_key)
        .bind(message_id)
        .fetch_all(self.pool.inner())
        .await
        .map_err(common::KlyntbotError::from)?;
        Ok(rows.into_iter().map(row_to_snapshot).collect())
    }
}

fn row_to_snapshot(row: sqlx::sqlite::SqliteRow) -> Snapshot {
    Snapshot {
        id: row.get("id"),
        session_key: row.get("session_key"),
        message_id: row.get("message_id"),
        file_path: row.get("file_path"),
        content_before: row.get("content_before"),
        file_existed: row.get::<i64, _>("file_existed") != 0,
        content_hash: row.get("content_hash"),
        ghost_commit_sha: row.get("ghost_commit_sha"),
        ghost_repo_root: row.get("ghost_repo_root"),
        ghost_preexisting_untracked_json: row.get("ghost_preexisting_untracked_json"),
        created_at: row.get("created_at"),
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
        let id = repo
            .record("sess1", Some("msg1"), "/tmp/foo.txt", b"old", true)
            .await
            .unwrap();
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

    #[tokio::test]
    async fn record_ghost_stores_sha_with_empty_blob() {
        let pool = StoragePool::connect_in_memory().await.unwrap();
        let repo = SnapshotRepo::new(pool.clone());
        let id = repo
            .record_ghost("sess1", Some("msg1"), "deadbeef0123", "/tmp/repo", None)
            .await
            .unwrap();
        let snap = repo.get(id).await.unwrap().expect("exists");
        assert_eq!(snap.ghost_commit_sha.as_deref(), Some("deadbeef0123"));
        assert!(snap.content_before.is_empty(), "ghost rows have empty BLOB");
    }

    #[tokio::test]
    async fn try_record_with_ghost_falls_back_to_blob_outside_repo() {
        let tmp = tempfile::TempDir::new().unwrap();
        let file = tmp.path().join("x.txt");
        std::fs::write(&file, b"hi").unwrap();
        let pool = StoragePool::connect_in_memory().await.unwrap();
        let repo = SnapshotRepo::new(pool.clone());
        let id = repo
            .try_record_with_ghost("s", None, &file.to_string_lossy(), b"hi", true)
            .await
            .unwrap();
        let snap = repo.get(id).await.unwrap().unwrap();
        // No ghost SHA because tempdir is not a git repo.
        assert!(snap.ghost_commit_sha.is_none());
        assert_eq!(snap.content_before, b"hi");
    }
}
