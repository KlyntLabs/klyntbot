//! Phase 1 of the Reforge cycle: gather all inputs needed for nightly synthesis.

use std::collections::HashMap;

use chrono::{Duration, Utc};
use tracing::debug;

use crate::repos::{
    load_user_model, EpisodicMemoryRepo, ProceduralRuleRepo, SemanticFactRepo, RULE_DOMAINS,
};
use crate::services::reforge::skill_files::{SkillFile, SkillFileManager};
use crate::services::reforge::types::{
    BehavioralMetrics, GraphHealthMetrics, ReforgeCollected, RoutingSummary, SessionContext,
};

/// Scan skill files on disk and record a new version row for any file whose
/// content differs from the latest known version in the database.
///
/// This detects manual edits the user made between Reforge cycles so the
/// nightly synthesizer always works from the current on-disk state.
///
/// Returns the files read from disk so callers can reuse them without a
/// redundant `read_all()` call.
pub async fn detect_user_edits(
    skill_mgr: &SkillFileManager,
    version_repo: &storage::repos::SkillVersionRepo,
) -> HashMap<String, Vec<SkillFile>> {
    let all_files = skill_mgr.read_all();
    for (skill_name, files) in &all_files {
        for file in files {
            let latest = version_repo
                .latest_version(skill_name, &file.file_path)
                .await
                .ok()
                .flatten();
            if let Some(latest) = latest {
                let known_hash = super::skill_files::content_hash(&latest.content);
                if known_hash != file.content_hash {
                    let diff = super::skill_files::compute_diff(&latest.content, &file.content);
                    let row = storage::rows::SkillVersionRow {
                        id: uuid::Uuid::new_v4().to_string(),
                        skill_name: skill_name.clone(),
                        version: latest.version + 1,
                        file_path: file.file_path.clone(),
                        content: file.content.clone(),
                        diff: Some(diff),
                        source: "User".to_string(),
                        reason: Some("Detected manual file edit".to_string()),
                        created_at: chrono::Utc::now().to_rfc3339(),
                    };
                    if let Err(e) = version_repo.insert(&row).await {
                        tracing::warn!(
                            "Failed to record user edit for {}/{}: {e}",
                            skill_name,
                            file.file_path
                        );
                    }
                    tracing::debug!("Detected user edit to {}/{}", skill_name, file.file_path);
                }
            }
        }
    }
    all_files
}

/// Optional data sources for feedback collection.
/// All fields are optional — missing sources are gracefully skipped.
pub struct FeedbackSources<'a> {
    pub outcome_repo: Option<&'a storage::OutcomeRepo>,
    pub event_log_repo: Option<&'a crate::repos::EventLogRepo>,
    pub co_activation_repo: Option<&'a crate::repos::CoActivationRepo>,
    pub suggestion_repo: Option<&'a storage::ReforgeSuggestionRepo>,
    pub pool: Option<&'a sqlx::SqlitePool>,
    pub density_repo: Option<&'a crate::repos::ConversationDensityRepo>,
}

/// Days elapsed since an RFC 3339 timestamp, defaulting to 7 if missing or unparseable.
fn days_since(ts: Option<&str>) -> i64 {
    match ts {
        Some(s) => chrono::DateTime::parse_from_rfc3339(s)
            .map(|dt| (Utc::now() - dt.with_timezone(&Utc)).num_days().max(1))
            .unwrap_or(7),
        None => 7,
    }
}

