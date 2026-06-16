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
use feature_coaching::{FeedbackTracker, InterventionRouter, PatternDetector, SignalAccumulator};
use feature_focus::DndManager;
use feature_notes::repo::{NoteRepo, PracticeSessionRepo};
use feature_productivity::{DailyAggregator, FocusManager};
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
    pub active_streams: Arc<crate::handlers::chat::ActiveStreams>,
    /// Assistant-mode thread runtime (lazily initialized).
    pub assistant_runtime: std::sync::OnceLock<Arc<dyn crate::runtime::ThreadRuntime>>,
    /// Pending ask_user interaction oneshot senders keyed by session_key.
    /// Value is (request_id, sender). Only one interaction can be pending per session
    /// because the ask_user tool blocks the agent loop until answered.
    pub pending_interactions:
        Arc<dashmap::DashMap<String, (String, oneshot::Sender<FormResponse>)>>,
    /// Notes repo (always available).
    pub note_repo: NoteRepo,
    /// Practice session repo (always available — backed by the same DB as notes).
    pub practice_repo: PracticeSessionRepo,

    pub user_situation: Option<Arc<Mutex<UserSituation>>>,

    /// Consecutive coaching nudges that were auto-collapsed (ignored).
    /// Resets on explicit user feedback. Delivery skipped when >= 2.
    pub consecutive_coaching_ignores: Arc<AtomicI32>,
    /// Transport-agnostic event emitter — set by the desktop adapter (Tauri events)
    /// or left as `NoopEmitter` for CLI / tests. Used by the MCP server to push
    /// entity updates to the frontend after tool mutations.
    pub event_emitter: Arc<dyn AppEventEmitter>,

    /// Unified TemporalScheduler — sole firing source post-4.4c.

    /// Feature host — holds plugin handles and provides typed lookup.
    pub host: crate::plugin::host::FeatureHost,
}

impl AppCore {
    // ── Error code constants ──────────────────────────────────────────
    const ERR_FEATURE_DISABLED: &'static str = "FEATURE_DISABLED";
    const ERR_NOT_AVAILABLE: &'static str = "NOT_AVAILABLE";

