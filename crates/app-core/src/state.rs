use std::sync::atomic::AtomicI32;
use std::sync::Arc;

use crate::handlers::launcher::LauncherSearchEngine;
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
use storage::{Repos, StoragePool};
use tokio::sync::{broadcast, oneshot, Mutex, RwLock};
use tokio_util::sync::CancellationToken;
use tracing::{error, info};

use crate::events::AppEventEmitter;

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
    pub mode: common::AppMode,
    pub repos: Repos,
    pub storage_pool: StoragePool,
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
    pub coaching_intervention_log_repo: Option<storage::CoachingInterventionLogRepo>,
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
    /// Unified activity log ingestion service.
    pub activity_ingestion_service: Option<Arc<activity_log::ActivityIngestionService>>,
    /// Transport-agnostic event emitter — set by the desktop adapter (Tauri events)
    /// or left as `NoopEmitter` for CLI / tests. Used by the MCP server to push
    /// entity updates to the frontend after tool mutations.
    pub event_emitter: Arc<dyn AppEventEmitter>,
    /// Note embedding handler for semantic search (None when vector store unavailable).
    pub note_embedding_handler:
        Option<Arc<dyn feature_notes::handlers::embedding::NoteEmbeddingHandler>>,
    /// Launcher search engine (None when launcher feature is disabled).
    pub launcher_engine: Option<Arc<LauncherSearchEngine>>,
    /// Proactive suggestion handler (None when tasks AI is not configured).
    pub proactive_handler: Option<Arc<dyn feature_tasks::ProactiveHandler>>,
    /// Suggestion applier handler (None when tasks AI is not configured).
    pub suggestion_applier: Option<Arc<dyn feature_tasks::SuggestionApplier>>,
    /// Decomposition handler (None when tasks AI is not configured).
    pub decomposition_handler: Option<Arc<dyn feature_tasks::DecompositionHandler>>,
    /// Forecast handler (None when tasks AI is not configured).
    pub forecast_handler: Option<Arc<dyn feature_tasks::ForecastHandler>>,
    /// Insight service for versioned insight reviews (None when cognitive feature unavailable).
    pub insight_service: Option<Arc<feature_insights::InsightService>>,
    /// Flashcard repo for FSRS spaced repetition (None when cognitive feature unavailable).
    pub flashcard_repo: Option<cognitive::FlashcardRepo>,
    /// Persona repo for Insight Review personas (None when cognitive feature unavailable).
    pub persona_repo: Option<cognitive::PersonaRepo>,
    /// Squad repo for Insight Review persona squads (None when cognitive feature unavailable).
    pub squad_repo: Option<cognitive::SquadRepo>,
    /// AutoTuner orchestrator (None when autotuner is disabled).
    pub autotuner: Option<Arc<agent::autotuner::AutoTunerOrchestrator>>,
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

    pub fn coaching_log_repo(&self) -> Result<&storage::CoachingInterventionLogRepo, ApiError> {
        self.coaching_intervention_log_repo
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

    /// Return persona repo or a "not available" error.
    pub fn persona_repo(&self) -> Result<&cognitive::PersonaRepo, ApiError> {
        self.persona_repo
            .as_ref()
            .ok_or_else(|| ApiError::new("NOT_AVAILABLE", "Persona repo not available"))
    }

    /// Return flashcard repo or a "not available" error.
    pub fn flashcard_repo(&self) -> Result<&cognitive::FlashcardRepo, ApiError> {
        self.flashcard_repo
            .as_ref()
            .ok_or_else(|| ApiError::new("NOT_AVAILABLE", "Flashcard repo not available"))
    }

    /// Return launcher search engine or a "feature disabled" error.
    pub fn launcher_engine(&self) -> Result<&Arc<LauncherSearchEngine>, ApiError> {
        self.launcher_engine
            .as_ref()
            .ok_or_else(|| ApiError::new("FEATURE_DISABLED", "launcher feature is not enabled"))
    }

    /// Return launcher clipboard repo or a "feature disabled" error.
    pub fn launcher_clipboard_repo(&self) -> Result<&feature_launcher::ClipboardRepo, ApiError> {
        self.launcher_engine
            .as_ref()
            .map(|e| &e.clipboard_repo)
            .ok_or_else(|| ApiError::new("FEATURE_DISABLED", "launcher feature is not enabled"))
    }

    /// Return autotuner orchestrator or `None` when disabled.
    pub fn autotuner_orchestrator(&self) -> Option<&agent::autotuner::AutoTunerOrchestrator> {
        self.autotuner.as_deref()
    }

    /// Return proactive handler or a "not initialized" error.
    pub fn proactive_handler(&self) -> Result<&dyn feature_tasks::ProactiveHandler, ApiError> {
        self.proactive_handler
            .as_deref()
            .ok_or_else(|| ApiError::new("INTERNAL", "ProactiveHandler not initialized"))
    }

    /// Return suggestion applier or a "not initialized" error.
    pub fn suggestion_applier(&self) -> Result<&dyn feature_tasks::SuggestionApplier, ApiError> {
        self.suggestion_applier
            .as_deref()
            .ok_or_else(|| ApiError::new("INTERNAL", "SuggestionApplier not initialized"))
    }

    /// Return decomposition handler or a "not initialized" error.
    pub fn decomposition_handler(
        &self,
    ) -> Result<&dyn feature_tasks::DecompositionHandler, ApiError> {
        self.decomposition_handler
            .as_deref()
            .ok_or_else(|| ApiError::new("INTERNAL", "DecompositionHandler not initialized"))
    }

    /// Return forecast handler or a "not initialized" error.
    pub fn forecast_handler(&self) -> Result<&dyn feature_tasks::ForecastHandler, ApiError> {
        self.forecast_handler
            .as_deref()
            .ok_or_else(|| ApiError::new("INTERNAL", "ForecastHandler not initialized"))
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