/// Collect all inputs required for a Reforge cycle.
///
/// - `last_run_at` — RFC 3339 timestamp of the previous run, or `None` for a
///   first-run bootstrap (uses a 7-day look-back window instead).
/// - `pre_read_skill_files` — if provided, reuses previously-read skill files
///   (e.g. from `detect_user_edits`) to avoid a redundant `read_all()` call.
/// - `mirror_repo` — optional mirror repo for routing snapshots and meta-rules.
/// - `feedback_repo` — optional retrieval feedback repo for precision stats.
///
/// Returns `None` when there is nothing new to process (no sessions and no
/// episodic memories since the last run and we are not in bootstrap mode).
#[allow(clippy::too_many_arguments)]
pub async fn collect(
    last_run_at: Option<&str>,
    session_memory_repo: &storage::SessionMemoryRepo,
    fact_repo: &SemanticFactRepo,
    episodic_repo: &EpisodicMemoryRepo,
    rule_repo: &ProceduralRuleRepo,
    skill_mgr: &SkillFileManager,
    pre_read_skill_files: Option<HashMap<String, Vec<SkillFile>>>,
    mirror_repo: Option<&crate::mirror::MirrorRepo>,
    feedback_repo: Option<&storage::RetrievalFeedbackRepo>,
    feedback_sources: Option<&FeedbackSources<'_>>,
) -> Option<ReforgeCollected> {
    let is_bootstrap = last_run_at.is_none();

    // Determine the "since" timestamp: last run or 7 days ago for bootstrap.
    let bootstrap_since;
    let since: &str = match last_run_at {
        Some(ts) => ts,
        None => {
            bootstrap_since = (Utc::now() - Duration::days(7)).to_rfc3339();
            &bootstrap_since
        }
    };

    // --- Session scratchpads ---
    let session_rows = match session_memory_repo.list_since(since).await {
        Ok(rows) => rows,
        Err(e) => {
            tracing::warn!("Reforge collector: failed to load session memories: {e}");
            vec![]
        }
    };
    let sessions: Vec<SessionContext> = session_rows
        .into_iter()
        .map(|r| SessionContext {
            session_key: r.session_key,
            scratchpad: r.content,
            updated_at: r.updated_at.to_string(),
            turn_count: r.turn_count,
        })
        .collect();

    // --- Episodic memories ---
    let now_str = Utc::now().to_rfc3339();
    let episodic_memories = match episodic_repo.list_range(since, &now_str).await {
        Ok(mems) => mems,
        Err(e) => {
            tracing::warn!("Reforge collector: failed to load episodic memories: {e}");
            vec![]
        }
    };

    // --- Skip gate ---
    // If neither sessions nor episodics have new data and this is not a
    // bootstrap run, there is nothing to do.
    if !is_bootstrap && sessions.is_empty() && episodic_memories.is_empty() {
        debug!("Reforge collector: no new data since {since} — skipping cycle");
        return None;
    }

    // --- User model ---
    let user_model = load_user_model(fact_repo).await;

    // --- Procedural rules ---
    let mut rules = Vec::new();
    for domain in RULE_DOMAINS {
        match rule_repo.list_active(domain).await {
            Ok(mut domain_rules) => rules.append(&mut domain_rules),
            Err(e) => {
                tracing::warn!("Reforge collector: failed to load rules for {domain}: {e}");
            }
        }
    }

    // --- Skill files ---
    let skill_files = pre_read_skill_files.unwrap_or_else(|| skill_mgr.read_all());

    // --- Routing summaries (from Mirror routing snapshots) ---
    let routing_summaries = if let Some(mirror) = mirror_repo {
        match mirror.get_routing_history(7).await {
            Ok(snapshots) => aggregate_routing_snapshots(&snapshots),
            Err(e) => {
                tracing::warn!("Reforge collector: failed to load routing history: {e}");
                vec![]
            }
        }
    } else {
        vec![]
    };

    // --- Pending meta-rules ---
    let pending_meta_rules = if let Some(mirror) = mirror_repo {
        match mirror
            .get_meta_rules_by_status(crate::mirror::MetaRuleStatus::Pending)
            .await
        {
            Ok(rules) => rules.into_iter().map(|r| r.trigger_condition).collect(),
            Err(e) => {
                tracing::warn!("Reforge collector: failed to load pending meta-rules: {e}");
                vec![]
            }
        }
    } else {
        vec![]
    };

    // --- Retrieval precision ---
    let retrieval_precision = if let Some(fb_repo) = feedback_repo {
        let days = days_since(last_run_at);
        match fb_repo.avg_precision_since(days).await {
            Ok(avg) if avg > 0.0 => Some(avg),
            Ok(_) => None,
            Err(e) => {
                tracing::warn!("Reforge collector: failed to load retrieval precision: {e}");
                None
            }
        }
    } else {
        None
    };

    // --- Feedback signals ---
    let (
        tool_failures,
        correction_summaries,
        behavioral_metrics,
        graph_health,
        previous_suggestions,
        retrieval_precision_by_domain,
        extraction_yield_by_domain,
    ) = if let Some(fb) = feedback_sources {
        let since_dt = chrono::DateTime::parse_from_rfc3339(since)
            .map(|dt| dt.with_timezone(&chrono::Utc))
            .unwrap_or_else(|_| chrono::Utc::now() - Duration::days(7));

        let tool_failures = if let Some(repo) = fb.outcome_repo {
            super::feedback::load_tool_failures(repo, since_dt).await
        } else {
            vec![]
        };

        let correction_summaries = if let Some(repo) = fb.event_log_repo {
            super::feedback::load_correction_summaries(repo, since).await
        } else {
            vec![]
        };

        let behavioral_metrics = if let Some(pool) = fb.pool {
            super::feedback::load_behavioral_metrics(pool).await
        } else {
            BehavioralMetrics::default()
        };

        let graph_health = super::feedback::load_graph_health(
            fact_repo,
            rule_repo,
            fb.co_activation_repo.unwrap_or(
                // Fallback: create a temporary repo from fact_repo's pool
                &crate::repos::CoActivationRepo::new(fact_repo.pool().clone()),
            ),
        )
        .await;

        let previous_suggestions = if let Some(repo) = fb.suggestion_repo {
            super::feedback::load_previous_suggestions(repo).await
        } else {
            vec![]
        };

        let retrieval_precision_by_domain = if let Some(repo) = feedback_repo {
            repo.avg_precision_by_domain_since(days_since(last_run_at))
                .await
                .unwrap_or_default()
        } else {
            vec![]
        };

        let extraction_yield_by_domain = if let Some(repo) = fb.event_log_repo {
            super::feedback::load_extraction_yield(repo, since).await
        } else {
            vec![]
        };

        (
            tool_failures,
            correction_summaries,
            behavioral_metrics,
            graph_health,
            previous_suggestions,
            retrieval_precision_by_domain,
            extraction_yield_by_domain,
        )
    } else {
        (
            vec![],
            vec![],
            BehavioralMetrics::default(),
            GraphHealthMetrics::default(),
            vec![],
            vec![],
            vec![],
        )
    };

    debug!(
        sessions = sessions.len(),
        episodics = episodic_memories.len(),
        rules = rules.len(),
        skills = skill_files.len(),
        routing = routing_summaries.len(),
        meta_rules = pending_meta_rules.len(),
        tool_failures = tool_failures.len(),
        corrections = correction_summaries.len(),
        ?retrieval_precision,
        is_bootstrap,
        "Reforge collector: gathered inputs"
    );

    let feedback_pool = feedback_sources.and_then(|f| f.pool);

    let pending_medium_count =
        if let Some(Some(density_repo)) = feedback_sources.map(|f| f.density_repo) {
            density_repo
                .count_pending_by_tier("medium", since)
                .await
                .unwrap_or(0)
        } else {
            0
        };

    Some(ReforgeCollected {
        sessions,
        episodic_memories,
        user_model,
        rules,
        routing_summaries,
        pending_meta_rules,
        skill_files,
        retrieval_precision,
        is_bootstrap,
        autotuner_ctx: None,
        tool_failures,
        correction_summaries,
        retrieval_precision_by_domain,
        behavioral_metrics,
        graph_health,
        previous_suggestions,
        extraction_yield_by_domain,
        pending_enrichment_turns: pending_medium_count,
        graph_consolidation_needed: pending_medium_count > 5,
        runtime_signal_summary: if let Some(pool) = feedback_pool {
            Some(super::feedback::load_runtime_signals(pool, since).await)
        } else {
            None
        },
        validation_warning_counts: if let Some(pool) = feedback_pool {
            super::feedback::load_validation_warnings(pool, since).await
        } else {
            Vec::new()
        },
        near_miss_patterns: if let Some(pool) = feedback_pool {
            super::feedback::load_near_miss_count(pool).await
        } else {
            0
        },
        coaching_behavioral: if let Some(pool) = feedback_pool {
            super::feedback::load_coaching_behavioral(pool).await
        } else {
            None
        },
        distraction_rules_to_promote: 0,
    })
}

