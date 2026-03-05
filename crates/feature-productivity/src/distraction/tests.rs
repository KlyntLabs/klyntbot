//! Integration tests for the full distraction interceptor flow.

use crate::config::FocusConfig;
use crate::distraction::interceptor::{DistractionInterceptor, InterceptDecision};
use crate::repos::learned_rule::{LearnedRule, LearnedRuleRepo};
use crate::ProductivityFeature;
use chrono::Utc;

async fn setup() -> (DistractionInterceptor, LearnedRuleRepo) {
    let pool = storage::StoragePool::connect_in_memory().await.unwrap();
    let inner = pool.inner().clone();
    storage::StoragePool::run_feature_migrations(&inner, &ProductivityFeature::migrations_static())
        .await
        .unwrap();
    let repo = LearnedRuleRepo::new(inner);
    let interceptor = DistractionInterceptor::new(FocusConfig::default(), repo.clone());
    (interceptor, repo)
}

#[tokio::test]
async fn full_flow_whitelist_then_reset() {
    let (mut interceptor, _) = setup().await;

    // YouTube triggers overlay
    let d = interceptor
        .evaluate("Chrome", Some("cat videos - YouTube"))
        .await;
    assert!(matches!(
        d,
        InterceptDecision::ShowOverlay { needs_llm: true }
    ));

    // User marks as work-related -> whitelisted
    interceptor.whitelist_for_session("cat videos - youtube");
    let d = interceptor
        .evaluate("Chrome", Some("cat videos - YouTube"))
        .await;
    assert!(matches!(d, InterceptDecision::Allow { .. }));

    // Session ends -> whitelist cleared
    interceptor.reset_session();
    let d = interceptor
        .evaluate("Chrome", Some("cat videos - YouTube"))
        .await;
    assert!(matches!(d, InterceptDecision::ShowOverlay { .. }));
}

#[tokio::test]
async fn learned_rule_auto_allows_after_threshold() {
    let (mut interceptor, repo) = setup().await;
    let now = Utc::now();

    // Insert a learned rule with hit_count >= threshold (default 3)
    let rule = LearnedRule {
        id: None,
        pattern: "react tutorial - youtube".into(),
        pattern_type: "title_keyword".into(),
        classification: "educational".into(),
        confidence: 0.8,
        hit_count: 3,
        last_used_at: now,
        created_at: now,
    };
    repo.insert(&rule).await.unwrap();

    // Should auto-allow
    let d = interceptor
        .evaluate("Chrome", Some("React Tutorial - YouTube"))
        .await;
    assert!(matches!(d, InterceptDecision::Allow { .. }));
}

#[tokio::test]
async fn learned_rule_below_threshold_still_shows_overlay() {
    let (mut interceptor, repo) = setup().await;
    let now = Utc::now();

    let rule = LearnedRule {
        id: None,
        pattern: "some video - youtube".into(),
        pattern_type: "title_keyword".into(),
        classification: "educational".into(),
        confidence: 0.5,
        hit_count: 1, // below threshold
        last_used_at: now,
        created_at: now,
    };
    repo.insert(&rule).await.unwrap();

    let d = interceptor
        .evaluate("Chrome", Some("some video - YouTube"))
        .await;
    assert!(matches!(d, InterceptDecision::ShowOverlay { .. }));
}

#[tokio::test]
async fn productive_content_never_triggers_overlay() {
    let (mut interceptor, _) = setup().await;

    let d = interceptor
        .evaluate("Chrome", Some("Rust lifetimes - Stack Overflow"))
        .await;
    assert!(matches!(d, InterceptDecision::Allow { .. }));

    let d = interceptor
        .evaluate("Safari", Some("std::vec::Vec - docs.rs"))
        .await;
    assert!(matches!(d, InterceptDecision::Allow { .. }));
}

#[tokio::test]
async fn always_distracting_apps_no_llm() {
    let (mut interceptor, _) = setup().await;

    let d = interceptor.evaluate("Netflix", None).await;
    assert_eq!(d, InterceptDecision::ShowOverlay { needs_llm: false });

    let d = interceptor.evaluate("TikTok", Some("For You")).await;
    assert_eq!(d, InterceptDecision::ShowOverlay { needs_llm: false });
}
