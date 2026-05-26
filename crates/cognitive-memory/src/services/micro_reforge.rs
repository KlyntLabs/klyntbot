//! Micro-Reforge service (KCA Track 4).

use async_trait::async_trait;
use config::schema::MicroReforgeConfig;
use jiff::Timestamp;
use std::sync::Arc;
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
        let row: (i64,) =
            sqlx::query_as("SELECT turns_since_last_run FROM micro_reforge_state WHERE id = 1")
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

    pub async fn run(
        &self,
        trigger: &str,
        handler: Arc<dyn MicroReforgeHandler>,
        rule_repo: &crate::repos::ProceduralRuleRepo,
        episodic_repo: &crate::repos::EpisodicMemoryRepo,
        observation_repo: &crate::repos::AccumulatedObservationRepo,
    ) -> common::Result<u32> {
        // 1. Collect input.
        let recent_episodics: Vec<crate::services::micro_reforge_types::EpisodicRef> =
            episodic_repo
                .list_recent(50)
                .await
                .unwrap_or_default()
                .into_iter()
                .map(|e| crate::services::micro_reforge_types::EpisodicRef {
                    id: e.id,
                    domain: e.domain,
                    summary: e.summary.unwrap_or_default(),
                    importance: e.importance,
                    recorded_at: e.recorded_at,
                })
                .collect();

        let recent_observations: Vec<crate::services::micro_reforge_types::ObservationRef> =
            observation_repo
                .list_recent(100)
                .await
                .into_iter()
                .map(|o| crate::services::micro_reforge_types::ObservationRef {
                    content_truncated: common::helpers::truncate_chars(&o.content, 200, "…"),
                    domain: o.domain,
                    importance: o.importance,
                })
                .collect();

        let existing_rules: Vec<crate::services::micro_reforge_types::RuleSummary> = rule_repo
            .list_all_active()
            .await
            .unwrap_or_default()
            .into_iter()
            .take(50)
            .map(|r| crate::services::micro_reforge_types::RuleSummary {
                domain: r.domain,
                rule_text: r.rule_text,
                confidence: r.confidence,
            })
            .collect();

        let turn_row: (i64,) =
            sqlx::query_as("SELECT turns_since_last_run FROM micro_reforge_state WHERE id = 1")
                .fetch_one(self.pool.inner())
                .await
                .map_err(|e| common::KlyntbotError::Storage(e.to_string()))?;

        let input = crate::services::micro_reforge_types::MicroReforgeInput {
            recent_episodics,
            recent_observations,
            existing_rules_summary: existing_rules,
            session_count: 0,
            turn_count_since_last_run: turn_row.0 as u32,
        };

        // 2. Synthesize.
        let out = handler.synthesize(input).await?;

        // 3. Apply: write rules above min_confidence, dedup against existing.
        let mut accepted = 0u32;
        for proposed in &out.proposed_rules {
            if proposed.confidence < self.cfg.min_confidence {
                continue;
            }
            if rule_text_already_exists(rule_repo, &proposed.domain, &proposed.rule_text).await {
                continue;
            }
            let rule = crate::types::ProceduralRule {
                id: uuid::Uuid::new_v4().to_string(),
                domain: proposed.domain.clone(),
                rule_text: proposed.rule_text.clone(),
                confidence: proposed.confidence,
                source: "reflected_online".into(),
                signal_count: proposed.signal_count as i64,
                created_at: Timestamp::now().to_string(),
                updated_at: Timestamp::now().to_string(),
                active: true,
                project_id: None,
                scope_type: "system".to_string(),
                scope_id: None,
                effectiveness_score: 0.0,
                stability: 1.0,
                scope_repo_id: None,
                last_applied: None,
                application_count: 0,
                metadata: None,
            };
            if let Err(e) = rule_repo.upsert(&rule).await {
                tracing::warn!(error = %e, "micro_reforge: rule upsert failed");
                continue;
            }
            accepted += 1;
        }

        // 4. Record run.
        self.record_run(trigger, out.proposed_rules.len() as u32, accepted, None)
            .await?;
        Ok(accepted)
    }
}

