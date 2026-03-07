use std::sync::Arc;

use cognitive::decay::retrievability;
use cognitive::repos::{load_user_model, SemanticFactRepo, USER_MODEL_DOMAINS};
use cognitive::types::{ProceduralRule, SemanticFact};
use desktop_shared::cognitive_commands::*;
use desktop_shared::errors::ApiError;
use tauri::State;

use crate::app_core::AppCore;

fn fact_to_response(f: &SemanticFact) -> SemanticFactResponse {
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

fn rule_to_response(r: &ProceduralRule) -> ProceduralRuleResponse {
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

fn active_fact_count(model: &cognitive::types::UserModel) -> usize {
    model.identity.len()
        + model.energy.len()
        + model.work.len()
        + model.finance.len()
        + model.learning.len()
        + model.preferences.len()
}

fn fact_preview(fact: &SemanticFact) -> String {
    format!("{} = {}", fact.predicate, fact.object)
}

// ── Memory Reads ────────────────────────────────────────────────────────

#[tauri::command]
pub async fn cognitive_user_model(
    state: State<'_, Arc<AppCore>>,
) -> Result<UserModelSummaryResponse, ApiError> {
    let pool = state.repos.pool();
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

#[tauri::command]
pub async fn cognitive_facts_list(
    state: State<'_, Arc<AppCore>>,
    domain: Option<String>,
) -> Result<Vec<SemanticFactResponse>, ApiError> {
    let pool = state.repos.pool();
    let fact_repo = SemanticFactRepo::new(pool.clone());

    let domains: Vec<&str> = match domain.as_deref() {
        Some(d) => vec![d],
        None => USER_MODEL_DOMAINS.to_vec(),
    };

    let mut all_facts = Vec::new();
    for d in &domains {
        let facts = fact_repo
            .list_active(d)
            .await
            .map_err(super::map_cognitive_err)?;
        all_facts.extend(facts);
    }

    Ok(all_facts.iter().map(fact_to_response).collect())
}

#[tauri::command]
pub async fn cognitive_episodic_list(
    state: State<'_, Arc<AppCore>>,
    domain: Option<String>,
    limit: Option<i64>,
) -> Result<Vec<EpisodicMemoryResponse>, ApiError> {
    let pool = state.repos.pool();
    let repo = cognitive::repos::EpisodicMemoryRepo::new(pool.clone());
    let limit = limit.unwrap_or(50);

    let memories = match domain.as_deref() {
        Some(d) => repo
            .list_by_domain(d, limit)
            .await
            .map_err(super::map_cognitive_err)?,
        None => {
            let mut all = Vec::new();
            for d in USER_MODEL_DOMAINS {
                let mems = repo
                    .list_by_domain(d, limit)
                    .await
                    .map_err(super::map_cognitive_err)?;
                all.extend(mems);
            }
            all
        }
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

#[tauri::command]
pub async fn cognitive_rules_list(
    state: State<'_, Arc<AppCore>>,
    domain: Option<String>,
) -> Result<Vec<ProceduralRuleResponse>, ApiError> {
    let pool = state.repos.pool();
    let repo = cognitive::repos::ProceduralRuleRepo::new(pool.clone());

    let domains: Vec<&str> = match domain.as_deref() {
        Some(d) => vec![d],
        None => USER_MODEL_DOMAINS.to_vec(),
    };

    let mut all_rules = Vec::new();
    for d in &domains {
        let rules = repo
            .list_active(d)
            .await
            .map_err(super::map_cognitive_err)?;
        all_rules.extend(rules);
    }

    Ok(all_rules.iter().map(rule_to_response).collect())
}

#[tauri::command]
pub async fn cognitive_memory_stats(
    state: State<'_, Arc<AppCore>>,
) -> Result<MemoryStatsResponse, ApiError> {
    let pool = state.repos.pool();
    let fact_repo = SemanticFactRepo::new(pool.clone());
    let episodic_repo = cognitive::repos::EpisodicMemoryRepo::new(pool.clone());
    let rule_repo = cognitive::repos::ProceduralRuleRepo::new(pool.clone());

    let model = load_user_model(&fact_repo).await;
    let active_facts = active_fact_count(&model);

    // TODO: add a `count_archived()` query to SemanticFactRepo — archive_superseded() is
    // a destructive mutation and must NOT be used for reads.
    let archived = 0u64;

    let mut episodic_count = 0;
    for d in USER_MODEL_DOMAINS {
        episodic_count += episodic_repo
            .list_by_domain(d, 10000)
            .await
            .map(|v| v.len())
            .unwrap_or(0);
    }

    let mut rules_count = 0;
    for d in USER_MODEL_DOMAINS {
        rules_count += rule_repo
            .list_active(d)
            .await
            .map(|v| v.len())
            .unwrap_or(0);
    }

    Ok(MemoryStatsResponse {
        active_facts,
        archived_facts: archived,
        episodic_count,
        rules_count,
        last_compaction: None,
    })
}

// ── Coaching Reads ──────────────────────────────────────────────────────

#[tauri::command]
pub async fn coaching_situation(
    state: State<'_, Arc<AppCore>>,
) -> Result<UserSituationResponse, ApiError> {
    let sit = state.user_situation()?.lock().await;
    Ok(UserSituationResponse {
        energy_level: sit.energy_level,
        focus_state: sit.focus_state,
        deadline_pressure: sit.deadline_pressure,
        distraction_risk: sit.distraction_risk,
        coaching_receptivity: sit.coaching_receptivity,
        task_avoidance_detected: sit.task_avoidance_detected,
        hours_active_today: sit.hours_active_today,
        mins_since_break: sit.mins_since_break,
        hour_of_day: sit.hour_of_day,
        recent_context_switches: sit.recent_context_switches,
    })
}

#[tauri::command]
pub async fn coaching_signals(
    state: State<'_, Arc<AppCore>>,
) -> Result<SignalWindowResponse, ApiError> {
    let acc = state.signal_accumulator()?.lock().await;

    Ok(SignalWindowResponse {
        window_size: acc.window_size(),
        signals: vec![],
        triggers: vec![],
    })
}

#[tauri::command]
pub async fn coaching_patterns(
    state: State<'_, Arc<AppCore>>,
) -> Result<Vec<DetectedPatternResponse>, ApiError> {
    let detector = state.pattern_detector()?.lock().await;
    let patterns = detector.detect_patterns();

    Ok(patterns
        .iter()
        .map(|p| DetectedPatternResponse {
            name: p.name.clone(),
            confidence: p.confidence,
            signal_count: p.signal_count,
            description: p.description.clone(),
            domain: p.domain.clone(),
        })
        .collect())
}

#[tauri::command]
pub async fn coaching_feedback_stats(
    state: State<'_, Arc<AppCore>>,
) -> Result<Vec<StrategyFeedbackResponse>, ApiError> {
    let tracker = state.feedback_tracker()?.lock().await;

    Ok(tracker
        .all_strategies()
        .iter()
        .map(|s| StrategyFeedbackResponse {
            strategy_type: s.strategy_type.clone(),
            domain: s.domain.clone(),
            times_used: s.times_used,
            acceptance_rate: s.acceptance_rate(),
            effectiveness: s.effectiveness(),
            behavioral_positive: s.behavioral_positive,
            behavioral_negative: s.behavioral_negative,
        })
        .collect())
}

#[tauri::command]
pub async fn coaching_router_status(
    state: State<'_, Arc<AppCore>>,
) -> Result<RouterStatusResponse, ApiError> {
    let router = state.intervention_router()?.lock().await;

    Ok(RouterStatusResponse {
        hourly_count: router.hourly_count(),
        hourly_limit: 3,
        daily_count: 0, // TODO: expose daily_count on InterventionRouter
        daily_limit: 10,
    })
}

// ── System Status ───────────────────────────────────────────────────────

#[tauri::command]
pub async fn cognitive_system_status(
    state: State<'_, Arc<AppCore>>,
) -> Result<SystemStatusResponse, ApiError> {
    let pool = state.repos.pool();
    let fact_repo = SemanticFactRepo::new(pool.clone());
    let model = load_user_model(&fact_repo).await;
    let active_facts = active_fact_count(&model);

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
            handler_type: "heuristic".into(),
            notes: "HeuristicExtractionHandler in agent crate".into(),
        },
        ComponentStatusResponse {
            name: "Consolidation".into(),
            status: "wired".into(),
            handler_type: "heuristic".into(),
            notes: "HeuristicConsolidationHandler in agent crate".into(),
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
            status: "built".into(),
            handler_type: "heuristic".into(),
            notes: "ReflectionHandler trait defined, needs scheduling".into(),
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
            status: "stub".into(),
            handler_type: "llm".into(),
            notes: "Trait defined, not yet implemented".into(),
        },
        ComponentStatusResponse {
            name: "LLM Consolidation".into(),
            status: "stub".into(),
            handler_type: "llm".into(),
            notes: "Trait defined, not yet implemented".into(),
        },
        ComponentStatusResponse {
            name: "LLM Reflection".into(),
            status: "stub".into(),
            handler_type: "llm".into(),
            notes: "ReflectionHandler trait, not yet implemented".into(),
        },
        ComponentStatusResponse {
            name: "LLM Coaching Reasoner".into(),
            status: "stub".into(),
            handler_type: "llm".into(),
            notes: "CoachingReasonerHandler trait, heuristic fallback exists".into(),
        },
        ComponentStatusResponse {
            name: "Background Consolidation".into(),
            status: "wired".into(),
            handler_type: "n/a".into(),
            notes: "DomainEventBus subscriber, accumulation buffers".into(),
        },
    ];

    Ok(SystemStatusResponse {
        domain_bus_subscribers: 0, // TODO: expose subscriber_count on DomainEventBus
        domain_bus_published: 0,
        background_service_running: true,
        background_events_processed: 0,
        active_facts,
        episodic_count: 0,
        rules_count: 0,
        components,
    })
}

// ── Mutations ───────────────────────────────────────────────────────────

#[tauri::command]
pub async fn cognitive_fact_create(
    state: State<'_, Arc<AppCore>>,
    params: FactCreateParams,
) -> Result<SemanticFactResponse, ApiError> {
    let pool = state.repos.pool();
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

    fact_repo
        .upsert(&fact)
        .await
        .map_err(super::map_cognitive_err)?;

    Ok(fact_to_response(&fact))
}

#[tauri::command]
pub async fn cognitive_fact_update(
    state: State<'_, Arc<AppCore>>,
    id: String,
    params: FactUpdateParams,
) -> Result<SemanticFactResponse, ApiError> {
    let pool = state.repos.pool();
    let fact_repo = SemanticFactRepo::new(pool.clone());

    let mut fact = fact_repo
        .get(&id)
        .await
        .map_err(super::map_cognitive_err)?
        .ok_or_else(|| ApiError::new("NOT_FOUND", format!("fact {id} not found")))?;

    if let Some(obj) = params.object {
        fact.object = obj;
    }
    if let Some(conf) = params.confidence {
        fact.confidence = conf;
    }

    fact_repo
        .upsert(&fact)
        .await
        .map_err(super::map_cognitive_err)?;

    Ok(fact_to_response(&fact))
}

#[tauri::command]
pub async fn cognitive_fact_delete(
    state: State<'_, Arc<AppCore>>,
    id: String,
) -> Result<bool, ApiError> {
    let pool = state.repos.pool();
    let fact_repo = SemanticFactRepo::new(pool.clone());

    fact_repo
        .supersede(&id, "deleted_by_debug_dashboard")
        .await
        .map_err(super::map_cognitive_err)?;

    Ok(true)
}

#[tauri::command]
pub async fn cognitive_rule_create(
    state: State<'_, Arc<AppCore>>,
    params: RuleCreateParams,
) -> Result<ProceduralRuleResponse, ApiError> {
    let pool = state.repos.pool();
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

    repo.upsert(&rule)
        .await
        .map_err(super::map_cognitive_err)?;

    Ok(rule_to_response(&rule))
}

#[tauri::command]
pub async fn cognitive_rule_deactivate(
    state: State<'_, Arc<AppCore>>,
    id: String,
) -> Result<bool, ApiError> {
    let pool = state.repos.pool();
    let repo = cognitive::repos::ProceduralRuleRepo::new(pool.clone());

    repo.deactivate(&id)
        .await
        .map_err(super::map_cognitive_err)?;

    Ok(true)
}

#[tauri::command]
pub async fn cognitive_run_compaction(
    state: State<'_, Arc<AppCore>>,
) -> Result<CompactionResultResponse, ApiError> {
    let pool = state.repos.pool();
    let fact_repo = SemanticFactRepo::new(pool.clone());
    let episodic_repo = cognitive::repos::EpisodicMemoryRepo::new(pool.clone());

    let archived = fact_repo
        .archive_superseded(90)
        .await
        .map_err(super::map_cognitive_err)?;

    let deleted_episodic = episodic_repo
        .delete_old(90, 2)
        .await
        .map_err(super::map_cognitive_err)?;

    Ok(CompactionResultResponse {
        archived_count: archived,
        deleted_episodic,
    })
}

// ── Coaching Mutations ──────────────────────────────────────────────────

#[tauri::command]
pub async fn coaching_reset_dismissals(
    state: State<'_, Arc<AppCore>>,
    trigger_name: Option<String>,
) -> Result<bool, ApiError> {
    let mut router = state.intervention_router()?.lock().await;

    if let Some(name) = trigger_name {
        router.reset_dismissals(&name);
    }

    Ok(true)
}

#[tauri::command]
pub async fn coaching_clear_signals(
    state: State<'_, Arc<AppCore>>,
) -> Result<bool, ApiError> {
    let mut acc = state.signal_accumulator()?.lock().await;
    *acc = feature_coaching::SignalAccumulator::new();
    Ok(true)
}
