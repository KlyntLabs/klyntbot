use common::time::bridge::jiff_to_chrono;
use sqlx::SqlitePool;
use tracing::warn;

use crate::types::{NudgeRecord, NudgeType};

#[derive(sqlx::FromRow)]
struct NudgeRow {
    id: Option<i64>,
    nudge_type: String,
    message: String,
    channel: Option<String>,
    acknowledged: bool,
    created_at: String,
}

impl From<NudgeRow> for NudgeRecord {
    fn from(row: NudgeRow) -> Self {
        let nudge_type = row.nudge_type.parse::<NudgeType>().unwrap_or_else(|_| {
            warn!(raw = %row.nudge_type, "unknown nudge_type in DB, defaulting to BreakReminder");
            NudgeType::BreakReminder
        });
        let created_at = common::parse_datetime(&row.created_at, "UTC").unwrap_or_else(|| {
            warn!(raw = %row.created_at, "unparseable created_at in nudge_history, using now()");
            jiff_to_chrono(jiff::Timestamp::now())
        });
        Self {
            id: row.id,
            nudge_type,
            message: row.message,
            channel: row.channel,
            acknowledged: row.acknowledged,
            created_at,
        }
    }
}

#[derive(Debug, Clone)]
pub struct NudgeRepo {
    pool: SqlitePool,
}

impl NudgeRepo {
    pub fn new(pool: SqlitePool) -> Self {
        Self { pool }
    }

    pub async fn insert(&self, nudge: &NudgeRecord) -> common::Result<i64> {
        let nudge_type = nudge.nudge_type.to_string();
        let id = sqlx::query_scalar::<_, i64>(
            r#"INSERT INTO nudge_history (nudge_type, message, channel, acknowledged, created_at)
               VALUES (?1, ?2, ?3, ?4, ?5)
               RETURNING id"#,
        )
        .bind(&nudge_type)
        .bind(&nudge.message)
        .bind(&nudge.channel)
        .bind(nudge.acknowledged)
        .bind(nudge.created_at)
        .fetch_one(&self.pool)
        .await
        .map_err(|e| common::KlyntbotError::Storage(e.to_string()))?;
        Ok(id)
    }

    pub async fn last_of_type(&self, nudge_type: NudgeType) -> common::Result<Option<NudgeRecord>> {
        let type_str = nudge_type.to_string();
        let row = sqlx::query_as::<_, NudgeRow>(
            r#"SELECT id, nudge_type, message, channel, acknowledged, created_at
               FROM nudge_history WHERE nudge_type = ?1 ORDER BY created_at DESC LIMIT 1"#,
        )
        .bind(&type_str)
        .fetch_optional(&self.pool)
        .await
        .map_err(|e| common::KlyntbotError::Storage(e.to_string()))?;
        Ok(row.map(NudgeRecord::from))
    }

    pub async fn acknowledge(&self, id: i64) -> common::Result<()> {
        sqlx::query("UPDATE nudge_history SET acknowledged = TRUE WHERE id = ?1")
            .bind(id)
            .execute(&self.pool)
            .await
            .map_err(|e| common::KlyntbotError::Storage(e.to_string()))?;
        Ok(())
    }

    pub async fn list_recent(&self, limit: i64) -> common::Result<Vec<NudgeRecord>> {
        let rows = sqlx::query_as::<_, NudgeRow>(
            r#"SELECT id, nudge_type, message, channel, acknowledged, created_at
               FROM nudge_history ORDER BY created_at DESC LIMIT ?1"#,
        )
        .bind(limit)
        .fetch_all(&self.pool)
        .await
        .map_err(|e| common::KlyntbotError::Storage(e.to_string()))?;
        Ok(rows.into_iter().map(NudgeRecord::from).collect())
    }
}