/// Load autotuner context for Phase 3 prompt and Phase 6 evaluation.
///
/// Called from the cron handler (which has access to the orchestrator and
/// metric source). The cognitive crate doesn't depend on autotuner, so the
/// caller must pass metric snapshots pre-collected from the autotuner layer.
pub async fn load_autotuner_context(
    trial_repo: &storage::TrialRepo,
    metrics_24h: super::types::MetricsSnapshot,
    metrics_7d: super::types::MetricsSnapshot,
    champion: &common::TrialParams,
) -> super::types::AutotunerContext {
    // Active trial count
    let active_trial_count = trial_repo.count_active().await.unwrap_or(0);

    // Recent experiment history (last 5 completed experiments)
    let trial_history = load_trial_history(trial_repo).await;

    // Format champion params
    let champion_summary = format_champion_params(champion);

    super::types::AutotunerContext {
        champion_summary,
        trial_history,
        metrics_24h,
        metrics_7d,
        active_trial_count,
    }
}

async fn load_trial_history(
    trial_repo: &storage::TrialRepo,
) -> Vec<super::types::TrialHistoryEntry> {
    use super::types::{TrialHistoryEntry, TrialOutcome};

    let experiments = match trial_repo.get_experiments(5).await {
        Ok(exps) => exps,
        Err(e) => {
            tracing::warn!("Reforge collector: failed to load experiments: {e}");
            return vec![];
        }
    };

    let completed_trials = match trial_repo.get_recent_completed(20).await {
        Ok(t) => t,
        Err(e) => {
            tracing::warn!("Reforge collector: failed to load completed trials: {e}");
            return vec![];
        }
    };

    let now = chrono::Utc::now();

    experiments
        .into_iter()
        .filter_map(|exp| {
            let days_ago = chrono::DateTime::parse_from_rfc3339(&exp.created_at)
                .ok()
                .map(|dt| (now - dt.with_timezone(&chrono::Utc)).num_days().max(0) as u32)
                .unwrap_or(0);

            let trials: Vec<TrialOutcome> = completed_trials
                .iter()
                .filter(|t| t.experiment_id == exp.id)
                .map(|t| {
                    let result_status = t.status.clone();
                    let params_summary = serde_json::from_str::<common::TrialParams>(&t.params)
                        .ok()
                        .map(|p| format_champion_params(&p))
                        .unwrap_or_else(|| "(parse error)".to_string());

                    // Parse constraint failures from result JSON
                    let (constraint_failures, improvement) = t
                        .result
                        .as_deref()
                        .and_then(|r| serde_json::from_str::<serde_json::Value>(r).ok())
                        .map(|v| {
                            let failures = v
                                .get("constraint_failures")
                                .and_then(|f| f.as_array())
                                .map(|arr| {
                                    arr.iter()
                                        .filter_map(|s| s.as_str().map(String::from))
                                        .collect()
                                })
                                .unwrap_or_default();
                            let imp = v.get("correction_rate").and_then(|c| c.as_f64());
                            (failures, imp)
                        })
                        .unwrap_or_default();

                    TrialOutcome {
                        params_summary,
                        result: result_status,
                        constraint_failures,
                        improvement,
                    }
                })
                .collect();

            if trials.is_empty() {
                return None;
            }

            Some(TrialHistoryEntry {
                experiment_id: exp.id,
                days_ago,
                trials,
            })
        })
        .collect()
}

