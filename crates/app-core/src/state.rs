use std::sync::atomic::AtomicI32;
use std::sync::Arc;

use crate::handlers::launcher::LauncherSearchEngine;
use agent::AgentLoop;
use bus::{DomainEventBus, MessageBus};
use channels::ChannelManager;
use cognitive::situation::UserSituation;
use common::FormResponse;
use desktop_shared::errors::ApiError;
use desktop_shared::types::EntityKind;
use desktop_shared::HooksTomlSnapshot;
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
    /// Phase 4 polling fallback. Held forever so the watcher runs for the
    /// process lifetime; cancelled implicitly on `AppCore` drop.
    pub _data_version_watcher_token: Option<CancellationToken>,
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
    /// Coding-memory Distiller — processes ingest events into semantic facts & episodic memories.
    pub distiller: Option<Arc<coding_memory::distiller::Distiller>>,
    /// Coding-memory recall service (Phase 4).
    pub recall: Option<Arc<coding_memory::recall::CodingRecallService>>,
    /// MCP toolset for coding-memory recall tools.
    pub coding_toolset: Option<coding_memory::CodingMemoryToolset>,
    /// Phase-5 session-end light pass.
    pub session_end_pass: Option<Arc<coding_memory::reforge::SessionEndPass>>,
    /// Causal edge repo (Phase 6).
    pub causal_edge_repo: Option<Arc<coding_memory::causal::CausalEdgeRepo>>,
    /// Tree-sitter symbol extractor (Phase 6).
    pub symbol_extractor: Option<Arc<dyn coding_memory::symbols::SymbolExtractor>>,
    /// Map of repo_id → filesystem root (Phase 6 symbol validation).
    pub repo_roots: Arc<std::sync::RwLock<std::collections::HashMap<String, std::path::PathBuf>>>,
    pub tracing_registry: std::sync::Arc<crate::tracing::TracingRegistry>,
    /// Tracks which sessions have already fired the SessionStart hook (coding mode).
    pub session_start_fired: Arc<dashmap::DashMap<String, ()>>,
    /// Tracks which sessions have already fired the SessionEnd hook (prevents double-fire on cancel).
    pub session_end_fired: Arc<dashmap::DashMap<String, ()>>,
    /// Skill activator for coding mode — path-conditional + dynamic discovery.
    pub coding_skill_activator: Arc<tokio::sync::Mutex<Option<klynt_skill_loader::SkillActivator>>>,
    /// Mirror-learned approval history repo (Phase 2 Layer 3).
    pub coding_approval_history_repo: Option<Arc<storage::repos::CodingApprovalHistoryRepo>>,
    /// File snapshot repo for /sessions rewind (Phase 2).
    pub snapshot_repo: Option<Arc<klynt_core::snapshots::SnapshotRepo>>,
    // ── Phase 4: Coding thread events ─────────────────────────────────
    /// Typed broker for ThreadEvent — publish from agent loop, subscribe from Tauri adapter.
    pub thread_events: bus::TypedBroker<desktop_shared::coding::ThreadEvent>,
    /// Typed broker for CostUpdate — publish after each provider call.
    pub cost_events: bus::TypedBroker<desktop_shared::coding::CostUpdate>,
    /// Typed broker for SubagentEvent — publish from subagent manager, subscribe from Tauri.
    pub subagent_events: bus::TypedBroker<desktop_shared::coding::SubagentEvent>,
    /// Active thread subscriptions keyed by subscription_id.
    pub thread_subscriptions: Arc<dashmap::DashMap<String, ThreadSubscription>>,
    /// Per-turn steer queue — accepts mid-turn user corrections injected via
    /// `coding_turn_steer`. The turn handler drains the receiver between
    /// iterations and persists each entry as a synthetic user message so the
    /// next iteration's prompt assembly picks it up.
    pub steer_queue: Arc<crate::coding::steer_queue::SteerQueue>,
    /// Coding ToolKitBuilder — set by init/mod.rs after the kit is registered with
    /// the agent runtime. Cloned on demand by handlers that need to construct a
    /// scoped (e.g. read-only) registry — currently `coding_review_start`.
    pub tool_kit: std::sync::Mutex<Option<Arc<klynt_core::ToolKitBuilder>>>,
    /// Desktop approval channel — shared with the agent's ApprovalGate.
    /// Used by `respond_approval` to resolve pending requests.
    pub desktop_approval_channel:
        Option<Arc<crate::desktop_approval_channel::DesktopApprovalChannel>>,
    /// Approval grants repo — shared with the agent's ApprovalGate.
    pub approval_grants_repo: Option<Arc<approval::ApprovalGrantsRepo>>,
    /// Per-coding-thread approval policy. PlanMode variant is set/cleared by
    /// coding_plan_enter / coding_plan_cancel / coding_plan_ratify.
    pub coding_policies:
        Arc<dashmap::DashMap<String, Arc<parking_lot::RwLock<approval::CodingApprovalPolicy>>>>,
    /// Snapshot of items at the moment plan mode was entered, used to compute
    /// ratify counts. Keyed by plan_session_id. In-memory only.
    pub plan_snapshots: Arc<dashmap::DashMap<String, Vec<feature_coding_todo::types::TodoItem>>>,
    /// Shared context update queue for injecting one-shot reminders into the agent loop.
    pub context_update_queue: Option<Arc<bus::ContextUpdateQueue>>,
}

