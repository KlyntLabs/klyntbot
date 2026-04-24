use std::collections::HashMap;
use std::sync::Arc;

use async_trait::async_trait;

use crate::repos::CategorizationCacheRepo;
use crate::types::*;

use super::tracking_rules::TrackingRulesEngine;

/// Trait for AI-powered activity classification fallback.
/// Implemented in the agent crate (via ProductivityHandler) to avoid circular deps.
#[async_trait]
pub trait AiClassifier: Send + Sync {
    async fn classify(
        &self,
        app: &str,
        title: &str,
        url: Option<&str>,
    ) -> common::Result<(String, IntelligenceSessionType)>;
}

/// Maximum entries in the in-memory classification cache.
/// Prevents unbounded growth over long-running sessions.
const MEM_CACHE_MAX: usize = 2048;

/// Classifies activities using rules first, then cache, then optional AI fallback.
pub struct CategorizationService {
    rules_engine: TrackingRulesEngine,
    cache_repo: CategorizationCacheRepo,
    mem_cache: HashMap<String, ClassificationResult>,
    ai_classifier: Option<Arc<dyn AiClassifier>>,
}

impl CategorizationService {
    pub fn new(
        rules_engine: TrackingRulesEngine,
        cache_repo: CategorizationCacheRepo,
        ai_classifier: Option<Arc<dyn AiClassifier>>,
    ) -> Self {
        Self {
            rules_engine,
            cache_repo,
            mem_cache: HashMap::new(),
            ai_classifier,
        }
    }

    /// Categorize an activity. Tries rules -> in-memory cache -> DB cache -> AI fallback.
    pub async fn categorize(
        &mut self,
        app: &str,
        url: Option<&str>,
        title: Option<&str>,
    ) -> ClassificationResult {
        // 1. Rules engine (sync, O(1) for exact app match)
        let result = self.rules_engine.classify(app, url, title);
        if result.source != ClassificationSource::Default {
            if let Some(rule_id) = &result.rule_id {
                let _ = self.rules_engine.record_hit(rule_id).await;
            }
            return result;
        }

        let cache_key = make_cache_key(app, url);

        // 2. In-memory cache
        if let Some(cached) = self.mem_cache.get(&cache_key) {
            return cached.clone();
        }

        // 3. DB cache
        if let Ok(Some(entry)) = self.cache_repo.get(&cache_key).await {
            let result = ClassificationResult {
                category: entry.category,
                session_type: entry
                    .session_type
                    .parse()
                    .unwrap_or(IntelligenceSessionType::Focus),
                confidence: entry.confidence,
                rule_id: None,
                source: match entry.source.as_str() {
                    "ai" => ClassificationSource::AiFallback,
                    _ => ClassificationSource::Default,
                },
            };
            self.cache_insert(cache_key, result.clone());
            return result;
        }

        // 4. AI fallback
        if let Some(classifier) = &self.ai_classifier {
            if let Ok((category, session_type)) =
                classifier.classify(app, title.unwrap_or(""), url).await
            {
                let result = ClassificationResult {
                    category: category.clone(),
                    session_type,
                    confidence: 0.7,
                    rule_id: None,
                    source: ClassificationSource::AiFallback,
                };

                let now = jiff::Timestamp::now();
                let expires = now + jiff::SignedDuration::from_hours(24);
                let entry = CategorizationCacheEntry {
                    cache_key: cache_key.clone(),
                    category,
                    session_type: session_type.to_string(),
                    confidence: 0.7,
                    source: "ai".to_string(),
                    created_at: now.to_string(),
                    expires_at: expires.to_string(),
                };
                let _ = self.cache_repo.upsert(&entry).await;
                self.cache_insert(cache_key, result.clone());
                return result;
            }
        }

        // 5. Default
        ClassificationResult {
            category: "uncategorized".to_string(),
            session_type: IntelligenceSessionType::Focus,
            confidence: 0.0,
            rule_id: None,
            source: ClassificationSource::Default,
        }
    }

    /// Insert into mem_cache with a size cap to prevent unbounded growth.
    /// Evicts ~25% of entries on overflow instead of clearing everything,
    /// which prevents a thundering herd of LLM/DB calls after each wipe.
    fn cache_insert(&mut self, key: String, result: ClassificationResult) {
        if self.mem_cache.len() >= MEM_CACHE_MAX {
            let to_remove = MEM_CACHE_MAX / 4;
            let mut removed = 0;
            self.mem_cache.retain(|_, _| {
                if removed < to_remove {
                    removed += 1;
                    false
                } else {
                    true
                }
            });
        }
        self.mem_cache.insert(key, result);
    }

