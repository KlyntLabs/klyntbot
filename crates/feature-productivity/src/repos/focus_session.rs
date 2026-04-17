use chrono::{DateTime, Utc};
use common::time::bridge::jiff_to_chrono;
use jiff::SignedDuration;
use sqlx::SqlitePool;
use tracing::warn;

use crate::types::{DistractionEvent, FocusSession, SessionSource, SessionType};

#[derive(sqlx::FromRow)]
struct SessionRow {
    id: String,
    action_id: Option<String>,
    project_id: Option<String>,
    session_type: String,
    target_mins: Option<i64>,
    started_at: String,
    ended_at: Option<String>,
    actual_mins: Option<i64>,
    interruptions: i64,
    distraction_events: Option<String>,
    quality_score: Option<f64>,
    completed: bool,
    notes: Option<String>,
    source: String,
}

impl From<SessionRow> for FocusSession {
    fn from(row: SessionRow) -> Self {
        let session_type = row.session_type.parse::<SessionType>().unwrap_or_else(|_| {
            warn!(raw = %row.session_type, "unknown session_type in DB, defaulting to Focus");
            SessionType::Focus
        });
        let distraction_events: Vec<DistractionEvent> = row
            .distraction_events
            .as_deref()
            .and_then(|s| match serde_json::from_str(s) {
                Ok(v) => Some(v),
                Err(e) => {
                    warn!(field = "distraction_events", %e, "failed to deserialize JSON from productivity_sessions");
                    None
                }
            })
            .unwrap_or_default();
        let started_at = common::parse_datetime(&row.started_at, "UTC")
            .unwrap_or_else(|| {
                warn!(raw = %row.started_at, "unparseable started_at in productivity_sessions, using now()");
                jiff_to_chrono(jiff::Timestamp::now())
            });
        let ended_at = row
            .ended_at
            .as_deref()
            .and_then(|s| common::parse_datetime(s, "UTC"));
        let source = row.source.parse::<SessionSource>().unwrap_or_else(|_| {
            warn!(raw = %row.source, "unknown source in DB, defaulting to Manual");
            SessionSource::Manual
        });
        Self {
            id: row.id,
            action_id: row.action_id,
            project_id: row.project_id,
            session_type,
            target_mins: row.target_mins,
            started_at,
            ended_at,
            actual_mins: row.actual_mins,
            interruptions: row.interruptions,
            distraction_events,
            quality_score: row.quality_score,
            completed: row.completed,
            notes: row.notes,
            source,
        }
    }
}

const SESSION_COLUMNS: &str = "id, action_id, project_id, session_type, target_mins, started_at, ended_at, actual_mins, interruptions, distraction_events, quality_score, completed, notes, source";

#[derive(Debug, Clone)]
pub struct FocusSessionRepo {
    pool: SqlitePool,
}

impl FocusSessionRepo {
    pub fn new(pool: SqlitePool) -> Self {
        Self { pool }
    }