/// State for an active thread subscription.
#[derive(Debug, Clone)]
pub struct ThreadSubscription {
    pub thread_id: String,
    pub created_at: i64,
}

impl AppCore {
    /// Return productivity repos or a "feature disabled" error.
    #[tracing::instrument(skip(self), err)]
    pub fn productivity_repos(&self) -> Result<&ProductivityRepos, ApiError> {
        self.productivity_repos
            .as_ref()
            .ok_or_else(|| ApiError::new("FEATURE_DISABLED", "productivity feature is not enabled"))
    }

    /// Return focus manager or a "feature disabled" error.
    #[tracing::instrument(skip(self), err)]
    pub fn focus_manager(&self) -> Result<&Arc<FocusManager>, ApiError> {
        self.focus_manager
            .as_ref()
            .ok_or_else(|| ApiError::new("FEATURE_DISABLED", "productivity feature is not enabled"))
    }

    /// Return DND manager or a "feature disabled" error.
    #[tracing::instrument(skip(self), err)]
    pub fn dnd_manager(&self) -> Result<&Arc<DndManager>, ApiError> {
        self.dnd_manager
            .as_ref()
            .ok_or_else(|| ApiError::new("FEATURE_DISABLED", "focus (DND) feature is not enabled"))
    }

    /// Return daily aggregator or a "feature disabled" error.
    #[tracing::instrument(skip(self), err)]
    pub fn aggregator(&self) -> Result<&Arc<DailyAggregator>, ApiError> {
        self.aggregator
            .as_ref()
            .ok_or_else(|| ApiError::new("FEATURE_DISABLED", "productivity feature is not enabled"))
    }

    /// Return distraction interceptor or a "not available" error.
    #[tracing::instrument(skip(self), err)]
    pub fn distraction_interceptor(
        &self,
    ) -> Result<&Arc<Mutex<feature_productivity::distraction::DistractionInterceptor>>, ApiError>
    {
        self.distraction_interceptor.as_ref().ok_or_else(|| {
            ApiError::new("NOT_AVAILABLE", "Distraction interceptor not initialized")
        })
    }

    #[tracing::instrument(skip(self), err)]
    pub fn signal_accumulator(&self) -> Result<&Arc<Mutex<SignalAccumulator>>, ApiError> {
        self.signal_accumulator
            .as_ref()
            .ok_or_else(|| ApiError::new("FEATURE_DISABLED", "coaching engine is not available"))
    }

    #[tracing::instrument(skip(self), err)]
    pub fn pattern_detector(&self) -> Result<&Arc<Mutex<PatternDetector>>, ApiError> {
        self.pattern_detector
            .as_ref()
            .ok_or_else(|| ApiError::new("FEATURE_DISABLED", "coaching engine is not available"))
    }

    #[tracing::instrument(skip(self), err)]
    pub fn intervention_router(&self) -> Result<&Arc<Mutex<InterventionRouter>>, ApiError> {
        self.intervention_router
            .as_ref()
            .ok_or_else(|| ApiError::new("FEATURE_DISABLED", "coaching engine is not available"))
    }

    #[tracing::instrument(skip(self), err)]
    pub fn feedback_tracker(&self) -> Result<&Arc<Mutex<FeedbackTracker>>, ApiError> {
        self.feedback_tracker
            .as_ref()
            .ok_or_else(|| ApiError::new("FEATURE_DISABLED", "coaching engine is not available"))
    }

