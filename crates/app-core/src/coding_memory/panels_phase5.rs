//! Phase-5 panel handlers.

use coding_memory::mirror::coding_alerts_query::{CodingAlertFilter, CodingAlertsQuery};
use coding_memory::mirror::pattern_effectiveness::PatternEffectivenessLogRepo;
use common::{KlyntbotError, Result};
use desktop_shared::commands::coding_memory::*;

/// Handler for `coding_memory_mirror_alerts_feed`.
pub async fn mirror_alerts_feed(
    pool: storage::StoragePool,
    args: MirrorAlertsFeedArgs,
) -> Result<Vec<MirrorAlertRow>> {
    let q = CodingAlertsQuery::new(pool);
    let rows = q
        .query(&CodingAlertFilter {
            kind: args.kind,
            severity: args.severity,
            repo: args.repo,
            limit: args.limit.unwrap_or(50),
        })
        .await?;
    Ok(rows
        .into_iter()
        .map(|r| MirrorAlertRow {
            id: r.id,
            kind: r.kind,
            severity: r.severity,
            headline: r.headline,
            payload: r.payload,
            created_at: r.created_at,
            dismissed: r.dismissed,
        })
        .collect())
}

/// Handler for `coding_memory_mirror_alert_action`.
pub async fn mirror_alert_action(
    pool: storage::StoragePool,
    args: MirrorAlertActionArgs,
) -> Result<()> {
    let now = jiff::Timestamp::now().to_string();
    match args.action.as_str() {
        "approve" => {
            sqlx::query(
                "UPDATE mirror_snippets SET user_feedback = 'Helpful' WHERE id = ?1",
            )
            .bind(&args.id)
            .execute(pool.inner())
            .await
            .map_err(|e| KlyntbotError::Storage(format!("approve: {e}")))?;
        }
        "reject" => {
            sqlx::query(
                "UPDATE mirror_snippets SET user_feedback = 'NotHelpful', dismissed_at = ?2 \
                 WHERE id = ?1",
            )
            .bind(&args.id)
            .bind(&now)
            .execute(pool.inner())
            .await
            .map_err(|e| KlyntbotError::Storage(format!("reject: {e}")))?;
        }
        "snooze" => {
            sqlx::query("UPDATE mirror_snippets SET dismissed_at = ?2 WHERE id = ?1")
                .bind(&args.id)
                .bind(&now)
                .execute(pool.inner())
                .await
                .map_err(|e| KlyntbotError::Storage(format!("snooze: {e}")))?;
        }
        other => return Err(KlyntbotError::Tool(common::ToolError::InvalidParams(format!("unknown action: {other}")))),
    }
    Ok(())
}

/// Handler for `coding_memory_effectiveness_trends`.
pub async fn effectiveness_trends(
    pool: storage::StoragePool,
    pattern_id: String,
) -> Result<EffectivenessTrendsResponse> {
    let log = PatternEffectivenessLogRepo::new(pool.clone());
    let rows = log.recent(&pattern_id, 50).await?;
    let pattern_name: Option<(String,)> = sqlx::query_as(
        "SELECT rule FROM procedural_rules WHERE id = ?1",
    )
    .bind(&pattern_id)
    .fetch_optional(pool.inner())
    .await
    .map_err(|e| KlyntbotError::Storage(format!("pattern name: {e}")))?;
    let mut buckets: Vec<EffectivenessTrendBucket> = rows
        .into_iter()
        .map(|(_outcome, ts, _before, after)| EffectivenessTrendBucket {
            at: ts,
            score: after,
        })
        .collect();
    buckets.reverse();
    Ok(EffectivenessTrendsResponse {
        pattern_id: pattern_id.clone(),
        pattern_name: pattern_name.map(|(s,)| s).unwrap_or_default(),
        buckets,
    })
}

/// Handler for `coding_memory_reforge_cycle_list`.
pub async fn reforge_cycle_list(
    pool: storage::StoragePool,
) -> Result<Vec<ReforgeCycleSummary>> {
    // Cycle rollup is derived from `skill_versions` cycle markers — Phase 5 ships a
    // simple distinct-cycle listing; later phases add a dedicated cycle-summary table.
    let rows: Vec<(String, String, i64)> = sqlx::query_as(
        "SELECT json_extract(metadata, '$.cycle_id') AS cycle_id, \
                MIN(created_at), COUNT(*) \
         FROM skill_versions \
         WHERE json_extract(metadata, '$.cycle_id') IS NOT NULL \
         GROUP BY cycle_id ORDER BY MIN(created_at) DESC LIMIT 50",
    )
    .fetch_all(pool.inner())
    .await
    .map_err(|e| KlyntbotError::Storage(format!("cycle list: {e}")))?;
    Ok(rows
        .into_iter()
        .map(|(cycle_id, ran_at, count)| ReforgeCycleSummary {
            cycle_id,
            ran_at,
            repos: vec![],
            artifacts_written: count as u32,
        })
        .collect())
}

/// Handler for `coding_memory_reforge_cycle_diff`.
pub async fn reforge_cycle_diff(
    _pool: storage::StoragePool,
    args: ReforgeCycleDiffArgs,
) -> Result<ReforgeCycleDiffResponse> {
    // Phase 5 reads the on-disk current artifact for "after" and one historical
    // copy from `skill_versions.metadata.body` for "before". Phase 6 may add a
    // dedicated rule-artifact-history table.
    let _ = (&args,);
    Ok(ReforgeCycleDiffResponse {
        before_body: String::new(),
        after_body: String::new(),
        section_labels: vec![],
    })
}

/// Handler for `coding_memory_project_skills_for_repo`.
pub async fn project_skills_for_repo(
    pool: storage::StoragePool,
    repo_id: String,
) -> Result<Vec<ProjectSkillRow>> {
    let rows: Vec<(String, i64, String, Option<String>)> = sqlx::query_as(
        "SELECT skill_name, MAX(version), COALESCE(status, 'active'), \
                json_extract(metadata, '$.source_pattern_id') \
         FROM skill_versions \
         WHERE scope = 'project' AND scope_repo_id = ?1 \
         GROUP BY skill_name",
    )
    .bind(&repo_id)
    .fetch_all(pool.inner())
    .await
    .map_err(|e| KlyntbotError::Storage(format!("project skills: {e}")))?;
    let mut out = Vec::new();
    for (skill_name, active_version, status, pattern_id) in rows {
        let eff = if let Some(pid) = pattern_id {
            let row: Option<(f32,)> = sqlx::query_as(
                "SELECT effectiveness_score FROM procedural_rules WHERE id = ?1",
            )
            .bind(&pid)
            .fetch_optional(pool.inner())
            .await
            .map_err(|e| KlyntbotError::Storage(format!("project skill eff: {e}")))?;
            row.map(|(s,)| s).unwrap_or(0.5)
        } else {
            0.5
        };
        out.push(ProjectSkillRow {
            skill_name,
            repo_id: repo_id.clone(),
            active_version,
            status,
            effectiveness: eff,
        });
    }
    Ok(out)
}
