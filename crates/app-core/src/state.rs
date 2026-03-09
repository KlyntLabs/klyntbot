use std::sync::atomic::AtomicI32;
use std::sync::Arc;

use agent::{AgentLoop, PersonaManager};
use bus::{DomainEventBus, MessageBus};
use channels::ChannelManager;
use cognitive::situation::UserSituation;
use common::FormResponse;
use desktop_shared::errors::ApiError;
use desktop_shared::types::EntityKind;
use feature_coaching::{FeedbackTracker, InterventionRouter, PatternDetector, SignalAccumulator};
use feature_notes::repo::NoteRepo;
use feature_productivity::repos::ProductivityRepos;
use feature_productivity::{DailyAggregator, FocusManager, NudgeService, ProductivityEngine};
use scheduling::CronService;
use storage::Repos;
use tokio::sync::{broadcast, oneshot, Mutex, RwLock};
use tokio_util::sync::CancellationToken;
use tracing::{error, info};

/// An entity that was mutated by a handler — callers use this to emit update events.
pub struct EntityUpdate {
    pub kind: EntityKind,
    pub id: String,
}

/// Result type for mutating handlers: data + list of entities that changed.
pub type HandlerResult<T> = Result<(T, Vec<EntityUpdate>), ApiError>;

/// Central application state — embeds AgentLoop, Bus, Channels, Cron.
///
/// This struct is transport-agnostic: no Tauri, no Axum references.
/// Desktop and dev-server each wrap it with their own event wiring.
pub struct AppCore {
    pub repos: Repos,
    pub agent: Arc<AgentLoop>,
    pub bus: Arc<MessageBus>,
    pub persona_manager: Arc<RwLock<PersonaManager>>,
    pub config: RwLock<config::Config>,

    pub channel_manager: Arc<Mutex<ChannelManager>>,
    pub cron_service: Arc<CronService>,
    pub shutdown_token: CancellationToken,
    /// Active streaming cancellation tokens keyed by session_key.
    pub active_streams: Arc<dashmap::DashMap<String, CancellationToken>>,
    /// Pending ask_user interaction oneshot senders keyed by session_key.
    /// Value is (request_id, sender). Only one interaction can be pending per session
    /// because the ask_user tool blocks the agent loop until answered.
    pub pending_interactions:
        Arc<dashmap::DashMap<String, (String, oneshot::Sender<FormResponse>)>>,
    /// Notes repo (always available).
    pub note_repo: NoteRepo,
    pub productivity_repos: Option<ProductivityRepos>,
    pub focus_manager: Option<Arc<FocusManager>>,
    pub productivity_engine: Option<Arc<Mutex<ProductivityEngine>>>,
    pub aggregator: Option<Arc<DailyAggregator>>,
    pub nudge_service: Option<Arc<Mutex<NudgeService>>>,
    pub distraction_interceptor:
        Option<Arc<Mutex<feature_productivity::distraction::DistractionInterceptor>>>,
    /// Cognitive domain event bus.
    pub domain_event_bus: Option<Arc<DomainEventBus>>,
    pub signal_accumulator: Option<Arc<Mutex<SignalAccumulator>>>,
    pub pattern_detector: Option<Arc<Mutex<PatternDetector>>>,
    pub intervention_router: Option<Arc<Mutex<InterventionRouter>>>,
    pub feedback_tracker: Option<Arc<Mutex<FeedbackTracker>>>,
    pub user_situation: Option<Arc<Mutex<UserSituation>>>,
    pub coaching_service: Option<Arc<Mutex<feature_coaching::CoachingService>>>,
    /// Cognitive LLM provider — shared across reflection, cron, and status reporting.
    pub cognitive_provider: Option<providers::DynProvider>,
    /// Broadcast sender for pipeline events — allows multiple subscribers (Tauri + dev server).
    pub pipeline_broadcast: Option<broadcast::Sender<cognitive::PipelineEvent>>,
    /// Event log repo for persisting domain and pipeline events.
    pub event_log_repo: Option<cognitive::EventLogRepo>,
    /// Consecutive coaching nudges that were auto-collapsed (ignored).
    /// Resets on explicit user feedback. Delivery skipped when >= 2.
    pub consecutive_coaching_ignores: Arc<AtomicI32>,
}

