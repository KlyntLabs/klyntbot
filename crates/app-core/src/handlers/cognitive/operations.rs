//! Cognitive operations — compaction, reflection, event/pipeline logs, event injection.

use cognitive::repos::SemanticFactRepo;
use desktop_shared::cognitive_commands::*;
use desktop_shared::errors::ApiError;

use crate::errors::map_cognitive_err;
use crate::state::AppCore;

use super::memory::build_reflection_handlers;

impl AppCore {
    pub async fn cognitive_run_compaction(&self) -> Result<CompactionResultResponse, ApiError> {
        let pool = self.repos.pool();
        let fact_repo = SemanticFactRepo::new(pool.clone());
        let episodic_repo = cognitive::repos::EpisodicMemoryRepo::new(pool.clone());

        let archived = fact_repo
            .archive_superseded(90)
            .await
            .map_err(map_cognitive_err)?;

        let deleted_episodic = episodic_repo
            .delete_old(90, 2)
            .await
            .map_err(map_cognitive_err)?;

        Ok(CompactionResultResponse {
            archived_count: archived,
            deleted_episodic,
        })
    }

    /// Trigger a weekly reflection cycle — creates procedural rules and
    /// updates facts based on recent episodic memories.
    pub async fn cognitive_run_reflection(&self) -> Result<ReflectionResultResponse, ApiError> {
        let pool = self.repos.pool();
        let fact_repo = SemanticFactRepo::new(pool.clone());
        let episodic_repo = cognitive::repos::EpisodicMemoryRepo::new(pool.clone());
        let rule_repo = cognitive::repos::ProceduralRuleRepo::new(pool.clone());

        let config = self.config.read().await;
        let (reflection_handler, consolidation_handler) =
            build_reflection_handlers(&self.cognitive_provider, &config);
        drop(config);

        let output = cognitive::reflection::run_weekly_reflection(
            reflection_handler.as_ref(),
            consolidation_handler.as_ref(),
            &fact_repo,
            &episodic_repo,
            &rule_repo,
            None,
        )
        .await
        .map_err(|e| ApiError::new("REFLECTION_FAILED", e.to_string()))?;

        Ok(ReflectionResultResponse {
            fact_updates: output.fact_updates.len(),
            rule_updates: output.rule_updates.len(),
            summary: output.summary,
        })
    }

    /// Fetch recent domain events from the persistent log.
    pub async fn cognitive_event_log(
        &self,
        limit: Option<i64>,
    ) -> Result<Vec<cognitive::DomainEventRow>, ApiError> {
        let repo = self
            .event_log_repo
            .as_ref()
            .ok_or_else(|| ApiError::new("FEATURE_DISABLED", "event log not available"))?;
        repo.list_recent_domain_events(limit.unwrap_or(100))
            .await
            .map_err(map_cognitive_err)
    }

    /// Fetch recent pipeline events from the persistent log.
    pub async fn cognitive_pipeline_log(
        &self,
        limit: Option<i64>,
    ) -> Result<Vec<cognitive::PipelineEventRow>, ApiError> {
        let repo = self
            .event_log_repo
            .as_ref()
            .ok_or_else(|| ApiError::new("FEATURE_DISABLED", "event log not available"))?;
        repo.list_recent_pipeline_events(limit.unwrap_or(100))
            .await
            .map_err(map_cognitive_err)
    }

    pub async fn cognitive_inject_event(
        &self,
        event_type: String,
        payload: serde_json::Value,
    ) -> Result<bool, ApiError> {
        let bus = self.domain_event_bus()?;
        let event = match event_type.as_str() {
            "UserStatedFact" => bus::DomainEvent::UserStatedFact {
                fact: payload["fact"].as_str().unwrap_or("").to_string(),
                domain: payload["domain"].as_str().unwrap_or("general").to_string(),
            },
            "UserCorrectedAI" => bus::DomainEvent::UserCorrectedAI {
                original: payload["original"].as_str().unwrap_or("").to_string(),
                correction: payload["correction"].as_str().unwrap_or("").to_string(),
            },
            "BudgetAlert" => bus::DomainEvent::BudgetAlert {
                category: payload["category"]
                    .as_str()
                    .unwrap_or("general")
                    .to_string(),
                spent: payload["spent"].as_f64().unwrap_or(0.0),
                limit: payload["limit"].as_f64().unwrap_or(0.0),
            },
            "DistractionDetected" => bus::DomainEvent::DistractionDetected {
                app: payload["app"].as_str().unwrap_or("unknown").to_string(),
                duration_secs: payload["duration_secs"].as_i64(),
                context: payload["context"].as_str().unwrap_or("").to_string(),
            },
            "FocusSessionStarted" => bus::DomainEvent::FocusSessionStarted {
                session_type: payload["session_type"]
                    .as_str()
                    .unwrap_or("Focus")
                    .to_string(),
                target_mins: payload["target_mins"].as_i64().unwrap_or(25),
            },
            "FocusSessionEnded" => bus::DomainEvent::FocusSessionEnded {
                quality: payload["quality"].as_f64().unwrap_or(0.5),
                duration_secs: payload["duration_secs"].as_i64().unwrap_or(1500),
                interruptions: payload["interruptions"].as_i64().unwrap_or(0) as i32,
            },
            "TaskDeferred" => bus::DomainEvent::TaskDeferred {
                task_id: payload["task_id"]
                    .as_str()
                    .unwrap_or("test-task")
                    .to_string(),
                times_deferred: payload["times_deferred"].as_i64().unwrap_or(1) as i32,
            },
            "ActivitySessionCompleted" => bus::DomainEvent::ActivitySessionCompleted {
                date: payload["date"]
                    .as_str()
                    .unwrap_or(&chrono::Utc::now().format("%Y-%m-%d").to_string())
                    .to_string(),
                total_active_secs: payload["total_active_secs"].as_i64().unwrap_or(300),
                productive_secs: payload["productive_secs"].as_i64().unwrap_or(240),
                distracting_secs: payload["distracting_secs"].as_i64().unwrap_or(60),
            },
            "ProductivityScoreComputed" => bus::DomainEvent::ProductivityScoreComputed {
                date: payload["date"]
                    .as_str()
                    .unwrap_or(&chrono::Utc::now().format("%Y-%m-%d").to_string())
                    .to_string(),
                score: payload["score"].as_f64().unwrap_or(0.0),
            },
            "ChatTurnCompleted" => bus::DomainEvent::ChatTurnCompleted {
                user_message: payload["user_message"].as_str().unwrap_or("").to_string(),
                session_key: payload["session_key"]
                    .as_str()
                    .unwrap_or("debug")
                    .to_string(),
            },
            other => {
                return Err(ApiError::new(
                    "VALIDATION",
                    format!("unknown event type: {other}"),
                ));
            }
        };
        bus.publish(event);
        Ok(true)
    }
}