    #[tracing::instrument(skip(self), err)]
    pub fn coaching_log_repo(&self) -> Result<&storage::CoachingInterventionLogRepo, ApiError> {
        self.coaching_intervention_log_repo
            .as_ref()
            .ok_or_else(|| ApiError::new("FEATURE_DISABLED", "coaching engine is not available"))
    }

    #[tracing::instrument(skip(self), err)]
    pub fn user_situation(&self) -> Result<&Arc<Mutex<UserSituation>>, ApiError> {
        self.user_situation
            .as_ref()
            .ok_or_else(|| ApiError::new("FEATURE_DISABLED", "coaching engine is not available"))
    }

    #[tracing::instrument(skip(self), err)]
    pub fn domain_event_bus(&self) -> Result<&Arc<DomainEventBus>, ApiError> {
        self.domain_event_bus
            .as_ref()
            .ok_or_else(|| ApiError::new("FEATURE_DISABLED", "domain event bus is not available"))
    }

    /// Return flashcard repo or a "not available" error.
    #[tracing::instrument(skip(self), err)]
    pub fn flashcard_repo(&self) -> Result<&cognitive::FlashcardRepo, ApiError> {
        self.flashcard_repo
            .as_ref()
            .ok_or_else(|| ApiError::new("NOT_AVAILABLE", "Flashcard repo not available"))
    }

    /// Return knowledge atom repo or a "not available" error.
    #[tracing::instrument(skip(self), err)]
    pub fn knowledge_atom_repo(&self) -> Result<&cognitive::KnowledgeAtomRepo, ApiError> {
        self.knowledge_atom_repo
            .as_ref()
            .ok_or_else(|| ApiError::new("NOT_AVAILABLE", "Knowledge atom repo not available"))
    }

    /// Return practice session repo.
    #[tracing::instrument(skip(self))]
    pub fn practice_repo(&self) -> &PracticeSessionRepo {
        &self.practice_repo
    }

    /// Return review session repo or a "not available" error.
    #[tracing::instrument(skip(self), err)]
    pub fn review_session_repo(&self) -> Result<&cognitive::ReviewSessionRepo, ApiError> {
        self.review_session_repo
            .as_ref()
            .ok_or_else(|| ApiError::new("NOT_AVAILABLE", "Review session repo not available"))
    }

    /// Return deck preference repo or a "not available" error.
    #[tracing::instrument(skip(self), err)]
    pub fn deck_preference_repo(&self) -> Result<&cognitive::DeckPreferenceRepo, ApiError> {
        self.deck_preference_repo
            .as_ref()
            .ok_or_else(|| ApiError::new("NOT_AVAILABLE", "Deck preference repo not available"))
    }

    /// Return launcher search engine or a "feature disabled" error.
    #[tracing::instrument(skip(self), err)]
    pub fn launcher_engine(&self) -> Result<&Arc<LauncherSearchEngine>, ApiError> {
        self.launcher_engine
            .as_ref()
            .ok_or_else(|| ApiError::new("FEATURE_DISABLED", "launcher feature is not enabled"))
    }

    /// Return launcher clipboard repo or a "feature disabled" error.
    #[tracing::instrument(skip(self), err)]
    pub fn launcher_clipboard_repo(&self) -> Result<&feature_launcher::ClipboardRepo, ApiError> {
        self.launcher_engine
            .as_ref()
            .map(|e| e.clipboard_repo.as_ref())
            .ok_or_else(|| ApiError::new("FEATURE_DISABLED", "launcher feature is not enabled"))
    }

    /// Return autotuner orchestrator or `None` when disabled.
    #[tracing::instrument(skip(self))]
    pub fn autotuner_orchestrator(&self) -> Option<&agent::autotuner::AutoTunerOrchestrator> {
        self.autotuner.as_deref()
    }

    /// Return mirror facade or a "not available" error.
    #[tracing::instrument(skip(self), err)]
    pub fn mirror_facade(&self) -> Result<&cognitive::mirror::MirrorFacade, ApiError> {
        self.mirror_facade
            .as_deref()
            .ok_or_else(|| ApiError::new("NOT_AVAILABLE", "Mirror facade not available"))
    }

    /// Return pending memory repo or a "not available" error.
    #[tracing::instrument(skip(self), err)]
    pub fn pending_memory_repo(&self) -> Result<&cognitive::repos::PendingMemoryRepo, ApiError> {
        self.pending_memory_repo
            .as_ref()
            .ok_or_else(|| ApiError::new("NOT_AVAILABLE", "Pending memory repo not available"))
    }