impl AppCore {
    /// Return productivity repos or a "feature disabled" error.
    pub fn productivity_repos(&self) -> Result<&ProductivityRepos, ApiError> {
        self.productivity_repos
            .as_ref()
            .ok_or_else(|| ApiError::new("FEATURE_DISABLED", "productivity feature is not enabled"))
    }

    /// Return focus manager or a "feature disabled" error.
    pub fn focus_manager(&self) -> Result<&Arc<FocusManager>, ApiError> {
        self.focus_manager
            .as_ref()
            .ok_or_else(|| ApiError::new("FEATURE_DISABLED", "productivity feature is not enabled"))
    }

    /// Return daily aggregator or a "feature disabled" error.
    pub fn aggregator(&self) -> Result<&Arc<DailyAggregator>, ApiError> {
        self.aggregator
            .as_ref()
            .ok_or_else(|| ApiError::new("FEATURE_DISABLED", "productivity feature is not enabled"))
    }

    /// Return distraction interceptor or a "not available" error.
    pub fn distraction_interceptor(
        &self,
    ) -> Result<&Arc<Mutex<feature_productivity::distraction::DistractionInterceptor>>, ApiError>
    {
        self.distraction_interceptor.as_ref().ok_or_else(|| {
            ApiError::new("NOT_AVAILABLE", "Distraction interceptor not initialized")
        })
    }

    pub fn signal_accumulator(&self) -> Result<&Arc<Mutex<SignalAccumulator>>, ApiError> {
        self.signal_accumulator
            .as_ref()
            .ok_or_else(|| ApiError::new("FEATURE_DISABLED", "coaching engine is not available"))
    }

    pub fn pattern_detector(&self) -> Result<&Arc<Mutex<PatternDetector>>, ApiError> {
        self.pattern_detector
            .as_ref()
            .ok_or_else(|| ApiError::new("FEATURE_DISABLED", "coaching engine is not available"))
    }

    pub fn intervention_router(&self) -> Result<&Arc<Mutex<InterventionRouter>>, ApiError> {
        self.intervention_router
            .as_ref()
            .ok_or_else(|| ApiError::new("FEATURE_DISABLED", "coaching engine is not available"))
    }

    pub fn feedback_tracker(&self) -> Result<&Arc<Mutex<FeedbackTracker>>, ApiError> {
        self.feedback_tracker
            .as_ref()
            .ok_or_else(|| ApiError::new("FEATURE_DISABLED", "coaching engine is not available"))
    }

    pub fn user_situation(&self) -> Result<&Arc<Mutex<UserSituation>>, ApiError> {
        self.user_situation
            .as_ref()
            .ok_or_else(|| ApiError::new("FEATURE_DISABLED", "coaching engine is not available"))
    }

    pub fn domain_event_bus(&self) -> Result<&Arc<DomainEventBus>, ApiError> {
        self.domain_event_bus
            .as_ref()
            .ok_or_else(|| ApiError::new("FEATURE_DISABLED", "domain event bus is not available"))
    }

    /// Graceful shutdown.
    pub async fn shutdown(&self) {
        info!("shutting down app core");
        // Stop productivity engine first to flush pending events.
        if let Some(ref engine) = self.productivity_engine {
            engine.lock().await.stop().await;
        }
        // Stop nudge service.
        if let Some(ref nudge) = self.nudge_service {
            nudge.lock().await.stop().await;
        }
        // Persist coaching feedback before stopping the service.
        if let Some(ref tracker) = self.feedback_tracker {
            tracker.lock().await.persist().await;
        }
        // Stop coaching service.
        if let Some(ref coaching) = self.coaching_service {
            coaching.lock().await.stop().await;
        }
        if let Err(e) = self.agent.shutdown().await {
            error!("agent shutdown error: {}", e);
        }
        self.shutdown_token.cancel();
        self.cron_service.stop().await;
        info!("app core stopped");
    }
}
