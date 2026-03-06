//! Unified `CognitiveContextSource` — replaces `LearningContextSource` +
//! `ProductivityContextSource` with a single cognitive-memory-backed source.
//!
//! Loads the structured `UserModel`, active procedural rules, and formats
//! them as context for the LLM prompt.

use async_trait::async_trait;
use context_engine::source::{ContextSource, SourceContext};
use tokio::sync::Mutex;
use tracing::warn;

use crate::repos::{load_user_model, ProceduralRuleRepo, SemanticFactRepo, RULE_DOMAINS};
use crate::types::UserModel;

/// Cache TTL for user model data (seconds).
const CACHE_TTL_SECS: u64 = 60;

struct CachedModel {
    model: UserModel,
    rules_text: String,
    cached_at: std::time::Instant,
}

/// Context source that injects cognitive memory into the LLM prompt.
///
/// Priority 60 — appears after identity but before task context.
pub struct CognitiveContextSource {
    fact_repo: SemanticFactRepo,
    rule_repo: ProceduralRuleRepo,
    cache: Mutex<Option<CachedModel>>,
}

impl CognitiveContextSource {
    pub fn new(fact_repo: SemanticFactRepo, rule_repo: ProceduralRuleRepo) -> Self {
        Self {
            fact_repo,
            rule_repo,
            cache: Mutex::new(None),
        }
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

    async fn provide(&self, _ctx: &SourceContext) -> Option<String> {
        let (model, rules_text) = self.get_cached_or_load().await;

        let mut sections = Vec::new();
        sections.push("# User Understanding".to_string());

        // Format each domain's facts
        let domain_sections = [
            ("Identity", &model.identity),
            ("Energy & Rhythms", &model.energy),
            ("Work Patterns", &model.work),
            ("Finance", &model.finance),
            ("Learning", &model.learning),
            ("Preferences", &model.preferences),
        ];

        for (label, facts) in &domain_sections {
            if !facts.is_empty() {
                let lines: Vec<String> = facts
                    .iter()
                    .map(|f| format!("- {}: {} = {}", f.subject, f.predicate, f.object))
                    .collect();
                sections.push(format!("## {label}\n{}", lines.join("\n")));
            }
        }

        if !rules_text.is_empty() {
            sections.push(format!("## Learned Patterns\n{rules_text}"));
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
            recorded_at: "2026-03-06".into(),
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
}