    pub fn rules_engine(&self) -> &TrackingRulesEngine {
        &self.rules_engine
    }

    pub fn rules_engine_mut(&mut self) -> &mut TrackingRulesEngine {
        &mut self.rules_engine
    }

    pub fn apply_privacy(
        &self,
        app: &str,
        title: Option<&str>,
        url: Option<&str>,
    ) -> PrivacyFilterResult {
        self.rules_engine.apply_privacy(app, title, url)
    }
}

fn make_cache_key(app: &str, url: Option<&str>) -> String {
    match url {
        Some(u) => format!("app:{}:url:{}", app.to_lowercase(), u.to_lowercase()),
        None => format!("app:{}", app.to_lowercase()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::repos::{PrivacyRuleRepo, RuleEvolutionLogRepo, TrackingRuleRepo};
    use crate::ProductivityFeature;
    use jiff::Timestamp;
    use sqlx::SqlitePool;

    async fn setup_pool() -> SqlitePool {
        let pool = storage::StoragePool::connect_in_memory().await.unwrap();
        let inner = pool.inner().clone();
        storage::StoragePool::run_feature_migrations(&inner, &crate::productivity_migrations())
            .await
            .unwrap();
        inner
    }

    async fn setup_service(
        pool: &SqlitePool,
        ai: Option<Arc<dyn AiClassifier>>,
    ) -> CategorizationService {
        sqlx::query("DELETE FROM productivity_tracking_rules")
            .execute(pool)
            .await
            .unwrap();

        let engine = TrackingRulesEngine::load(
            TrackingRuleRepo::new(pool.clone()),
            PrivacyRuleRepo::new(pool.clone()),
            RuleEvolutionLogRepo::new(pool.clone()),
        )
        .await
        .unwrap();
        CategorizationService::new(engine, CategorizationCacheRepo::new(pool.clone()), ai)
    }

    #[tokio::test]
    async fn test_categorize_uses_rules_first() {
        let pool = setup_pool().await;
        let mut service = setup_service(&pool, None).await;

        // Add a rule and reload
        let rule_repo = TrackingRuleRepo::new(pool.clone());
        let now = Timestamp::now();
        let rule = TrackingRule {
            id: "cat-rule-1".to_string(),
            rule_type: RuleType::App,
            match_field: "app_name".to_string(),
            match_pattern: "Code".to_string(),
            match_mode: MatchMode::Exact,
            category: "coding".to_string(),
            session_type: IntelligenceSessionType::Focus,
            priority: 10,
            source: RuleSource::System,
            confidence: 1.0,
            hit_count: 0,
            last_hit_at: None,
            is_active: true,
            created_at: now,
            updated_at: now,
        };
        rule_repo.create(&rule).await.unwrap();
        service.rules_engine_mut().reload().await.unwrap();

        let result = service.categorize("Code", None, None).await;
        assert_eq!(result.category, "coding");
        assert_eq!(result.source, ClassificationSource::Rule);
    }

    #[tokio::test]
    async fn test_categorize_falls_back_to_cache() {
        let pool = setup_pool().await;
        let mut service = setup_service(&pool, None).await;

        // Seed DB cache entry
        let cache_repo = CategorizationCacheRepo::new(pool.clone());
        let entry = CategorizationCacheEntry {
            cache_key: "app:cachedapp".to_string(),
            category: "design".to_string(),
            session_type: "focus".to_string(),
            confidence: 0.8,
            source: "ai".to_string(),
            created_at: "2026-03-09T10:00:00Z".to_string(),
            expires_at: "2026-12-31T23:59:59Z".to_string(),
        };
        cache_repo.upsert(&entry).await.unwrap();

        // First call hits DB cache
        let result = service.categorize("CachedApp", None, None).await;
        assert_eq!(result.category, "design");
        assert_eq!(result.source, ClassificationSource::AiFallback);

        // Second call hits in-memory cache (same result)
        let result = service.categorize("CachedApp", None, None).await;
        assert_eq!(result.category, "design");
    }

    #[tokio::test]
    async fn test_categorize_default_when_no_handler() {
        let pool = setup_pool().await;
        let mut service = setup_service(&pool, None).await;

        let result = service.categorize("UnknownApp", None, None).await;
        assert_eq!(result.source, ClassificationSource::Default);
        assert_eq!(result.category, "uncategorized");
    }
}
