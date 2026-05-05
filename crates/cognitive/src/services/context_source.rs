//! `CognitiveContextSource` — cognitive-memory-backed context source.
//!
//! Loads the structured `UserModel`, active procedural rules, and formats
//! them as static context for the LLM prompt.

use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::Arc;

use async_trait::async_trait;
use context_engine::source::{ContextSource, SourceContext};
use tokio::sync::Mutex;
use tracing::warn;

use crate::repos::{load_user_model, ProceduralRuleRepo, SemanticFactRepo, RULE_DOMAINS};
use crate::types::UserModel;

/// Cache TTL for user model data (seconds).
const CACHE_TTL_SECS: u64 = 60;

/// Returns a freshness label for a semantic fact based on convergence score,
/// confidence, and recency of last access.
fn freshness_label(fact: &crate::types::SemanticFact) -> &'static str {
    // A just-recorded fact has `last_accessed = None` because nothing has
    // queried it yet. Treat that as fresh by falling back to `recorded_at`
    // — otherwise every newly extracted fact would be labeled `"weak"` and
    // the LLM would refuse to use it. (Use `recorded_at` rather than
    // `last_accessed.unwrap_or(recorded_at)` so we cover both the new-fact
    // case and the never-accessed-but-old case symmetrically.)
    let reference_ts = fact
        .last_accessed
        .as_deref()
        .or(Some(fact.recorded_at.as_str()));
    let days_old = reference_ts
        .and_then(|ts| ts.parse::<jiff::Timestamp>().ok())
        .map(|ts| (jiff::Timestamp::now().as_millisecond() - ts.as_millisecond()) / 86_400_000)
        .unwrap_or(90);
    if fact.convergence_score >= 0.4 || (fact.confidence >= 0.8 && days_old <= 7) {
        "trusted"
    } else if fact.confidence >= 0.5 && days_old <= 30 {
        "noted"
    } else {
        // Was "weak -- verify" — but the LLM, seeing "verify" in the
        // label, would reflexively hedge ("you may want to verify this")
        // even when the fact was right in front of it. Use a neutral
        // descriptor; the LLM can still reason about confidence from
        // explicit numeric scores when needed.
        "low-confidence"
    }
}

struct CachedModel {
    model: UserModel,
    rules_text: String,
    cached_at: std::time::Instant,
}

/// Config subset for retrieval (avoids depending on full config crate).
#[derive(Debug, Clone)]
pub struct CognitiveRetrievalConfig {
    pub dynamic_facts_enabled: bool,
    pub static_fact_limit: usize,
    pub dynamic_fact_limit: usize,
    pub vector_top_k: usize,
    pub min_similarity: f64,
    pub max_stability: f64,
    pub relevance_weight_semantic: f64,
    pub relevance_weight_retrievability: f64,
    pub relevance_weight_importance: f64,
    pub relevance_weight_frequency: f64,
    pub relevance_weight_situation: f64,
    pub relevance_weight_temporal: f64,
    pub relevance_weight_hierarchy: f64,
    pub relevance_weight_path_coherence: f64,
    pub relevance_weight_community: f64,
    pub relevance_weight_cross_note: f64,
    pub relevance_weight_recall_support: f64,
    pub relevance_weight_graph_path_boost: f64,
}

impl Default for CognitiveRetrievalConfig {
    fn default() -> Self {
        Self {
            dynamic_facts_enabled: true,
            static_fact_limit: 10,
            dynamic_fact_limit: 15,
            vector_top_k: 30,
            min_similarity: 0.55,
            max_stability: 30.0,
            relevance_weight_semantic: 0.3,
            relevance_weight_retrievability: 0.2,
            relevance_weight_importance: 0.15,
            relevance_weight_frequency: 0.1,
            relevance_weight_situation: 0.25,
            relevance_weight_temporal: 0.05,
            relevance_weight_hierarchy: 0.10,
            relevance_weight_path_coherence: 0.05,
            relevance_weight_community: 0.15,
            relevance_weight_cross_note: 0.10,
            relevance_weight_recall_support: 0.08,
            relevance_weight_graph_path_boost: 0.06,
        }
    }
}

