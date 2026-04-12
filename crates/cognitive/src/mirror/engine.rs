//! MirrorEngine — lifecycle manager for the Mirror self-reflection layer.
//!
//! Starts all event-driven subscribers and wires them to the domain event bus,
//! then produces a configured [`MirrorFacade`] for use by Tauri commands and
//! MCP handlers.

use std::sync::Arc;

use dashmap::DashMap;
use tokio::task::JoinHandle;
use tokio_util::sync::CancellationToken;

use crate::mirror::{
    AutotunerBridge, ConfigArchiver, MetaRuleDetector, MirrorFacade, MirrorRepo, NarrativeHandler,
    RoutingMirrorSubscriber, SchemaMirrorSubscriber, TrialPreviewSubscriber,
};
use crate::repos::{EpisodicMemoryRepo, ProceduralRuleRepo};

// ---------------------------------------------------------------------------
// MirrorEngine
// ---------------------------------------------------------------------------

/// Starts Mirror subscribers and produces the [`MirrorFacade`].
///
/// # Usage
///
/// ```rust,ignore
/// let (facade, handles, shutdown) = MirrorEngine::start(repo, Arc::new(bus), Some(handler), None, None, None);
/// // Store `handles` to keep subscribers alive; drop or cancel `shutdown` to stop them.
/// ```
pub struct MirrorEngine;

impl MirrorEngine {
    /// Start all Mirror subscribers against `bus` and return the facade,
    /// a list of [`JoinHandle`]s for the background tasks, and a
    /// [`CancellationToken`] that can be cancelled to request a graceful
    /// shutdown.
    ///
    /// If `narrative_handler` is `Some`, LLM-powered methods on the facade
    /// will be available. If `None`, those methods return an error.
    pub fn start(
        repo: MirrorRepo,
        bus: Arc<bus::DomainEventBus>,
        narrative_handler: Option<Arc<dyn NarrativeHandler>>,
        autotuner_bridge: Option<Arc<dyn AutotunerBridge>>,
        episodic_repo: Option<EpisodicMemoryRepo>,
        rule_repo: Option<ProceduralRuleRepo>,
        trial_evaluator: Option<Arc<dyn crate::mirror::types::EarlyTrialEvaluator>>,
    ) -> (MirrorFacade, Vec<JoinHandle<()>>, CancellationToken) {
        let shutdown = CancellationToken::new();

        let meta_rule_repo = repo.clone();
        let version_repo = repo.clone();
        let trial_repo = repo.clone();

        // Shared active timers between TrialPreviewSubscriber and MirrorFacade
        let active_timers: Arc<DashMap<String, JoinHandle<()>>> = Arc::new(DashMap::new());

        let routing_sub = Arc::new(RoutingMirrorSubscriber::new(repo.clone()));
        let meta_rule_detector = MetaRuleDetector::new(meta_rule_repo);
        let config_archiver = ConfigArchiver::new(version_repo, autotuner_bridge.clone());
        let trial_sub = Arc::new(TrialPreviewSubscriber::new(
            trial_repo,
            active_timers.clone(),
            trial_evaluator,
        ));

        let schema_sub = SchemaMirrorSubscriber::new(repo.pool().clone());

        let handles = vec![
            tokio::spawn(routing_sub.run(bus.subscribe(), shutdown.clone())),
            tokio::spawn(meta_rule_detector.run(bus.subscribe(), shutdown.clone())),
            tokio::spawn(config_archiver.run(bus.subscribe(), shutdown.clone())),
            tokio::spawn(trial_sub.run(bus.subscribe(), shutdown.clone())),
            tokio::spawn(schema_sub.run(bus.subscribe(), shutdown.clone())),
        ];

        let mut facade = MirrorFacade::new(repo);
        facade = facade.with_active_timers(active_timers);
        facade = facade.with_domain_event_bus(bus);
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

        (facade, handles, shutdown)
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
        // Five subscribers (routing + meta_rule + config_archiver + trial_preview + schema).
        assert_eq!(bus.subscriber_count(), 5);

        shutdown.cancel();
        for handle in handles {
            handle.await.unwrap();
        }
    }
}
