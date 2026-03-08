//! Cognitive memory handlers — reads, mutations, and system status.

use cognitive::decay::retrievability;
use cognitive::repos::{load_user_model, SemanticFactRepo, RULE_DOMAINS};
use cognitive::types::{ProceduralRule, SemanticFact};
use desktop_shared::cognitive_commands::*;
use desktop_shared::errors::ApiError;

use crate::errors::map_cognitive_err;
use crate::state::AppCore;

pub(crate) fn fact_to_response(f: &SemanticFact) -> SemanticFactResponse {
    let elapsed_days = chrono::Utc::now()
        .signed_duration_since(
            f.last_accessed
                .as_deref()
                .and_then(|s| chrono::DateTime::parse_from_rfc3339(s).ok())
                .map(|dt| dt.with_timezone(&chrono::Utc))
                .unwrap_or_else(|| {
                    chrono::DateTime::parse_from_rfc3339(&f.recorded_at)
                        .map(|dt| dt.with_timezone(&chrono::Utc))
                        .unwrap_or_else(|_| chrono::Utc::now())
                }),
        )
        .num_seconds() as f64
        / 86400.0;
    let r = retrievability(elapsed_days, f.stability);

    let status = if f.superseded_at.is_some() {
        "superseded"
    } else {
        "active"
    };

    SemanticFactResponse {
        id: f.id.clone(),
        domain: f.domain.clone(),
        subject: f.subject.clone(),
        predicate: f.predicate.clone(),
        object: f.object.clone(),
        confidence: f.confidence,
        source: f.source.clone(),
        valid_from: f.valid_from.clone(),
        valid_until: f.valid_until.clone(),
        stability: f.stability,
        retrievability: r,
        last_accessed: f.last_accessed.clone(),
        access_count: f.access_count,
        status: status.to_string(),
    }
}

pub(crate) fn rule_to_response(r: &ProceduralRule) -> ProceduralRuleResponse {
    ProceduralRuleResponse {
        id: r.id.clone(),
        domain: r.domain.clone(),
        rule_text: r.rule_text.clone(),
        confidence: r.confidence,
        source: r.source.clone(),
        signal_count: r.signal_count,
        active: r.active,
        created_at: r.created_at.clone(),
        updated_at: r.updated_at.clone(),
    }
}

pub(crate) fn fact_preview(fact: &SemanticFact) -> String {
    format!("{} = {}", fact.predicate, fact.object)
}

// ── Memory Reads ────────────────────────────────────────────────────────

impl AppCore {
    pub async fn cognitive_user_model(&self) -> Result<UserModelSummaryResponse, ApiError> {
        let pool = self.repos.pool();
        let fact_repo = SemanticFactRepo::new(pool.clone());
        let model = load_user_model(&fact_repo).await;

        Ok(UserModelSummaryResponse {
            identity_count: model.identity.len(),
            energy_count: model.energy.len(),
            work_count: model.work.len(),
            finance_count: model.finance.len(),
            learning_count: model.learning.len(),
            preferences_count: model.preferences.len(),
            identity_preview: model.identity.iter().take(3).map(fact_preview).collect(),
            energy_preview: model.energy.iter().take(3).map(fact_preview).collect(),
            work_preview: model.work.iter().take(3).map(fact_preview).collect(),
            finance_preview: model.finance.iter().take(3).map(fact_preview).collect(),
            learning_preview: model.learning.iter().take(3).map(fact_preview).collect(),
            preferences_preview: model.preferences.iter().take(3).map(fact_preview).collect(),
        })
    }

    pub async fn cognitive_facts_list(
        &self,
        domain: Option<String>,
    ) -> Result<Vec<SemanticFactResponse>, ApiError> {
        let pool = self.repos.pool();
        let fact_repo = SemanticFactRepo::new(pool.clone());

        let all_facts = match domain.as_deref() {
            Some(d) => fact_repo.list_active(d).await.map_err(map_cognitive_err)?,
            None => fact_repo.list_all_active().await.map_err(map_cognitive_err)?,
        };

        Ok(all_facts.iter().map(fact_to_response).collect())
    }