    pub fn tracing_registry(&self) -> &std::sync::Arc<crate::tracing::TracingRegistry> {
        &self.tracing_registry
    }

    /// Approve a pending memory: deserialize fact, upsert to semantic_facts, remove from pending.
    #[tracing::instrument(skip(self), err)]
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
    #[tracing::instrument(skip(self), err)]
    pub async fn dismiss_pending_memory(&self, id: &str) -> Result<(), ApiError> {
        let repo = self.pending_memory_repo()?;
        repo.remove(id).await.map_err(|e| {
            ApiError::new("DB_ERROR", format!("failed to remove pending memory: {e}"))
        })?;
        Ok(())
    }

    /// Return voice service or a "feature disabled" error.
    #[tracing::instrument(skip(self), err)]
    pub fn voice_service(&self) -> Result<&Arc<VoiceService>, ApiError> {
        self.voice_service
            .as_ref()
            .ok_or_else(|| ApiError::new("FEATURE_DISABLED", "voice feature is not enabled"))
    }

    /// Return voice conversation manager or a "not available" error.
    #[tracing::instrument(skip(self), err)]
    pub fn voice_conversation_manager(
        &self,
    ) -> Result<&Arc<crate::handlers::voice_conversation::VoiceConversationManager>, ApiError> {
        self.voice_conversation_manager.as_ref().ok_or_else(|| {
            ApiError::new("VOICE_NOT_AVAILABLE", "Voice conversation not initialized")
        })
    }

    /// Cross-domain check from string domain name (used by Tauri commands).
    #[tracing::instrument(skip(self))]
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
    #[tracing::instrument(skip(self))]
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
    #[tracing::instrument(skip(self))]
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

    #[tracing::instrument(skip(self), err)]
    pub async fn chat_save_starlark_rule(
        &self,
        _request_id: String,
        rule_source: String,
        suggested_filename: Option<String>,
    ) -> common::Result<String> {
        let _ = klynt_execpolicy::parse_to_policy(
            &rule_source,
            std::path::Path::new("inline.rules"),
        )
        .map_err(|e| common::KlyntbotError::NotImplemented(format!("invalid Starlark: {e}")))?;

        let rules_dir = self.config.read().await.data_dir_path().join("rules");
        tokio::fs::create_dir_all(&rules_dir)
            .await
            .map_err(common::KlyntbotError::Io)?;

        let filename = suggested_filename.unwrap_or_else(|| {
            let now = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs();
            format!("rule-{now}.rules")
        });
        let path = rules_dir.join(filename);
        tokio::fs::write(&path, &rule_source)
            .await
            .map_err(common::KlyntbotError::Io)?;

        Ok(path.to_string_lossy().into_owned())
    }

    /// Respond to a pending approval request — resolves the oneshot that the
    /// agent's tool execution is blocked on.
    #[tracing::instrument(skip(self), err)]
    pub async fn respond_approval(
        &self,
        request_id: &str,
        decision: crate::coding::approval_handler::AppApprovalDecision,
    ) -> common::Result<()> {
        let channel = self.desktop_approval_channel.as_ref().ok_or_else(|| {
            common::KlyntbotError::NotImplemented("desktop approval channel not initialized".into())
        })?;
        let grants_repo = self.approval_grants_repo.as_ref().ok_or_else(|| {
            common::KlyntbotError::NotImplemented("approval grants repo not initialized".into())
        })?;
        crate::coding::approval_handler::respond_approval(
            channel.clone(),
            grants_repo.clone(),
            self.domain_event_bus.clone(),
            request_id,
            decision,
        )
        .await
        .map_err(|e| common::KlyntbotError::NotImplemented(e.to_string()))
    }

    #[tracing::instrument(skip(self), err)]
    pub async fn coding_hooks_list(&self) -> common::Result<HooksTomlSnapshot> {
        let path = self.config.read().await.data_dir_path().join("hooks.toml");
        let content = tokio::fs::read_to_string(&path).await.unwrap_or_default();
        let exists = !content.is_empty();
        Ok(HooksTomlSnapshot {
            path: path.to_string_lossy().into_owned(),
            exists,
            content,
        })
    }

    #[tracing::instrument(skip(self), err)]
    pub async fn coding_permissions_clear_mirror(
        &self,
        tool: String,
        repo_id: Option<String>,
    ) -> common::Result<u64> {
        let repo = self.coding_approval_history_repo.clone().ok_or_else(|| {
            common::KlyntbotError::Storage("approval history repo not initialized".into())
        })?;
        repo.clear_for_tool(&tool, repo_id.as_deref())
            .await
            .map_err(Into::into)
    }