/// Format champion params as a grouped string for the LLM prompt.
pub fn format_champion_params(params: &common::TrialParams) -> String {
    use std::fmt::Write;
    let mut out = String::new();

    // Helper macro to format optional params
    macro_rules! fmt_opt {
        ($out:expr, $name:literal, $val:expr) => {
            if let Some(v) = $val {
                write!($out, "{}={:.2}, ", $name, *v as f64).unwrap();
            }
        };
    }

    out.push_str("routing: ");
    fmt_opt!(out, "skill_keyword_weight", &params.skill_keyword_weight);
    fmt_opt!(out, "skill_semantic_weight", &params.skill_semantic_weight);
    fmt_opt!(
        out,
        "activation_threshold",
        &params.skill_activation_threshold
    );
    fmt_opt!(
        out,
        "heuristic_confidence",
        &params.heuristic_confidence_threshold
    );

    out.push_str("\nretrieval: ");
    fmt_opt!(out, "semantic", &params.relevance_weight_semantic);
    fmt_opt!(
        out,
        "retrievability",
        &params.relevance_weight_retrievability
    );
    fmt_opt!(out, "situation", &params.relevance_weight_situation);
    fmt_opt!(out, "importance", &params.relevance_weight_importance);
    fmt_opt!(out, "frequency", &params.relevance_weight_frequency);
    fmt_opt!(out, "temporal", &params.relevance_weight_temporal);
    fmt_opt!(out, "hierarchy", &params.relevance_weight_hierarchy);
    fmt_opt!(
        out,
        "path_coherence",
        &params.relevance_weight_path_coherence
    );
    fmt_opt!(out, "community", &params.relevance_weight_community);
    fmt_opt!(out, "cross_note", &params.relevance_weight_cross_note);

    out.push_str("\nmemory: ");
    fmt_opt!(out, "vector_top_k", &params.vector_top_k.map(|v| v as f64));
    fmt_opt!(out, "min_similarity", &params.min_similarity);
    fmt_opt!(
        out,
        "fsrs_desired_retention",
        &params.fsrs_desired_retention
    );

    out.push_str("\nrewriting: ");
    fmt_opt!(
        out,
        "rewrite_confidence",
        &params.rewrite_confidence_threshold
    );

    out
}

