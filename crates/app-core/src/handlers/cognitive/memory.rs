//! Cognitive memory reads and helper functions.

use std::collections::HashMap;

use cognitive::decay::retrievability;
use cognitive::repos::{load_user_model, SemanticFactRepo, RULE_DOMAINS};
use cognitive::types::{ProceduralRule, SemanticFact};
use desktop_shared::cognitive_commands::*;
use desktop_shared::errors::ApiError;

use crate::errors::map_cognitive_err;
use crate::state::AppCore;

/// Build reflection + consolidation handler pair from a cognitive provider and config.
///
/// Returns LLM-backed handlers when a provider is available, heuristic fallbacks otherwise.
/// Used by both the manual "Run Reflection" API and the weekly cron job.
pub(crate) fn build_reflection_handlers(
    cognitive_provider: &Option<providers::DynProvider>,
    config: &config::Config,
) -> (
    Box<dyn cognitive::ReflectionHandler>,
    Box<dyn cognitive::ConsolidationHandler>,
) {
    let reflection: Box<dyn cognitive::ReflectionHandler> = if let Some(ref cp) = cognitive_provider
    {
        let params = providers::cognitive_chat_params(config, 2048);
        Box::new(agent::cognitive_handlers::LlmReflectionHandler::new(
            cp.clone(),
            params,
        ))
    } else {
        Box::new(agent::cognitive_handlers::HeuristicReflectionHandler)
    };

    let consolidation: Box<dyn cognitive::ConsolidationHandler> =
        if let Some(ref cp) = cognitive_provider {
            let params = providers::cognitive_chat_params(config, 1024);
            Box::new(agent::cognitive_handlers::LlmConsolidationHandler::new(
                cp.clone(),
                params,
            ))
        } else {
            Box::new(agent::cognitive_handlers::HeuristicConsolidationHandler)
        };

    (reflection, consolidation)
}

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

