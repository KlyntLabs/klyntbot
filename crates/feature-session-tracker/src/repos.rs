use crate::types::{
    BrainstormConversation, BrainstormMessage, BrainstormMode, PinnedMessage, SessionStatus,
    TrackedSession,
};
use chrono::{DateTime, Utc};
use sqlx::SqlitePool;
use storage::StorageError;

#[derive(Debug, Clone)]
pub struct SessionTrackerRepos {
    pool: SqlitePool,
}

impl SessionTrackerRepos {
    pub fn new(pool: SqlitePool) -> Self {
        Self { pool }
    }

    // --- Tracked Sessions ---

    pub async fn upsert_session(&self, session: &TrackedSession) -> Result<(), StorageError> {
        sqlx::query(
            r#"INSERT INTO tracked_sessions (session_id, project_path, project_name, jsonl_path, status, first_message_preview, message_count, git_branch, last_activity, created_at)
               VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)
               ON CONFLICT(session_id) DO UPDATE SET
                 status = ?5,
                 first_message_preview = COALESCE(?6, tracked_sessions.first_message_preview),
                 message_count = ?7,
                 git_branch = COALESCE(?8, tracked_sessions.git_branch),
                 last_activity = ?9"#,
        )
        .bind(&session.session_id)
        .bind(&session.project_path)
        .bind(&session.project_name)
        .bind(&session.jsonl_path)
        .bind(session.status.as_str())
        .bind(&session.first_message_preview)
        .bind(session.message_count)
        .bind(&session.git_branch)
        .bind(session.last_activity)
        .bind(session.created_at)
        .execute(&self.pool)
        .await
        .map_err(StorageError::from)?;

