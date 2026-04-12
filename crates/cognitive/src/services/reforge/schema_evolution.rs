//! Phase 2.5: Schema Evolution — proposes schema changes based on Mirror observations.

use async_trait::async_trait;
use serde::{Deserialize, Serialize};

/// Input for schema evolution LLM call.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SchemaEvolutionInput {
    pub database_id: String,
    pub database_name: String,
    /// Current field definitions (name, slug, type, usage_count).
    pub fields: Vec<FieldUsageSummary>,
    /// Skill content for domain context.
    pub skill_content: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FieldUsageSummary {
    pub field_id: String,
    pub field_slug: String,
    pub field_name: String,
    pub field_type: String,
    pub usage_count: i64,
    pub last_used: Option<String>,
    pub days_since_last_use: Option<i64>,
}

/// Output from schema evolution LLM call.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct SchemaEvolutionOutput {
    pub proposals: Vec<SchemaProposal>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SchemaProposal {
    pub action_type: String, // "add_field", "remove_field", "modify_field", "hide_field"
    pub action_json: String, // JSON details (field name, type, etc.)
    pub confidence: f64,
    pub reasoning: String,
}

/// Handler trait for schema evolution — dependency inversion.
/// Implemented in the agent crate.
#[async_trait]
pub trait SchemaEvolutionHandler: Send + Sync {
    /// Given field usage data for a database, propose schema changes.
    async fn propose_schema_evolution(
        &self,
        input: &SchemaEvolutionInput,
    ) -> common::Result<SchemaEvolutionOutput>;
}

/// Collector: read mirror_schema_observations and build per-database usage summaries.
pub async fn collect_schema_observations(
    pool: &sqlx::SqlitePool,
) -> Vec<(String, Vec<FieldUsageSummary>)> {
    let rows: Vec<(String, String, String, i64, String)> = sqlx::query_as(
        "SELECT database_id, field_id, usage_type, count, last_used_at
         FROM mirror_schema_observations
         ORDER BY database_id, field_id",
    )
    .fetch_all(pool)
    .await
    .unwrap_or_default();

    let mut by_db: std::collections::HashMap<String, Vec<FieldUsageSummary>> =
        std::collections::HashMap::new();

    for (db_id, field_id, _usage_type, count, last_used) in rows {
        let entry = by_db.entry(db_id).or_default();
        if let Some(existing) = entry.iter_mut().find(|f| f.field_id == field_id) {
            existing.usage_count += count;
        } else {
            entry.push(FieldUsageSummary {
                field_id: field_id.clone(),
                field_slug: field_id.clone(), // Will be enriched by caller
                field_name: field_id,
                field_type: String::new(),
                usage_count: count,
                last_used: Some(last_used),
                days_since_last_use: None,
            });
        }
    }

    by_db.into_iter().collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn schema_proposal_serde_roundtrip() {
        let proposal = SchemaProposal {
            action_type: "add_field".into(),
            action_json: r#"{"name":"Source","type":"select"}"#.into(),
            confidence: 0.72,
            reasoning: "User frequently mentions source in conversations".into(),
        };
        let json = serde_json::to_string(&proposal).unwrap();
        let parsed: SchemaProposal = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.action_type, "add_field");
        assert!((parsed.confidence - 0.72).abs() < f64::EPSILON);
    }

    #[tokio::test]
    async fn collect_empty_returns_empty() {
        let pool = storage::StoragePool::connect_in_memory().await.unwrap();
        storage::StoragePool::run_feature_migrations(
            pool.inner(),
            &crate::repos::cognitive_migrations(),
        )
        .await
        .unwrap();
        let result = collect_schema_observations(pool.inner()).await;
        assert!(result.is_empty());
    }
}
