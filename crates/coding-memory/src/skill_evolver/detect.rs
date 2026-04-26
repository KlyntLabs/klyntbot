//! Detect workflow-pattern candidates eligible for promotion to project skills.

use common::{KlyntbotError, Result};
use storage::StoragePool;

/// One procedural rule that passed the detection filter.
#[derive(Debug, Clone)]
pub struct WorkflowPatternCandidate {
    /// Procedural rule id.
    pub id: String,
    /// Rule text (becomes skill name / procedure).
    pub rule_text: String,
    /// Confidence score (0.0 – 1.0).
    pub confidence: f32,
    /// Effectiveness score.
    pub effectiveness: f32,
}

/// Query the cognitive store for high-confidence `workflow_pattern` rules that
/// have not yet been expressed as a project skill.
pub async fn detect_candidates(
    pool: &StoragePool,
    repo_id: &str,
) -> Result<Vec<WorkflowPatternCandidate>> {
    let rows: Vec<(String, String, f64, f64)> = sqlx::query_as(
        "SELECT pr.id, pr.rule_text, pr.confidence, COALESCE(pr.effectiveness_score, 0.5)
         FROM procedural_rules pr
         WHERE pr.scope_repo_id = ?1
           AND pr.confidence >= 0.7
           AND json_extract(pr.metadata, '$.kind') = 'workflow_pattern'
           AND NOT EXISTS (
               SELECT 1 FROM skill_versions sv
               WHERE sv.scope = 'project'
                 AND sv.source_pattern_id = pr.id
           )
         ORDER BY pr.confidence DESC",
    )
    .bind(repo_id)
    .fetch_all(pool.inner())
    .await
    .map_err(|e| KlyntbotError::Storage(format!("detect_candidates: {e}")))?;

    Ok(rows
        .into_iter()
        .map(|(id, rule_text, confidence, effectiveness)| WorkflowPatternCandidate {
            id,
            rule_text,
            confidence: confidence as f32,
            effectiveness: effectiveness as f32,
        })
        .collect())
}
