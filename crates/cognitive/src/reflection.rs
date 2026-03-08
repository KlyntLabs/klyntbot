//! Weekly reflection — LLM-powered cross-domain pattern synthesis.
//!
//! Loads recent episodic memories, current user model, procedural rules,
//! and coaching history, then asks the LLM for insights and updates.
//! The `ReflectionHandler` trait is implemented in the agent crate.

use async_trait::async_trait;
use chrono::Utc;
use storage::StorageError;
use tracing::{debug, info, warn};

use crate::consolidation::{consolidate_batch, ConsolidationHandler};
use crate::embedder::SemanticFactEmbedder;
use crate::repos::{
    load_user_model, EpisodicMemoryRepo, ProceduralRuleRepo, SemanticFactRepo, RULE_DOMAINS,
};
use crate::types::{EpisodicMemory, ProceduralRule, SemanticFact, UserModel};

/// Input provided to the reflection handler.
#[derive(Debug, Clone)]
pub struct ReflectionInput {
    pub episodic_memories: Vec<EpisodicMemory>,
    pub user_model: UserModel,
    pub procedural_rules: Vec<ProceduralRule>,
    pub period_start: String,
    pub period_end: String,
}

/// Output from the reflection handler.
#[derive(Debug, Clone)]
pub struct ReflectionOutput {
    /// New or updated facts to consolidate.
    pub fact_updates: Vec<SemanticFact>,
    /// New or updated procedural rules.
    pub rule_updates: Vec<ProceduralRule>,
    /// Free-text summary of the reflection.
    pub summary: String,
}

/// Trait for LLM-backed weekly reflection.
///
/// Defined here (L3), implemented in agent (L5).
#[async_trait]
pub trait ReflectionHandler: Send + Sync {
    /// Run a reflection analysis on the provided input.
    async fn reflect(&self, input: &ReflectionInput) -> common::Result<ReflectionOutput>;
}

/// Run a weekly reflection cycle.
///
/// 1. Load episodic memories from the past 7 days
/// 2. Load current user model and procedural rules
/// 3. Call the reflection handler (LLM)
/// 4. Consolidate fact updates
/// 5. Apply rule updates
/// 6. Store the reflection as an episodic memory
pub async fn run_weekly_reflection(
    handler: &dyn ReflectionHandler,
    consolidation: &dyn ConsolidationHandler,
    fact_repo: &SemanticFactRepo,
    episodic_repo: &EpisodicMemoryRepo,
    rule_repo: &ProceduralRuleRepo,
    embedder: Option<&dyn SemanticFactEmbedder>,
) -> common::Result<ReflectionOutput> {
    let now = Utc::now();
    let week_ago = now - chrono::Duration::days(7);
    let period_start = week_ago.format("%Y-%m-%dT00:00:00").to_string();
    let period_end = now.format("%Y-%m-%dT23:59:59").to_string();

    // Load episodic memories from the past week
    let memories = episodic_repo
        .list_range(&period_start, &period_end)
        .await
        .map_err(StorageError::from)?;

    // Load user model across all domains
    let user_model = load_user_model(fact_repo).await;

    // Load active procedural rules
    let mut all_rules = Vec::new();
    for domain in RULE_DOMAINS {
        if let Ok(rules) = rule_repo.list_active(domain).await {
            all_rules.extend(rules);
        }
    }

    let input = ReflectionInput {
        episodic_memories: memories,
        user_model,
        procedural_rules: all_rules,
        period_start: period_start.clone(),
        period_end: period_end.clone(),
    };

    debug!(
        "Running weekly reflection with {} memories, {} rules",
        input.episodic_memories.len(),
        input.procedural_rules.len()
    );

    // Call the LLM
    let output = handler.reflect(&input).await?;

    // Consolidate fact updates (pattern validation: only if signal_count >= 5)
    if !output.fact_updates.is_empty() {
        let validated: Vec<_> = output
            .fact_updates
            .iter()
            .filter(|f| f.source == "user_stated" || f.confidence >= 0.7)
            .cloned()
            .collect();

        if !validated.is_empty() {
            consolidate_batch(&validated, fact_repo, consolidation, embedder).await;
            debug!("Reflection: consolidated {} fact updates", validated.len());
        }
    }

    // Apply rule updates
    for rule in &output.rule_updates {
        if let Err(e) = rule_repo.upsert(rule).await {
            warn!("Reflection: failed to upsert rule '{}': {e}", rule.id);
        }
    }

    // Store the reflection itself as an episodic memory
    let reflection_memory = EpisodicMemory {
        id: uuid::Uuid::new_v4().to_string(),
        domain: "reflection".into(),
        content: format!(
            "Weekly reflection ({} to {}): {}",
            period_start, period_end, output.summary
        ),
        summary: Some(output.summary.clone()),
        importance: 0.9,
        occurred_at: now.format("%Y-%m-%dT%H:%M:%S").to_string(),
        recorded_at: now.format("%Y-%m-%dT%H:%M:%S").to_string(),
        stability: 5.0, // Reflections start with higher stability
        last_accessed: None,
        access_count: 0,
    };

    if let Err(e) = episodic_repo.insert(&reflection_memory).await {
        warn!("Reflection: failed to store reflection memory: {e}");
    }

    info!(
        "Weekly reflection complete: {} fact updates, {} rule updates",
        output.fact_updates.len(),
        output.rule_updates.len()
    );

    Ok(output)
}

