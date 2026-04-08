//! Phase 1 of the Reforge cycle: gather all inputs needed for nightly synthesis.

use chrono::{Duration, Utc};
use tracing::debug;

use crate::repos::{
    load_user_model, EpisodicMemoryRepo, ProceduralRuleRepo, SemanticFactRepo, RULE_DOMAINS,
};
use crate::services::reforge::skill_files::SkillFileManager;
use crate::services::reforge::types::{ReforgeCollected, SessionContext};

/// Scan skill files on disk and record a new version row for any file whose
/// content differs from the latest known version in the database.
///
/// This detects manual edits the user made between Reforge cycles so the
/// nightly synthesizer always works from the current on-disk state.
pub async fn detect_user_edits(
    skill_mgr: &SkillFileManager,
    version_repo: &storage::repos::SkillVersionRepo,
) {
    let all_files = skill_mgr.read_all();
    for (skill_name, files) in &all_files {
        for file in files {
            let latest = version_repo
                .latest_version(skill_name, &file.file_path)
                .await
                .ok()
                .flatten();
            if let Some(latest) = latest {
                let known_hash =
                    super::skill_files::content_hash(&latest.content);
                if known_hash != file.content_hash {
                    let diff =
                        super::skill_files::compute_diff(&latest.content, &file.content);
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
                    let _ = version_repo.insert(&row).await;
                    tracing::debug!(
                        "Detected user edit to {}/{}",
                        skill_name,
                        file.file_path
                    );
                }
            }
        }
    }
}

/// Collect all inputs required for a Reforge cycle.
///
/// - `last_run_at` — RFC 3339 timestamp of the previous run, or `None` for a
///   first-run bootstrap (uses a 7-day look-back window instead).
///
/// Returns `None` when there is nothing new to process (no sessions and no
/// episodic memories since the last run and we are not in bootstrap mode).
pub async fn collect(
    last_run_at: Option<&str>,
    session_memory_repo: &storage::SessionMemoryRepo,
    fact_repo: &SemanticFactRepo,
    episodic_repo: &EpisodicMemoryRepo,
    rule_repo: &ProceduralRuleRepo,
    skill_mgr: &SkillFileManager,
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
    let skill_files = skill_mgr.read_all();

    debug!(
        sessions = sessions.len(),
        episodics = episodic_memories.len(),
        rules = rules.len(),
        skills = skill_files.len(),
        is_bootstrap,
        "Reforge collector: gathered inputs"
    );

    Some(ReforgeCollected {
        sessions,
        episodic_memories,
        user_model,
        rules,
        routing_summaries: vec![],
        pending_meta_rules: vec![],
        skill_files,
        retrieval_precision: None,
        is_bootstrap,
    })
}
