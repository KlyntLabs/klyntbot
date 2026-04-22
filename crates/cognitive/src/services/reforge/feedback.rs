//! Feedback signal loading for Reforge Phase 1.
//!
//! Reads existing SQL tables to surface tool failures, corrections,
//! behavioral metrics, and graph health for the Reforge cycle.

use std::collections::HashMap;

use tracing::warn;

use super::types::*;

/// Event type string for user correction events in the domain_event_log.
const EVENT_USER_CORRECTED_AI: &str = bus::DomainEvent::KIND_USER_CORRECTED_AI;

/// Load tool failure stats from the learning_outcomes table.
pub async fn load_tool_failures(
    outcome_repo: &storage::OutcomeRepo,
    since: jiff::Timestamp,
) -> Vec<ToolFailureSummary> {
    match outcome_repo.tool_failure_stats_since(since).await {
        Ok(rows) => rows
            .into_iter()
            .map(|r| ToolFailureSummary {
                tool_name: r.tool_name,
                total_calls: r.total_calls as u32,
                failure_count: r.failure_count as u32,
                failure_rate: if r.total_calls > 0 {
                    r.failure_count as f64 / r.total_calls as f64
                } else {
                    0.0
                },
                error_types: r
                    .error_types
                    .unwrap_or_default()
                    .split(',')
                    .filter(|s| !s.is_empty())
                    .map(String::from)
                    .collect(),
            })
            .collect(),
        Err(e) => {
            warn!("Reforge feedback: failed to load tool failures: {e}");
            vec![]
        }
    }
}

/// Load correction summaries grouped by skill from the domain_event_log.
pub async fn load_correction_summaries(
    event_log_repo: &crate::repos::EventLogRepo,
    since: &str,
) -> Vec<CorrectionSummary> {
    let events = match event_log_repo
        .list_by_event_type_since(EVENT_USER_CORRECTED_AI, since)
        .await
    {
        Ok(rows) => rows,
        Err(e) => {
            warn!("Reforge feedback: failed to load correction events: {e}");
            return vec![];
        }
    };

    let mut by_skill: HashMap<String, Vec<String>> = HashMap::new();
    for event in &events {
        let payload: serde_json::Value = match serde_json::from_str(&event.payload) {
            Ok(v) => v,
            Err(_) => continue,
        };
        let skill = payload
            .get("active_skill")
            .and_then(|v| v.as_str())
            .unwrap_or("unknown")
            .to_string();
        let correction = payload
            .get("correction")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string();
        by_skill.entry(skill).or_default().push(correction);
    }

    by_skill
        .into_iter()
        .map(|(skill_name, corrections)| CorrectionSummary {
            correction_count: corrections.len() as u32,
            sample_corrections: corrections.into_iter().take(3).collect(),
            skill_name,
        })
        .collect()
}

