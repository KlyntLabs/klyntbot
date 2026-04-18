//! ProductivityIntelligenceLayer — orchestrator that subscribes to
//! ActivityTick broadcast, classifies, aggregates into sessions,
//! scores quality, and publishes high-level DomainEvents.

use std::sync::Arc;

use tokio::sync::broadcast;
use tokio_util::sync::CancellationToken;
use tracing::{info, warn};

use bus::DomainEventBus;
use serde::Serialize;

use crate::handler::ProductivityHandler;
use crate::repos::ProductivityRepos;
use crate::types::*;

use super::categorization::CategorizationService;
use super::intervention_router::InterventionRouter;
use super::quality_scorer::QualityScorer;
use super::session_aggregator::{ClassifiedTick, SessionAggregator};
use super::tracking_rules::TrackingRulesEngine;

pub struct ProductivityIntelligenceLayer {
    tick_rx: broadcast::Receiver<ActivityTick>,
    categorization: CategorizationService,
    session_agg: SessionAggregator,
    quality_scorer: QualityScorer,
    intervention_router: InterventionRouter,
    cancel: CancellationToken,
    /// Optional handler for LLM-powered session summaries.
    summary_handler: Option<Arc<dyn ProductivityHandler>>,
    /// Session repo for reading/updating session data after scoring.
    session_repo: crate::repos::IntelligenceSessionRepo,
    /// Event bus for publishing cross-feature events.
    event_bus: Arc<DomainEventBus>,
}

impl ProductivityIntelligenceLayer {
    pub async fn new(
        tick_sender: &broadcast::Sender<ActivityTick>,
        event_bus: Arc<DomainEventBus>,
        repos: ProductivityRepos,
        handler: Option<Arc<dyn ProductivityHandler>>,
        cancel: CancellationToken,
    ) -> common::Result<Self> {
        let tick_rx = tick_sender.subscribe();

        let rules_engine = TrackingRulesEngine::load(
            repos.tracking_rules.clone(),
            repos.privacy_rules.clone(),
            repos.rule_evolution_log.clone(),
        )
        .await?;

        // Clone the handler for session summary generation before consuming it for classification
        let summary_handler = handler.clone();
        let session_repo = repos.intelligence_sessions.clone();

        let categorization = CategorizationService::new(
            rules_engine,
            repos.categorization_cache.clone(),
            handler.map(|h| {
                Arc::new(HandlerClassifier(h)) as Arc<dyn super::categorization::AiClassifier>
            }),
        );

        let session_agg =
            SessionAggregator::new(repos.intelligence_sessions.clone(), event_bus.clone());

        let quality_scorer = QualityScorer::new(
            repos.intelligence_sessions.clone(),
            repos.quality_scores.clone(),
            event_bus.clone(),
        );

        let intervention_router = InterventionRouter::new();

        Ok(Self {
            tick_rx,
            categorization,
            session_agg,
            quality_scorer,
            intervention_router,
            cancel,
            summary_handler,
            session_repo,
            event_bus,
        })
    }

    /// Start the intelligence layer event loop. Returns a JoinHandle.
    pub fn start(self) -> tokio::task::JoinHandle<()> {
        tokio::spawn(self.run())
    }

    async fn run(mut self) {
        info!("ProductivityIntelligenceLayer started");

        loop {
            tokio::select! {
                _ = self.cancel.cancelled() => {
                    self.session_agg.flush().await;
                    info!("ProductivityIntelligenceLayer shutting down");
                    break;
                }
                result = self.tick_rx.recv() => {
                    match result {
                        Ok(tick) => self.process_tick(tick).await,
                        Err(broadcast::error::RecvError::Lagged(n)) => {
                            warn!("Intelligence layer lagged {n} ticks");
                        }
                        Err(broadcast::error::RecvError::Closed) => {
                            info!("Tick channel closed, stopping intelligence layer");
                            break;
                        }
                    }
                }
            }
        }
    }

    async fn process_tick(&mut self, tick: ActivityTick) {
        if tick.is_idle {
            return;
        }

        // 1. Classify
        let classification = self
            .categorization
            .categorize(
                &tick.app_name,
                tick.url.as_deref(),
                tick.window_title.as_deref(),
            )
            .await;

        let classified = ClassifiedTick {
            timestamp: tick.timestamp,
            app_name: tick.app_name.clone(),
            category: classification.category.clone(),
            session_type: classification.session_type,
            confidence: classification.confidence,
            is_idle: tick.is_idle,
            is_context_switch: tick.is_context_switch,
        };

        // 2. Feed to session aggregator
        let session_event = self.session_agg.process_tick(classified.clone()).await;

        // 3. On session end, score quality, then generate summary
        if let Some(SessionEvent::Ended { ref session_id, .. }) = session_event {
            if let Err(e) = self.quality_scorer.score_session(session_id).await {
                warn!(error = %e, "Failed to score session {session_id}");
            }

            // 3b. Generate LLM session summary (title + description)
            self.generate_session_summary(session_id).await;
        }

        // 4. Check for interventions
        if let Some(intervention) = self.intervention_router.evaluate(&classified) {
            info!(
                intervention_type = ?intervention.intervention_type,
                urgency = ?intervention.urgency,
                message = %intervention.message,
                "Productivity intervention triggered"
            );
            self.event_bus
                .publish(bus::DomainEvent::InterventionTriggered {
                    intervention_type: enum_to_snake(&intervention.intervention_type),
                    urgency: enum_to_snake(&intervention.urgency),
                    message: intervention.message.clone(),
                    suggested_action: enum_to_snake(&intervention.suggested_action),
                });
        }
    }