// -- Memory Reads -------------------------------------------------------------

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
            None => fact_repo
                .list_all_active()
                .await
                .map_err(map_cognitive_err)?,
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
        let limit = limit.unwrap_or(100).min(500);

        let memories = match domain.as_deref() {
            Some(d) => repo
                .list_by_domain(d, limit)
                .await
                .map_err(map_cognitive_err)?,
            None => repo.list_recent(limit).await.map_err(map_cognitive_err)?,
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

        let archived = fact_repo.count_archived().await.unwrap_or(0);

        let episodic_count = episodic_repo.count_all().await.unwrap_or(0) as usize;

        let rules_count = rule_repo.count_all_active().await.unwrap_or(0) as usize;

        Ok(MemoryStatsResponse {
            active_facts,
            archived_facts: archived,
            episodic_count,
            rules_count,
            last_compaction: None,
        })
    }

    // -- Knowledge Trust ------------------------------------------------------

    pub async fn memory_health(&self) -> Result<MemoryHealthResponse, ApiError> {
        use agent::autotuner::KNOWLEDGE_TRUST_SNAPSHOT_KEY;
        use cognitive::DomainHealthRow;

        let fact_repo = SemanticFactRepo::new(self.repos.pool().clone());
        let domains = fact_repo
            .fact_health_by_domain(90)
            .await
            .map_err(map_cognitive_err)?;

        let overall = DomainHealthRow::average_health(&domains);

        // Single pass: aggregate totals and build DTO entries together
        let mut total_facts_90d: i64 = 0;
        let mut fast_failures_90d: i64 = 0;
        let mut domain_entries = Vec::with_capacity(domains.len());
        for d in &domains {
            total_facts_90d += d.total_facts;
            fast_failures_90d += d.fast_failures;
            domain_entries.push(DomainHealthEntry {
                domain: d.domain.clone(),
                score: d.health_score(),
                total_facts: d.total_facts,
                active_facts: d.active_facts,
                fast_failures: d.fast_failures,
            });
        }

        // Read-only: trend is computed from a snapshot persisted by the autotuner cron,
        // not written here (writing on every UI read would corrupt the delta).
        let trend_pct = if let Ok(Some(snapshot)) = self
            .repos
            .learning_state
            .get_value(KNOWLEDGE_TRUST_SNAPSHOT_KEY)
            .await
        {
            snapshot
                .get("score")
                .and_then(|s| s.as_f64())
                .map(|prev| overall - prev)
        } else {
            None
        };

        Ok(MemoryHealthResponse {
            overall,
            domains: domain_entries,
            total_facts_90d,
            fast_failures_90d,
            trend_pct,
            computed_at: chrono::Utc::now().to_rfc3339(),
        })
    }

    // -- System Status --------------------------------------------------------

    pub async fn cognitive_system_status(&self) -> Result<SystemStatusResponse, ApiError> {
        let pool = self.repos.pool();
        let fact_repo = SemanticFactRepo::new(pool.clone());
        let model = load_user_model(&fact_repo).await;
        let active_facts = model.active_fact_count();
        let has_llm = self.cognitive_provider.is_some();

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

    // -- Memory Reference Detail -------------------------------------------------

    pub async fn memory_reference_detail(
        &self,
        ref_type: &str,
        ref_id: &str,
    ) -> Result<MemoryReferenceDetail, ApiError> {
        let pool = self.repos.pool();

        match ref_type {
            "fact" => {
                let fact_repo = SemanticFactRepo::new(pool.clone());
                match fact_repo.get(ref_id).await.map_err(map_cognitive_err)? {
                    Some(f) => {
                        let mut details = HashMap::new();
                        details.insert("Domain".to_string(), f.domain.clone());
                        details.insert(
                            "Confidence".to_string(),
                            format!("{:.0}%", f.confidence * 100.0),
                        );
                        details.insert("Stability".to_string(), format!("{:.1}", f.stability));
                        details.insert("Source".to_string(), f.source.clone());
                        if f.convergence_score > 0.0 {
                            details.insert(
                                "Convergence".to_string(),
                                format!("{:.0}%", f.convergence_score * 100.0),
                            );
                        }
                        Ok(MemoryReferenceDetail {
                            ref_type: "fact".to_string(),
                            title: format!("{}: {} = {}", f.subject, f.predicate, f.object),
                            subtitle: format!("{} domain", f.domain),
                            details,
                        })
                    }
                    None => Ok(MemoryReferenceDetail {
                        ref_type: "fact".to_string(),
                        title: "Unknown fact".to_string(),
                        subtitle: format!("ID: {}", ref_id),
                        details: HashMap::new(),
                    }),
                }
            }
            "rule" => {
                let rule_repo = cognitive::repos::ProceduralRuleRepo::new(pool.clone());
                let all_rules = rule_repo
                    .list_all_active()
                    .await
                    .map_err(map_cognitive_err)?;
                match all_rules.iter().find(|r| r.id == ref_id) {
                    Some(r) => {
                        let mut details = HashMap::new();
                        details.insert("Domain".to_string(), r.domain.clone());
                        details.insert(
                            "Confidence".to_string(),
                            format!("{:.0}%", r.confidence * 100.0),
                        );
                        details.insert("Signals".to_string(), r.signal_count.to_string());
                        details.insert("Source".to_string(), r.source.clone());
                        Ok(MemoryReferenceDetail {
                            ref_type: "rule".to_string(),
                            title: r.rule_text.clone(),
                            subtitle: format!("{} domain", r.domain),
                            details,
                        })
                    }
                    None => Ok(MemoryReferenceDetail {
                        ref_type: "rule".to_string(),
                        title: "Unknown rule".to_string(),
                        subtitle: format!("ID: {}", ref_id),
                        details: HashMap::new(),
                    }),
                }
            }
            "episode" => {
                let repo = cognitive::repos::EpisodicMemoryRepo::new(pool.clone());
                match repo.get(ref_id).await.map_err(map_cognitive_err)? {
                    Some(m) => {
                        let mut details = HashMap::new();
                        details.insert("Domain".to_string(), m.domain.clone());
                        details.insert(
                            "Importance".to_string(),
                            format!("{:.0}%", m.importance * 100.0),
                        );
                        details.insert("Occurred".to_string(), m.occurred_at.clone());
                        Ok(MemoryReferenceDetail {
                            ref_type: "episode".to_string(),
                            title: m
                                .summary
                                .clone()
                                .unwrap_or_else(|| m.content.chars().take(120).collect()),
                            subtitle: format!("{} domain", m.domain),
                            details,
                        })
                    }
                    None => Ok(MemoryReferenceDetail {
                        ref_type: "episode".to_string(),
                        title: "Unknown episode".to_string(),
                        subtitle: format!("ID: {}", ref_id),
                        details: HashMap::new(),
                    }),
                }
            }
            "note" => match self.note_repo.get_note(ref_id).await {
                Ok(Some(note)) => {
                    let tags = self.note_repo.get_tags(ref_id).await.unwrap_or_default();
                    let notebook_name = if let Some(ref nb_id) = note.notebook_id {
                        self.note_repo.list_notebooks().await.ok().and_then(|nbs| {
                            nbs.into_iter()
                                .find(|nb| &nb.id == nb_id)
                                .map(|nb| nb.title)
                        })
                    } else {
                        None
                    };
                    let mut details = HashMap::new();
                    if !tags.is_empty() {
                        details.insert("Tags".to_string(), tags.join(", "));
                    }
                    details.insert("Last edited".to_string(), note.updated_at.clone());
                    let body_preview: String = note.body.chars().take(200).collect();
                    if !body_preview.is_empty() {
                        details.insert("Preview".to_string(), body_preview);
                    }
                    Ok(MemoryReferenceDetail {
                        ref_type: "note".to_string(),
                        title: note.title.clone(),
                        subtitle: notebook_name.unwrap_or_else(|| "Note".to_string()),
                        details,
                    })
                }
                _ => Ok(MemoryReferenceDetail {
                    ref_type: "note".to_string(),
                    title: "Note".to_string(),
                    subtitle: format!("ID: {}", ref_id),
                    details: HashMap::new(),
                }),
            },
            "task" => match self.repos.tasks.get(ref_id).await {
                Ok(Some(task)) => {
                    let mut details = HashMap::new();
                    if let Some(ref desc) = task.description {
                        let preview: String = desc.chars().take(200).collect();
                        if !preview.is_empty() {
                            details.insert("Description".to_string(), preview);
                        }
                    }
                    if let Some(proj_id) = &task.project_id {
                        details.insert("Project".to_string(), proj_id.clone());
                    }
                    if task.area_id != "default" {
                        details.insert("Area".to_string(), task.area_id.clone());
                    }
                    if let Some(p) = task.priority {
                        let label = match p {
                            1 => "urgent",
                            2 => "high",
                            3 => "medium",
                            4 => "low",
                            _ => "none",
                        };
                        details.insert("Priority".to_string(), label.to_string());
                    }
                    let subtitle = if let Some(due) = task.due_date {
                        format!("{} · due {}", task.status, due.format("%Y-%m-%d"))
                    } else {
                        task.status.clone()
                    };
                    Ok(MemoryReferenceDetail {
                        ref_type: "task".to_string(),
                        title: task.title.clone(),
                        subtitle,
                        details,
                    })
                }
                _ => Ok(MemoryReferenceDetail {
                    ref_type: "task".to_string(),
                    title: "Task".to_string(),
                    subtitle: format!("ID: {}", ref_id),
                    details: HashMap::new(),
                }),
            },
            _ => Ok(MemoryReferenceDetail {
                ref_type: ref_type.to_string(),
                title: "Unknown reference type".to_string(),
                subtitle: format!("Type: {}, ID: {}", ref_type, ref_id),
                details: HashMap::new(),
            }),
        }
    }
}