    pub async fn cognitive_episodic_list(
        &self,
        domain: Option<String>,
        limit: Option<i64>,
    ) -> Result<Vec<EpisodicMemoryResponse>, ApiError> {
        let pool = self.repos.pool();
        let repo = cognitive::repos::EpisodicMemoryRepo::new(pool.clone());
        let limit = limit.unwrap_or(50);

        let memories = match domain.as_deref() {
            Some(d) => repo
                .list_by_domain(d, limit)
                .await
                .map_err(map_cognitive_err)?,
            None => repo
                .list_recent(limit)
                .await
                .map_err(map_cognitive_err)?,
        };

        Ok(memories
            .iter()
            .map(|m| EpisodicMemoryResponse {
                id: m.id.clone(),
                domain: m.domain.clone(),
                content: m.content.clone(),
                summary: m.summary.clone(),
                importance: m.importance,
                occurred_at: m.occurred_at.clone(),
                recorded_at: m.recorded_at.clone(),
                stability: m.stability,
                access_count: m.access_count,
            })
            .collect())
    }

    pub async fn cognitive_rules_list(
        &self,
        domain: Option<String>,
    ) -> Result<Vec<ProceduralRuleResponse>, ApiError> {
        let pool = self.repos.pool();
        let repo = cognitive::repos::ProceduralRuleRepo::new(pool.clone());

        let domains: Vec<&str> = match domain.as_deref() {
            Some(d) => vec![d],
            None => RULE_DOMAINS.to_vec(),
        };

        let mut all_rules = Vec::new();
        for d in &domains {
            let rules = repo.list_active(d).await.map_err(map_cognitive_err)?;
            all_rules.extend(rules);
        }

        Ok(all_rules.iter().map(rule_to_response).collect())
    }

    pub async fn cognitive_memory_stats(&self) -> Result<MemoryStatsResponse, ApiError> {
        let pool = self.repos.pool();
        let fact_repo = SemanticFactRepo::new(pool.clone());
        let episodic_repo = cognitive::repos::EpisodicMemoryRepo::new(pool.clone());
        let rule_repo = cognitive::repos::ProceduralRuleRepo::new(pool.clone());

        let model = load_user_model(&fact_repo).await;
        let active_facts = model.active_fact_count();

        // TODO: add a `count_archived()` query to SemanticFactRepo — archive_superseded() is
        // a destructive mutation and must NOT be used for reads.
        let archived = 0u64;

        let episodic_count = episodic_repo
            .count_all()
            .await
            .unwrap_or(0) as usize;

        let mut rules_count = 0;
        for d in RULE_DOMAINS {
            rules_count += rule_repo.list_active(d).await.map(|v| v.len()).unwrap_or(0);
        }

        Ok(MemoryStatsResponse {
            active_facts,
            archived_facts: archived,
            episodic_count,
            rules_count,
            last_compaction: None,
        })
    }

    // ── System Status ───────────────────────────────────────────────────

