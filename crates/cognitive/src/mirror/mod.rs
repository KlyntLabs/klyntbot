pub mod engine;
pub mod facade;
pub mod narratives;
pub mod repo;
pub mod subscribers;
pub mod types;
pub use engine::MirrorEngine;
pub use facade::MirrorFacade;
pub use narratives::{snippet_from_alert, NarrativeHandler};
pub use repo::MirrorRepo;
pub use subscribers::{
    ConfigArchiver, MetaRuleDetector, RoutingMirrorSubscriber, TrialPreviewSubscriber,
};
pub use types::{
    AutotunerBridge, BrainVersion, EarlyTrialEvaluator, FeedbackTarget, GeneratedNarrative,
    MetaRule, MetaRuleAction, MetaRuleSource, MetaRuleStatus, MirrorAlert, MirrorAlertType,
    MirrorResponse, MirrorState, NarrativeContext, NarrativeSnippet, PreviewRecommendation,
    RoutingSnapshot, SkillRouteStats, SuggestedAction, TrendDirection, TrendNarrative,
    TrialEarlySignals, TrialPreview, UserFeedback,
};

#[cfg(test)]
pub(crate) async fn test_mirror_repo() -> MirrorRepo {
    let pool = storage::StoragePool::connect_in_memory().await.unwrap();
    storage::StoragePool::run_feature_migrations(
        pool.inner(),
        &crate::repos::cognitive_migrations(),
    )
    .await
    .unwrap();
    MirrorRepo::new(pool)
}