    /// Generate and store a human-readable title + description for a session.
    /// Uses LLM when available, falls back to template-based generation.
    async fn generate_session_summary(&self, session_id: &str) {
        let session = match self.session_repo.get(session_id).await {
            Ok(Some(s)) => s,
            _ => return,
        };

        let dur_mins = session.duration_secs.unwrap_or(0) / 60;
        let category = session.dominant_category.as_deref().unwrap_or("unknown");
        let apps = session.app_breakdown.as_deref().unwrap_or("{}");
        let quality = session
            .quality_score
            .map(|q| format!("{q:.0}"))
            .unwrap_or_else(|| "N/A".into());
        let switches = session.context_switches;

        let context = format!(
            "Session: {dur_mins}min, Category: {category}, Apps: {apps}, Quality: {quality}/100, Context switches: {switches}"
        );

        let (title, description) = if let Some(ref handler) = self.summary_handler {
            match handler.generate_session_summary(&context).await {
                Ok(summary) => (summary.title, summary.description),
                Err(_) => self.fallback_summary(&session),
            }
        } else {
            self.fallback_summary(&session)
        };

        // Store: tags = title, notes = description
        if let Err(e) = self
            .session_repo
            .update_summary(session_id, &title, &description)
            .await
        {
            warn!(error = %e, "Failed to store session summary for {session_id}");
        } else {
            info!(session_id = %session_id, title = %title, "session summary generated");
        }
    }

    fn fallback_summary(&self, session: &ProductivitySession) -> (String, String) {
        let title = session.fallback_title();
        let description = session.fallback_description(None).unwrap_or_default();
        (title, description)
    }
}

/// Adapter that wraps ProductivityHandler to implement AiClassifier.
struct HandlerClassifier(Arc<dyn ProductivityHandler>);

#[async_trait::async_trait]
impl super::categorization::AiClassifier for HandlerClassifier {
    async fn classify(
        &self,
        app: &str,
        title: &str,
        url: Option<&str>,
    ) -> common::Result<(String, IntelligenceSessionType)> {
        let result_str = self.0.classify_activity(app, title, url).await?;

        // Parse response — expects "category:session_type" or just "category"
        let parts: Vec<&str> = result_str.splitn(2, ':').collect();
        let category = parts[0].to_string();
        let session_type = parts
            .get(1)
            .and_then(|s| s.parse().ok())
            .unwrap_or(IntelligenceSessionType::Focus);

        Ok((category, session_type))
    }
}

/// Serialize a `#[serde(rename_all = "snake_case")]` enum to its canonical string form.
fn enum_to_snake<T: Serialize + std::fmt::Debug>(val: &T) -> String {
    serde_json::to_value(val)
        .ok()
        .and_then(|v| v.as_str().map(String::from))
        .unwrap_or_else(|| format!("{val:?}"))
}

#[cfg(test)]
mod tests {
    use super::*;
    async fn setup_pool() -> sqlx::SqlitePool {
        let pool = storage::StoragePool::connect_in_memory().await.unwrap();
        let inner = pool.inner().clone();
        storage::StoragePool::run_feature_migrations(
            &inner,
            &crate::ProductivityFeature::migrations_static(),
        )
        .await
        .unwrap();
        inner
    }

    fn make_tick(app: &str, is_idle: bool, is_context_switch: bool) -> ActivityTick {
        ActivityTick {
            timestamp: jiff::Timestamp::now(),
            app_name: app.to_string(),
            bundle_id: None,
            window_title: Some("test".to_string()),
            site_name: None,
            url: None,
            category_id: None,
            category_type: None,
            is_idle,
            idle_secs: 0.0,
            is_context_switch,
            project_id: None,
        }
    }

    #[tokio::test]
    async fn test_intelligence_layer_processes_tick() {
        let pool = setup_pool().await;
        let repos = ProductivityRepos::new(pool);
        let bus = Arc::new(DomainEventBus::new(64));
        let cancel = CancellationToken::new();

        let (tick_tx, _) = broadcast::channel(128);

        let layer =
            ProductivityIntelligenceLayer::new(&tick_tx, bus.clone(), repos, None, cancel.clone())
                .await
                .unwrap();

        let handle = layer.start();

        // Send a non-idle tick
        let tick = make_tick("Code", false, false);
        tick_tx.send(tick).unwrap();

        // Small yield to let the layer process
        tokio::time::sleep(std::time::Duration::from_millis(50)).await;

        cancel.cancel();
        handle.await.unwrap();
    }

    #[tokio::test]
    async fn test_intelligence_layer_respects_cancellation() {
        let pool = setup_pool().await;
        let repos = ProductivityRepos::new(pool);
        let bus = Arc::new(DomainEventBus::new(64));
        let cancel = CancellationToken::new();

        let (tick_tx, _) = broadcast::channel(128);

        let layer = ProductivityIntelligenceLayer::new(&tick_tx, bus, repos, None, cancel.clone())
            .await
            .unwrap();

        let handle = layer.start();

        // Cancel immediately
        cancel.cancel();

        // Should complete without hanging
        tokio::time::timeout(std::time::Duration::from_secs(2), handle)
            .await
            .expect("Layer should stop within 2s")
            .unwrap();
    }
}
