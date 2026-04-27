//! SQLite-backed repository for `focus_sessions`.

use jiff::Timestamp;
use sqlx::SqlitePool;
use storage::error::StorageError;

use crate::FocusMode;

// ── Row ──────────────────────────────────────────────────────────────────────

#[derive(Debug, sqlx::FromRow)]
struct FocusSessionRow {
    id: i64,
    mode: String,
    started_at: String,
    ends_at: String,
    ended_at: Option<String>,
    alarm_id: Option<String>,
    source: String,
}

impl FocusSessionRow {
    fn into_session(self) -> Result<FocusSession, String> {
        let mode = self.mode.parse::<FocusMode>()?;
        let started_at = self
            .started_at
            .parse::<Timestamp>()
            .map_err(|e| format!("invalid started_at: {e}"))?;
        let ends_at = self
            .ends_at
            .parse::<Timestamp>()
            .map_err(|e| format!("invalid ends_at: {e}"))?;
        let ended_at = self
            .ended_at
            .map(|s| s.parse::<Timestamp>())
            .transpose()
            .map_err(|e| format!("invalid ended_at: {e}"))?;
        Ok(FocusSession {
            id: self.id,
            mode,
            started_at,
            ends_at,
            ended_at,
            alarm_id: self.alarm_id,
            source: self.source,
        })
    }
}

// ── Public domain type ────────────────────────────────────────────────────────

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct FocusSession {
    pub id: i64,
    pub mode: FocusMode,
    #[specta(type = String)]
    pub started_at: Timestamp,
    #[specta(type = String)]
    pub ends_at: Timestamp,
    #[specta(type = Option<String>)]
    pub ended_at: Option<Timestamp>,
    pub alarm_id: Option<String>,
    pub source: String,
}

// ── Repo ─────────────────────────────────────────────────────────────────────

#[derive(Debug, Clone)]
pub struct FocusSessionRepo {
    pool: SqlitePool,
}

impl FocusSessionRepo {
    pub fn new(pool: SqlitePool) -> Self {
        Self { pool }
    }

    /// Return the currently-active (not yet ended) session for the given mode.
    pub async fn active(&self, mode: FocusMode) -> Result<Option<FocusSession>, StorageError> {
        let row: Option<FocusSessionRow> = sqlx::query_as(
            "SELECT id, mode, started_at, ends_at, ended_at, alarm_id, source
               FROM focus_sessions
              WHERE mode = ?1 AND ended_at IS NULL
              LIMIT 1",
        )
        .bind(mode.as_str())
        .fetch_optional(&self.pool)
        .await?;

        match row {
            None => Ok(None),
            Some(r) => r.into_session().map(Some).map_err(StorageError::Migration),
        }
    }

    /// Insert a new active session. Returns the inserted row.
    pub async fn insert_active(
        &self,
        mode: FocusMode,
        started_at: Timestamp,
        ends_at: Timestamp,
        alarm_id: Option<&str>,
        source: &str,
    ) -> Result<FocusSession, StorageError> {
        let started_str = started_at.to_string();
        let ends_str = ends_at.to_string();
        let id: i64 = sqlx::query_scalar(
            "INSERT INTO focus_sessions (mode, started_at, ends_at, alarm_id, source)
             VALUES (?1, ?2, ?3, ?4, ?5)
             RETURNING id",
        )
        .bind(mode.as_str())
        .bind(&started_str)
        .bind(&ends_str)
        .bind(alarm_id)
        .bind(source)
        .fetch_one(&self.pool)
        .await?;

        Ok(FocusSession {
            id,
            mode,
            started_at,
            ends_at,
            ended_at: None,
            alarm_id: alarm_id.map(str::to_owned),
            source: source.to_owned(),
        })
    }

    /// Mark a session as ended.
    pub async fn end(&self, id: i64, ended_at: Timestamp) -> Result<(), StorageError> {
        sqlx::query("UPDATE focus_sessions SET ended_at = ?1 WHERE id = ?2")
            .bind(ended_at.to_string())
            .bind(id)
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    /// Extend a session: update `ends_at` and optionally the `alarm_id`.
    pub async fn extend(
        &self,
        id: i64,
        new_ends_at: Timestamp,
        new_alarm_id: Option<&str>,
    ) -> Result<(), StorageError> {
        sqlx::query("UPDATE focus_sessions SET ends_at = ?1, alarm_id = ?2 WHERE id = ?3")
            .bind(new_ends_at.to_string())
            .bind(new_alarm_id)
            .bind(id)
            .execute(&self.pool)
            .await?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use storage::StoragePool;
    use tools_core::FeatureMigration;

    async fn setup() -> FocusSessionRepo {
        let pool = StoragePool::connect_in_memory().await.unwrap();
        StoragePool::run_feature_migrations(
            pool.inner(),
            &[FeatureMigration {
                feature_name: "focus".into(),
                version: 1,
                description: "focus_sessions table".into(),
                sql: include_str!("migrations/001_focus_sessions.sql").into(),
            }],
        )
        .await
        .unwrap();
        FocusSessionRepo::new(pool.inner().clone())
    }

    fn ts(secs: i64) -> Timestamp {
        Timestamp::from_second(secs).unwrap()
    }

    #[tokio::test]
    async fn insert_then_active_returns_session() {
        let repo = setup().await;
        let session = repo
            .insert_active(FocusMode::Dnd, ts(0), ts(3600), None, "launcher")
            .await
            .unwrap();

        assert_eq!(session.mode, FocusMode::Dnd);
        assert_eq!(session.started_at, ts(0));
        assert_eq!(session.ends_at, ts(3600));
        assert!(session.alarm_id.is_none());

        let active = repo.active(FocusMode::Dnd).await.unwrap();
        assert!(active.is_some());
        let active = active.unwrap();
        assert_eq!(active.id, session.id);
        assert_eq!(active.ends_at, ts(3600));
    }

    #[tokio::test]
    async fn end_makes_active_none() {
        let repo = setup().await;
        let session = repo
            .insert_active(FocusMode::Dnd, ts(0), ts(3600), None, "launcher")
            .await
            .unwrap();

        repo.end(session.id, ts(1800)).await.unwrap();

        let active = repo.active(FocusMode::Dnd).await.unwrap();
        assert!(active.is_none());
    }

    #[tokio::test]
    async fn extend_updates_ends_at_and_alarm_id() {
        let repo = setup().await;
        let session = repo
            .insert_active(FocusMode::Dnd, ts(0), ts(3600), Some("alarm-1"), "launcher")
            .await
            .unwrap();

        repo.extend(session.id, ts(7200), Some("alarm-2"))
            .await
            .unwrap();

        let active = repo.active(FocusMode::Dnd).await.unwrap().unwrap();
        assert_eq!(active.ends_at, ts(7200));
        assert_eq!(active.alarm_id.as_deref(), Some("alarm-2"));
    }

    #[tokio::test]
    async fn unique_active_index_blocks_double_insert() {
        let repo = setup().await;
        repo.insert_active(FocusMode::Dnd, ts(0), ts(3600), None, "launcher")
            .await
            .unwrap();

        // Second insert for same mode while first is still active → constraint error.
        let result = repo
            .insert_active(FocusMode::Dnd, ts(100), ts(4000), None, "launcher")
            .await;
        assert!(result.is_err());
    }
}
