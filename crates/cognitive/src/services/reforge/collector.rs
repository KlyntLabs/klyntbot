//! Phase 1 of the Reforge cycle: gather all inputs needed for nightly synthesis.

use std::collections::HashMap;

use chrono::{Duration, Utc};
use tracing::debug;

use crate::repos::{
    load_user_model, EpisodicMemoryRepo, ProceduralRuleRepo, SemanticFactRepo, RULE_DOMAINS,
};
use crate::services::reforge::skill_files::{SkillFile, SkillFileManager};
use crate::services::reforge::types::{ReforgeCollected, RoutingSummary, SessionContext};

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
            updated_at: r.updated_at,
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
        let days = match last_run_at {
            Some(ts) => {
                // Calculate days since last run.
                chrono::DateTime::parse_from_rfc3339(ts)
                    .map(|dt| (Utc::now() - dt.with_timezone(&Utc)).num_days().max(1))
                    .unwrap_or(7)
            }
            None => 7,
        };
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

    debug!(
        sessions = sessions.len(),
        episodics = episodic_memories.len(),
        rules = rules.len(),
        skills = skill_files.len(),
        routing = routing_summaries.len(),
        meta_rules = pending_meta_rules.len(),
        ?retrieval_precision,
        is_bootstrap,
        "Reforge collector: gathered inputs"
    );

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
    })
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
