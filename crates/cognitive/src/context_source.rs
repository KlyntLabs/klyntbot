//! Unified `CognitiveContextSource` — cognitive-memory-backed context source.
//!
//! Loads the structured `UserModel`, active procedural rules, and formats
//! them as context for the LLM prompt. Optionally injects dynamic
//! vector-searched facts relevant to the current query.

use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::Arc;

use async_trait::async_trait;
use context_engine::source::{ContextSource, SourceContext};
use tokio::sync::Mutex;
use tracing::warn;

use crate::embedder::SemanticFactEmbedder;
use crate::repos::{load_user_model, ProceduralRuleRepo, SemanticFactRepo, RULE_DOMAINS};
use crate::situation::UserSituation;
use crate::types::UserModel;

/// Cache TTL for user model data (seconds).
const CACHE_TTL_SECS: u64 = 60;

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
        }
    }
}

/// Context source that injects cognitive memory into the LLM prompt.
///
/// Priority 60 — appears after identity but before task context.
///
/// Two-tier injection:
/// - **Static tier:** Top facts by importance across all domains (identity baseline)
/// - **Dynamic tier:** Vector-searched facts relevant to the current query
pub struct CognitiveContextSource {
    fact_repo: SemanticFactRepo,
    rule_repo: ProceduralRuleRepo,
    embedder: Option<Arc<dyn SemanticFactEmbedder>>,
    cache: Mutex<Option<CachedModel>>,
    config: CognitiveRetrievalConfig,
    confidence_bits: Option<Arc<AtomicU32>>,
    situation: Option<Arc<Mutex<UserSituation>>>,
}

impl CognitiveContextSource {
    pub fn new(fact_repo: SemanticFactRepo, rule_repo: ProceduralRuleRepo) -> Self {
        Self {
            fact_repo,
            rule_repo,
            embedder: None,
            cache: Mutex::new(None),
            config: CognitiveRetrievalConfig::default(),
            confidence_bits: None,
            situation: None,
        }
    }

    pub fn with_embedder(mut self, embedder: Arc<dyn SemanticFactEmbedder>) -> Self {
        self.embedder = Some(embedder);
        self
    }

    pub fn with_embedder_opt(mut self, embedder: Option<Arc<dyn SemanticFactEmbedder>>) -> Self {
        self.embedder = embedder;
        self
    }

    pub fn with_config(mut self, config: CognitiveRetrievalConfig) -> Self {
        self.config = config;
        self
    }

    pub fn with_confidence_threshold(mut self, bits: Arc<AtomicU32>) -> Self {
        self.confidence_bits = Some(bits);
        self
    }

    pub fn with_situation(mut self, situation: Arc<Mutex<UserSituation>>) -> Self {
        self.situation = Some(situation);
        self
    }