    pub async fn create(&self, session: &FocusSession) -> common::Result<()> {
        let session_type = session.session_type.to_string();
        let source = session.source.to_string();
        let distractions_json =
            serde_json::to_string(&session.distraction_events).unwrap_or_else(|e| {
                warn!(%e, "failed to serialize distraction_events");
                "[]".to_string()
            });
        sqlx::query(
            r#"INSERT INTO productivity_sessions (id, action_id, project_id, session_type, target_mins, started_at, ended_at, actual_mins, interruptions, distraction_events, quality_score, completed, notes, source)
               VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14)"#,
        )
        .bind(&session.id)
        .bind(&session.action_id)
        .bind(&session.project_id)
        .bind(&session_type)
        .bind(session.target_mins)
        .bind(session.started_at)
        .bind(session.ended_at)
        .bind(session.actual_mins)
        .bind(session.interruptions)
        .bind(&distractions_json)
        .bind(session.quality_score)
        .bind(session.completed)
        .bind(&session.notes)
        .bind(&source)
        .execute(&self.pool)
        .await
        .map_err(|e| common::KlyntbotError::Storage(e.to_string()))?;
        Ok(())
    }

    pub async fn get(&self, id: &str) -> common::Result<Option<FocusSession>> {
        let row = sqlx::query_as::<_, SessionRow>(&format!(
            "SELECT {SESSION_COLUMNS} FROM productivity_sessions WHERE id = ?1"
        ))
        .bind(id)
        .fetch_optional(&self.pool)
        .await
        .map_err(|e| common::KlyntbotError::Storage(e.to_string()))?;
        Ok(row.map(FocusSession::from))
    }

    pub async fn get_active(&self) -> common::Result<Option<FocusSession>> {
        let row = sqlx::query_as::<_, SessionRow>(
            &format!("SELECT {SESSION_COLUMNS} FROM productivity_sessions WHERE ended_at IS NULL ORDER BY started_at DESC LIMIT 1"),
        )
        .fetch_optional(&self.pool)
        .await
        .map_err(|e| common::KlyntbotError::Storage(e.to_string()))?;
        Ok(row.map(FocusSession::from))
    }

    pub async fn update(&self, session: &FocusSession) -> common::Result<()> {
        let session_type = session.session_type.to_string();
        let source = session.source.to_string();
        let distractions_json =
            serde_json::to_string(&session.distraction_events).unwrap_or_else(|e| {
                warn!(%e, "failed to serialize distraction_events");
                "[]".to_string()
            });
        sqlx::query(
            r#"UPDATE productivity_sessions SET
                   action_id = ?2, project_id = ?3, session_type = ?4, target_mins = ?5,
                   ended_at = ?6, actual_mins = ?7, interruptions = ?8,
                   distraction_events = ?9, quality_score = ?10, completed = ?11, notes = ?12,
                   source = ?13, updated_at = datetime('now')
               WHERE id = ?1"#,
        )
        .bind(&session.id)
        .bind(&session.action_id)
        .bind(&session.project_id)
        .bind(&session_type)
        .bind(session.target_mins)
        .bind(session.ended_at)
        .bind(session.actual_mins)
        .bind(session.interruptions)
        .bind(&distractions_json)
        .bind(session.quality_score)
        .bind(session.completed)
        .bind(&session.notes)
        .bind(&source)
        .execute(&self.pool)
        .await
        .map_err(|e| common::KlyntbotError::Storage(e.to_string()))?;
        Ok(())
    }

    pub async fn list_range(
        &self,
        start: &DateTime<Utc>,
        end: &DateTime<Utc>,
        limit: Option<i64>,
    ) -> common::Result<Vec<FocusSession>> {
        let limit = limit.unwrap_or(1000);
        let rows = sqlx::query_as::<_, SessionRow>(
            &format!("SELECT {SESSION_COLUMNS} FROM productivity_sessions WHERE started_at >= ?1 AND started_at < ?2 ORDER BY started_at DESC LIMIT ?3"),
        )
        .bind(start)
        .bind(end)
        .bind(limit)
        .fetch_all(&self.pool)
        .await
        .map_err(|e| common::KlyntbotError::Storage(e.to_string()))?;
        Ok(rows.into_iter().map(FocusSession::from).collect())
    }

    /// Average quality score for focus sessions in a project over the last N days.
    /// Returns `None` if no qualifying sessions exist.
    pub async fn avg_quality_by_project(
        &self,
        project_id: &str,
        days: i64,
    ) -> common::Result<Option<f64>> {
        let cutoff =
            (jiff::Timestamp::now() - SignedDuration::from_secs(days * 86_400)).to_string();
        let row: (Option<f64>,) = sqlx::query_as(
            "SELECT AVG(quality_score) FROM productivity_sessions
             WHERE project_id = ?1 AND quality_score IS NOT NULL AND started_at >= ?2",
        )
        .bind(project_id)
        .bind(&cutoff)
        .fetch_one(&self.pool)
        .await
        .map_err(|e| common::KlyntbotError::Storage(e.to_string()))?;
        Ok(row.0)
    }

    pub async fn list_by_action(&self, action_id: &str) -> common::Result<Vec<FocusSession>> {
        let rows = sqlx::query_as::<_, SessionRow>(
            &format!("SELECT {SESSION_COLUMNS} FROM productivity_sessions WHERE action_id = ?1 ORDER BY started_at DESC"),
        )
        .bind(action_id)
        .fetch_all(&self.pool)
        .await
        .map_err(|e| common::KlyntbotError::Storage(e.to_string()))?;
        Ok(rows.into_iter().map(FocusSession::from).collect())
    }
}