/// Load behavioral metrics from feature crate tables.
/// Uses raw SQL because cognitive doesn't depend on feature crates.
pub async fn load_behavioral_metrics(pool: &sqlx::SqlitePool) -> BehavioralMetrics {
    let mut metrics = BehavioralMetrics::default();

    // Task estimation bias (from task_estimation_history)
    if let Ok(row) = sqlx::query_as::<_, (f64,)>(
        "SELECT AVG(deviation_pct) FROM task_estimation_history
         WHERE created_at > datetime('now', '-7 days')
         HAVING COUNT(*) >= 3",
    )
    .fetch_optional(pool)
    .await
    {
        metrics.task_estimation_bias = row.map(|r| r.0);
    }

    // Coaching acceptance rate (from coaching_strategies)
    if let Ok(row) = sqlx::query_as::<_, (f64,)>(
        "SELECT CAST(SUM(times_accepted) AS REAL) / NULLIF(SUM(times_used), 0)
         FROM coaching_strategies
         WHERE times_used > 0",
    )
    .fetch_optional(pool)
    .await
    {
        metrics.coaching_acceptance_rate =
            row.and_then(|r| if r.0 > 0.0 { Some(r.0) } else { None });
    }

    // Focus quality trend (7d avg vs 14d avg from daily_summaries)
    if let Ok(Some((recent, prev))) = sqlx::query_as::<_, (f64, f64)>(
        "SELECT
           (SELECT AVG(avg_session_quality) FROM daily_summaries WHERE date > date('now', '-7 days')),
           (SELECT AVG(avg_session_quality) FROM daily_summaries WHERE date > date('now', '-14 days') AND date <= date('now', '-7 days'))",
    )
    .fetch_optional(pool)
    .await
    {
        if prev > 0.0 {
            metrics.focus_quality_trend = Some(recent - prev);
        }
    }

    // Suggestion dismiss rate (from task_suggestions)
    if let Ok(row) = sqlx::query_as::<_, (f64,)>(
        "SELECT CAST(SUM(CASE WHEN status = 'dismissed' THEN 1 ELSE 0 END) AS REAL)
              / NULLIF(COUNT(*), 0)
         FROM task_suggestions
         WHERE resolved_at > datetime('now', '-7 days')
         HAVING COUNT(*) >= 3",
    )
    .fetch_optional(pool)
    .await
    {
        metrics.suggestion_dismiss_rate = row.map(|r| r.0);
    }

    // Forecast accuracy (from productivity_forecasts)
    if let Ok(row) = sqlx::query_as::<_, (f64,)>(
        "SELECT AVG(ABS(prediction_error)) FROM productivity_forecasts
         WHERE created_at > datetime('now', '-7 days')
         HAVING COUNT(*) >= 3",
    )
    .fetch_optional(pool)
    .await
    {
        metrics.forecast_accuracy = row.map(|r| r.0);
    }

    metrics
}

/// Load knowledge graph health metrics.
pub async fn load_graph_health(
    fact_repo: &crate::repos::SemanticFactRepo,
    rule_repo: &crate::repos::ProceduralRuleRepo,
    co_activation_repo: &crate::repos::CoActivationRepo,
) -> GraphHealthMetrics {
    let (active_facts, active_rules, co_activation_pairs, facts_per_domain, avg_fact_stability) = tokio::join!(
        fact_repo.count_active(),
        rule_repo.count_all_active(),
        co_activation_repo.count_all(),
        fact_repo.count_by_domain(),
        fact_repo.avg_stability(),
    );

    GraphHealthMetrics {
        active_facts: active_facts.unwrap_or(0) as u32,
        active_rules: active_rules.unwrap_or(0) as u32,
        co_activation_pairs: co_activation_pairs.unwrap_or(0) as u32,
        facts_per_domain: facts_per_domain.unwrap_or_default(),
        avg_fact_stability: avg_fact_stability.unwrap_or(1.0),
    }
}

/// Load previous Reforge suggestions that haven't been acted upon.
pub async fn load_previous_suggestions(
    suggestion_repo: &storage::ReforgeSuggestionRepo,
) -> Vec<ReforgeSuggestion> {
    match suggestion_repo.recent_unacted(10).await {
        Ok(rows) => rows
            .into_iter()
            .map(|r| ReforgeSuggestion {
                suggestion_type: r.suggestion_type,
                content: r.content,
                reason: r.reason,
                confidence: r.confidence,
                cycle_run_at: r.cycle_run_at,
            })
            .collect(),
        Err(e) => {
            warn!("Reforge feedback: failed to load previous suggestions: {e}");
            vec![]
        }
    }
}