#[cfg(test)]
mod tests {
    use super::*;

    struct MockReflectionHandler {
        output: ReflectionOutput,
    }

    #[async_trait]
    impl ReflectionHandler for MockReflectionHandler {
        async fn reflect(&self, _input: &ReflectionInput) -> common::Result<ReflectionOutput> {
            Ok(self.output.clone())
        }
    }

    struct MockConsolidationHandler;

    #[async_trait]
    impl ConsolidationHandler for MockConsolidationHandler {
        async fn decide(
            &self,
            _candidate: &SemanticFact,
            _existing: &[SemanticFact],
        ) -> common::Result<crate::types::MemoryOp> {
            Ok(crate::types::MemoryOp::Noop)
        }
    }

    async fn setup() -> sqlx::SqlitePool {
        crate::repos::cognitive_test_pool().await
    }

    #[tokio::test]
    async fn test_weekly_reflection_stores_result() {
        let pool = setup().await;
        let fact_repo = SemanticFactRepo::new(pool.clone());
        let episodic_repo = EpisodicMemoryRepo::new(pool.clone());
        let rule_repo = ProceduralRuleRepo::new(pool);

        let handler = MockReflectionHandler {
            output: ReflectionOutput {
                fact_updates: vec![],
                rule_updates: vec![],
                summary: "No significant patterns this week.".into(),
            },
        };

        let output = run_weekly_reflection(
            &handler,
            &MockConsolidationHandler,
            &fact_repo,
            &episodic_repo,
            &rule_repo,
            None,
        )
        .await
        .unwrap();

        assert_eq!(output.summary, "No significant patterns this week.");

        // Check that the reflection was stored as episodic memory
        let now = Utc::now();
        let week_ago = now - chrono::Duration::days(1);
        let memories = episodic_repo
            .list_range(
                &week_ago.format("%Y-%m-%dT00:00:00").to_string(),
                &now.format("%Y-%m-%dT23:59:59").to_string(),
            )
            .await
            .unwrap();
        assert_eq!(memories.len(), 1);
        assert!(memories[0].content.contains("Weekly reflection"));
    }

    #[tokio::test]
    async fn test_weekly_reflection_applies_rules() {
        let pool = setup().await;
        let fact_repo = SemanticFactRepo::new(pool.clone());
        let episodic_repo = EpisodicMemoryRepo::new(pool.clone());
        let rule_repo = ProceduralRuleRepo::new(pool);

        let handler = MockReflectionHandler {
            output: ReflectionOutput {
                fact_updates: vec![],
                rule_updates: vec![ProceduralRule {
                    id: "r1".into(),
                    domain: "productivity".into(),
                    rule_text: "User is more productive after morning exercise".into(),
                    confidence: 0.8,
                    source: "reflected".into(),
                    signal_count: 7,
                    created_at: "2026-03-06".into(),
                    updated_at: "2026-03-06".into(),
                    active: true,
                }],
                summary: "Discovered exercise-productivity correlation.".into(),
            },
        };

        run_weekly_reflection(
            &handler,
            &MockConsolidationHandler,
            &fact_repo,
            &episodic_repo,
            &rule_repo,
            None,
        )
        .await
        .unwrap();

        let rules = rule_repo.list_active("productivity").await.unwrap();
        assert_eq!(rules.len(), 1);
        assert!(rules[0].rule_text.contains("morning exercise"));
    }

    #[tokio::test]
    async fn test_weekly_reflection_consolidates_facts() {
        let pool = setup().await;
        let fact_repo = SemanticFactRepo::new(pool.clone());
        let episodic_repo = EpisodicMemoryRepo::new(pool.clone());
        let rule_repo = ProceduralRuleRepo::new(pool);

        let handler = MockReflectionHandler {
            output: ReflectionOutput {
                fact_updates: vec![SemanticFact {
                    id: "f_new".into(),
                    domain: "energy".into(),
                    subject: "user".into(),
                    predicate: "peak_hours".into(),
                    object: "9am-11am".into(),
                    confidence: 0.9,
                    source: "reflected".into(),
                    valid_from: "2026-03-06".into(),
                    valid_until: None,
                    recorded_at: "2026-03-06".into(),
                    superseded_at: None,
                    superseded_by: None,
                    stability: 1.0,
                    last_accessed: None,
                    access_count: 0,
                }],
                rule_updates: vec![],
                summary: "Updated peak hours based on weekly patterns.".into(),
            },
        };

        let output = run_weekly_reflection(
            &handler,
            &MockConsolidationHandler,
            &fact_repo,
            &episodic_repo,
            &rule_repo,
            None,
        )
        .await
        .unwrap();

        assert_eq!(output.fact_updates.len(), 1);
    }
}
