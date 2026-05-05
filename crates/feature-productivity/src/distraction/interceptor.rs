//! DistractionInterceptor — decides whether to show the overlay for a distraction alert.

use std::collections::{HashMap, HashSet};
use std::time::Instant;

use super::heuristics::{self, HeuristicVerdict};
use crate::config::FocusConfig;
use crate::repos::learned_rule::LearnedRuleRepo;

/// The interceptor's decision for a given distraction alert.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum InterceptDecision {
    /// Skip — content is whitelisted or classified as productive.
    Allow { reason: String },
    /// Show the overlay — content is distracting or ambiguous.
    ShowOverlay { needs_llm: bool },
}

pub struct DistractionInterceptor {
    config: FocusConfig,
    learned_rules_repo: LearnedRuleRepo,
    /// Titles whitelisted for the current focus session.
    session_whitelist: HashSet<String>,
    /// Temporary passes: pattern -> expiry instant.
    temp_passes: HashMap<String, Instant>,
}

impl DistractionInterceptor {
    pub fn new(config: FocusConfig, learned_rules_repo: LearnedRuleRepo) -> Self {
        Self {
            config,
            learned_rules_repo,
            session_whitelist: HashSet::new(),
            temp_passes: HashMap::new(),
        }
    }

    /// Clear all session state (call when focus session ends).
    pub fn reset_session(&mut self) {
        self.session_whitelist.clear();
        self.temp_passes.clear();
    }

    /// Add a title pattern to the session whitelist ("This is work-related").
    pub fn whitelist_for_session(&mut self, pattern: &str) {
        self.session_whitelist.insert(pattern.to_lowercase());
    }

    /// Grant a temporary pass for the given pattern.
    pub fn grant_temp_pass(&mut self, pattern: &str) {
        let expiry = Instant::now()
            + std::time::Duration::from_secs(self.config.soft_block_temp_pass_mins * 60);
        self.temp_passes.insert(pattern.to_lowercase(), expiry);
    }

    /// Evaluate a distraction alert and decide what to do.
    pub async fn evaluate(
        &mut self,
        app_name: &str,
        window_title: Option<&str>,
    ) -> InterceptDecision {
        if !self.config.soft_block_enabled {
            return InterceptDecision::Allow {
                reason: "soft_block_enabled is false".into(),
            };
        }

        let (key, pattern_type) = Self::make_key(app_name, window_title);
        let title_lower = window_title.map(|t| t.to_ascii_lowercase());

        // 1. Check session whitelist — exact match, then keyword containment.
        if self.session_whitelist.contains(&key)
            || (!self.session_whitelist.is_empty()
                && title_lower.as_deref().is_some_and(|t| {
                    self.session_whitelist
                        .iter()
                        .any(|pattern| t.contains(pattern.as_str()))
                }))
        {
            return InterceptDecision::Allow {
                reason: "session whitelist".into(),
            };
        }

        // 2. Check temp passes — exact match OR keyword containment.
        {
            let now = Instant::now();
            let has_active_pass = self.temp_passes.get(&key).is_some_and(|exp| *exp > now)
                || (!self.temp_passes.is_empty()
                    && title_lower.as_deref().is_some_and(|t| {
                        self.temp_passes
                            .iter()
                            .any(|(pattern, exp)| *exp > now && t.contains(pattern.as_str()))
                    }));
            if has_active_pass {
                return InterceptDecision::Allow {
                    reason: "temporary pass active".into(),
                };
            }
            // Lazy prune expired entries
            if !self.temp_passes.is_empty() {
                self.temp_passes.retain(|_, exp| *exp > now);
            }
        }

        // 3. Run heuristics first (free, no I/O).
        let verdict = heuristics::classify(app_name, window_title);
        match verdict {
            HeuristicVerdict::ConfidentProductive => {
                return InterceptDecision::Allow {
                    reason: "heuristic: confident productive".into(),
                };
            }
            HeuristicVerdict::ConfidentDistracting => {
                return InterceptDecision::ShowOverlay { needs_llm: false };
            }
            HeuristicVerdict::Ambiguous => {}
        }

        // 4. Check persistent learned rules (only for ambiguous content).
        if let Ok(Some(rule)) = self
            .learned_rules_repo
            .find_by_pattern(&key, pattern_type)
            .await
        {
            if rule.hit_count >= self.config.learned_rule_threshold as i64 {
                if let Err(e) = self
                    .learned_rules_repo
                    .record_hit(rule.id.unwrap_or(0))
                    .await
                {
                    tracing::debug!("failed to record learned rule hit: {e}");
                }
                return InterceptDecision::Allow {
                    reason: format!(
                        "learned rule: {} ({}x)",
                        rule.classification, rule.hit_count
                    ),
                };
            }
        }

        // 5. Ambiguous — show overlay with optional LLM classification.
        InterceptDecision::ShowOverlay {
            needs_llm: self.config.soft_block_llm_enabled,
        }
    }

