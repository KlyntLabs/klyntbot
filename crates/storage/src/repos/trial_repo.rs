//! Repository for autotuner experiment and trial tables.

use sqlx::SqlitePool;

use crate::error::StorageError;
use crate::rows::trial::{ExperimentRow, TrialRow};

/// DDL for the autotuner tables.  Execute once before the repo is used
/// (typically during feature migration or in-memory test setup).
pub const MIGRATION_SQL: &str = r#"
CREATE TABLE IF NOT EXISTS autotuner_experiments (
    id                      TEXT PRIMARY KEY,
    hypothesis              TEXT NOT NULL,
    trend_analysis          TEXT NOT NULL,
    recommendation_for_next TEXT NOT NULL,
    created_at              TEXT NOT NULL DEFAULT (datetime('now'))
);

CREATE TABLE IF NOT EXISTS autotuner_trials (
    id                    TEXT PRIMARY KEY,
    experiment_id         TEXT NOT NULL REFERENCES autotuner_experiments(id),
    params                TEXT NOT NULL,
    generation_reasoning  TEXT NOT NULL,
    status                TEXT NOT NULL DEFAULT 'pending',
    created_at            TEXT NOT NULL DEFAULT (datetime('now')),
    completed_at          TEXT,
    result                TEXT
);

CREATE INDEX IF NOT EXISTS idx_autotuner_trials_status
    ON autotuner_trials(status);

CREATE INDEX IF NOT EXISTS idx_autotuner_trials_experiment_id
    ON autotuner_trials(experiment_id);

CREATE TABLE IF NOT EXISTS autotuner_shadow_log (
    id                         INTEGER PRIMARY KEY AUTOINCREMENT,
    trial_id                   TEXT    NOT NULL REFERENCES autotuner_trials(id),
    message_timestamp          TEXT    NOT NULL,
    chat_id                    TEXT    NOT NULL,
    predicted_orchestrator     TEXT    NOT NULL,
    predicted_mode             TEXT    NOT NULL,
    confidence                 REAL    NOT NULL,
    predicted_iteration_budget INTEGER NOT NULL,
    control_orchestrator       TEXT    NOT NULL,
    control_mode               TEXT    NOT NULL,
    user_corrected             INTEGER NOT NULL DEFAULT 0,
    created_at                 TEXT    NOT NULL DEFAULT (datetime('now'))
);

CREATE INDEX IF NOT EXISTS idx_autotuner_shadow_log_trial_id
    ON autotuner_shadow_log(trial_id);
"#;

/// Repository for autotuner experiment + trial persistence.
#[derive(Debug, Clone)]
pub struct TrialRepo {
    pool: SqlitePool,
}

impl TrialRepo {
    pub fn new(pool: SqlitePool) -> Self {
        Self { pool }
    }

    /// Run the autotuner DDL migrations against the underlying pool.
    pub async fn migrate(&self) -> Result<(), StorageError> {
        sqlx::query(MIGRATION_SQL).execute(&self.pool).await?;
        Ok(())
    }

    // ── Experiments ─────────────────────────────────────────────────────

