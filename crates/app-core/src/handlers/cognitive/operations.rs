//! Cognitive operations — compaction, event/pipeline logs, event injection.

use cognitive::repos::SemanticFactRepo;
use desktop_shared::cognitive_commands::*;
use desktop_shared::errors::ApiError;

use crate::errors::map_cognitive_err;
use crate::state::AppCore;

impl AppCore {
    #[tracing::instrument(skip(self), err)]
    pub async fn cognitive_run_compaction(&self) -> Result<CompactionResultResponse, ApiError> {
        let pool = self.repos.pool();
        let fact_repo = SemanticFactRepo::new(pool.clone());
        let episodic_repo = cognitive::repos::EpisodicMemoryRepo::new(pool.clone());
        let rule_repo = cognitive::repos::ProceduralRuleRepo::new(pool.clone());

        let result = cognitive::services::compaction::run_compaction(
            &fact_repo,
            &episodic_repo,
            Some(&rule_repo),
            None,
            None,
            None,
            None,
            None,
        )
        .await
        .map_err(map_cognitive_err)?;

        Ok(CompactionResultResponse {
            archived_count: result.facts_archived,
            deleted_episodic: result.episodic_deleted,
            rules_deactivated: result.rules_deactivated,
        })
    }

    /// Fetch recent domain events from the persistent log.
    #[tracing::instrument(skip(self), err)]
    pub async fn cognitive_event_log(
        &self,
        limit: Option<i64>,
    ) -> Result<Vec<cognitive::DomainEventRow>, ApiError> {
        let repo = self
            .event_log_repo()
            .ok_or_else(|| ApiError::feature_disabled("event log not available"))?;
        repo.list_recent_domain_events(limit.unwrap_or(100).min(500))
            .await
            .map_err(map_cognitive_err)
    }

    /// Fetch recent pipeline events from the persistent log.
    #[tracing::instrument(skip(self), err)]
    pub async fn cognitive_pipeline_log(
        &self,
        limit: Option<i64>,
    ) -> Result<Vec<cognitive::PipelineEventRow>, ApiError> {
        let repo = self
            .event_log_repo()
            .ok_or_else(|| ApiError::feature_disabled("event log not available"))?;
        repo.list_recent_pipeline_events(limit.unwrap_or(100).min(500))
            .await
            .map_err(map_cognitive_err)
    }

    #[tracing::instrument(skip(self, payload), err)]
    pub async fn cognitive_inject_event(
        &self,
        event_type: String,
        payload: serde_json::Value,
    ) -> Result<bool, ApiError> {
        let bus = self.domain_event_bus()?;
        let event = match event_type.as_str() {
            bus::DomainEvent::KIND_USER_STATED_FACT => bus::DomainEvent::UserStatedFact {
                fact: payload["fact"].as_str().unwrap_or("").to_string(),
                domain: payload["domain"].as_str().unwrap_or("general").to_string(),
            },
            bus::DomainEvent::KIND_USER_CORRECTED_AI => bus::DomainEvent::UserCorrectedAI {
                original: payload["original"].as_str().unwrap_or("").to_string(),
                correction: payload["correction"].as_str().unwrap_or("").to_string(),
                kind: serde_json::from_value(payload["kind"].clone())
                    .unwrap_or(bus::CorrectionKind::KeywordPrefix),
                strength: payload["strength"].as_f64().unwrap_or(1.0),
                session_key: payload["session_key"]
                    .as_str()
                    .unwrap_or("unknown")
                    .to_string(),
                active_skill: payload["active_skill"].as_str().map(|s| s.to_string()),
            },
            bus::DomainEvent::KIND_DISTRACTION_DETECTED => bus::DomainEvent::DistractionDetected {
                app: payload["app"].as_str().unwrap_or("unknown").to_string(),
                duration_secs: payload["duration_secs"].as_i64(),
                context: payload["context"].as_str().unwrap_or("").to_string(),
            },
            bus::DomainEvent::KIND_FOCUS_SESSION_STARTED => bus::DomainEvent::FocusSessionStarted {
                session_type: payload["session_type"]
                    .as_str()
                    .unwrap_or("Focus")
                    .to_string(),
                target_mins: payload["target_mins"].as_i64().unwrap_or(25),
            },
            bus::DomainEvent::KIND_FOCUS_SESSION_ENDED => bus::DomainEvent::FocusSessionEnded {
                quality: payload["quality"].as_f64().unwrap_or(0.5),
                duration_secs: payload["duration_secs"].as_i64().unwrap_or(1500),
                interruptions: payload["interruptions"].as_i64().unwrap_or(0) as i32,
            },
            bus::DomainEvent::KIND_TASK_DEFERRED => bus::DomainEvent::TaskDeferred {
                task_id: payload["task_id"]
                    .as_str()
                    .unwrap_or("test-task")
                    .to_string(),
                times_deferred: payload["times_deferred"].as_i64().unwrap_or(1) as i32,
            },
            bus::DomainEvent::KIND_ACTIVITY_SESSION_COMPLETED => {
                bus::DomainEvent::ActivitySessionCompleted {
                    date: payload["date"]
                        .as_str()
                        .unwrap_or(&jiff::Zoned::now().strftime("%Y-%m-%d").to_string())
                        .to_string(),
                    total_active_secs: payload["total_active_secs"].as_i64().unwrap_or(300),
                    productive_secs: payload["productive_secs"].as_i64().unwrap_or(240),
                    distracting_secs: payload["distracting_secs"].as_i64().unwrap_or(60),
                }
            }
            bus::DomainEvent::KIND_PRODUCTIVITY_SCORE_COMPUTED => {
                bus::DomainEvent::ProductivityScoreComputed {
                    date: payload["date"]
                        .as_str()
                        .unwrap_or(&jiff::Zoned::now().strftime("%Y-%m-%d").to_string())
                        .to_string(),
                    score: payload["score"].as_f64().unwrap_or(0.0),
                }
            }
            bus::DomainEvent::KIND_CHAT_TURN_COMPLETED => bus::DomainEvent::ChatTurnCompleted {
                session_key: payload["session_key"]
                    .as_str()
                    .unwrap_or("debug")
                    .to_string(),
                user_message: payload
                    .get("user_message")
                    .and_then(|v| v.as_str())
                    .map(|s| s.to_string()),
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
