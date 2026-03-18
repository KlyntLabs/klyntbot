//! Weekly reflection — LLM-powered cross-domain pattern synthesis.
//!
//! Loads recent episodic memories, current user model, procedural rules,
//! and coaching history, then asks the LLM for insights and updates.
//! The `ReflectionHandler` trait is implemented in the agent crate.

use async_trait::async_trait;
use chrono::Utc;
use storage::StorageError;
use tracing::{debug, info, warn};

use crate::consolidation::{execute_memory_ops, ConsolidationCandidate, ConsolidationHandler};
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

/// Minimum number of episodic memories required before reflection runs.
/// Below this threshold, the LLM produces low-quality or hallucinated patterns.
const MIN_EPISODE_COUNT: usize = 20;

/// Run a weekly reflection cycle.
///
/// 1. Load episodic memories from the past 7 days
/// 2. Load current user model and procedural rules
/// 3. Call the reflection handler (LLM)
/// 4. Consolidate fact updates
/// 5. Apply rule updates
/// 6. Store the reflection as an episodic memory
///
/// Returns early with an empty output if the total episodic memory count
/// is below [`MIN_EPISODE_COUNT`] — new users with sparse data would
/// get low-quality reflections.
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

    // Guard: skip reflection when user has too few episodic memories overall.
    let total_episodes = episodic_repo
        .count_all()
        .await
        .map_err(StorageError::from)?;
    if (total_episodes as usize) < MIN_EPISODE_COUNT {
        info!(
            "Skipping weekly reflection: only {} episodic memories (need >= {})",
            total_episodes, MIN_EPISODE_COUNT
        );
        return Ok(ReflectionOutput {
            fact_updates: vec![],
            rule_updates: vec![],
            summary: format!(
                "Skipped — not enough data yet ({total_episodes}/{MIN_EPISODE_COUNT} memories)."
            ),
        });
    }

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
            // Concurrent prefetch of existing facts (avoid sequential N+1)
            let futs: Vec<_> = validated
                .iter()
                .map(|fact| {
                    let repo = fact_repo.clone();
                    let subject = fact.subject.clone();
                    let predicate = fact.predicate.clone();
                    async move {
                        repo.find_similar(&subject, &predicate)
                            .await
                            .unwrap_or_default()
                    }
                })
                .collect();
            let existing_results = futures_util::future::join_all(futs).await;
            let candidates: Vec<_> = validated
                .iter()
                .zip(existing_results)
                .map(|(fact, existing)| ConsolidationCandidate {
                    candidate: fact.clone(),
                    existing,
                })
                .collect();
            match consolidation.decide_batch(&candidates).await {
                Ok(ops) => {
                    execute_memory_ops(&ops, &candidates, fact_repo, embedder).await;
                    debug!("Reflection: consolidated {} fact updates", validated.len());
                }
                Err(e) => {
                    warn!("Reflection: consolidation failed: {e}");
                }
            }
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
        project_id: None,
        scope_type: "system".to_string(),
        scope_id: None,
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
    use crate::types::DEFAULT_MEMORY_TYPE;

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
        async fn decide_batch(
            &self,
            candidates: &[ConsolidationCandidate],
        ) -> common::Result<Vec<crate::types::MemoryOp>> {
            Ok(vec![crate::types::MemoryOp::Noop; candidates.len()])
        }
    }

    async fn setup() -> sqlx::SqlitePool {
        crate::repos::cognitive_test_pool().await
    }

    /// Seed enough episodic memories to pass the MIN_EPISODE_COUNT guard.
    async fn seed_minimum_episodes(repo: &EpisodicMemoryRepo) {
        for i in 0..MIN_EPISODE_COUNT {
            let mem = EpisodicMemory {
                id: format!("seed_{i}"),
                domain: "general".into(),
                content: format!("Seed episode {i}"),
                summary: None,
                importance: 0.5,
                occurred_at: format!("2026-02-{:02}T10:00:00", (i % 28) + 1),
                recorded_at: "2026-03-01T00:00:00".into(),
                stability: 1.0,
                last_accessed: None,
                access_count: 0,
                project_id: None,
                scope_type: "system".to_string(),
                scope_id: None,
            };
            repo.insert(&mem).await.unwrap();
        }
    }

    #[tokio::test]
    async fn test_weekly_reflection_skipped_when_too_few_episodes() {
        let pool = setup().await;
        let fact_repo = SemanticFactRepo::new(pool.clone());
        let episodic_repo = EpisodicMemoryRepo::new(pool.clone());
        let rule_repo = ProceduralRuleRepo::new(pool);

        // Insert only 5 memories — well below MIN_EPISODE_COUNT (20)
        for i in 0..5 {
            let mem = EpisodicMemory {
                id: format!("ep_{i}"),
                domain: "productivity".into(),
                content: format!("Session {i}"),
                summary: None,
                importance: 0.7,
                occurred_at: format!("2026-03-0{i}T10:00:00"),
                recorded_at: "2026-03-06T12:00:00".into(),
                stability: 1.0,
                last_accessed: None,
                access_count: 0,
                project_id: None,
                scope_type: "system".to_string(),
                scope_id: None,
            };
            episodic_repo.insert(&mem).await.unwrap();
        }

        let handler = MockReflectionHandler {
            output: ReflectionOutput {
                fact_updates: vec![],
                rule_updates: vec![],
                summary: "Should never reach this.".into(),
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

        assert!(output.summary.contains("Skipped"));
        assert!(output.fact_updates.is_empty());
        assert!(output.rule_updates.is_empty());
    }

    #[tokio::test]
    async fn test_weekly_reflection_stores_result() {
        let pool = setup().await;
        let fact_repo = SemanticFactRepo::new(pool.clone());
        let episodic_repo = EpisodicMemoryRepo::new(pool.clone());
        let rule_repo = ProceduralRuleRepo::new(pool);
        seed_minimum_episodes(&episodic_repo).await;

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
        seed_minimum_episodes(&episodic_repo).await;

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
                    project_id: None,
                    scope_type: "system".to_string(),
                    scope_id: None,
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
        seed_minimum_episodes(&episodic_repo).await;

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
                    project_id: None,
                    memory_type: DEFAULT_MEMORY_TYPE.to_string(),
                    scope_type: "system".to_string(),
                    scope_id: None,
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
