//! Repository for persisting enhancement pipeline traces for Reforge analysis.

use sqlx::SqlitePool;

pub struct EnhancementTraceRepo {
    pool: SqlitePool,
}

const MIGRATION: &str = "
CREATE TABLE IF NOT EXISTS enhancement_trace_log (
    id               TEXT PRIMARY KEY,
    session_key      TEXT NOT NULL,
    depth_mode       TEXT NOT NULL,
    stages_json      TEXT NOT NULL,
    total_latency_ms INTEGER NOT NULL,
    total_llm_calls  INTEGER NOT NULL,
    query_confidence REAL NOT NULL,
    created_at       TEXT NOT NULL DEFAULT (datetime('now'))
);
CREATE INDEX IF NOT EXISTS idx_enhancement_trace_created ON enhancement_trace_log(created_at);
";

#[derive(Debug, Clone, sqlx::FromRow)]
pub struct EnhancementAggregate {
    pub depth_mode: String,
    pub total_runs: i64,
    pub avg_latency_ms: f64,
    pub avg_llm_calls: f64,
    pub avg_confidence: f64,
}

impl EnhancementTraceRepo {
    pub fn new(pool: SqlitePool) -> Self {
        Self { pool }
    }

    /// Run the table migration to create the enhancement_trace_log table and index.
    pub async fn migrate(&self) -> Result<(), sqlx::Error> {
        for stmt in MIGRATION.split(';') {
            let stmt = stmt.trim();
            if stmt.is_empty() {
                continue;
            }
            sqlx::query(stmt).execute(&self.pool).await?;
        }
        Ok(())
    }

    /// Insert a new enhancement pipeline trace record.
    #[allow(clippy::too_many_arguments)]
    pub async fn insert(
        &self,
        id: &str,
        session_key: &str,
        depth_mode: &str,
        stages_json: &str,
        total_latency_ms: i64,
        total_llm_calls: i64,
        query_confidence: f64,
    ) -> Result<(), sqlx::Error> {
        sqlx::query(
            "INSERT INTO enhancement_trace_log \
             (id, session_key, depth_mode, stages_json, total_latency_ms, total_llm_calls, query_confidence) \
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
        )
        .bind(id)
        .bind(session_key)
        .bind(depth_mode)
        .bind(stages_json)
        .bind(total_latency_ms)
        .bind(total_llm_calls)
        .bind(query_confidence)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    /// Delete trace records older than the given number of days.
    /// Returns the number of rows deleted.
    pub async fn delete_older_than(&self, days: i64) -> Result<u64, sqlx::Error> {
        let result = sqlx::query(
            "DELETE FROM enhancement_trace_log \
             WHERE julianday('now') - julianday(created_at) > ?1",
        )
        .bind(days)
        .execute(&self.pool)
        .await?;
        Ok(result.rows_affected())
    }

    /// Load aggregated enhancement metrics grouped by depth_mode since the given timestamp.
    pub async fn load_aggregates_since(
        &self,
        since: &str,
    ) -> Result<Vec<EnhancementAggregate>, sqlx::Error> {
        sqlx::query_as::<_, EnhancementAggregate>(
            "SELECT depth_mode,
                    COUNT(*) AS total_runs,
                    AVG(CAST(total_latency_ms AS REAL)) AS avg_latency_ms,
                    AVG(CAST(total_llm_calls AS REAL)) AS avg_llm_calls,
                    AVG(query_confidence) AS avg_confidence
             FROM enhancement_trace_log
             WHERE created_at >= ?1
             GROUP BY depth_mode",
        )
        .bind(since)
        .fetch_all(&self.pool)
        .await
    }
}