    /// Insert a new experiment record.
    pub async fn create_experiment(&self, row: &ExperimentRow) -> Result<(), StorageError> {
        sqlx::query(
            "INSERT INTO autotuner_experiments
                (id, hypothesis, trend_analysis, recommendation_for_next, created_at)
             VALUES (?1, ?2, ?3, ?4, ?5)",
        )
        .bind(&row.id)
        .bind(&row.hypothesis)
        .bind(&row.trend_analysis)
        .bind(&row.recommendation_for_next)
        .bind(&row.created_at)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    /// Return the most recent experiments, newest first.
    pub async fn get_experiments(&self, limit: u32) -> Result<Vec<ExperimentRow>, StorageError> {
        let rows = sqlx::query_as::<_, ExperimentRow>(
            "SELECT * FROM autotuner_experiments ORDER BY created_at DESC LIMIT ?1",
        )
        .bind(limit)
        .fetch_all(&self.pool)
        .await?;
        Ok(rows)
    }

    // ── Trials ──────────────────────────────────────────────────────────

    /// Insert a new trial record.
    pub async fn create_trial(&self, row: &TrialRow) -> Result<(), StorageError> {
        sqlx::query(
            "INSERT INTO autotuner_trials
                (id, experiment_id, params, generation_reasoning, status, created_at,
                 completed_at, result)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
        )
        .bind(&row.id)
        .bind(&row.experiment_id)
        .bind(&row.params)
        .bind(&row.generation_reasoning)
        .bind(&row.status)
        .bind(&row.created_at)
        .bind(&row.completed_at)
        .bind(&row.result)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    /// Update only the status field of a trial.
    pub async fn update_trial_status(&self, id: &str, status: &str) -> Result<(), StorageError> {
        sqlx::query("UPDATE autotuner_trials SET status = ?1 WHERE id = ?2")
            .bind(status)
            .bind(id)
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    /// Mark a trial as completed, recording its JSON result and timestamp.
    pub async fn complete_trial(&self, id: &str, result_json: &str) -> Result<(), StorageError> {
        sqlx::query(
            "UPDATE autotuner_trials
             SET status = 'completed',
                 completed_at = datetime('now'),
                 result = ?1
             WHERE id = ?2",
        )
        .bind(result_json)
        .bind(id)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    /// Return all trials with status `active`.
    pub async fn get_active_trials(&self) -> Result<Vec<TrialRow>, StorageError> {
        let rows = sqlx::query_as::<_, TrialRow>(
            "SELECT * FROM autotuner_trials WHERE status = 'active' ORDER BY created_at ASC",
        )
        .fetch_all(&self.pool)
        .await?;
        Ok(rows)
    }

    /// Return the most recently completed trials, newest first.
    pub async fn get_recent_completed(&self, limit: u32) -> Result<Vec<TrialRow>, StorageError> {
        let rows = sqlx::query_as::<_, TrialRow>(
            "SELECT * FROM autotuner_trials
             WHERE status = 'completed'
             ORDER BY completed_at DESC
             LIMIT ?1",
        )
        .bind(limit)
        .fetch_all(&self.pool)
        .await?;
        Ok(rows)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    async fn setup() -> TrialRepo {
        let pool = sqlx::SqlitePool::connect("sqlite::memory:").await.unwrap();
        sqlx::query("PRAGMA foreign_keys=ON;")
            .execute(&pool)
            .await
            .unwrap();
        sqlx::query(MIGRATION_SQL).execute(&pool).await.unwrap();
        TrialRepo::new(pool)
    }

    fn make_experiment(id: &str) -> ExperimentRow {
        ExperimentRow {
            id: id.to_string(),
            hypothesis: "Reducing iteration budget improves latency".to_string(),
            trend_analysis: "p95 latency increased 200ms over last 7 days".to_string(),
            recommendation_for_next: "Try budget=3 for simple queries".to_string(),
            created_at: "2026-03-19T00:00:00Z".to_string(),
        }
    }

    fn make_trial(id: &str, experiment_id: &str, status: &str) -> TrialRow {
        TrialRow {
            id: id.to_string(),
            experiment_id: experiment_id.to_string(),
            params: r#"{"iterationBudget":3}"#.to_string(),
            generation_reasoning: "Lower budget to cut latency".to_string(),
            status: status.to_string(),
            created_at: "2026-03-19T00:01:00Z".to_string(),
            completed_at: None,
            result: None,
        }
    }

    #[tokio::test]
    async fn create_and_retrieve_trial() {
        let repo = setup().await;

        let exp = make_experiment("exp-1");
        repo.create_experiment(&exp).await.unwrap();

        let trial = make_trial("trial-1", "exp-1", "active");
        repo.create_trial(&trial).await.unwrap();

        let active = repo.get_active_trials().await.unwrap();
        assert_eq!(active.len(), 1);
        assert_eq!(active[0].id, "trial-1");
        assert_eq!(active[0].experiment_id, "exp-1");
        assert_eq!(active[0].status, "active");

        let experiments = repo.get_experiments(10).await.unwrap();
        assert_eq!(experiments.len(), 1);
        assert_eq!(experiments[0].id, "exp-1");
    }

    #[tokio::test]
    async fn complete_trial_sets_result() {
        let repo = setup().await;

        let exp = make_experiment("exp-2");
        repo.create_experiment(&exp).await.unwrap();

        let trial = make_trial("trial-2", "exp-2", "active");
        repo.create_trial(&trial).await.unwrap();

        let result_json = r#"{"acceptanceRate":0.82,"latencyDeltaMs":-45}"#;
        repo.complete_trial("trial-2", result_json).await.unwrap();

        let completed = repo.get_recent_completed(10).await.unwrap();
        assert_eq!(completed.len(), 1);
        assert_eq!(completed[0].id, "trial-2");
        assert_eq!(completed[0].status, "completed");
        assert_eq!(completed[0].result.as_deref(), Some(result_json));
        assert!(completed[0].completed_at.is_some());

        // Should no longer appear in active list
        let active = repo.get_active_trials().await.unwrap();
        assert!(active.is_empty());
    }
}