    /// Normalize app_name + window_title into a lookup key and its pattern type.
    pub fn make_key(app_name: &str, window_title: Option<&str>) -> (String, &'static str) {
        match window_title {
            Some(title) => (title.to_lowercase(), "title_keyword"),
            None => (app_name.to_lowercase(), "app_name"),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    

    async fn setup_with_config(config: FocusConfig) -> DistractionInterceptor {
        let pool = storage::StoragePool::connect_in_memory().await.unwrap();
        let inner = pool.inner().clone();
        storage::StoragePool::run_feature_migrations(&inner, &crate::productivity_migrations())
            .await
            .unwrap();
        let repo = LearnedRuleRepo::new(inner);
        DistractionInterceptor::new(config, repo)
    }

    async fn setup() -> DistractionInterceptor {
        setup_with_config(FocusConfig::default()).await
    }

    #[tokio::test]
    async fn disabled_soft_block_allows_all() {
        let mut interceptor = setup_with_config(FocusConfig {
            soft_block_enabled: false,
            ..FocusConfig::default()
        })
        .await;

        let decision = interceptor.evaluate("Netflix", None).await;
        assert!(matches!(decision, InterceptDecision::Allow { .. }));
    }

    #[tokio::test]
    async fn session_whitelist_allows() {
        let mut interceptor = setup().await;
        interceptor.whitelist_for_session("funny cats - youtube");

        let decision = interceptor
            .evaluate("Chrome", Some("Funny Cats - YouTube"))
            .await;
        assert!(matches!(decision, InterceptDecision::Allow { .. }));
    }

    #[tokio::test]
    async fn temp_pass_allows_then_expires() {
        let mut interceptor = setup_with_config(FocusConfig {
            soft_block_temp_pass_mins: 0, // expires immediately
            ..FocusConfig::default()
        })
        .await;

        interceptor.grant_temp_pass("reddit");
        std::thread::sleep(std::time::Duration::from_millis(10));
        let decision = interceptor.evaluate("Reddit", None).await;
        assert!(matches!(decision, InterceptDecision::ShowOverlay { .. }));
    }

    #[tokio::test]
    async fn netflix_shows_overlay_no_llm() {
        let mut interceptor = setup().await;
        let decision = interceptor.evaluate("Netflix", None).await;
        assert_eq!(
            decision,
            InterceptDecision::ShowOverlay { needs_llm: false }
        );
    }

    #[tokio::test]
    async fn youtube_shows_overlay_with_llm() {
        let mut interceptor = setup().await;
        let decision = interceptor
            .evaluate("Chrome", Some("Some Video - YouTube"))
            .await;
        assert_eq!(decision, InterceptDecision::ShowOverlay { needs_llm: true });
    }

    #[tokio::test]
    async fn stackoverflow_in_title_allows() {
        let mut interceptor = setup().await;
        let decision = interceptor
            .evaluate("Chrome", Some("Rust lifetime - Stack Overflow"))
            .await;
        assert!(matches!(decision, InterceptDecision::Allow { .. }));
    }

    #[tokio::test]
    async fn reset_session_clears_state() {
        let mut interceptor = setup().await;
        interceptor.whitelist_for_session("youtube");
        interceptor.reset_session();
        let decision = interceptor
            .evaluate("Chrome", Some("cat video - YouTube"))
            .await;
        assert!(matches!(decision, InterceptDecision::ShowOverlay { .. }));
    }
}