    pub async fn cognitive_system_status(&self) -> Result<SystemStatusResponse, ApiError> {
        let pool = self.repos.pool();
        let fact_repo = SemanticFactRepo::new(pool.clone());
        let model = load_user_model(&fact_repo).await;
        let active_facts = model.active_fact_count();
        let has_llm = self.has_cognitive_provider;

        let components = vec![
            ComponentStatusResponse {
                name: "DomainEvent Bus".into(),
                status: "wired".into(),
                handler_type: "n/a".into(),
                notes: "tokio::broadcast, 256 capacity".into(),
            },
            ComponentStatusResponse {
                name: "Salience Filter".into(),
                status: "wired".into(),
                handler_type: "heuristic".into(),
                notes: "Rule-based classification".into(),
            },
            ComponentStatusResponse {
                name: "Extraction".into(),
                status: "wired".into(),
                handler_type: if has_llm { "llm" } else { "heuristic" }.into(),
                notes: if has_llm {
                    "LlmExtractionHandler in agent crate"
                } else {
                    "HeuristicExtractionHandler in agent crate"
                }
                .into(),
            },
            ComponentStatusResponse {
                name: "Consolidation".into(),
                status: "wired".into(),
                handler_type: if has_llm { "llm" } else { "heuristic" }.into(),
                notes: if has_llm {
                    "LlmConsolidationHandler in agent crate"
                } else {
                    "HeuristicConsolidationHandler in agent crate"
                }
                .into(),
            },
            ComponentStatusResponse {
                name: "FSRS Decay".into(),
                status: "wired".into(),
                handler_type: "n/a".into(),
                notes: "R = exp(ln(0.9) * elapsed / stability)".into(),
            },
            ComponentStatusResponse {
                name: "Compaction".into(),
                status: "wired".into(),
                handler_type: "n/a".into(),
                notes: "Archive superseded, prune episodic, size budget".into(),
            },
            ComponentStatusResponse {
                name: "Reflection".into(),
                status: "wired".into(),
                handler_type: if has_llm { "llm" } else { "heuristic" }.into(),
                notes: "Scheduled via CronService (Monday 9am)".into(),
            },
            ComponentStatusResponse {
                name: "Context Source".into(),
                status: "wired".into(),
                handler_type: "n/a".into(),
                notes: "CognitiveContextSource at priority 60, 60s cache".into(),
            },
            ComponentStatusResponse {
                name: "UserSituation".into(),
                status: "wired".into(),
                handler_type: "n/a".into(),
                notes: "compute_situation() from SituationInputs".into(),
            },
            ComponentStatusResponse {
                name: "Signal Accumulator".into(),
                status: "wired".into(),
                handler_type: "n/a".into(),
                notes: "30min rolling window, 7 trigger conditions".into(),
            },
            ComponentStatusResponse {
                name: "Pattern Detector".into(),
                status: "wired".into(),
                handler_type: "n/a".into(),
                notes: "5 pattern types detected".into(),
            },
            ComponentStatusResponse {
                name: "Intervention Router".into(),
                status: "wired".into(),
                handler_type: "n/a".into(),
                notes: "Rate limits: 3/hr, 10/day, exponential backoff".into(),
            },
            ComponentStatusResponse {
                name: "Feedback Tracker".into(),
                status: "wired".into(),
                handler_type: "n/a".into(),
                notes: "3 channels: explicit, behavioral, outcome".into(),
            },
            ComponentStatusResponse {
                name: "LLM Extraction".into(),
                status: if has_llm { "wired" } else { "built" }.into(),
                handler_type: "llm".into(),
                notes: if has_llm {
                    "LlmExtractionHandler active"
                } else {
                    "Built, using heuristic fallback (no cognitive provider)"
                }
                .into(),
            },
            ComponentStatusResponse {
                name: "LLM Consolidation".into(),
                status: if has_llm { "wired" } else { "built" }.into(),
                handler_type: "llm".into(),
                notes: if has_llm {
                    "LlmConsolidationHandler active"
                } else {
                    "Built, using heuristic fallback (no cognitive provider)"
                }
                .into(),
            },
            ComponentStatusResponse {
                name: "LLM Reflection".into(),
                status: if has_llm { "wired" } else { "built" }.into(),
                handler_type: "llm".into(),
                notes: if has_llm {
                    "LlmReflectionHandler active"
                } else {
                    "Built, using heuristic fallback (no cognitive provider)"
                }
                .into(),
            },
            ComponentStatusResponse {
                name: "LLM Coaching Reasoner".into(),
                status: if has_llm { "wired" } else { "built" }.into(),
                handler_type: "llm".into(),
                notes: if has_llm {
                    "LlmCoachingReasonerHandler active"
                } else {
                    "Built, using heuristic fallback (no cognitive provider)"
                }
                .into(),
            },
            ComponentStatusResponse {
                name: "Background Consolidation".into(),
                status: "wired".into(),
                handler_type: "n/a".into(),
                notes: "DomainEventBus subscriber, accumulation buffers".into(),
            },
        ];

        let bus_subscribers = self
            .domain_event_bus()
            .map(|b| b.subscriber_count())
            .unwrap_or(0);

        Ok(SystemStatusResponse {
            domain_bus_subscribers: bus_subscribers,
            domain_bus_published: 0,
            background_service_running: true,
            background_events_processed: 0,
            active_facts,
            episodic_count: 0,
            rules_count: 0,
            components,
        })
    }

    // ── Mutations ───────────────────────────────────────────────────────

    pub async fn cognitive_fact_create(
        &self,
        params: FactCreateParams,
    ) -> Result<SemanticFactResponse, ApiError> {
        let pool = self.repos.pool();
        let fact_repo = SemanticFactRepo::new(pool.clone());

        let now = chrono::Utc::now().to_rfc3339();
        let fact = SemanticFact {
            id: uuid::Uuid::new_v4().to_string(),
            domain: params.domain,
            subject: params.subject,
            predicate: params.predicate,
            object: params.object,
            confidence: params.confidence,
            source: "debug_dashboard".to_string(),
            valid_from: now.clone(),
            valid_until: None,
            recorded_at: now,
            superseded_at: None,
            superseded_by: None,
            stability: 1.0,
            last_accessed: None,
            access_count: 0,
        };

        fact_repo.upsert(&fact).await.map_err(map_cognitive_err)?;

        Ok(fact_to_response(&fact))
    }

