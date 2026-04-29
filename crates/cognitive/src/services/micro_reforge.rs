//! Micro-Reforge service (KCA Track 4).

use async_trait::async_trait;
use config::schema::MicroReforgeConfig;
use jiff::Timestamp;
use storage::StoragePool;

use crate::services::micro_reforge_types::{MicroReforgeInput, MicroReforgeOutput};

#[async_trait]
pub trait MicroReforgeHandler: Send + Sync {
    async fn synthesize(&self, input: MicroReforgeInput) -> common::Result<MicroReforgeOutput>;
}

pub struct NoopMicroReforgeHandler;

#[async_trait]
impl MicroReforgeHandler for NoopMicroReforgeHandler {
    async fn synthesize(&self, _input: MicroReforgeInput) -> common::Result<MicroReforgeOutput> {
        Ok(MicroReforgeOutput::default())
    }
}

pub struct MicroReforgeService {
    pool: StoragePool,
    cfg: MicroReforgeConfig,
}

impl MicroReforgeService {
    pub fn new(pool: StoragePool, cfg: MicroReforgeConfig) -> Self {
        Self { pool, cfg }
    }

    pub async fn note_turn(&self) -> common::Result<()> {
        sqlx::query(
            "UPDATE micro_reforge_state SET turns_since_last_run = turns_since_last_run + 1 WHERE id = 1",
        )
        .execute(self.pool.inner())
        .await
        .map_err(|e| common::KlyntbotError::Storage(format!("note_turn: {e}")))?;
        Ok(())
    }

    pub async fn should_run(&self) -> common::Result<bool> {
        if !self.cfg.enabled {
            return Ok(false);
        }
        let row: (i64, Option<String>) = sqlx::query_as(
            "SELECT turns_since_last_run, last_run_at FROM micro_reforge_state WHERE id = 1",
        )
        .fetch_one(self.pool.inner())
        .await
        .map_err(|e| common::KlyntbotError::Storage(format!("should_run: {e}")))?;

        let turns_since_last_run: u32 = row.0 as u32;
        if turns_since_last_run >= self.cfg.turn_threshold {
            return Ok(true);
        }

        if let Some(last) = row.1 {
            let last_ts: Timestamp = last.parse().unwrap_or_else(|_| Timestamp::now());
            let elapsed_min = (Timestamp::now() - last_ts)
                .total(jiff::Unit::Minute)
                .unwrap_or(0.0);
            if elapsed_min >= self.cfg.minute_threshold as f64 {
                return Ok(true);
            }
        }
        Ok(false)
    }

    pub async fn record_run(
        &self,
        trigger: &str,
        proposed_count: u32,
        accepted_count: u32,
        error: Option<&str>,
    ) -> common::Result<()> {
        let id = uuid::Uuid::new_v4().to_string();
        let started = Timestamp::now().to_string();
        let row: (i64,) = sqlx::query_as(
            "SELECT turns_since_last_run FROM micro_reforge_state WHERE id = 1",
        )
        .fetch_one(self.pool.inner())
        .await
        .map_err(|e| common::KlyntbotError::Storage(e.to_string()))?;
        let turn_count_at_run = row.0;
        let pc = proposed_count as i64;
        let ac = accepted_count as i64;
        sqlx::query(
            r#"INSERT INTO micro_reforge_runs
               (id, started_at, finished_at, trigger, turn_count_at_run, proposed_rule_count, accepted_rule_count, error)
               VALUES (?1, ?2, ?2, ?3, ?4, ?5, ?6, ?7)"#,
        )
        .bind(&id)
        .bind(&started)
        .bind(trigger)
        .bind(turn_count_at_run)
        .bind(pc)
        .bind(ac)
        .bind(error)
        .execute(self.pool.inner())
        .await
        .map_err(|e| common::KlyntbotError::Storage(e.to_string()))?;

        sqlx::query(
            r#"UPDATE micro_reforge_state SET
                  last_run_at = ?1,
                  turns_since_last_run = 0,
                  total_runs = total_runs + 1,
                  total_rules_promoted = total_rules_promoted + ?2
               WHERE id = 1"#,
        )
        .bind(&started)
        .bind(ac)
        .execute(self.pool.inner())
        .await
        .map_err(|e| common::KlyntbotError::Storage(e.to_string()))?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::repos::cognitive_test_pool;
    use crate::services::micro_reforge_types::*;

    #[tokio::test]
    async fn should_run_returns_true_after_turn_threshold() {
        let pool = cognitive_test_pool().await;
        let pool = StoragePool::from_existing(pool);
        let service = MicroReforgeService::new(pool.clone(), MicroReforgeConfig::default());

        for _ in 0..10 {
            service.note_turn().await.unwrap();
        }
        assert!(service.should_run().await.unwrap());
    }

    #[tokio::test]
    async fn should_run_returns_false_below_threshold() {
        let pool = cognitive_test_pool().await;
        let pool = StoragePool::from_existing(pool);
        let service = MicroReforgeService::new(pool.clone(), MicroReforgeConfig::default());

        for _ in 0..9 {
            service.note_turn().await.unwrap();
        }
        assert!(!service.should_run().await.unwrap());
    }

    #[tokio::test]
    async fn should_run_returns_false_when_disabled() {
        let pool = cognitive_test_pool().await;
        let pool = StoragePool::from_existing(pool);
        let mut cfg = MicroReforgeConfig::default();
        cfg.enabled = false;
        let service = MicroReforgeService::new(pool.clone(), cfg);

        for _ in 0..50 {
            service.note_turn().await.unwrap();
        }
        assert!(!service.should_run().await.unwrap());
    }
}
