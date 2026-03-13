mod agent;
mod channels;
mod coaching;
mod cognitive;
mod cron;
mod productivity;
mod storage;

use std::sync::Arc;

use ::agent::AgentLoop;
use ::channels::ChannelManager;
use bus::MessageBus;
use feature_productivity::auto_focus::AutoFocusEvent;
use tokio::sync::{mpsc, Mutex, RwLock};
use tokio_util::sync::CancellationToken;
use tracing::info;

use crate::state::AppCore;

/// Bundle of receiver channels that callers wire to their transport (Tauri, SSE, etc.).
pub struct EventChannels {
    pub intervention_rx: mpsc::Receiver<feature_coaching::router::DeliveredIntervention>,
    pub domain_event_bus: Arc<bus::DomainEventBus>,
    pub pipeline_rx: tokio::sync::broadcast::Receiver<::cognitive::PipelineEvent>,
    pub auto_focus_rx: Option<mpsc::Receiver<AutoFocusEvent>>,
    pub nudge_rx: Option<mpsc::Receiver<feature_productivity::types::NudgeRecord>>,
    pub dashboard_tick_rx:
        Option<tokio::sync::broadcast::Receiver<feature_productivity::ActivityTick>>,
    pub dashboard_poll_interval_secs: u64,
}

impl AppCore {
    /// Initialize the full agent stack.
    ///
    /// Mirrors the initialization order from `serve.rs`:
    /// config → storage → bus → provider → cron → persona → agent → channels
    ///
    /// Returns `(AppCore, EventChannels)`. The caller wires `EventChannels`
    /// receivers to their transport layer (Tauri events, SSE, etc.).
    pub async fn init(
        mode: common::AppMode,
        config_override: Option<config::Config>,
    ) -> Result<(Self, EventChannels), String> {
        Self::init_with_sender(mode, config_override, None).await
    }