/// Context source that injects cognitive memory into the LLM prompt.
///
/// Priority 60 — appears after identity but before task context.
///
/// Static injection: Top facts by importance across all domains (identity baseline).
pub struct CognitiveContextSource {
    fact_repo: SemanticFactRepo,
    rule_repo: ProceduralRuleRepo,
    cache: Mutex<Option<CachedModel>>,
    static_fact_limit: usize,
    confidence_bits: Option<Arc<AtomicU32>>,
    recall_registry: Option<ai_core::RecallProviderRegistry>,
}

impl CognitiveContextSource {
    pub fn new(fact_repo: SemanticFactRepo, rule_repo: ProceduralRuleRepo) -> Self {
        Self {
            fact_repo,
            rule_repo,
            cache: Mutex::new(None),
            static_fact_limit: CognitiveRetrievalConfig::default().static_fact_limit,
            confidence_bits: None,
            recall_registry: None,
        }
    }

    pub fn with_static_fact_limit(mut self, limit: usize) -> Self {
        self.static_fact_limit = limit;
        self
    }

    pub fn with_confidence_threshold(mut self, bits: Arc<AtomicU32>) -> Self {
        self.confidence_bits = Some(bits);
        self
    }

    pub fn with_recall_registry(mut self, reg: ai_core::RecallProviderRegistry) -> Self {
        self.recall_registry = Some(reg);
        self
    }

    async fn load_rules_text(&self) -> String {
        let mut sections = Vec::new();

        for domain in RULE_DOMAINS {
            match self.rule_repo.list_active(domain).await {
                Ok(rules) if !rules.is_empty() => {
                    let rules_text: Vec<String> = rules
                        .iter()
                        .map(|r| {
                            format!(
                                "- [@rule:{}] {} (confidence: {:.0}%, signals: {})",
                                r.id,
                                r.rule_text,
                                r.confidence * 100.0,
                                r.signal_count
                            )
                        })
                        .collect();
                    sections.push(format!("### {}\n{}", domain, rules_text.join("\n")));
                }
                Ok(_) => {} // empty
                Err(e) => {
                    warn!("CognitiveContextSource: failed to load {domain} rules: {e}");
                }
            }
        }

        sections.join("\n\n")
    }

    async fn get_cached_or_load(&self) -> (UserModel, String) {
        // Check cache — release lock before any DB queries
        {
            let cache = self.cache.lock().await;
            if let Some(ref cached) = *cache {
                if cached.cached_at.elapsed().as_secs() < CACHE_TTL_SECS {
                    return (cached.model.clone(), cached.rules_text.clone());
                }
            }
        }

        // Load outside lock to avoid blocking concurrent callers
        let model = load_user_model(&self.fact_repo).await;
        let rules = self.load_rules_text().await;

        let mut cache = self.cache.lock().await;
        *cache = Some(CachedModel {
            model: model.clone(),
            rules_text: rules.clone(),
            cached_at: std::time::Instant::now(),
        });

        (model, rules)
    }
}

#[async_trait]
impl ContextSource for CognitiveContextSource {
    fn name(&self) -> &str {
        "cognitive"
    }

    fn priority(&self) -> u8 {
        60
    }