        Ok(())
    }

    pub async fn list_sessions(&self) -> Result<Vec<TrackedSession>, StorageError> {
        let rows = sqlx::query_as::<_, TrackedSessionRow>(
            "SELECT * FROM tracked_sessions ORDER BY last_activity DESC",
        )
        .fetch_all(&self.pool)
        .await
        .map_err(StorageError::from)?;

        Ok(rows.into_iter().map(Into::into).collect())
    }

    pub async fn list_sessions_by_status(
        &self,
        statuses: &[SessionStatus],
    ) -> Result<Vec<TrackedSession>, StorageError> {
        if statuses.is_empty() {
            return Ok(Vec::new());
        }

        let placeholders: Vec<&str> = statuses.iter().map(|_| "?").collect();
        let sql = format!(
            "SELECT * FROM tracked_sessions WHERE status IN ({}) ORDER BY last_activity DESC",
            placeholders.join(", ")
        );

        let mut query = sqlx::query_as::<_, TrackedSessionRow>(&sql);
        for status in statuses {
            query = query.bind(status.as_str());
        }

        let rows = query
            .fetch_all(&self.pool)
            .await
            .map_err(StorageError::from)?;

        Ok(rows.into_iter().map(Into::into).collect())
    }

    pub async fn get_session(
        &self,
        session_id: &str,
    ) -> Result<Option<TrackedSession>, StorageError> {
        let row = sqlx::query_as::<_, TrackedSessionRow>(
            "SELECT * FROM tracked_sessions WHERE session_id = ?",
        )
        .bind(session_id)
        .fetch_optional(&self.pool)
        .await
        .map_err(StorageError::from)?;

        Ok(row.map(Into::into))
    }

    pub async fn update_session_status(
        &self,
        session_id: &str,
        status: &SessionStatus,
    ) -> Result<(), StorageError> {
        sqlx::query("UPDATE tracked_sessions SET status = ? WHERE session_id = ?")
            .bind(status.as_str())
            .bind(session_id)
            .execute(&self.pool)
            .await
            .map_err(StorageError::from)?;

        Ok(())
    }

    pub async fn update_file_offset(
        &self,
        session_id: &str,
        offset: i64,
    ) -> Result<(), StorageError> {
        sqlx::query("UPDATE tracked_sessions SET file_offset = ? WHERE session_id = ?")
            .bind(offset)
            .bind(session_id)
            .execute(&self.pool)
            .await
            .map_err(StorageError::from)?;

        Ok(())
    }

    pub async fn increment_message_count(&self, session_id: &str) -> Result<(), StorageError> {
        sqlx::query(
            "UPDATE tracked_sessions SET message_count = message_count + 1, last_activity = ? WHERE session_id = ?",
        )
        .bind(Utc::now())
        .bind(session_id)
        .execute(&self.pool)
        .await
        .map_err(StorageError::from)?;

        Ok(())
    }

    // --- Pinned Messages ---

    pub async fn pin_message(&self, pin: &PinnedMessage) -> Result<(), StorageError> {
        sqlx::query(
            r#"INSERT INTO pinned_messages (session_id, message_uuid, message_content, message_role, pin_order)
               VALUES (?1, ?2, ?3, ?4, ?5)
               ON CONFLICT(session_id, message_uuid) DO UPDATE SET pin_order = ?5"#,
        )
        .bind(&pin.session_id)
        .bind(&pin.message_uuid)
        .bind(&pin.message_content)
        .bind(&pin.message_role)
        .bind(pin.pin_order)
        .execute(&self.pool)
        .await
        .map_err(StorageError::from)?;

        Ok(())
    }

    pub async fn unpin_message(&self, id: i64) -> Result<(), StorageError> {
        sqlx::query("DELETE FROM pinned_messages WHERE id = ?")
            .bind(id)
            .execute(&self.pool)
            .await
            .map_err(StorageError::from)?;

        Ok(())
    }

    pub async fn list_pins(&self, session_id: &str) -> Result<Vec<PinnedMessage>, StorageError> {
        let rows = sqlx::query_as::<_, PinnedMessageRow>(
            "SELECT * FROM pinned_messages WHERE session_id = ? ORDER BY pin_order",
        )
        .bind(session_id)
        .fetch_all(&self.pool)
        .await
        .map_err(StorageError::from)?;

        Ok(rows.into_iter().map(Into::into).collect())
    }

    // --- Brainstorm Conversations ---

    pub async fn create_conversation(
        &self,
        conv: &BrainstormConversation,
    ) -> Result<(), StorageError> {
        sqlx::query(
            r#"INSERT INTO brainstorm_conversations (id, session_id, title, mode, model_key, agent_profile, created_at)
               VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)"#,
        )
        .bind(&conv.id)
        .bind(&conv.session_id)
        .bind(&conv.title)
        .bind(conv.mode.as_str())
        .bind(&conv.model_key)
        .bind(&conv.agent_profile)
        .bind(conv.created_at)
        .execute(&self.pool)
        .await
        .map_err(StorageError::from)?;

        Ok(())
    }

    pub async fn list_conversations(
        &self,
        session_id: &str,
    ) -> Result<Vec<BrainstormConversation>, StorageError> {
        let rows = sqlx::query_as::<_, BrainstormConversationRow>(
            "SELECT * FROM brainstorm_conversations WHERE session_id = ? ORDER BY created_at DESC",
        )
        .bind(session_id)
        .fetch_all(&self.pool)
        .await
        .map_err(StorageError::from)?;

        Ok(rows.into_iter().map(Into::into).collect())
    }

    pub async fn get_conversation(
        &self,
        id: &str,
    ) -> Result<Option<BrainstormConversation>, StorageError> {
        let row = sqlx::query_as::<_, BrainstormConversationRow>(
            "SELECT * FROM brainstorm_conversations WHERE id = ?",
        )
        .bind(id)
        .fetch_optional(&self.pool)
        .await
        .map_err(StorageError::from)?;

        Ok(row.map(Into::into))
    }

    // --- Brainstorm Messages ---

    pub async fn add_brainstorm_message(
        &self,
        msg: &BrainstormMessage,
    ) -> Result<(), StorageError> {
        let mut tx = self.pool.begin().await.map_err(StorageError::from)?;

        sqlx::query(
            r#"INSERT INTO brainstorm_messages (id, conversation_id, role, content, is_result_block, edited_content, sent_to_cc, created_at)
               VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)"#,
        )
        .bind(&msg.id)
        .bind(&msg.conversation_id)
        .bind(&msg.role)
        .bind(&msg.content)
        .bind(msg.is_result_block)
        .bind(&msg.edited_content)
        .bind(msg.sent_to_cc)
        .bind(msg.created_at)
        .execute(&mut *tx)
        .await
        .map_err(StorageError::from)?;

        sqlx::query("UPDATE brainstorm_conversations SET updated_at = ? WHERE id = ?")
            .bind(Utc::now())
            .bind(&msg.conversation_id)
            .execute(&mut *tx)
            .await
            .map_err(StorageError::from)?;

        tx.commit().await.map_err(StorageError::from)?;

        Ok(())
    }

    pub async fn list_brainstorm_messages(
        &self,
        conversation_id: &str,
    ) -> Result<Vec<BrainstormMessage>, StorageError> {
        let rows = sqlx::query_as::<_, BrainstormMessageRow>(
            "SELECT * FROM brainstorm_messages WHERE conversation_id = ? ORDER BY created_at",
        )
        .bind(conversation_id)
        .fetch_all(&self.pool)
        .await
        .map_err(StorageError::from)?;

        Ok(rows.into_iter().map(Into::into).collect())
    }

    pub async fn update_brainstorm_message_edit(
        &self,
        message_id: &str,
        edited_content: &str,
    ) -> Result<(), StorageError> {
        sqlx::query("UPDATE brainstorm_messages SET edited_content = ? WHERE id = ?")
            .bind(edited_content)
            .bind(message_id)
            .execute(&self.pool)
            .await
            .map_err(StorageError::from)?;

        Ok(())
    }

    pub async fn mark_sent_to_cc(&self, message_id: &str) -> Result<(), StorageError> {
        sqlx::query("UPDATE brainstorm_messages SET sent_to_cc = 1 WHERE id = ?")
            .bind(message_id)
            .execute(&self.pool)
            .await
            .map_err(StorageError::from)?;

        Ok(())
    }

    // --- Session Summaries ---

    pub async fn save_summary(
        &self,
        summary: &crate::types::ChunkSummary,
    ) -> Result<(), StorageError> {
        sqlx::query(
            r#"INSERT INTO session_summaries (session_id, chunk_start, chunk_end, summary, files_touched, key_decisions, rolling_summary)
               VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)"#,
        )
        .bind(&summary.session_id)
        .bind(summary.chunk_start)
        .bind(summary.chunk_end)
        .bind(&summary.summary)
        .bind(serde_json::to_string(&summary.files_touched).ok())
        .bind(serde_json::to_string(&summary.key_decisions).ok())
        .bind(&summary.rolling_summary)
        .execute(&self.pool)
        .await
        .map_err(StorageError::from)?;

        Ok(())
    }

    pub async fn get_latest_summary(
        &self,
        session_id: &str,
    ) -> Result<Option<String>, StorageError> {
        let row: Option<(String,)> = sqlx::query_as(
            "SELECT rolling_summary FROM session_summaries WHERE session_id = ? ORDER BY id DESC LIMIT 1",
        )
        .bind(session_id)
        .fetch_optional(&self.pool)
        .await
        .map_err(StorageError::from)?;

        Ok(row.map(|r| r.0))
    }
}