    /// Lookup a typed handle from the FeatureHost or return a feature-disabled error.
    fn require_host<T: Send + Sync + 'static>(&self, msg: &str) -> Result<Arc<T>, ApiError> {
        self.host
            .get::<T>()
            .ok_or_else(|| ApiError::new(Self::ERR_FEATURE_DISABLED, msg))
    }

    // ── Accessor methods ──────────────────────────────────────────────

    /// Return productivity repos or a "feature disabled" error.
    #[tracing::instrument(skip(self), err)]
    pub fn productivity_repos(
        &self,
    ) -> Result<Arc<feature_productivity::repos::ProductivityRepos>, ApiError> {
        self.host
            .get::<feature_productivity::repos::ProductivityRepos>()
            .ok_or_else(|| {
                ApiError::new(
                    Self::ERR_FEATURE_DISABLED,
                    "productivity feature is not enabled",
                )
            })
    }

    /// Return focus manager or a "feature disabled" error.
    #[tracing::instrument(skip(self), err)]
    pub fn focus_manager(&self) -> Result<Arc<FocusManager>, ApiError> {
        self.require_host("productivity feature is not enabled")
    }

    /// Return DND manager or a "feature disabled" error.
    #[tracing::instrument(skip(self), err)]
    pub fn dnd_manager(&self) -> Result<Arc<DndManager>, ApiError> {
        self.require_host("focus (DND) feature is not enabled")
    }

    /// Return daily aggregator or a "feature disabled" error.
    #[tracing::instrument(skip(self), err)]
    pub fn aggregator(&self) -> Result<Arc<DailyAggregator>, ApiError> {
        self.require_host("productivity feature is not enabled")
    }

    /// Return distraction interceptor or a "not available" error.
    #[tracing::instrument(skip(self), err)]
    pub fn distraction_interceptor(
        &self,
    ) -> Result<Arc<Mutex<feature_productivity::distraction::DistractionInterceptor>>, ApiError>
    {
        self.require_host("Distraction interceptor not initialized")
    }

    #[tracing::instrument(skip(self), err)]
    pub fn signal_accumulator(&self) -> Result<Arc<Mutex<SignalAccumulator>>, ApiError> {
        self.require_host("coaching engine is not available")
    }

    #[tracing::instrument(skip(self), err)]
    pub fn pattern_detector(&self) -> Result<Arc<Mutex<PatternDetector>>, ApiError> {
        self.require_host("coaching engine is not available")
    }

    #[tracing::instrument(skip(self), err)]
    pub fn intervention_router(&self) -> Result<Arc<Mutex<InterventionRouter>>, ApiError> {
        self.require_host("coaching engine is not available")
    }

    #[tracing::instrument(skip(self), err)]
    pub fn feedback_tracker(&self) -> Result<Arc<Mutex<FeedbackTracker>>, ApiError> {
        self.require_host("coaching engine is not available")
    }

    #[tracing::instrument(skip(self), err)]
    pub fn coaching_log_repo(&self) -> Result<Arc<storage::CoachingInterventionLogRepo>, ApiError> {
        self.require_host("coaching engine is not available")
    }

    #[tracing::instrument(skip(self))]
    pub fn pipeline_broadcast(&self) -> Option<broadcast::Sender<cognitive::PipelineEvent>> {
        self.host
            .get_cloned::<broadcast::Sender<cognitive::PipelineEvent>>()
    }

    pub fn embedding_engine(&self) -> Option<Arc<tools::embedding_engine::EmbeddingEngine>> {
        self.host.get::<tools::embedding_engine::EmbeddingEngine>()
    }

    pub fn vector_store(&self) -> Option<VectorStore> {
        self.host.get_cloned::<VectorStore>()
    }

    pub fn cognitive_provider(&self) -> Option<providers::DynProvider> {
        self.host.get_cloned::<providers::DynProvider>()
    }

    /// Returns the shared active-view handle if one was registered during init.
    /// `None` on partial-init paths (e.g. tests) that skip the handle — callers
    /// treat that as "no active view" rather than panicking.
    pub fn active_view(&self) -> Option<Arc<RwLock<Option<context_engine::ActiveView>>>> {
        self.host
            .get::<RwLock<Option<context_engine::ActiveView>>>()
    }

    pub fn user_situation(&self) -> Result<&Arc<Mutex<UserSituation>>, ApiError> {
        self.user_situation.as_ref().ok_or_else(|| {
            ApiError::new(
                Self::ERR_FEATURE_DISABLED,
                "coaching engine is not available",
            )
        })
    }

    #[tracing::instrument(skip(self), err)]
    pub fn domain_event_bus(&self) -> Result<Arc<DomainEventBus>, ApiError> {
        self.host.get::<DomainEventBus>().ok_or_else(|| {
            ApiError::new(
                Self::ERR_FEATURE_DISABLED,
                "domain event bus is not available",
            )
        })
    }

    /// Return event log repo or `None` when unavailable.
    #[tracing::instrument(skip(self))]
    pub fn note_embedding_handler(
        &self,
    ) -> Option<Arc<dyn feature_notes::handlers::embedding::NoteEmbeddingHandler>> {
        self.host
            .get::<crate::plugins::notes::NotesInitResult>()
            .and_then(|r| r.note_embedding_handler.clone())
    }

    pub fn event_log_repo(&self) -> Option<cognitive::EventLogRepo> {
        self.host
            .get::<crate::plugins::cognitive::CognitiveInitResult>()
            .and_then(|r| r.event_log_repo.clone())
    }

    /// Lookup a field from `CognitiveInitResult` or return a "not available" error.
    fn require_cognitive<
        T: Clone,
        F: FnOnce(&crate::plugins::cognitive::CognitiveInitResult) -> Option<T>,
    >(
        &self,
        extractor: F,
        msg: &str,
    ) -> Result<T, ApiError> {
        self.host
            .get::<crate::plugins::cognitive::CognitiveInitResult>()
            .and_then(|r| extractor(&r))
            .ok_or_else(|| ApiError::not_available(msg))
    }

    /// Return flashcard repo or a "not available" error.
    #[tracing::instrument(skip(self), err)]
    pub fn flashcard_repo(&self) -> Result<cognitive::FlashcardRepo, ApiError> {
        self.require_cognitive(|r| r.flashcard_repo.clone(), "Flashcard repo not available")
    }

    /// Return knowledge atom repo or a "not available" error.
    #[tracing::instrument(skip(self), err)]
    pub fn knowledge_atom_repo(&self) -> Result<cognitive::KnowledgeAtomRepo, ApiError> {
        self.require_cognitive(
            |r| r.knowledge_atom_repo.clone(),
            "Knowledge atom repo not available",
        )
    }

    /// Return practice session repo.
    #[tracing::instrument(skip(self))]
    pub fn practice_repo(&self) -> &PracticeSessionRepo {
        &self.practice_repo
    }

    /// Return review session repo or a "not available" error.
    #[tracing::instrument(skip(self), err)]
    pub fn review_session_repo(&self) -> Result<cognitive::ReviewSessionRepo, ApiError> {
        self.require_cognitive(
            |r| r.review_session_repo.clone(),
            "Review session repo not available",
        )
    }

    /// Return deck preference repo or a "not available" error.
    #[tracing::instrument(skip(self), err)]
    pub fn deck_preference_repo(&self) -> Result<cognitive::DeckPreferenceRepo, ApiError> {
        self.require_cognitive(
            |r| r.deck_preference_repo.clone(),
            "Deck preference repo not available",
        )
    }

    /// Return activity ingestion service or a "feature disabled" error.
    #[tracing::instrument(skip(self), err)]
    pub fn activity_ingestion_service(
        &self,
    ) -> Result<Arc<activity_log::ActivityIngestionService>, ApiError> {
        self.host
            .get::<activity_log::ActivityIngestionService>()
            .ok_or_else(|| {
                ApiError::new(Self::ERR_FEATURE_DISABLED, "activity log is not available")
            })
    }

    /// Return launcher search engine or a "feature disabled" error.
    #[tracing::instrument(skip(self), err)]
    pub fn launcher_engine(&self) -> Result<Arc<LauncherSearchEngine>, ApiError> {
        self.host.get::<LauncherSearchEngine>().ok_or_else(|| {
            ApiError::new(
                Self::ERR_FEATURE_DISABLED,
                "launcher feature is not enabled",
            )
        })
    }

    /// Return launcher clipboard repo or a "feature disabled" error.
    #[tracing::instrument(skip(self), err)]
    pub fn launcher_clipboard_repo(
        &self,
    ) -> Result<Arc<feature_launcher::ClipboardRepo>, ApiError> {
        self.launcher_engine()
            .map(|e| Arc::clone(&e.clipboard_repo))
    }

    /// Return insight service or `None` when unavailable.
    #[tracing::instrument(skip(self))]
    pub fn insight_service(&self) -> Option<Arc<feature_insights::InsightService>> {
        self.host.get::<feature_insights::InsightService>()
    }

    /// Return autotuner orchestrator or `None` when disabled.
    #[tracing::instrument(skip(self))]
    /// Return journey tracker or `None` when not initialized.
    #[tracing::instrument(skip(self))]
    pub fn journey_tracker(&self) -> Option<crate::journey::JourneyTracker> {
        self.host.get_cloned::<crate::journey::JourneyTracker>()
    }

    pub fn autotuner_orchestrator(&self) -> Option<Arc<agent::autotuner::AutoTunerOrchestrator>> {
        self.host.get::<agent::autotuner::AutoTunerOrchestrator>()
    }

    /// Return mirror facade or a "not available" error.
    /// Return mirror facade or a "not available" error.
    #[tracing::instrument(skip(self), err)]
    pub fn mirror_facade(&self) -> Result<Arc<cognitive::mirror::MirrorFacade>, ApiError> {
        self.host
            .get::<cognitive::mirror::MirrorFacade>()
            .ok_or_else(|| ApiError::new(Self::ERR_NOT_AVAILABLE, "Mirror facade not available"))
    }

    /// Return pending memory repo or a "not available" error.
    #[tracing::instrument(skip(self), err)]
    pub fn pending_memory_repo(
        &self,
    ) -> Result<Arc<cognitive::repos::PendingMemoryRepo>, ApiError> {
        self.host
            .get::<cognitive::repos::PendingMemoryRepo>()
            .ok_or_else(|| {
                ApiError::new(Self::ERR_NOT_AVAILABLE, "Pending memory repo not available")
            })
    }

    pub fn tracing_registry(&self) -> std::sync::Arc<crate::tracing::TracingRegistry> {
        self.host
            .get::<crate::tracing::TracingRegistry>()
            .expect("tracing registry initialized")
    }

    /// Return the AI feature registry.
    pub fn feature_registry(&self) -> Arc<ai_core::AiFeatureRegistry> {
        self.host
            .get::<ai_core::AiFeatureRegistry>()
            .expect("feature registry built by host")
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

        let cog = self
            .host
            .get::<crate::plugins::cognitive::CognitiveInitResult>()
            .ok_or_else(|| {
                common::KlyntbotError::Storage("cognitive system not initialized".into())
            })?;
        cog.semantic_fact_repo
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
    pub fn voice_service(&self) -> Result<Arc<VoiceService>, ApiError> {
        self.host
            .get::<crate::plugins::voice::VoiceInitResult>()
            .and_then(|r| r.voice_service.clone())
            .ok_or_else(|| {
                ApiError::new(Self::ERR_FEATURE_DISABLED, "voice feature is not enabled")
            })
    }

    /// Return voice conversation manager or a "not available" error.
    #[tracing::instrument(skip(self), err)]
    pub fn voice_conversation_manager(
        &self,
    ) -> Result<Arc<crate::handlers::voice_conversation::VoiceConversationManager>, ApiError> {
        self.host
            .get::<crate::plugins::voice::VoiceInitResult>()
            .and_then(|r| r.voice_conversation_manager.clone())
            .ok_or_else(|| {
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
        if let Some(svc) = self.insight_service() {
            let created_at = created_at_str
                .and_then(|s| s.parse::<jiff::Timestamp>().ok())
                .unwrap_or_else(jiff::Timestamp::now);
            svc.check_cross_domain(domain, id.to_string(), title.to_string(), created_at)
                .await;
        }
    }

    /// Execute an async closure if a handle of the given type exists in the host.
    #[allow(dead_code)]
    async fn with_handle<T, F, Fut>(&self, f: F)
    where
        T: Send + Sync + 'static,
        F: FnOnce(Arc<T>) -> Fut,
        Fut: std::future::Future<Output = ()> + Send,
    {
        if let Some(handle) = self.host.get::<T>() {
            f(handle).await;
        }
    }

    /// Graceful shutdown.
    #[tracing::instrument(skip(self))]
    pub async fn shutdown(&self) {
        info!("shutting down app core");
        // Stop productivity engine first to flush pending events.
        self.with_handle::<Mutex<feature_productivity::ProductivityEngine>, _, _>(
            |engine| async move {
                engine.lock().await.stop().await;
            },
        )
        .await;
        // Stop nudge service.
        self.with_handle::<Arc<Mutex<feature_productivity::NudgeService>>, _, _>(
            |nudge| async move {
                nudge.lock().await.stop().await;
            },
        )
        .await;
        // Persist coaching feedback before stopping the service.
        self.with_handle::<Mutex<FeedbackTracker>, _, _>(|tracker| async move {
            tracker.lock().await.persist().await;
        })
        .await;
        // Stop coaching service.
        self.with_handle::<Arc<Mutex<feature_coaching::CoachingService>>, _, _>(
            |coaching| async move {
                coaching.lock().await.stop().await;
            },
        )
        .await;
        // Stop BrainVoice signal router.
        self.with_handle::<crate::brain_voice::BrainVoice, _, _>(|bv| async move {
            bv.shutdown();
        })
        .await;
        if let Err(e) = self.agent.shutdown().await {
            error!("agent shutdown error: {}", e);
        }
        // Cancel mirror subscribers before the main shutdown token
        // so they stop consuming domain events immediately.
        self.with_handle::<tokio_util::sync::CancellationToken, _, _>(|token| async move {
            token.cancel();
        })
        .await;
        // Stop the NotificationDispatcher select loop.
        self.with_handle::<notifications::NotificationDispatcherHandle, _, _>(
            |handle| async move {
                handle.shutdown.cancel();
            },
        )
        .await;
        // Abort the voice conversation loop if still running.
        self.with_handle::<crate::plugins::voice::VoiceInitResult, _, _>(|result| async move {
            if let Some(handle) = result.voice_loop_handle.lock().unwrap().take() {
                handle.abort();
            }
        })
        .await;
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

    // ── Workspace lifecycle ───────────────────────────────────────────────

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
            .cognitive_provider()
            .ok_or_else(|| ApiError::new(Self::ERR_NOT_AVAILABLE, "LLM provider not configured"))?;
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

    // ── Background bash jobs (coding background tasks) ─────────────────

    /// Return the assistant-mode thread runtime, constructing it on first call.
    pub fn assistant_runtime(self: Arc<Self>) -> Arc<dyn crate::runtime::ThreadRuntime> {
        let core = Arc::clone(&self);
        self.assistant_runtime
            .get_or_init(|| Arc::new(crate::runtime::assistant::AssistantThreadRuntime::new(core)))
            .clone()
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
        let provider = match self.cognitive_provider() {
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
        let cog = self
            .host
            .get::<crate::plugins::cognitive::CognitiveInitResult>()
            .ok_or_else(|| {
                common::KlyntbotError::Storage("cognitive system not initialized".into())
            })?;
        let count = cognitive::services::background::run_graph_consolidation(
            &cog.semantic_fact_repo,
            &cog.entity_repo,
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

    /// Test AppCore with a custom LLM provider and event emitter.
    pub async fn for_tests(
        provider: providers::DynProvider,
        emitter: std::sync::Arc<dyn crate::events::AppEventEmitter>,
    ) -> Result<Self, String> {
        let mut config = config::Config::default();
        // Persist the temp dir for the process lifetime rather than letting the
        // `TempDir` guard drop at the end of this constructor. If it dropped here
        // the directory would be removed out from under the SQLite pool we just
        // opened, surfacing later as an intermittent `unable to open database
        // file` (code 14) under parallel test load. The OS reaps the leaked dir
        // from the system temp root.
        let data_dir = tempfile::tempdir().map_err(|e| e.to_string())?.keep();
        config.data_dir = Some(data_dir.to_string_lossy().into_owned());
        let (core, _channels) = Self::init_with_sender(
            common::AppMode::Server,
            Some(config),
            None,
            Some(emitter),
            Some(provider),
        )
        .await?;
        Ok(core)
    }
}