    async fn provide(&self, ctx: &SourceContext) -> Option<String> {
        let (model, rules_text) = self.get_cached_or_load().await;

        let mut sections = Vec::new();
        sections.push("# User Understanding".to_string());

        // ── Static tier: top facts by importance across all domains ──
        let domain_sections: Vec<(&str, &Vec<crate::types::SemanticFact>)> = vec![
            ("Identity", &model.identity),
            ("Energy & Rhythms", &model.energy),
            ("Work Patterns", &model.work),
            ("Finance", &model.finance),
            ("Learning", &model.learning),
            ("Preferences", &model.preferences),
            ("Other Context", &model.other),
        ];

        for (label, facts) in &domain_sections {
            if !facts.is_empty() {
                let mut domain_facts = facts.to_vec();
                domain_facts.sort_by(|a, b| {
                    let a_score = a.confidence * a.stability;
                    let b_score = b.confidence * b.stability;
                    b_score
                        .partial_cmp(&a_score)
                        .unwrap_or(std::cmp::Ordering::Equal)
                });
                domain_facts.truncate(self.static_fact_limit);
                let lines: Vec<String> = domain_facts
                    .iter()
                    .map(|f| {
                        format!(
                            "- {}: {} = {} [{}]",
                            f.subject,
                            f.predicate,
                            f.object,
                            freshness_label(f)
                        )
                    })
                    .collect();
                sections.push(format!("## {label}\n{}", lines.join("\n")));
            }
        }

        if !rules_text.is_empty() {
            sections.push(format!("## Learned Patterns\n{rules_text}"));
        }

        // ── Themes (Wave 8) — top-3 community summaries by member count ──
        if std::env::var("KCA_COMMUNITY_SUMMARIES").ok().as_deref() == Some("1") {
            let community_repo =
                crate::repos::community::CommunityRepo::new(self.fact_repo.pool().clone());
            if let Ok(mut communities) = community_repo.list_active_communities().await {
                communities.retain(|c| !c.summary.trim().is_empty());
                communities.sort_by(|a, b| b.member_count.cmp(&a.member_count));
                communities.truncate(3);
                if !communities.is_empty() {
                    let lines: Vec<String> = communities
                        .iter()
                        .map(|c| format!("- {}: {}", c.name, c.summary))
                        .collect();
                    sections.push(format!("## Themes\n{}", lines.join("\n")));
                }
            }
        }

        // ── Confidence calibration ──
        if let Some(ref bits) = self.confidence_bits {
            let threshold = f32::from_bits(bits.load(Ordering::Relaxed));
            sections.push(format!(
                "## Confidence Calibration\n\
                 Current confidence threshold: {threshold:.2}. \
                 When uncertain about user intent, ask for clarification rather than guessing.\n\n\
                 **Answer style for memory-grounded facts:** State retrieved facts \
                 directly. Do NOT prefix answers with disclaimers like \"weak confidence\", \
                 \"verify\", \"weakly-confirmed\", \"you may want to confirm\", or \
                 \"this is marked as low-confidence\" — fact metadata is for your own \
                 reasoning, not user-facing prose. If a fact's `[trusted]` tag is \
                 present, treat it as authoritative; if it's `[low-confidence]`, \
                 you may add a brief \"recently mentioned\" qualifier but never tell \
                 the user to verify.\n\n\
                 **CRITICAL — never claim ignorance about facts shown above.** \
                 If a fact like `Alice lives_in SF` appears under \"User Understanding\", \
                 you MUST use it to answer questions about Alice. Saying \"I don't have \
                 information about where Alice lives\" while that fact is in your context \
                 is a serious hallucination. Trust the User Understanding section as \
                 ground truth — it is your memory."
            ));
        }

        // ── Recall registry: ranked domain hints ──
        if let (Some(reg), Some(msg)) = (&self.recall_registry, ctx.message.as_deref()) {
            let query = ai_core::RecallQuery {
                message: msg.to_string(),
                intent_summary: ctx.intent_summary.clone(),
            };
            let ranked = reg.rank(&query);
            if !ranked.is_empty() {
                let lines: Vec<String> = ranked
                    .iter()
                    .map(|(d, s)| format!("- {} (score {:.2})", d.as_str(), s))
                    .collect();
                sections.push(format!("## Relevant Domains\n{}", lines.join("\n")));
            }
        }

        let output = sections.join("\n\n");

        // Only include if there's actual content
        if output.lines().count() <= 1 {
            None
        } else {
            Some(output)
        }
    }