// --- SQLx row types ---

#[derive(sqlx::FromRow)]
struct TrackedSessionRow {
    session_id: String,
    project_path: String,
    project_name: String,
    jsonl_path: String,
    status: String,
    first_message_preview: Option<String>,
    message_count: i64,
    git_branch: Option<String>,
    last_activity: Option<DateTime<Utc>>,
    #[allow(dead_code)]
    file_offset: i64,
    created_at: DateTime<Utc>,
}

impl From<TrackedSessionRow> for TrackedSession {
    fn from(row: TrackedSessionRow) -> Self {
        Self {
            session_id: row.session_id,
            project_path: row.project_path,
            project_name: row.project_name,
            jsonl_path: row.jsonl_path,
            status: SessionStatus::from_db(&row.status),
            first_message_preview: row.first_message_preview,
            message_count: row.message_count,
            git_branch: row.git_branch,
            last_activity: row.last_activity,
            created_at: row.created_at,
        }
    }
}

#[derive(sqlx::FromRow)]
struct PinnedMessageRow {
    id: i64,
    session_id: String,
    message_uuid: String,
    message_content: String,
    message_role: String,
    pin_order: i64,
    created_at: DateTime<Utc>,
}

impl From<PinnedMessageRow> for PinnedMessage {
    fn from(row: PinnedMessageRow) -> Self {
        Self {
            id: row.id,
            session_id: row.session_id,
            message_uuid: row.message_uuid,
            message_content: row.message_content,
            message_role: row.message_role,
            pin_order: row.pin_order,
            created_at: row.created_at,
        }
    }
}

