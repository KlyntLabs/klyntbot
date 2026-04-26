//! Pattern effectiveness EMA source + repo.
//!
//! Accumulates `PatternOutcome` domain events, computes an EMA update on flush,
//! and writes audit rows to `pattern_effectiveness_log`.

use ai_core::{mirror::mirror_flush_secs, AiSignal, MirrorSignalSource, MirrorSnapshotSpec};
use async_trait::async_trait;
use common::{KlyntbotError, Result};
use tokio::sync::Mutex;
use uuid::Uuid;

/// One buffered pattern-outcome event.
#[derive(Debug, Clone)]
struct BufferedOutcome {
    pattern_id: String,
    outcome: String,
    evidence: String,
}

/// Repo for the `pattern_effectiveness_log` table.
#[derive(Debug, Clone)]
pub struct PatternEffectivenessLogRepo {
    pool: storage::StoragePool,
}

impl PatternEffectivenessLogRepo {
    /// Construct.
    pub fn new(pool: storage::StoragePool) -> Self {
        Self { pool }
    }

    /// Pool ref.
    pub fn pool(&self) -> &storage::StoragePool {
        &self.pool
    }

    /// Insert one effectiveness log row.
    #[allow(clippy::too_many_arguments)]
    pub async fn insert(
        &self,
        pattern_id: &str,
        pattern_kind: &str,
        repo_id: Option<&str>,
        outcome: &str,
        score_before: f32,
        score_after: f32,
        evidence: Option<&str>,
    ) -> Result<()> {
        sqlx::query(
            "INSERT INTO pattern_effectiveness_log \
             (id, pattern_id, pattern_kind, repo_id, outcome, score_before, score_after, evidence) \
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
        )
        .bind(format!("pel_{}", Uuid::new_v4().simple()))
        .bind(pattern_id)
        .bind(pattern_kind)
        .bind(repo_id)
        .bind(outcome)
        .bind(score_before)
        .bind(score_after)
        .bind(evidence)
        .execute(self.pool.inner())
        .await
        .map_err(|e| KlyntbotError::Storage(format!("pattern_effectiveness_log insert: {e}")))?;
        Ok(())
    }

    /// Fetch recent log rows for a pattern.
    pub async fn recent(
        &self,
        pattern_id: &str,
        limit: u32,
    ) -> Result<Vec<(String, String, f32, f32)>> {
        let rows: Vec<(String, String, f32, f32)> = sqlx::query_as(
            "SELECT outcome, measured_at, score_before, score_after \
             FROM pattern_effectiveness_log \
             WHERE pattern_id = ?1 \
             ORDER BY measured_at DESC LIMIT ?2",
        )
        .bind(pattern_id)
        .bind(limit as i64)
        .fetch_all(self.pool.inner())
        .await
        .map_err(|e| KlyntbotError::Storage(format!("pattern_effectiveness_log recent: {e}")))?;
        Ok(rows)
    }
}

/// Source that accumulates `PatternOutcome` events and flushes EMA updates.
pub struct PatternEffectivenessSource {
    pool: storage::StoragePool,
    log: PatternEffectivenessLogRepo,
    pending: Mutex<Vec<BufferedOutcome>>,
}

impl PatternEffectivenessSource {
    /// Construct.
    pub fn new(pool: storage::StoragePool, log: PatternEffectivenessLogRepo) -> Self {
        Self {
            pool,
            log,
            pending: Mutex::new(Vec::new()),
        }
    }

    fn outcome_value(outcome: &str) -> f32 {
        match outcome {
            "success" => 1.0,
            "partial" => 0.5,
            "failure" => 0.0,
            _ => 0.0,
        }
    }
}

#[async_trait]
impl MirrorSignalSource for PatternEffectivenessSource {
    fn spec(&self) -> MirrorSnapshotSpec {
        MirrorSnapshotSpec {
            name: "pattern_effectiveness",
            subscribed_kinds: &["PatternOutcome"],
            flush_interval_secs: Some(mirror_flush_secs(3600)),
        }
    }

    fn name(&self) -> &'static str {
        "pattern-effectiveness-source"
    }

    async fn accumulate(&self, signal: &AiSignal) -> common::Result<()> {
        if let Some(bus::DomainEvent::PatternOutcome {
            pattern_id,
            outcome,
            evidence,
            measured_at: _,
        }) = &signal.raw_event
        {
            let mut pending = self.pending.lock().await;
            pending.push(BufferedOutcome {
                pattern_id: pattern_id.clone(),
                outcome: outcome.clone(),
                evidence: evidence.clone(),
            });
        }
        Ok(())
    }

    async fn flush(&self) -> common::Result<()> {
        let mut pending = self.pending.lock().await;
        if pending.is_empty() {
            return Ok(());
        }

        let to_process: Vec<BufferedOutcome> = std::mem::take(&mut *pending);
        drop(pending);

        for item in to_process {
            let outcome_value = Self::outcome_value(&item.outcome);

            let current: Option<(f32,)> = sqlx::query_as(
                "SELECT effectiveness_score FROM procedural_rules WHERE id = ?1",
            )
            .bind(&item.pattern_id)
            .fetch_optional(self.pool.inner())
            .await
            .map_err(|e| KlyntbotError::Storage(format!("effectiveness_score fetch: {e}")))?;

            let Some((score_before,)) = current else {
                continue;
            };
            let score_after = 0.9 * score_before + 0.1 * outcome_value;

            sqlx::query(
                "UPDATE procedural_rules \
                 SET effectiveness_score = ?1, updated_at = ?2 \
                 WHERE id = ?3",
            )
            .bind(score_after)
            .bind(jiff::Timestamp::now().to_string())
            .bind(&item.pattern_id)
            .execute(self.pool.inner())
            .await
            .map_err(|e| KlyntbotError::Storage(format!("procedural_rules update: {e}")))?;

            self.log
                .insert(
                    &item.pattern_id,
                    "unknown",
                    None,
                    &item.outcome,
                    score_before,
                    score_after,
                    Some(item.evidence.as_str()).filter(|s| !s.is_empty()),
                )
                .await?;
        }

        Ok(())
    }
}