/// Aggregate routing snapshots into per-skill summaries.
///
/// For each skill present in the snapshots, sums message counts and computes
/// a weighted average of confidence values (weighted by message count).
fn aggregate_routing_snapshots(
    snapshots: &[crate::mirror::RoutingSnapshot],
) -> Vec<RoutingSummary> {
    use std::collections::HashMap;

    struct Accum {
        message_count: u32,
        confidence_sum: f64,
        total_messages_across_snapshots: u32,
        fallback_count: u32,
        snapshot_count: u32,
    }

    let mut map: HashMap<String, Accum> = HashMap::new();

    for snap in snapshots {
        for (skill, stats) in &snap.distribution {
            let entry = map.entry(skill.clone()).or_insert_with(|| Accum {
                message_count: 0,
                confidence_sum: 0.0,
                total_messages_across_snapshots: 0,
                fallback_count: 0,
                snapshot_count: 0,
            });
            entry.message_count += stats.count;
            // Weight confidence by message count for a meaningful average.
            entry.confidence_sum += stats.avg_confidence * stats.count as f64;
            entry.total_messages_across_snapshots += snap.total_messages;
            entry.fallback_count +=
                (snap.fallback_rate * snap.total_messages as f64).round() as u32;
            entry.snapshot_count += 1;
        }
    }

    map.into_iter()
        .map(|(skill, acc)| {
            let avg_confidence = if acc.message_count > 0 {
                acc.confidence_sum / acc.message_count as f64
            } else {
                0.0
            };
            let fallback_rate = if acc.total_messages_across_snapshots > 0 {
                acc.fallback_count as f64
                    / (acc.total_messages_across_snapshots as f64 / acc.snapshot_count as f64)
            } else {
                0.0
            };
            RoutingSummary {
                skill_name: skill,
                message_count: acc.message_count,
                avg_confidence,
                fallback_rate,
            }
        })
        .collect()
}