    #[tracing::instrument(skip(self), err)]
    pub async fn coding_sessions_rewind(
        &self,
        session_key: String,
        message_id: String,
    ) -> common::Result<desktop_shared::RewindResult> {
        let snap_repo = self.snapshot_repo.clone().ok_or_else(|| {
            common::KlyntbotError::Storage("snapshot repo not initialized".into())
        })?;
        let snaps: Vec<klynt_core::snapshots::Snapshot> = snap_repo
            .list_after_message(&session_key, &message_id)
            .await?;
        let mut restored: usize = 0;
        let mut deleted: usize = 0;
        // Apply newest-first to undo in reverse order
        for snap in snaps.iter().rev() {
            if snap.file_existed {
                tokio::fs::write(&snap.file_path, &snap.content_before).await?;
                restored += 1;
            } else {
                // file didn't exist before — undo by deleting
                let _ = tokio::fs::remove_file(&snap.file_path).await;
                deleted += 1;
            }
        }
        let removed = self
            .repos
            .sessions
            .rewind_to_message(&session_key, &message_id)
            .await?;
        Ok(desktop_shared::RewindResult {
            messages_removed: removed,
            files_restored: restored,
            files_deleted: deleted,
        })
    }

    #[tracing::instrument(skip(self), err)]
    pub async fn coding_sessions_export(
        &self,
        session_key: String,
        format: desktop_shared::ExportFormat,
    ) -> common::Result<desktop_shared::SessionExportResult> {
        let bytes = match format {
            desktop_shared::ExportFormat::Md => {
                self.repos.sessions.export_session_md(&session_key).await?
            }
            desktop_shared::ExportFormat::Json => {
                self.repos
                    .sessions
                    .export_session_json(&session_key)
                    .await?
            }
        };
        let dir = self.config.read().await.data_dir_path().join("exports");
        tokio::fs::create_dir_all(&dir).await?;
        let ext = match format {
            desktop_shared::ExportFormat::Md => "md",
            desktop_shared::ExportFormat::Json => "json",
        };
        let path = dir.join(format!("{session_key}.{ext}"));
        tokio::fs::write(&path, &bytes).await?;
        Ok(desktop_shared::SessionExportResult {
            path: path.to_string_lossy().into_owned(),
            bytes_written: bytes.len(),
        })
    }

    #[tracing::instrument(skip(self), err)]
    pub async fn coding_sessions_fork(
        &self,
        session_key: String,
        up_to_message: Option<String>,
    ) -> common::Result<desktop_shared::SessionForkResult> {
        let new_key = self
            .repos
            .sessions
            .fork_session(&session_key, up_to_message.as_deref())
            .await?;
        Ok(desktop_shared::SessionForkResult {
            new_session_key: new_key,
        })
    }

    // ── Workspace lifecycle (Cursor/Codex-style "open folder") ────────────
    //
    // Workspaces are registered folders on disk. The `id` UUID flows into
    // `sessions.repo_id`, `coding_approval_history.repo_id`, and `GuardCtx.repo_id`
    // (Phase 2). `project_id` (optional) links to a Klyntbot organizational
    // project; null for one-off folders.

    #[tracing::instrument(skip(self), err)]
    pub async fn list_workspaces(&self) -> common::Result<serde_json::Value> {
        let rows = self.repos.workspaces.list_all().await?;
        let dtos: Vec<_> = rows.into_iter().map(workspace_row_to_dto).collect();
        Ok(serde_json::Value::Array(dtos))
    }

    #[tracing::instrument(skip(self), err)]
    pub async fn add_workspace(&self, path: String) -> common::Result<serde_json::Value> {
        let abs = std::path::PathBuf::from(&path);
        let canonical = tokio::fs::canonicalize(&abs).await.map_err(|e| {
            common::KlyntbotError::Storage(format!("invalid workspace path '{path}': {e}"))
        })?;
        let meta = tokio::fs::metadata(&canonical).await.map_err(|e| {
            common::KlyntbotError::Storage(format!("workspace path stat failed: {e}"))
        })?;
        if !meta.is_dir() {
            return Err(common::KlyntbotError::Storage(format!(
                "workspace path is not a directory: {path}"
            )));
        }
        let path_str = canonical.to_string_lossy().into_owned();
        if let Some(existing) = self.repos.workspaces.get_by_path(&path_str).await? {
            return Ok(workspace_row_to_dto(existing));
        }
        let name = canonical
            .file_name()
            .and_then(|s| s.to_str())
            .unwrap_or("workspace")
            .to_string();
        let id = format!("ws-{}", uuid::Uuid::new_v4());
        let row = self
            .repos
            .workspaces
            .insert(storage::repos::NewWorkspace {
                id: &id,
                name: &name,
                path: &path_str,
                kind: "main",
                parent_id: None,
                project_id: None,
                settings_json: "{}",
            })
            .await?;
        Ok(workspace_row_to_dto(row))
    }