    /// Initialize with an optional custom notification sender.
    ///
    /// When `sender` is `Some`, OS-native notifications are routed through it
    /// (e.g. Tauri's notification plugin, which shows the app icon). When
    /// `None`, the default platform command (`osascript` / `notify-send`) is used.
    pub async fn init_with_sender(
        mode: common::AppMode,
        config_override: Option<config::Config>,
        notification_sender: Option<Arc<dyn common::NotificationSender>>,
    ) -> Result<(Self, EventChannels), String> {
        // ── Phase 1: Storage ─────────────────────────────────────────────
        let storage::StorageResult {
            mut config,
            storage_pool,
            repos,
            vector_store,
            note_repo,
            provider,
        } = storage::init_storage(config_override).await?;

        // ── Shared: Bus + cognitive provider ─────────────────────────────
        // Created once here in the orchestrator. Both cron and agent receive
        // references to the same instances (matches the original single-fn flow).
        let bus = Arc::new(MessageBus::new(100));

        let cognitive_provider = providers::create_cognitive_provider(&config).ok().flatten();
        if cognitive_provider.is_some() {
            info!("cognitive provider created — using LLM handlers");
        } else {
            info!("no cognitive provider — using heuristic handlers");
        }

        // ── Phase 2: Cron ────────────────────────────────────────────────
        let cron::CronResult {
            cron_service,
            notification_dispatcher,
        } = cron::init_cron(
            &config,
            &repos,
            &bus,
            &notification_sender,
            cognitive_provider.clone(),
        )
        .await?;

        // ── Phase 3: Agent ───────────────────────────────────────────────
        let agent::AgentResult {
            cognitive_provider,
            persona_manager,
            agent,
            inbound_rx,
            pipeline_broadcast_tx,
            user_situation,
            domain_event_bus,
            activity_svc,
        } = agent::init_agent(
            &config,
            &storage_pool,
            &repos,
            provider,
            vector_store,
            &bus,
            cognitive_provider,
            &cron_service,
            &notification_dispatcher,
            &notification_sender,
        )
        .await?;

        // ── Phase 4: Channel manager ─────────────────────────────────────
        let channel_manager = channels::init_channels(&config, &bus)?;

        let shutdown_token = CancellationToken::new();

        // ── Phase 5: Productivity ────────────────────────────────────────
        let productivity::ProductivityResult {
            dashboard_poll_interval_secs,
            productivity_repos,
            focus_manager,
            productivity_engine,
            aggregator,
            nudge_service,
            distraction_interceptor,
            auto_focus_rx,
            nudge_rx,
            dashboard_tick_rx,
        } = productivity::init_productivity(
            &config,
            &storage_pool,
            &domain_event_bus,
            &activity_svc,
            &cognitive_provider,
            &shutdown_token,
        )
        .await;

        // ── Phase 6: Coaching ────────────────────────────────────────────
        let coaching::CoachingResult {
            intervention_rx,
            signal_accumulator,
            pattern_detector,
            intervention_router,
            feedback_tracker,
            coaching_service,
        } = coaching::init_coaching(
            mode,
            &config,
            &storage_pool,
            &repos,
            productivity_repos.as_ref(),
            &user_situation,
            &domain_event_bus,
            &cognitive_provider,
            &shutdown_token,
        )
        .await;

        // ── Phase 7: Cognitive (capture, file watcher, work context) ─────
        cognitive::init_cognitive(&mut config, &storage_pool, &activity_svc, &shutdown_token).await;

        // ── Assemble AppCore ─────────────────────────────────────────────
        let core = AppCore {
            mode,
            repos,
            storage_pool: storage_pool.clone(),
            agent: Arc::clone(&agent),
            bus: bus.clone(),
            persona_manager,
            config: RwLock::new(config),
            channel_manager: channel_manager.clone(),
            cron_service: cron_service.clone(),
            shutdown_token: shutdown_token.clone(),
            active_streams: Arc::new(dashmap::DashMap::new()),
            pending_interactions: Arc::new(dashmap::DashMap::new()),
            note_repo,
            productivity_repos,
            focus_manager,
            productivity_engine,
            aggregator,
            nudge_service,
            distraction_interceptor,
            domain_event_bus: Some(Arc::clone(&domain_event_bus)),
            signal_accumulator,
            pattern_detector,
            intervention_router,
            feedback_tracker,
            user_situation: Some(user_situation),
            coaching_service: coaching_service.map(|cs| Arc::new(Mutex::new(cs))),
            cognitive_provider,
            pipeline_broadcast: Some(pipeline_broadcast_tx),
            event_log_repo: Some(::cognitive::EventLogRepo::new(storage_pool.inner().clone())),
            consecutive_coaching_ignores: Arc::new(std::sync::atomic::AtomicI32::new(0)),
            activity_ingestion_service: Some(Arc::clone(&activity_svc)),
        };

        // ── Post-core background services ────────────────────────────────
        cognitive::spawn_post_core_services(
            &core,
            &domain_event_bus,
            activity_svc,
            &shutdown_token,
        );

        // Spawn background services (agent loop + channel manager).
        spawn_background(inbound_rx, channel_manager, &agent, &shutdown_token);

        // Spawn periodic situation recomputation (every 2 min).
        coaching::spawn_situation_recompute(
            core.productivity_repos.clone(),
            core.repos.clone(),
            core.intervention_router.clone(),
            core.user_situation.clone(),
            &shutdown_token,
        );

        let pipeline_rx = core
            .pipeline_broadcast
            .as_ref()
            .expect("pipeline broadcast initialized above")
            .subscribe();

        let channels = EventChannels {
            intervention_rx,
            domain_event_bus,
            pipeline_rx,
            auto_focus_rx,
            nudge_rx,
            dashboard_tick_rx,
            dashboard_poll_interval_secs,
        };

        Ok((core, channels))
    }
}

/// Spawn the agent loop and channel manager tasks.
fn spawn_background(
    inbound_rx: mpsc::Receiver<bus::InboundMessage>,
    channel_manager: Arc<Mutex<ChannelManager>>,
    agent: &Arc<AgentLoop>,
    shutdown_token: &CancellationToken,
) {
    let agent_clone = Arc::clone(agent);
    let token = shutdown_token.clone();

    tokio::spawn(async move {
        tokio::select! {
            result = agent_clone.run_with_rx(inbound_rx) => {
                if let Err(e) = result {
                    tracing::error!("agent loop error: {}", e);
                }
            }
            _ = token.cancelled() => {
                info!("agent loop shutdown via token");
            }
        }
    });

    tokio::spawn(async move {
        if let Err(e) = channel_manager.lock().await.start_all().await {
            tracing::error!("channel manager error: {}", e);
        }
    });

    info!("background services spawned");
}
