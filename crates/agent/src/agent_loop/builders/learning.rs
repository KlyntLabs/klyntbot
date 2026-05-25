//! Learning-service builder for the agent loop.

use std::sync::atomic::Ordering;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::RwLock;
use tracing::info;

use config::Config;

/// Result of the learning-service build phase.
pub(crate) struct LearningBuildResult {
    pub learning_service: Option<Arc<RwLock<crate::learning::LearningService>>>,
}

/// Build the learning service, register the LearningTool, and wire the
/// adaptive-threshold event subscriber.
pub(crate) async fn build_learning_service(
    config: &Config,
    repos: &storage::Repos,
    tool_registry: &mut tools::registry::ToolRegistry,
    outcome_store: &Option<Arc<RwLock<crate::learning::OutcomeStore>>>,
    confidence_bits: &Arc<std::sync::atomic::AtomicU32>,
    domain_event_bus: &Option<Arc<bus::DomainEventBus>>,
    cron_executor: &Option<(
        Arc<scheduling::temporal::cron_executor::CronExecutor>,
        storage::repos::cron::CronRepo,
    )>,
) -> LearningBuildResult {
    let Some(ref store) = outcome_store else {
        return LearningBuildResult {
            learning_service: None,
        };
    };

    let adaptive = Arc::new(RwLock::new(
        crate::learning::adaptive::AdaptiveThresholds::new(
            repos.learning_state.clone(),
            config.confidence.threshold,
            config.learning.min_threshold,
            config.learning.max_threshold,
            config.learning.min_outcomes_for_adaptation,
        )
        .await,
    ));

    // Register LearningTool
    let learning_handler = Arc::new(crate::LearningHandlerImpl::new(
        repos.strategies.clone(),
        Arc::clone(&adaptive),
    ));
    tool_registry.register(tools::LearningTool::new(Some(
        Arc::clone(&learning_handler) as Arc<dyn tools::LearningHandler>,
    )));

    // Event bus: subscriber updates cognitive confidence threshold
    let event_bus = Arc::new(bus::LearningEventBus::new(16));

    let threshold_for_subscriber = Arc::clone(confidence_bits);
    let mut event_rx = event_bus.subscribe();
    tokio::spawn(async move {
        while let Ok(event) = event_rx.recv().await {
            if let bus::LearningEvent::ThresholdChanged { new_threshold, .. } = event {
                threshold_for_subscriber.store(new_threshold.to_bits(), Ordering::Relaxed);
                info!(
                    "Confidence threshold updated by LearningService: {:.3}",
                    new_threshold
                );
            }
        }
    });

    let mut service = crate::learning::LearningService::new(
        Arc::clone(store),
        adaptive,
        None, // No confidence evaluator in flat architecture
        Duration::from_secs(config.learning.analysis_interval_secs),
    )
    .with_event_bus(event_bus);
    if let Some(ref domain_bus) = domain_event_bus {
        service = service.with_pattern_analyzer(crate::learning::PatternAnalyzer::new(
            repos.interaction_log.clone(),
            Arc::clone(domain_bus),
        ));
    }
    service.start();
    let svc_arc = Arc::new(RwLock::new(service));

    // Register cron handler so the learning analysis shows in the Automations page.
    // The handler triggers the existing background loop rather than running inline.
    if let Some((ref executor, _)) = cron_executor {
        let svc_for_cron = Arc::clone(&svc_arc);
        executor.register(
            "__klyntbot_learning_analysis",
            Arc::new(move |_job: &scheduling::CronJob| {
                let svc = Arc::clone(&svc_for_cron);
                // Trigger the analysis via the existing Notify mechanism
                if let Ok(guard) = svc.try_read() {
                    guard.trigger_analysis();
                }
                Ok(Some("Learning analysis triggered".to_string()))
            }),
        );
    }

    LearningBuildResult {
        learning_service: Some(svc_arc),
    }
}
