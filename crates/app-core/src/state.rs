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
use feature_focus::DndManager;
use feature_notes::repo::{NoteRepo, PracticeSessionRepo};
use feature_productivity::repos::ProductivityRepos;
use feature_productivity::{DailyAggregator, FocusManager, NudgeService, ProductivityEngine};
use scheduling::temporal::cron_bridge::CronBridge;
use scheduling::temporal::cron_executor::CronExecutor;
use storage::{repos::cron::CronRepo, Repos, StoragePool, VectorStore};
use tokio::sync::{broadcast, oneshot, Mutex, RwLock};
use tokio_util::sync::CancellationToken;
use tracing::{error, info};
use voice_engine::VoiceService;

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
    pub config: Arc<RwLock<config::Config>>,
    /// Shared hot-reloadable config subset — updated by file watcher and settings handlers.
    pub hot_config: Arc<RwLock<config::HotConfig>>,

    pub channel_manager: Arc<Mutex<ChannelManager>>,
    pub cron_executor: Arc<CronExecutor>,
    /// Direct SQL repo for cron job CRUD — used by handlers and the CronTool adapter.
    pub cron_repo: CronRepo,
    /// Bridge that syncs `cron_jobs` rows into `scheduled_fires` after mutations.
    pub cron_bridge: Arc<CronBridge>,
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
    /// Practice session repo (always available — backed by the same DB as notes).
    pub practice_repo: PracticeSessionRepo,
    pub productivity_repos: Option<ProductivityRepos>,
    /// Productivity (Pomodoro) focus manager — distinct from DND sessions.
    pub focus_manager: Option<Arc<FocusManager>>,
    /// DND session manager — controls timed Do-Not-Disturb sessions.
    pub dnd_manager: Option<Arc<DndManager>>,
    /// Background task that auto-deactivates DND sessions when the scheduled alarm fires.
    pub _dnd_end_subscriber_handle: Option<tokio::task::JoinHandle<()>>,
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
    /// Shared active desktop view for query rewriting context.
    pub active_view: Option<Arc<RwLock<Option<context_engine::ActiveView>>>>,
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
    /// Embedding engine for on-demand text embedding (shared across embedding tasks).
    pub embedding_engine: Option<Arc<tools::embedding_engine::EmbeddingEngine>>,
    /// LanceDB vector store for semantic similarity search (None when unavailable).
    pub vector_store: Option<VectorStore>,
    /// Launcher search engine (None when launcher feature is disabled).
    pub launcher_engine: Option<Arc<LauncherSearchEngine>>,
    /// Insight service for versioned insight reviews (None when cognitive feature unavailable).
    pub insight_service: Option<Arc<feature_insights::InsightService>>,
    /// Flashcard repo for FSRS spaced repetition (None when cognitive feature unavailable).
    pub flashcard_repo: Option<cognitive::FlashcardRepo>,
    /// Knowledge atom repo (None when cognitive feature unavailable).
    pub knowledge_atom_repo: Option<cognitive::KnowledgeAtomRepo>,
    /// Persona repo for Insight Review personas (None when cognitive feature unavailable).
    pub persona_repo: Option<cognitive::PersonaRepo>,
    /// Squad repo for Insight Review persona squads (None when cognitive feature unavailable).
    pub squad_repo: Option<cognitive::SquadRepo>,
    /// Review session repo for active recall sessions (None when cognitive feature unavailable).
    pub review_session_repo: Option<cognitive::ReviewSessionRepo>,
    /// Deck preference repo for per-deck answer mode settings (None when cognitive feature unavailable).
    pub deck_preference_repo: Option<cognitive::DeckPreferenceRepo>,
    /// AutoTuner orchestrator (None when autotuner is disabled).
    pub autotuner: Option<Arc<agent::autotuner::AutoTunerOrchestrator>>,
    /// Unified TemporalScheduler — sole firing source post-4.4c.
    pub temporal_scheduler: Option<scheduling::temporal::TemporalScheduler>,
    /// Join handle for the TemporalScheduler background loop.
    pub _temporal_scheduler_handle: Option<tokio::task::JoinHandle<()>>,
    /// Join handle for the SystemDidWake → scheduler.wake() subscriber.
    pub _temporal_wake_subscriber: Option<tokio::task::JoinHandle<()>>,
    /// Mirror self-reflection facade (None when cognitive provider is unavailable).
    pub mirror_facade: Option<Arc<cognitive::mirror::MirrorFacade>>,
    /// Pending memory repo for user-confirmable facts (None when cognitive unavailable).
    pub pending_memory_repo: Option<cognitive::repos::PendingMemoryRepo>,
    /// Join handles for MirrorEngine background subscribers — kept alive for app lifetime.
    pub _mirror_handles: Option<Vec<tokio::task::JoinHandle<()>>>,
    /// Cancellation token for the MirrorEngine background subscribers.
    pub _mirror_shutdown: Option<CancellationToken>,
    /// Phase-3 NotificationDispatcher handle (kept alive for app lifetime).
    pub notification_dispatcher_handle: Option<notifications::NotificationDispatcherHandle>,
    /// Cancellation token for the config file watcher background service.
    pub _config_watcher_token: Option<CancellationToken>,
    /// Lifecycle monitor handle (macOS sleep/wake + idle detection).
    pub _lifecycle_monitor: Option<platform_macos::lifecycle::LifecycleMonitor>,
    /// Wake orchestrator background task handle.
    pub _wake_orchestrator_handle: Option<tokio::task::JoinHandle<()>>,
    /// Voice capture service (None when voice feature is disabled).
    pub voice_service: Option<Arc<VoiceService>>,
    /// Voice conversation manager (None when voice feature is disabled).
    pub voice_conversation_manager:
        Option<Arc<crate::handlers::voice_conversation::VoiceConversationManager>>,
    /// Background task handle for the voice conversation loop.
    pub voice_loop_handle: Option<tokio::task::JoinHandle<()>>,
    /// BrainVoice signal router (None when domain event bus is unavailable).
    pub brain_voice: Option<crate::brain_voice::BrainVoice>,
    /// Onboarding journey milestone tracker.
    pub journey_tracker: Option<crate::journey::JourneyTracker>,
    /// AI pipeline SignalRouter — keeps the router alive for the app lifetime.
    pub _ai_pipeline_router: Option<ai_core::SignalRouter>,
    /// Registry of all AiFeature-derived features in the workspace.
    pub feature_registry: Arc<ai_core::AiFeatureRegistry>,
    /// Ingestion daemon handle; `None` when spawn failed or not yet wired.
    pub ingest_daemon: std::sync::Mutex<Option<coding_ingest::daemon::IngestDaemonHandle>>,
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

    /// Return DND manager or a "feature disabled" error.
    pub fn dnd_manager(&self) -> Result<&Arc<DndManager>, ApiError> {
        self.dnd_manager
            .as_ref()
            .ok_or_else(|| ApiError::new("FEATURE_DISABLED", "focus (DND) feature is not enabled"))
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

    /// Return knowledge atom repo or a "not available" error.
    pub fn knowledge_atom_repo(&self) -> Result<&cognitive::KnowledgeAtomRepo, ApiError> {
        self.knowledge_atom_repo
            .as_ref()
            .ok_or_else(|| ApiError::new("NOT_AVAILABLE", "Knowledge atom repo not available"))
    }

    /// Return practice session repo.
    pub fn practice_repo(&self) -> &PracticeSessionRepo {
        &self.practice_repo
    }

    /// Return review session repo or a "not available" error.
    pub fn review_session_repo(&self) -> Result<&cognitive::ReviewSessionRepo, ApiError> {
        self.review_session_repo
            .as_ref()
            .ok_or_else(|| ApiError::new("NOT_AVAILABLE", "Review session repo not available"))
    }

    /// Return deck preference repo or a "not available" error.
    pub fn deck_preference_repo(&self) -> Result<&cognitive::DeckPreferenceRepo, ApiError> {
        self.deck_preference_repo
            .as_ref()
            .ok_or_else(|| ApiError::new("NOT_AVAILABLE", "Deck preference repo not available"))
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

    /// Return mirror facade or a "not available" error.
    pub fn mirror_facade(&self) -> Result<&cognitive::mirror::MirrorFacade, ApiError> {
        self.mirror_facade
            .as_deref()
            .ok_or_else(|| ApiError::new("NOT_AVAILABLE", "Mirror facade not available"))
    }

    /// Return pending memory repo or a "not available" error.
    pub fn pending_memory_repo(&self) -> Result<&cognitive::repos::PendingMemoryRepo, ApiError> {
        self.pending_memory_repo
            .as_ref()
            .ok_or_else(|| ApiError::new("NOT_AVAILABLE", "Pending memory repo not available"))
    }

    /// Approve a pending memory: deserialize fact, upsert to semantic_facts, remove from pending.
    pub async fn approve_pending_memory(&self, id: &str) -> Result<(), ApiError> {
        let repo = self.pending_memory_repo()?;
        let row = repo
            .get(id)
            .await
            .map_err(|e| ApiError::new("DB_ERROR", format!("failed to fetch pending memory: {e}")))?
            .ok_or_else(|| {
                ApiError::new("NOT_FOUND", format!("pending memory '{id}' not found"))
            })?;

        let fact: cognitive::types::SemanticFact =
            serde_json::from_str(&row.fact_json).map_err(|e| {
                ApiError::new("INVALID_DATA", format!("failed to deserialize fact: {e}"))
            })?;

        let fact_repo = cognitive::repos::SemanticFactRepo::new(self.storage_pool.inner().clone());
        fact_repo
            .upsert(&fact)
            .await
            .map_err(|e| ApiError::new("DB_ERROR", format!("failed to upsert fact: {e}")))?;

        repo.remove(id).await.map_err(|e| {
            ApiError::new("DB_ERROR", format!("failed to remove pending memory: {e}"))
        })?;

        Ok(())
    }

    /// Dismiss a pending memory (discard without persisting the fact).
    pub async fn dismiss_pending_memory(&self, id: &str) -> Result<(), ApiError> {
        let repo = self.pending_memory_repo()?;
        repo.remove(id).await.map_err(|e| {
            ApiError::new("DB_ERROR", format!("failed to remove pending memory: {e}"))
        })?;
        Ok(())
    }

    /// Return voice service or a "feature disabled" error.
    pub fn voice_service(&self) -> Result<&Arc<VoiceService>, ApiError> {
        self.voice_service
            .as_ref()
            .ok_or_else(|| ApiError::new("FEATURE_DISABLED", "voice feature is not enabled"))
    }

    /// Return voice conversation manager or a "not available" error.
    pub fn voice_conversation_manager(
        &self,
    ) -> Result<&Arc<crate::handlers::voice_conversation::VoiceConversationManager>, ApiError> {
        self.voice_conversation_manager.as_ref().ok_or_else(|| {
            ApiError::new("VOICE_NOT_AVAILABLE", "Voice conversation not initialized")
        })
    }

    /// Cross-domain check from string domain name (used by Tauri commands).
    pub async fn check_cross_domain_str(
        &self,
        domain: &str,
        id: &str,
        title: &str,
        created_at_str: Option<&str>,
    ) {
        use feature_insights::cross_domain::EntityDomain;
        let entity_domain = match domain {
            "task" => EntityDomain::Task,
            "note" => EntityDomain::Note,
            "finance" => EntityDomain::Finance,
            _ => return,
        };
        self.check_cross_domain(entity_domain, id, title, created_at_str)
            .await;
    }

    /// Fire-and-forget cross-domain check when viewing any entity detail.
    pub async fn check_cross_domain(
        &self,
        domain: feature_insights::cross_domain::EntityDomain,
        id: &str,
        title: &str,
        created_at_str: Option<&str>,
    ) {
        if let Some(ref svc) = self.insight_service {
            let created_at = created_at_str
                .and_then(|s| s.parse::<jiff::Timestamp>().ok())
                .unwrap_or_else(jiff::Timestamp::now);
            svc.check_cross_domain(domain, id.to_string(), title.to_string(), created_at)
                .await;
        }
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
        // Stop BrainVoice signal router.
        if let Some(ref bv) = self.brain_voice {
            bv.shutdown();
        }
        if let Err(e) = self.agent.shutdown().await {
            error!("agent shutdown error: {}", e);
        }
        // Cancel mirror subscribers before the main shutdown token
        // so they stop consuming domain events immediately.
        if let Some(ref token) = self._mirror_shutdown {
            token.cancel();
        }
        // Stop the NotificationDispatcher select loop.
        if let Some(ref handle) = self.notification_dispatcher_handle {
            handle.shutdown.cancel();
        }
        // Abort the voice conversation loop if still running.
        if let Some(ref handle) = self.voice_loop_handle {
            handle.abort();
        }
        self.shutdown_token.cancel();
        if let Err(e) = self.storage_pool.optimize().await {
            tracing::warn!("SQLite PRAGMA optimize failed: {e}");
        }
        info!("app core stopped");
    }
}