    pub async fn cognitive_fact_update(
        &self,
        id: String,
        params: FactUpdateParams,
    ) -> Result<SemanticFactResponse, ApiError> {
        let pool = self.repos.pool();
        let fact_repo = SemanticFactRepo::new(pool.clone());

        let mut fact = fact_repo
            .get(&id)
            .await
            .map_err(map_cognitive_err)?
            .ok_or_else(|| ApiError::new("NOT_FOUND", format!("fact {id} not found")))?;

        if let Some(obj) = params.object {
            fact.object = obj;
        }
        if let Some(conf) = params.confidence {
            fact.confidence = conf;
        }

        fact_repo.upsert(&fact).await.map_err(map_cognitive_err)?;

        Ok(fact_to_response(&fact))
    }

    pub async fn cognitive_fact_delete(&self, id: String) -> Result<bool, ApiError> {
        let pool = self.repos.pool();
        let fact_repo = SemanticFactRepo::new(pool.clone());

        fact_repo
            .supersede(&id, "deleted_by_debug_dashboard")
            .await
            .map_err(map_cognitive_err)?;

        Ok(true)
    }

    pub async fn cognitive_rule_create(
        &self,
        params: RuleCreateParams,
    ) -> Result<ProceduralRuleResponse, ApiError> {
        let pool = self.repos.pool();
        let repo = cognitive::repos::ProceduralRuleRepo::new(pool.clone());
        let now = chrono::Utc::now().to_rfc3339();

        let rule = cognitive::types::ProceduralRule {
            id: uuid::Uuid::new_v4().to_string(),
            domain: params.domain,
            rule_text: params.rule_text,
            confidence: params.confidence,
            source: "debug_dashboard".to_string(),
            signal_count: 0,
            created_at: now.clone(),
            updated_at: now,
            active: true,
        };

        repo.upsert(&rule).await.map_err(map_cognitive_err)?;

        Ok(rule_to_response(&rule))
    }

    pub async fn cognitive_rule_deactivate(&self, id: String) -> Result<bool, ApiError> {
        let pool = self.repos.pool();
        let repo = cognitive::repos::ProceduralRuleRepo::new(pool.clone());

        repo.deactivate(&id).await.map_err(map_cognitive_err)?;

        Ok(true)
    }

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
        let cognitive_provider = providers::create_cognitive_provider(&config).ok().flatten();
        drop(config);

        let reflection_handler: Box<dyn cognitive::ReflectionHandler> =
            if let Some(ref cp) = cognitive_provider {
                let config = self.config.read().await;
                let params = providers::cognitive_chat_params(&config, 2048);
                drop(config);
                Box::new(agent::cognitive_handlers::LlmReflectionHandler::new(
                    cp.clone(),
                    params,
                ))
            } else {
                Box::new(agent::cognitive_handlers::HeuristicReflectionHandler)
            };

        let consolidation_handler: Box<dyn cognitive::ConsolidationHandler> =
            if let Some(ref cp) = cognitive_provider {
                let config = self.config.read().await;
                let params = providers::cognitive_chat_params(&config, 1024);
                drop(config);
                Box::new(agent::cognitive_handlers::LlmConsolidationHandler::new(
                    cp.clone(),
                    params,
                ))
            } else {
                Box::new(agent::cognitive_handlers::HeuristicConsolidationHandler)
            };

        let output = cognitive::reflection::run_weekly_reflection(
            reflection_handler.as_ref(),
            consolidation_handler.as_ref(),
            &fact_repo,
            &episodic_repo,
            &rule_repo,
            None,
        )
        .await
        .map_err(|e| ApiError::new("REFLECTION_FAILED", &e.to_string()))?;

        Ok(ReflectionResultResponse {
            fact_updates: output.fact_updates.len(),
            rule_updates: output.rule_updates.len(),
            summary: output.summary,
        })
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
                user_message: payload["user_message"]
                    .as_str()
                    .unwrap_or("")
                    .to_string(),
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
