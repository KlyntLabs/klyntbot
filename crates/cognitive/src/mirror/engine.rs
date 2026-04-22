//! MirrorEngine — lifecycle manager for the Mirror self-reflection layer.
//!
//! Produces a configured [`MirrorFacade`] and a list of [`MirrorSignalSource`]
//! implementations that should be wired into the workspace `SignalRouter`.

use std::sync::Arc;

use ai_core::MirrorSignalSource;
use dashmap::DashMap;
use tokio::task::JoinHandle;
use tokio_util::sync::CancellationToken;

use crate::mirror::{
    AutotunerBridge, ConfigArchiverSource, MetaRuleSignalSource, MirrorFacade, MirrorRepo,
    NarrativeHandler, RoutingSignalSource, TrialPreviewSource,
};
use crate::repos::{EpisodicMemoryRepo, ProceduralRuleRepo};

// ---------------------------------------------------------------------------
// MirrorEngine
// ---------------------------------------------------------------------------

/// Produces the [`MirrorFacade`] and a list of [`MirrorSignalSource`]s.
///
/// The caller is responsible for wrapping each source in a
/// [`MirrorSubscriberRunner`] and registering it with the `SignalRouter`.
///
/// # Usage
///
/// ```rust,ignore
/// let (facade, sources, active_timers) = MirrorEngine::start(repo, Some(handler), None, None, None);
/// // Wire sources into SignalRouter via MirrorSubscriberRunner...
/// ```
pub struct MirrorEngine;

impl MirrorEngine {
    /// Create the facade and signal sources. Does not spawn any tasks —
    /// the caller wires sources into the `SignalRouter`.
    pub fn start(
        repo: MirrorRepo,
        narrative_handler: Option<Arc<dyn NarrativeHandler>>,
        autotuner_bridge: Option<Arc<dyn AutotunerBridge>>,
        episodic_repo: Option<EpisodicMemoryRepo>,
        rule_repo: Option<ProceduralRuleRepo>,
        trial_evaluator: Option<Arc<dyn crate::mirror::types::EarlyTrialEvaluator>>,
    ) -> (
        MirrorFacade,
        Vec<Arc<dyn MirrorSignalSource>>,
        Arc<DashMap<String, JoinHandle<()>>>,
    ) {
        let meta_rule_repo = repo.clone();
        let version_repo = repo.clone();
        let trial_repo = repo.clone();

        // Shared active timers between TrialPreviewSource and MirrorFacade
        let active_timers: Arc<DashMap<String, JoinHandle<()>>> = Arc::new(DashMap::new());

        let routing_source: Arc<dyn MirrorSignalSource> =
            Arc::new(RoutingSignalSource::new(repo.clone()));
        let meta_rule_source: Arc<dyn MirrorSignalSource> =
            Arc::new(MetaRuleSignalSource::new(meta_rule_repo));
        let config_archiver: Arc<dyn MirrorSignalSource> =
            Arc::new(ConfigArchiverSource::new(version_repo, autotuner_bridge.clone()));
        let trial_source: Arc<dyn MirrorSignalSource> = Arc::new(TrialPreviewSource::new(
            trial_repo,
            active_timers.clone(),
            trial_evaluator,
        ));

        let sources: Vec<Arc<dyn MirrorSignalSource>> = vec![
            routing_source,
            meta_rule_source,
            config_archiver,
            trial_source,
        ];

        let mut facade = MirrorFacade::new(repo);
        facade = facade.with_active_timers(active_timers.clone());
        if let Some(handler) = narrative_handler {
            facade = facade.with_narrative_handler(handler);
        }
        if let Some(bridge) = autotuner_bridge {
            facade = facade.with_autotuner_bridge(bridge);
        }
        if let Some(episodic) = episodic_repo {
            facade = facade.with_episodic_repo(episodic);
        }
        if let Some(rule_repo) = rule_repo {
            facade = facade.with_rule_repo(rule_repo);
        }

        (facade, sources, active_timers)
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_engine_start_no_handler() {
        let repo = crate::mirror::test_mirror_repo().await;
        let bus = Arc::new(bus::DomainEventBus::new(16));

        let (facade, handles, shutdown) =
            MirrorEngine::start(repo, bus, None, None, None, None, None);

        // Facade is usable.
        let state = facade.get_state().await.unwrap();
        assert!(state.last_routing_snapshot.is_none());

        // Shutdown and wait for subscribers to exit.
        shutdown.cancel();
        for handle in handles {
            handle.await.unwrap();
        }
    }

    #[tokio::test]
    async fn test_engine_start_routes_skill_routed_events() {
        let repo = crate::mirror::test_mirror_repo().await;
        let bus = Arc::new(bus::DomainEventBus::new(16));

        let (_facade, handles, shutdown) =
            MirrorEngine::start(repo, Arc::clone(&bus), None, None, None, None, None);

        // Publish a SkillRouted event — the subscriber should accumulate it.
        bus.publish(bus::DomainEvent::SkillRouted {
            skill_name: "general".to_string(),
            confidence: 0.85,
            source: "keyword".to_string(),
            trigger_phrases: vec!["hello".to_string()],
            session_key: "test-session".to_string(),
        });

        // Give the subscriber a moment to process the event.
        tokio::time::sleep(std::time::Duration::from_millis(50)).await;

        shutdown.cancel();
        for handle in handles {
            handle.await.unwrap();
        }
    }

    #[tokio::test]
    async fn test_engine_subscriber_count() {
        let repo = crate::mirror::test_mirror_repo().await;
        let bus = Arc::new(bus::DomainEventBus::new(16));

        assert_eq!(bus.subscriber_count(), 0);
        let (_facade, handles, shutdown) =
            MirrorEngine::start(repo, Arc::clone(&bus), None, None, None, None, None);
        // Four subscribers (routing + meta_rule + config_archiver + trial_preview).
        assert_eq!(bus.subscriber_count(), 4);

        shutdown.cancel();
        for handle in handles {
            handle.await.unwrap();
        }
    }
}