async fn rule_text_already_exists(
    repo: &crate::repos::ProceduralRuleRepo,
    domain: &str,
    rule_text: &str,
) -> bool {
    repo.list_by_domain(domain, 100)
        .await
        .unwrap_or_default()
        .iter()
        .any(|r| normalize(&r.rule_text) == normalize(rule_text))
}

fn normalize(s: &str) -> String {
    s.to_lowercase()
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
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
        let cfg = MicroReforgeConfig {
            enabled: false,
            ..Default::default()
        };
        let service = MicroReforgeService::new(pool.clone(), cfg);

        for _ in 0..50 {
            service.note_turn().await.unwrap();
        }
        assert!(!service.should_run().await.unwrap());
    }

    use crate::repos::AccumulatedObservationRepo;
    use crate::repos::EpisodicMemoryRepo;
    use crate::repos::ProceduralRuleRepo;
    use std::sync::Arc;

    struct ScriptedHandler(MicroReforgeOutput);

    #[async_trait]
    impl MicroReforgeHandler for ScriptedHandler {
        async fn synthesize(
            &self,
            _input: MicroReforgeInput,
        ) -> common::Result<MicroReforgeOutput> {
            Ok(self.0.clone())
        }
    }

    #[tokio::test]
    async fn run_writes_proposed_rule_above_confidence_threshold() {
        let pool_sqlite = cognitive_test_pool().await;
        let pool = StoragePool::from_existing(pool_sqlite.clone());
        let cfg = MicroReforgeConfig::default();
        let service = MicroReforgeService::new(pool.clone(), cfg);
        let rule_repo = ProceduralRuleRepo::new(pool_sqlite.clone());

        let handler = Arc::new(ScriptedHandler(MicroReforgeOutput {
            proposed_rules: vec![ProposedRule {
                domain: "coding".into(),
                rule_text: "When tests fail, run clippy first.".into(),
                confidence: 0.85,
                signal_count: 4,
                evidence_episodic_ids: vec!["ep1".into()],
            }],
            notes: None,
        }));

        let accepted = service
            .run(
                "manual",
                handler,
                &rule_repo,
                &EpisodicMemoryRepo::new(pool_sqlite.clone()),
                &AccumulatedObservationRepo::new(pool_sqlite.clone()),
            )
            .await
            .unwrap();

        assert_eq!(accepted, 1);
        let rules = rule_repo.list_by_domain("coding", 100).await.unwrap();
        assert_eq!(rules.len(), 1);
        assert_eq!(rules[0].source, "reflected_online");
    }

    #[tokio::test]
    async fn run_skips_rule_below_confidence_threshold() {
        let pool_sqlite = cognitive_test_pool().await;
        let pool = StoragePool::from_existing(pool_sqlite.clone());
        let cfg = MicroReforgeConfig {
            min_confidence: 0.8,
            ..Default::default()
        };
        let service = MicroReforgeService::new(pool.clone(), cfg);
        let rule_repo = ProceduralRuleRepo::new(pool_sqlite.clone());

        let handler = Arc::new(ScriptedHandler(MicroReforgeOutput {
            proposed_rules: vec![ProposedRule {
                domain: "coding".into(),
                rule_text: "low conf rule".into(),
                confidence: 0.6,
                signal_count: 2,
                evidence_episodic_ids: vec![],
            }],
            notes: None,
        }));

        let accepted = service
            .run(
                "manual",
                handler,
                &rule_repo,
                &EpisodicMemoryRepo::new(pool_sqlite.clone()),
                &AccumulatedObservationRepo::new(pool_sqlite.clone()),
            )
            .await
            .unwrap();

        assert_eq!(accepted, 0);
        let rules = rule_repo.list_by_domain("coding", 100).await.unwrap();
        assert_eq!(rules.len(), 0);
    }
}
