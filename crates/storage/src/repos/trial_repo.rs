//! Repository for autotuner experiment and trial tables.

use chrono::{DateTime, Utc};
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
    message_id                 TEXT    NOT NULL DEFAULT '',
    predicted_orchestrator     TEXT    NOT NULL,
    predicted_mode             TEXT    NOT NULL,
    confidence                 REAL    NOT NULL,
    predicted_iteration_budget INTEGER NOT NULL,
    control_orchestrator       TEXT    NOT NULL,
    control_mode               TEXT    NOT NULL,
    user_corrected             INTEGER NOT NULL DEFAULT 0,
    tokens_used                INTEGER,
    response_time_ms           INTEGER,
    created_at                 TEXT    NOT NULL DEFAULT (datetime('now'))
);

CREATE INDEX IF NOT EXISTS idx_autotuner_shadow_log_trial_id
    ON autotuner_shadow_log(trial_id);

CREATE INDEX IF NOT EXISTS idx_autotuner_shadow_log_chat_id
    ON autotuner_shadow_log(chat_id);

CREATE TABLE IF NOT EXISTS autotuner_shadow_retrieval_log (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    trial_id TEXT NOT NULL REFERENCES autotuner_trials(id),
    chat_id TEXT NOT NULL,
    message_timestamp TEXT NOT NULL,
    variant_retrieved_count INTEGER NOT NULL,
    control_retrieved_count INTEGER NOT NULL,
    overlap_count INTEGER NOT NULL,
    variant_avg_score REAL NOT NULL,
    variant_avg_age_days REAL NOT NULL,
    created_at TEXT NOT NULL DEFAULT (datetime('now'))
);
CREATE INDEX IF NOT EXISTS idx_shadow_retrieval_log_trial_created
    ON autotuner_shadow_retrieval_log(trial_id, created_at);
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

    /// Deactivate stale active trials: older than `max_age_days` with fewer
    /// than `min_messages` shadow log entries. Returns the number expired.
    pub async fn expire_stale(
        &self,
        max_age_days: u32,
        min_messages: u32,
    ) -> Result<u32, StorageError> {
        let cutoff =
            (chrono::Utc::now() - chrono::Duration::days(max_age_days as i64)).to_rfc3339();
        let result = sqlx::query(
            "UPDATE autotuner_trials SET status = 'completed', completed_at = datetime('now')
             WHERE status = 'active'
               AND created_at < ?1
               AND (SELECT COUNT(*) FROM autotuner_shadow_log
                    WHERE autotuner_shadow_log.trial_id = autotuner_trials.id) < ?2",
        )
        .bind(&cutoff)
        .bind(min_messages)
        .execute(&self.pool)
        .await?;
        Ok(result.rows_affected() as u32)
    }

    /// Count trials with status `active`.
    pub async fn count_active(&self) -> Result<u32, StorageError> {
        let row: (i64,) =
            sqlx::query_as("SELECT COUNT(*) FROM autotuner_trials WHERE status = 'active'")
                .fetch_one(&self.pool)
                .await?;
        Ok(row.0 as u32)
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

    /// Insert a shadow log entry recording a shadow classification prediction.
    #[allow(clippy::too_many_arguments)]
    pub async fn insert_shadow_log(
        &self,
        trial_id: &str,
        message_timestamp: &str,
        chat_id: &str,
        message_id: &str,
        predicted_orchestrator: &str,
        predicted_mode: &str,
        confidence: f64,
        predicted_iteration_budget: i64,
        control_orchestrator: &str,
        control_mode: &str,
    ) -> Result<(), StorageError> {
        sqlx::query(
            "INSERT INTO autotuner_shadow_log
                (trial_id, message_timestamp, chat_id, message_id, predicted_orchestrator,
                 predicted_mode, confidence, predicted_iteration_budget,
                 control_orchestrator, control_mode)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)",
        )
        .bind(trial_id)
        .bind(message_timestamp)
        .bind(chat_id)
        .bind(message_id)
        .bind(predicted_orchestrator)
        .bind(predicted_mode)
        .bind(confidence)
        .bind(predicted_iteration_budget)
        .bind(control_orchestrator)
        .bind(control_mode)
        .execute(&self.pool)
        .await?;
        Ok(())
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

    // ── Shadow log helpers ────────────────────────────────────────────

    /// Back-fill the ground-truth orchestrator and mode for a recent shadow log
    /// entry whose control fields were originally set to `'pending'`.
    pub async fn update_shadow_log_ground_truth(
        &self,
        chat_id: &str,
        message_id: &str,
        control_orchestrator: &str,
        control_mode: &str,
    ) -> Result<(), StorageError> {
        sqlx::query(
            "UPDATE autotuner_shadow_log
             SET control_orchestrator = ?1, control_mode = ?2
             WHERE chat_id = ?3 AND message_id = ?4
               AND control_orchestrator = 'pending'
               AND created_at >= datetime('now', '-60 seconds')",
        )
        .bind(control_orchestrator)
        .bind(control_mode)
        .bind(chat_id)
        .bind(message_id)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    /// Update the most recent shadow log entry for a chat with execution metrics.
    pub async fn update_shadow_log_metrics(
        &self,
        chat_id: &str,
        tokens_used: u32,
        response_time_ms: u64,
    ) -> Result<(), sqlx::Error> {
        sqlx::query(
            "UPDATE autotuner_shadow_log
             SET tokens_used = ?1, response_time_ms = ?2
             WHERE id = (
                 SELECT id FROM autotuner_shadow_log
                 WHERE chat_id = ?3
                 ORDER BY created_at DESC LIMIT 1
             )",
        )
        .bind(tokens_used as i64)
        .bind(response_time_ms as i64)
        .bind(chat_id)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    /// Flag the two most recent shadow log entries for a chat within the given
    /// window as user-corrected.  Uses a subquery because SQLite does not
    /// support `ORDER BY` / `LIMIT` directly on `UPDATE`.
    pub async fn mark_recent_messages_corrected(
        &self,
        chat_id: &str,
        window_minutes: i32,
    ) -> Result<(), StorageError> {
        sqlx::query(
            "UPDATE autotuner_shadow_log
             SET user_corrected = 1
             WHERE id IN (
                 SELECT id FROM autotuner_shadow_log
                 WHERE chat_id = ?1
                   AND created_at >= datetime('now', ?2)
                 ORDER BY created_at DESC LIMIT 2
             )",
        )
        .bind(chat_id)
        .bind(format!("-{} minutes", window_minutes))
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    /// Compute the fraction of shadow log entries where the predicted mode
    /// matched the ground-truth control mode.  Returns `1.0` when there are no
    /// qualifying rows (no data ⇒ perfect agreement by convention).
    pub async fn shadow_log_agreement_rate(
        &self,
        trial_id: Option<&str>,
        since: DateTime<Utc>,
    ) -> Result<f64, StorageError> {
        // Format as YYYY-MM-DD HH:MM:SS to match SQLite's datetime() output.
        let since_str = since.format("%Y-%m-%d %H:%M:%S").to_string();
        let (total, agreed): (i64, i64) = if let Some(tid) = trial_id {
            sqlx::query_as::<_, (i64, i64)>(
                "SELECT COUNT(*) AS total,
                        COALESCE(SUM(CASE WHEN predicted_mode = control_mode THEN 1 ELSE 0 END), 0) AS agreed
                 FROM autotuner_shadow_log
                 WHERE trial_id = ?1 AND control_mode != 'pending'
                   AND created_at >= ?2",
            )
            .bind(tid)
            .bind(&since_str)
            .fetch_one(&self.pool)
            .await?
        } else {
            sqlx::query_as::<_, (i64, i64)>(
                "SELECT COUNT(*) AS total,
                        COALESCE(SUM(CASE WHEN predicted_mode = control_mode THEN 1 ELSE 0 END), 0) AS agreed
                 FROM autotuner_shadow_log
                 WHERE control_mode != 'pending' AND created_at >= ?1",
            )
            .bind(&since_str)
            .fetch_one(&self.pool)
            .await?
        };
        Ok(if total == 0 {
            1.0
        } else {
            agreed as f64 / total as f64
        })
    }

    /// Compute the correction rate for a specific trial from the shadow log.
    /// Returns `(total, corrected)` where `total` is the count of non-pending
    /// rows and `corrected` is the count where `user_corrected = 1`.
    pub async fn correction_rate_for_trial(
        &self,
        trial_id: &str,
        since: DateTime<Utc>,
    ) -> Result<(i64, i64), StorageError> {
        let since_str = since.format("%Y-%m-%d %H:%M:%S").to_string();
        sqlx::query_as::<_, (i64, i64)>(
            "SELECT COUNT(*) AS total,
                    COALESCE(SUM(CASE WHEN user_corrected = 1 THEN 1 ELSE 0 END), 0) AS corrected
             FROM autotuner_shadow_log
             WHERE trial_id = ?1 AND created_at >= ?2 AND control_mode != 'pending'",
        )
        .bind(trial_id)
        .bind(&since_str)
        .fetch_one(&self.pool)
        .await
        .map_err(Into::into)
    }

    // ── Trial counting helpers ────────────────────────────────────────

    /// Count trials that reached a terminal status (completed, promoted, or
    /// reverted) since the given timestamp.
    pub async fn count_trials_since(&self, since: DateTime<Utc>) -> Result<i64, StorageError> {
        let since_str = since.format("%Y-%m-%d %H:%M:%S").to_string();
        Ok(sqlx::query_scalar::<_, i64>(
            "SELECT COUNT(*) FROM autotuner_trials
             WHERE status IN ('completed', 'promoted', 'reverted')
               AND completed_at >= ?1",
        )
        .bind(&since_str)
        .fetch_one(&self.pool)
        .await?)
    }

    // ── Shadow retrieval log (Phase 2) ───────────────────────────────

    /// Insert a shadow retrieval log entry for Phase 2 memory scoring.
    #[allow(clippy::too_many_arguments)]
    pub async fn insert_shadow_retrieval_log(
        &self,
        trial_id: &str,
        chat_id: &str,
        message_timestamp: &str,
        variant_retrieved_count: i64,
        control_retrieved_count: i64,
        overlap_count: i64,
        variant_avg_score: f64,
        variant_avg_age_days: f64,
    ) -> Result<(), StorageError> {
        sqlx::query(
            "INSERT INTO autotuner_shadow_retrieval_log
                (trial_id, chat_id, message_timestamp,
                 variant_retrieved_count, control_retrieved_count, overlap_count,
                 variant_avg_score, variant_avg_age_days)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
        )
        .bind(trial_id)
        .bind(chat_id)
        .bind(message_timestamp)
        .bind(variant_retrieved_count)
        .bind(control_retrieved_count)
        .bind(overlap_count)
        .bind(variant_avg_score)
        .bind(variant_avg_age_days)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    /// Retrieval precision for a trial: average overlap_count / variant_retrieved_count.
    pub async fn retrieval_precision_for_trial(
        &self,
        trial_id: &str,
        since: DateTime<Utc>,
    ) -> Result<f64, StorageError> {
        let since_str = since.format("%Y-%m-%d %H:%M:%S").to_string();
        let result = sqlx::query_as::<_, (f64,)>(
            "SELECT COALESCE(AVG(CASE WHEN variant_retrieved_count > 0
                THEN CAST(overlap_count AS REAL) / variant_retrieved_count
                ELSE 0.0 END), 0.0)
             FROM autotuner_shadow_retrieval_log
             WHERE trial_id = ?1 AND created_at >= ?2",
        )
        .bind(trial_id)
        .bind(&since_str)
        .fetch_one(&self.pool)
        .await?;
        Ok(result.0)
    }

    /// Average memory freshness for a trial (avg_age_days from shadow retrieval).
    pub async fn avg_memory_freshness_for_trial(
        &self,
        trial_id: &str,
        since: DateTime<Utc>,
    ) -> Result<f64, StorageError> {
        let since_str = since.format("%Y-%m-%d %H:%M:%S").to_string();
        let result = sqlx::query_as::<_, (f64,)>(
            "SELECT COALESCE(AVG(variant_avg_age_days), 0.0)
             FROM autotuner_shadow_retrieval_log
             WHERE trial_id = ?1 AND created_at >= ?2",
        )
        .bind(trial_id)
        .bind(&since_str)
        .fetch_one(&self.pool)
        .await?;
        Ok(result.0)
    }

    /// Count trials that were promoted since the given timestamp.
    pub async fn count_promoted_since(&self, since: DateTime<Utc>) -> Result<i64, StorageError> {
        let since_str = since.format("%Y-%m-%d %H:%M:%S").to_string();
        Ok(sqlx::query_scalar::<_, i64>(
            "SELECT COUNT(*) FROM autotuner_trials
             WHERE status = 'promoted' AND completed_at >= ?1",
        )
        .bind(&since_str)
        .fetch_one(&self.pool)
        .await?)
    }
}

#[cfg(test)]
mod tests {
    use chrono::Utc;

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

    /// Helper to insert a shadow log entry with sensible defaults for tests.
    async fn insert_test_shadow_log(
        repo: &TrialRepo,
        trial_id: &str,
        chat_id: &str,
        predicted_mode: &str,
        control_orchestrator: &str,
        control_mode: &str,
    ) {
        repo.insert_shadow_log(
            trial_id,
            "2026-03-19T12:00:00Z",
            chat_id,
            "",
            "general",
            predicted_mode,
            0.85,
            5,
            control_orchestrator,
            control_mode,
        )
        .await
        .unwrap();
    }

    #[tokio::test]
    async fn update_ground_truth_fills_pending() {
        let repo = setup().await;
        let exp = make_experiment("exp-gt");
        repo.create_experiment(&exp).await.unwrap();
        let trial = make_trial("trial-gt", "exp-gt", "active");
        repo.create_trial(&trial).await.unwrap();

        // Insert a shadow log with control_orchestrator = "pending"
        insert_test_shadow_log(&repo, "trial-gt", "chat-1", "direct", "pending", "pending").await;

        // Update the ground truth
        repo.update_shadow_log_ground_truth("chat-1", "", "general", "reactive")
            .await
            .unwrap();

        // Verify via agreement rate — predicted_mode="direct" vs control_mode="reactive"
        // means 0% agreement
        let rate = repo
            .shadow_log_agreement_rate(Some("trial-gt"), Utc::now() - chrono::Duration::hours(1))
            .await
            .unwrap();
        assert!(
            (rate - 0.0).abs() < f64::EPSILON,
            "Expected 0.0 agreement when predicted != control, got {rate}"
        );
    }

    #[tokio::test]
    async fn mark_corrected_within_window() {
        let pool = sqlx::SqlitePool::connect("sqlite::memory:").await.unwrap();
        sqlx::query("PRAGMA foreign_keys=ON;")
            .execute(&pool)
            .await
            .unwrap();
        sqlx::query(MIGRATION_SQL).execute(&pool).await.unwrap();
        let repo = TrialRepo::new(pool.clone());

        let exp = make_experiment("exp-mc");
        repo.create_experiment(&exp).await.unwrap();
        let trial = make_trial("trial-mc", "exp-mc", "active");
        repo.create_trial(&trial).await.unwrap();

        // Insert two shadow log entries for the same chat
        insert_test_shadow_log(&repo, "trial-mc", "chat-2", "direct", "general", "direct").await;
        insert_test_shadow_log(
            &repo, "trial-mc", "chat-2", "reactive", "general", "reactive",
        )
        .await;

        // Mark recent messages as corrected (large window to catch them)
        repo.mark_recent_messages_corrected("chat-2", 60)
            .await
            .unwrap();

        // Verify that user_corrected was actually set to 1 in the database.
        let corrected = sqlx::query_scalar::<_, i64>(
            "SELECT user_corrected FROM autotuner_shadow_log WHERE chat_id = ?1 ORDER BY created_at DESC LIMIT 1",
        )
        .bind("chat-2")
        .fetch_one(&pool)
        .await
        .unwrap();
        assert_eq!(corrected, 1, "user_corrected should be set to 1");
    }

    #[tokio::test]
    async fn agreement_rate_no_data_returns_one() {
        let repo = setup().await;

        let rate = repo
            .shadow_log_agreement_rate(None, Utc::now() - chrono::Duration::hours(1))
            .await
            .unwrap();

        assert!(
            (rate - 1.0).abs() < f64::EPSILON,
            "Expected 1.0 when no shadow log data exists, got {rate}"
        );
    }

    #[tokio::test]
    async fn correction_rate_for_trial_computes_correctly() {
        let repo = setup().await;

        // Setup: create experiment + trial
        let exp = make_experiment("exp-cr");
        repo.create_experiment(&exp).await.unwrap();
        let trial = make_trial("trial-cr", "exp-cr", "active");
        repo.create_trial(&trial).await.unwrap();

        // Insert 3 shadow log rows with control_mode = 'pending' initially
        insert_test_shadow_log(
            &repo,
            "trial-cr",
            "chat-cr-1",
            "direct",
            "pending",
            "pending",
        )
        .await;
        insert_test_shadow_log(
            &repo,
            "trial-cr",
            "chat-cr-2",
            "direct",
            "pending",
            "pending",
        )
        .await;
        insert_test_shadow_log(
            &repo,
            "trial-cr",
            "chat-cr-3",
            "direct",
            "pending",
            "pending",
        )
        .await;

        // Back-fill ground truth so control_mode != 'pending'
        repo.update_shadow_log_ground_truth("chat-cr-1", "", "general", "direct")
            .await
            .unwrap();
        repo.update_shadow_log_ground_truth("chat-cr-2", "", "general", "direct")
            .await
            .unwrap();
        repo.update_shadow_log_ground_truth("chat-cr-3", "", "general", "direct")
            .await
            .unwrap();

        // Mark one entry as user-corrected
        repo.mark_recent_messages_corrected("chat-cr-3", 60)
            .await
            .unwrap();

        let since = Utc::now() - chrono::Duration::hours(1);
        let (total, corrected) = repo
            .correction_rate_for_trial("trial-cr", since)
            .await
            .unwrap();

        assert_eq!(total, 3, "Expected 3 non-pending rows");
        assert_eq!(corrected, 1, "Expected 1 corrected row");
    }

    #[tokio::test]
    async fn correction_rate_for_trial_empty_returns_zero() {
        let repo = setup().await;

        let since = Utc::now() - chrono::Duration::hours(1);
        let (total, corrected) = repo
            .correction_rate_for_trial("nonexistent-trial", since)
            .await
            .unwrap();

        assert_eq!(total, 0, "Expected 0 total on empty data");
        assert_eq!(corrected, 0, "Expected 0 corrected on empty data");
    }

    #[tokio::test]
    async fn count_trials_and_promoted_empty() {
        let repo = setup().await;

        let since = Utc::now() - chrono::Duration::days(7);
        let total = repo.count_trials_since(since).await.unwrap();
        let promoted = repo.count_promoted_since(since).await.unwrap();

        assert_eq!(total, 0, "Expected 0 completed trials on empty DB");
        assert_eq!(promoted, 0, "Expected 0 promoted trials on empty DB");
    }
}