/// Load runtime signal summaries from strategy_records since last run.
pub async fn load_runtime_signals(
    pool: &sqlx::SqlitePool,
    since: &str,
) -> super::types::RuntimeSignalSummary {
    let row: Option<(i64, f64, i64, f64)> = sqlx::query_as(
        "SELECT
            COALESCE(SUM(CASE WHEN budget_exhausted = 1 THEN 1 ELSE 0 END), 0),
            COALESCE(AVG(turns_used), 0),
            COALESCE(SUM(CASE WHEN loop_detected = 1 THEN 1 ELSE 0 END), 0),
            COALESCE(AVG(context_fill_pct), 0)
         FROM strategy_records WHERE timestamp > ?1",
    )
    .bind(since)
    .fetch_optional(pool)
    .await
    .ok()
    .flatten();

    match row {
        Some((exhaustions, avg_turns, loops, fill)) => super::types::RuntimeSignalSummary {
            budget_exhaustions: exhaustions as u32,
            avg_turns,
            loop_detections: loops as u32,
            avg_context_fill_pct: fill,
        },
        None => super::types::RuntimeSignalSummary::default(),
    }
}

/// Load validation warning counts since last run.
pub async fn load_validation_warnings(pool: &sqlx::SqlitePool, since: &str) -> Vec<(String, i64)> {
    sqlx::query_as(
        "SELECT warning_type, COUNT(*) FROM response_warnings
         WHERE created_at > ?1 GROUP BY warning_type",
    )
    .bind(since)
    .fetch_all(pool)
    .await
    .unwrap_or_default()
}

/// Load near-miss accumulation count.
pub async fn load_near_miss_count(pool: &sqlx::SqlitePool) -> u32 {
    let row: Option<(i64,)> = sqlx::query_as(
        "SELECT COALESCE(SUM(near_miss_count), 0) FROM accumulated_observations WHERE near_miss_count > 0",
    )
    .fetch_optional(pool)
    .await
    .ok()
    .flatten();
    row.map(|r| r.0 as u32).unwrap_or(0)
}

/// Load coaching behavioral outcome summary.
pub async fn load_coaching_behavioral(
    pool: &sqlx::SqlitePool,
) -> Option<super::types::CoachingBehavioralSummary> {
    let row: Option<(i64, i64, i64, i64)> = sqlx::query_as(
        "SELECT COALESCE(SUM(behavioral_positive), 0), COALESCE(SUM(behavioral_negative), 0),
                COALESCE(SUM(times_accepted), 0), COALESCE(SUM(times_used), 0)
         FROM coaching_strategies",
    )
    .fetch_optional(pool)
    .await
    .ok()
    .flatten();

    row.map(|(pos, neg, accepted, used)| {
        let acceptance_rate = if used > 0 {
            accepted as f64 / used as f64
        } else {
            0.0
        };
        super::types::CoachingBehavioralSummary {
            total_positive: pos as u32,
            total_negative: neg as u32,
            acceptance_rate,
        }
    })
}

/// Load enhancement pipeline aggregate metrics for Reforge analysis.
pub async fn collect_enhancement_signals(
    trace_repo: &crate::repos::EnhancementTraceRepo,
    since: &str,
) -> Vec<super::types::EnhancementSignal> {
    match trace_repo.load_aggregates_since(since).await {
        Ok(aggregates) => aggregates
            .into_iter()
            .map(|a| super::types::EnhancementSignal {
                depth_mode: a.depth_mode,
                total_runs: a.total_runs as u32,
                avg_latency_ms: a.avg_latency_ms,
                avg_llm_calls: a.avg_llm_calls,
                avg_confidence: a.avg_confidence,
            })
            .collect(),
        Err(e) => {
            tracing::warn!("Reforge feedback: failed to load enhancement signals: {e}");
            vec![]
        }
    }
}

/// Load extraction yield by domain from pipeline_event_log.
pub async fn load_extraction_yield(
    event_log_repo: &crate::repos::EventLogRepo,
    since: &str,
) -> Vec<(String, f64)> {
    match event_log_repo.extraction_yield_by_domain(since).await {
        Ok(rows) => rows,
        Err(e) => {
            warn!("Reforge feedback: failed to load extraction yield: {e}");
            vec![]
        }
    }
}