#[derive(sqlx::FromRow)]
struct BrainstormConversationRow {
    id: String,
    session_id: String,
    title: Option<String>,
    mode: String,
    model_key: Option<String>,
    agent_profile: Option<String>,
    created_at: DateTime<Utc>,
    updated_at: Option<DateTime<Utc>>,
}

impl From<BrainstormConversationRow> for BrainstormConversation {
    fn from(row: BrainstormConversationRow) -> Self {
        Self {
            id: row.id,
            session_id: row.session_id,
            title: row.title,
            mode: BrainstormMode::from_db(&row.mode),
            model_key: row.model_key,
            agent_profile: row.agent_profile,
            created_at: row.created_at,
            updated_at: row.updated_at,
        }
    }
}

#[derive(sqlx::FromRow)]
struct BrainstormMessageRow {
    id: String,
    conversation_id: String,
    role: String,
    content: String,
    is_result_block: bool,
    edited_content: Option<String>,
    sent_to_cc: bool,
    created_at: DateTime<Utc>,
}

impl From<BrainstormMessageRow> for BrainstormMessage {
    fn from(row: BrainstormMessageRow) -> Self {
        Self {
            id: row.id,
            conversation_id: row.conversation_id,
            role: row.role,
            content: row.content,
            is_result_block: row.is_result_block,
            edited_content: row.edited_content,
            sent_to_cc: row.sent_to_cc,
            created_at: row.created_at,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::SessionTrackerFeature;

    async fn setup_repos() -> SessionTrackerRepos {
        let pool = storage::StoragePool::connect_in_memory().await.unwrap();
        storage::StoragePool::run_feature_migrations(
            pool.inner(),
            &SessionTrackerFeature::migrations_static(),
        )
        .await
        .unwrap();
        SessionTrackerRepos::new(pool.inner().clone())
    }

    fn make_session(id: &str, status: SessionStatus) -> TrackedSession {
        TrackedSession {
            session_id: id.to_string(),
            project_path: "/test/project".to_string(),
            project_name: "project".to_string(),
            jsonl_path: format!("/test/{id}.jsonl"),
            status,
            first_message_preview: Some(format!("Preview for {id}")),
            message_count: 1,
            git_branch: None,
            last_activity: Some(Utc::now()),
            created_at: Utc::now(),
        }
    }

    #[tokio::test]
    async fn test_list_sessions_by_status() {
        let repos = setup_repos().await;

        // Insert sessions with different statuses
        repos
            .upsert_session(&make_session("sess-active", SessionStatus::Active))
            .await
            .unwrap();
        repos
            .upsert_session(&make_session("sess-idle", SessionStatus::Idle))
            .await
            .unwrap();
        repos
            .upsert_session(&make_session("sess-done", SessionStatus::Completed))
            .await
            .unwrap();

        // Query Active + Idle only
        let results = repos
            .list_sessions_by_status(&[SessionStatus::Active, SessionStatus::Idle])
            .await
            .unwrap();
        assert_eq!(results.len(), 2);
        assert!(results
            .iter()
            .all(|s| s.status == SessionStatus::Active || s.status == SessionStatus::Idle));
        // Completed should not appear
        assert!(!results.iter().any(|s| s.status == SessionStatus::Completed));

        // Query single status
        let active_only = repos
            .list_sessions_by_status(&[SessionStatus::Active])
            .await
            .unwrap();
        assert_eq!(active_only.len(), 1);
        assert_eq!(active_only[0].session_id, "sess-active");

        // Empty input returns empty vec
        let empty = repos.list_sessions_by_status(&[]).await.unwrap();
        assert!(empty.is_empty());
    }
}