    /// Resolve the cognitive LLM provider and chat parameters.
    /// Returns `Err(ApiError::NOT_AVAILABLE)` when no provider is configured.
    pub async fn cognitive_chat_context(
        &self,
        max_tokens: u32,
    ) -> Result<(providers::DynProvider, providers::ChatParams), ApiError> {
        let provider = self
            .cognitive_provider
            .clone()
            .ok_or_else(|| ApiError::new("NOT_AVAILABLE", "LLM provider not configured"))?;
        let config = self.config.read().await;
        let params = providers::cognitive_chat_params(&config, max_tokens);
        Ok((provider, params))
    }

    #[tracing::instrument(skip(self), err)]
    pub async fn is_workspace_path_dir(&self, path: String) -> common::Result<bool> {
        match tokio::fs::metadata(&path).await {
            Ok(m) => Ok(m.is_dir()),
            Err(_) => Ok(false),
        }
    }

    #[tracing::instrument(skip(self), err)]
    pub async fn remove_workspace(&self, id: String) -> common::Result<()> {
        self.repos.workspaces.remove(&id).await?;
        Ok(())
    }

    #[tracing::instrument(skip(self), err)]
    pub async fn connect_workspace(&self, id: String) -> common::Result<()> {
        self.repos.workspaces.set_connected(&id, true).await?;
        Ok(())
    }
}

fn workspace_row_to_dto(row: storage::repos::WorkspaceRow) -> serde_json::Value {
    let settings: serde_json::Value =
        serde_json::from_str(&row.settings).unwrap_or_else(|_| serde_json::json!({}));
    serde_json::json!({
        "id": row.id,
        "name": row.name,
        "path": row.path,
        "connected": row.connected != 0,
        "kind": row.kind,
        "parentId": row.parent_id,
        "projectId": row.project_id,
        "settings": settings,
    })
}

impl AppCore {
    /// Phase 3: trigger graph consolidation over the active fact set.
    ///
    /// Composes existing primitives — semantic fact repo, entity repo,
    /// LLM-backed graph link handler — into a one-shot consolidation
    /// pass. Bench callers fire this between ingest and QA;
    /// production callers fire it on a fact-counter threshold.
    /// Returns the number of facts processed.
    #[tracing::instrument(skip(self), err)]
    pub async fn trigger_graph_consolidation(&self) -> common::Result<u32> {
        use std::sync::Arc;
        let provider = match self.cognitive_provider.clone() {
            Some(p) => p,
            None => {
                tracing::warn!("graph_consolidation: no cognitive provider, skipping");
                return Ok(0);
            }
        };
        let cfg_guard = self.config.read().await;
        let params = providers::cognitive_chat_params(&cfg_guard, 1024);
        drop(cfg_guard);
        let handler: Arc<dyn cognitive::services::graph_linker::GraphLinkHandler> = Arc::new(
            agent::cognitive_handlers::LlmGraphLinkHandler::new(provider, params),
        );
        let fact_repo = cognitive::repos::SemanticFactRepo::new(self.storage_pool.inner().clone());
        let entity_repo = cognitive::repos::EntityRepo::new(self.storage_pool.inner().clone());
        let count = cognitive::services::background::run_graph_consolidation(
            &fact_repo,
            &entity_repo,
            handler,
            500,
        )
        .await;
        Ok(count)
    }

    /// Minimal AppCore for unit tests — uses in-memory storage and default config.
    pub async fn for_test(data_dir: Option<std::path::PathBuf>) -> Result<Self, String> {
        let mut config = config::Config::default();
        if let Some(dir) = data_dir {
            config.data_dir = Some(dir.to_string_lossy().into_owned());
        } else if let Ok(home) = std::env::var("KLYNTBOT_HOME") {
            config.data_dir = Some(home);
        }
        let (core, _channels) =
            Self::init_with_sender(common::AppMode::Server, Some(config), None, None, None).await?;
        Ok(core)
    }
}