    fn estimated_tokens(&self) -> usize {
        1000
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::{ProceduralRule, SemanticFact, DEFAULT_MEMORY_TYPE};

    async fn setup() -> sqlx::SqlitePool {
        crate::repos::cognitive_test_pool().await
    }

    fn test_fact(domain: &str, predicate: &str, object: &str) -> SemanticFact {
        SemanticFact {
            id: uuid::Uuid::new_v4().to_string(),
            domain: domain.into(),
            subject: "user".into(),
            predicate: predicate.into(),
            object: object.into(),
            confidence: 0.8,
            source: "observed".into(),
            valid_from: "2026-03-01".into(),
            valid_until: None,
            recorded_at: "2026-03-06T10:00:00".into(),
            superseded_at: None,
            superseded_by: None,
            stability: 1.0,
            last_accessed: None,
            access_count: 0,
            convergence_score: 0.0,
            project_id: None,
            memory_type: DEFAULT_MEMORY_TYPE.to_string(),
            scope_type: "system".to_string(),
            scope_id: None,
            scope_repo_id: None,
            metadata: None,
            speaker: None,
        }
    }

    fn test_rule(domain: &str, text: &str) -> ProceduralRule {
        ProceduralRule {
            id: uuid::Uuid::new_v4().to_string(),
            domain: domain.into(),
            rule_text: text.into(),
            confidence: 0.8,
            source: "reflected".into(),
            signal_count: 5,
            created_at: "2026-03-06".into(),
            updated_at: "2026-03-06".into(),
            active: true,
            project_id: None,
            scope_type: "system".to_string(),
            scope_id: None,
            effectiveness_score: 0.5,
            stability: 1.0,
            scope_repo_id: None,
            last_applied: None,
            application_count: 0,
            metadata: None,
        }
    }

    #[tokio::test]
    async fn test_context_source_returns_none_when_empty() {
        let pool = setup().await;
        let source = CognitiveContextSource::new(
            SemanticFactRepo::new(pool.clone()),
            ProceduralRuleRepo::new(pool),
        );

        let ctx = SourceContext {
            channel: "test".into(),
            chat_id: "c1".into(),
            message: None,
            intent_summary: None,
            project_id: None,
            session_mode: common::SessionMode::Assistant,
        };
        let result = source.provide(&ctx).await;
        assert!(result.is_none());
    }

    #[tokio::test]
    async fn test_context_source_includes_facts() {
        let pool = setup().await;
        let fact_repo = SemanticFactRepo::new(pool.clone());
        let rule_repo = ProceduralRuleRepo::new(pool);

        fact_repo
            .upsert(&test_fact("energy", "peak_hours", "10am-12pm"))
            .await
            .unwrap();
        fact_repo
            .upsert(&test_fact("preferences", "work_style", "deep focus"))
            .await
            .unwrap();

        let source = CognitiveContextSource::new(fact_repo, rule_repo);
        let ctx = SourceContext {
            channel: "test".into(),
            chat_id: "c1".into(),
            message: None,
            intent_summary: None,
            project_id: None,
            session_mode: common::SessionMode::Assistant,
        };

        let result = source.provide(&ctx).await.unwrap();
        assert!(result.contains("User Understanding"));
        assert!(result.contains("peak_hours"));
        assert!(result.contains("10am-12pm"));
        assert!(result.contains("work_style"));
    }

    #[tokio::test]
    async fn test_context_source_includes_rules() {
        let pool = setup().await;
        let fact_repo = SemanticFactRepo::new(pool.clone());
        let rule_repo = ProceduralRuleRepo::new(pool);

        rule_repo
            .upsert(&test_rule(
                "productivity",
                "User works best after morning exercise",
            ))
            .await
            .unwrap();

        let source = CognitiveContextSource::new(fact_repo, rule_repo);
        let ctx = SourceContext {
            channel: "test".into(),
            chat_id: "c1".into(),
            message: None,
            intent_summary: None,
            project_id: None,
            session_mode: common::SessionMode::Assistant,
        };

        let result = source.provide(&ctx).await.unwrap();
        assert!(result.contains("Learned Patterns"));
        assert!(result.contains("morning exercise"));
    }

    #[tokio::test]
    async fn test_priority_is_60() {
        let pool = setup().await;
        let source = CognitiveContextSource::new(
            SemanticFactRepo::new(pool.clone()),
            ProceduralRuleRepo::new(pool),
        );
        assert_eq!(source.priority(), 60);
    }

    #[tokio::test]
    async fn test_static_tier_without_message() {
        let pool = setup().await;
        let fact_repo = SemanticFactRepo::new(pool.clone());
        let rule_repo = ProceduralRuleRepo::new(pool);

        fact_repo
            .upsert(&test_fact("identity", "name", "Jayden"))
            .await
            .unwrap();

        let source = CognitiveContextSource::new(fact_repo, rule_repo);
        let ctx = SourceContext {
            channel: "test".into(),
            chat_id: "c1".into(),
            message: None,
            intent_summary: None,
            project_id: None,
            session_mode: common::SessionMode::Assistant,
        };

        let result = source.provide(&ctx).await.unwrap();
        assert!(result.contains("User Understanding"));
        assert!(result.contains("Jayden"));
        // Should NOT contain dynamic section (no message = no query)
        assert!(!result.contains("Relevant Personal Context"));
    }

    #[tokio::test]
    async fn test_static_only_with_message_present() {
        let pool = setup().await;
        let fact_repo = SemanticFactRepo::new(pool.clone());
        let rule_repo = ProceduralRuleRepo::new(pool);

        fact_repo
            .upsert(&test_fact("identity", "name", "Jayden"))
            .await
            .unwrap();

        let source = CognitiveContextSource::new(fact_repo, rule_repo);
        let ctx = SourceContext {
            channel: "test".into(),
            chat_id: "c1".into(),
            message: Some("what are my peak hours".into()),
            intent_summary: None,
            project_id: None,
            session_mode: common::SessionMode::Assistant,
        };

        let result = source.provide(&ctx).await.unwrap();
        assert!(result.contains("User Understanding"));
        assert!(result.contains("Jayden"));
        // Dynamic section should NOT appear (moved to UnifiedMemoryService)
        assert!(!result.contains("Relevant Personal Context"));
    }

    #[tokio::test]
    async fn context_source_uses_recall_registry() {
        let pool = setup().await;
        let registry = ai_core::RecallProviderRegistry::new()
            .with(feature_tasks::TasksFeature::default())
            .with(feature_finance::FinanceFeature::default());

        let fact_repo = SemanticFactRepo::new(pool.clone());
        let rule_repo = ProceduralRuleRepo::new(pool);
        let source =
            CognitiveContextSource::new(fact_repo, rule_repo).with_recall_registry(registry);

        let ctx = SourceContext {
            channel: "test".into(),
            chat_id: "c".into(),
            message: Some("when is my deadline".into()),
            intent_summary: None,
            project_id: None,
            session_mode: common::SessionMode::Assistant,
        };
        let out = source.provide(&ctx).await.unwrap();
        // Registry-ranked feature recommendations appear in the output
        assert!(out.contains("Relevant Domains") || out.contains("tasks"));
    }

    #[tokio::test]
    async fn test_static_facts_sorted_by_importance() {
        let pool = setup().await;
        let fact_repo = SemanticFactRepo::new(pool.clone());
        let rule_repo = ProceduralRuleRepo::new(pool);

        // Insert facts with different confidence × stability scores
        let mut high = test_fact("energy", "peak_hours", "10am-12pm");
        high.confidence = 0.95;
        high.stability = 5.0;
        fact_repo.upsert(&high).await.unwrap();

        let mut low = test_fact("energy", "caffeine_sensitivity", "high");
        low.confidence = 0.3;
        low.stability = 0.5;
        fact_repo.upsert(&low).await.unwrap();

        let source = CognitiveContextSource::new(fact_repo, rule_repo);
        let ctx = SourceContext {
            channel: "test".into(),
            chat_id: "c1".into(),
            message: None,
            intent_summary: None,
            project_id: None,
            session_mode: common::SessionMode::Assistant,
        };

        let result = source.provide(&ctx).await.unwrap();
        // Higher importance fact should appear first
        let peak_pos = result.find("peak_hours").unwrap();
        let caffeine_pos = result.find("caffeine_sensitivity").unwrap();
        assert!(peak_pos < caffeine_pos);
    }
}