    /// Derive a single 0.0–1.0 boost from the live `UserSituation`.
    ///
    /// Higher when the user is actively engaged (energetic, focused, under
    /// deadline pressure, low distraction) — causing more facts to clear the
    /// relevance threshold and appear in context.
    async fn current_situational_boost(&self) -> f64 {
        let Some(ref s) = self.situation else {
            return 0.0;
        };
        let guard = s.lock().await;
        (guard.energy_level * 0.25
            + guard.focus_state * 0.30
            + guard.deadline_pressure * 0.25
            + (1.0 - guard.distraction_risk) * 0.20)
            .clamp(0.0, 1.0)
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
                                "- {} (confidence: {:.0}%, signals: {})",
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
                domain_facts.truncate(self.config.static_fact_limit);
                let lines: Vec<String> = domain_facts
                    .iter()
                    .map(|f| format!("- {}: {} = {}", f.subject, f.predicate, f.object))
                    .collect();
                sections.push(format!("## {label}\n{}", lines.join("\n")));
            }
        }

        if !rules_text.is_empty() {
            sections.push(format!("## Learned Patterns\n{rules_text}"));
        }

        // ── Dynamic tier: vector-searched relevant facts ──
        let query = ctx
            .intent_summary
            .as_deref()
            .or(ctx.message.as_deref())
            .unwrap_or("");

        if self.config.dynamic_facts_enabled && !query.is_empty() {
            use crate::retrieval::retrieve_relevant_facts;

            use crate::repos::USER_MODEL_DOMAINS;
            let retrieval_domains = USER_MODEL_DOMAINS;

            let situational_boost = self.current_situational_boost().await;

            let retrieval_params = crate::retrieval::RetrievalParams {
                limit: self.config.dynamic_fact_limit,
                vector_top_k: self.config.vector_top_k,
                min_similarity: self.config.min_similarity,
                situational_boost,
                max_stability: self.config.max_stability,
                relevance_weight_semantic: self.config.relevance_weight_semantic,
                relevance_weight_retrievability: self.config.relevance_weight_retrievability,
                relevance_weight_importance: self.config.relevance_weight_importance,
                relevance_weight_frequency: self.config.relevance_weight_frequency,
                relevance_weight_situation: self.config.relevance_weight_situation,
            };
            let results = retrieve_relevant_facts(
                &self.fact_repo,
                self.embedder.as_deref(),
                query,
                retrieval_domains,
                &retrieval_params,
            )
            .await;

            if let Ok(facts) = results {
                let relevant_lines: Vec<String> = facts
                    .iter()
                    .filter(|f| f.score > 0.3) // minimum relevance threshold
                    .map(|f| {
                        if f.similarity.map(|s| s > 0.6).unwrap_or(false) {
                            format!(
                                "- {}: {} = {} (relevance: {:.2})",
                                f.fact.subject, f.fact.predicate, f.fact.object, f.score
                            )
                        } else {
                            format!(
                                "- {}: {} = {}",
                                f.fact.subject, f.fact.predicate, f.fact.object
                            )
                        }
                    })
                    .collect();

                if !relevant_lines.is_empty() {
                    sections.push(format!(
                        "## Relevant Personal Context (for this conversation)\n{}",
                        relevant_lines.join("\n")
                    ));
                }
            }
        }

        // ── Confidence calibration ──
        if let Some(ref bits) = self.confidence_bits {
            let threshold = f32::from_bits(bits.load(Ordering::Relaxed));
            sections.push(format!(
                "## Confidence Calibration\n\
                 Current confidence threshold: {threshold:.2}. \
                 When uncertain about user intent, ask for clarification rather than guessing."
            ));
        }

        let output = sections.join("\n\n");

        // Only include if there's actual content
        if output.lines().count() <= 1 {
            None
        } else {
            Some(output)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::{ProceduralRule, SemanticFact};

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
        };

        let result = source.provide(&ctx).await.unwrap();
        assert!(result.contains("User Understanding"));
        assert!(result.contains("Jayden"));
        // Should NOT contain dynamic section (no message = no query)
        assert!(!result.contains("Relevant Personal Context"));
    }

    #[tokio::test]
    async fn test_dynamic_tier_with_message_fallback() {
        let pool = setup().await;
        let fact_repo = SemanticFactRepo::new(pool.clone());
        let rule_repo = ProceduralRuleRepo::new(pool);

        // Insert facts across domains
        fact_repo
            .upsert(&test_fact("identity", "name", "Jayden"))
            .await
            .unwrap();
        fact_repo
            .upsert(&test_fact("energy", "peak_hours", "10am-12pm"))
            .await
            .unwrap();
        fact_repo
            .upsert(&test_fact("preferences", "editor", "neovim"))
            .await
            .unwrap();

        // No embedder — dynamic tier uses fallback path
        let source = CognitiveContextSource::new(fact_repo, rule_repo);
        let ctx = SourceContext {
            channel: "test".into(),
            chat_id: "c1".into(),
            message: Some("what are my peak hours".into()),
            intent_summary: None,
        };

        let result = source.provide(&ctx).await.unwrap();
        assert!(result.contains("User Understanding"));
        // Dynamic tier should appear (fallback path with query)
        assert!(result.contains("Relevant Personal Context"));
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
        };

        let result = source.provide(&ctx).await.unwrap();
        // Higher importance fact should appear first
        let peak_pos = result.find("peak_hours").unwrap();
        let caffeine_pos = result.find("caffeine_sensitivity").unwrap();
        assert!(peak_pos < caffeine_pos);
    }

    #[tokio::test]
    async fn test_situational_boost_from_active_situation() {
        let pool = setup().await;
        let source = CognitiveContextSource::new(
            SemanticFactRepo::new(pool.clone()),
            ProceduralRuleRepo::new(pool),
        );

        // No situation wired → boost is 0.0
        let boost = source.current_situational_boost().await;
        assert!((boost - 0.0).abs() < f64::EPSILON);

        // Wire a high-activity situation
        let sit = Arc::new(Mutex::new(UserSituation {
            energy_level: 0.8,
            focus_state: 0.9,
            deadline_pressure: 0.6,
            distraction_risk: 0.1,
            ..Default::default()
        }));
        let source = source.with_situation(sit);
        let boost = source.current_situational_boost().await;

        // 0.8*0.25 + 0.9*0.30 + 0.6*0.25 + 0.9*0.20 = 0.2+0.27+0.15+0.18 = 0.80
        assert!(boost > 0.7, "active situation should produce high boost: {boost}");
    }

    #[tokio::test]
    async fn test_situational_boost_low_when_depleted() {
        let pool = setup().await;
        let sit = Arc::new(Mutex::new(UserSituation {
            energy_level: 0.1,
            focus_state: 0.1,
            deadline_pressure: 0.0,
            distraction_risk: 0.8,
            ..Default::default()
        }));
        let source = CognitiveContextSource::new(
            SemanticFactRepo::new(pool.clone()),
            ProceduralRuleRepo::new(pool),
        )
        .with_situation(sit);

        let boost = source.current_situational_boost().await;
        // 0.1*0.25 + 0.1*0.30 + 0.0*0.25 + 0.2*0.20 = 0.025+0.03+0+0.04 = 0.095
        assert!(boost < 0.15, "depleted situation should produce low boost: {boost}");
    }
}
