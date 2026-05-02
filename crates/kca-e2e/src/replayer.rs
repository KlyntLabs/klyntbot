//! Replays a `ConversationFixture` through a real `AppCore` instance built
//! from an in-memory pool. Exposes hooks for snapshotting cognitive state.

use crate::fixtures::ConversationFixture;
use std::sync::Arc;
use storage::StoragePool;

pub struct ReplayContext {
    pub pool: StoragePool,
    pub app: Arc<app_core::AppCore>,
    pub turn_latencies_ms: Vec<u64>,
    pub captured_events: Arc<tokio::sync::Mutex<Vec<bus::DomainEvent>>>,
}

impl ReplayContext {
    pub async fn new() -> common::Result<Self> {
        let mut cfg = config::load_with_env_overrides().await.map_err(|e| {
            common::KlyntbotError::Config(common::ConfigError::Invalid(format!("config load: {e}")))
        })?;
        cfg.data_dir = Some(
            tempfile::tempdir()
                .map_err(common::KlyntbotError::Io)?
                .path()
                .to_string_lossy()
                .to_string(),
        );
        cfg.cognitive.intelligence_mode = config::schema::IntelligenceMode::Deep;
        cfg.cognitive.micro_reforge.enabled = true;
        cfg.cognitive.predictive_cache.enabled = true;
        cfg.cognitive.hierarchical.enabled = true;

        let (app, _channels) = app_core::AppCore::init(common::AppMode::Server, Some(cfg))
            .await
            .map_err(|e| common::KlyntbotError::Storage(format!("AppCore init failed: {e}")))?;

        let pool = app.storage_pool.clone();
        // Run cognitive migrations so cognitive tables exist for assertions.
        StoragePool::run_feature_migrations(pool.inner(), &cognitive::cognitive_migrations())
            .await
            .map_err(|e| {
                common::KlyntbotError::Storage(format!("cognitive migrations failed: {e}"))
            })?;

        let captured = Arc::new(tokio::sync::Mutex::new(Vec::new()));
        let bus_arc = app
            .domain_event_bus()
            .map_err(|e| common::KlyntbotError::Storage(format!("domain_event_bus: {e}")))?
            .clone();
        let mut rx = bus_arc.subscribe();
        let captured_clone = captured.clone();
        tokio::spawn(async move {
            while let Ok(ev) = rx.recv().await {
                captured_clone.lock().await.push(ev);
            }
        });

        Ok(Self {
            pool,
            app: Arc::new(app),
            turn_latencies_ms: Vec::new(),
            captured_events: captured,
        })
    }

    pub async fn replay(
        &mut self,
        fixture: &ConversationFixture,
    ) -> common::Result<ReplayMeasurements> {
        use std::time::Instant;
        let mut measurements = ReplayMeasurements::default();
        for turn in &fixture.turns {
            let session_key = common::SessionKey::from_parts("kca-e2e", &fixture.id);
            let started = Instant::now();
            let _ = self
                .chat_complete(turn.user.clone(), session_key.to_string())
                .await?;
            let elapsed = started.elapsed().as_millis() as u64;
            self.turn_latencies_ms.push(elapsed);
            measurements.turn_latencies_ms.push(elapsed);
            measurements.turns_replayed += 1;
            // Per-turn sync: wait for THIS turn's extraction to land
            // before publishing the next. Without this, turn N+1's
            // IngestionConsumer races ahead of turn N's `user→name`
            // write and the cross-turn identity-binding mirror skips,
            // losing facts like `Alice→lives_in=SF` even though the
            // raw `user→lives_in=SF` is in the store.
            self.await_cognitive_idle().await;
        }
        // One final settle in case the last turn left work in flight.
        self.await_cognitive_idle().await;
        Ok(measurements)
    }

    /// Block until the cognitive fact store stops growing, indicating that
    /// the background extraction pipeline has caught up.
    ///
    /// Polls every 750ms; returns once the count is unchanged across **four
    /// consecutive** polls (≈3s of quiet) or the 60s safety timeout fires.
    /// The longer stability window matters because two extraction paths
    /// run concurrently: the heuristic SPO extractor persists in <500ms,
    /// while the LLM-based `LlmExtractionHandler` takes 2-4s per batch.
    /// A shorter window declares idle after the heuristic finishes but
    /// before LLM-extracted (identity-bound) facts hit the repo, so the
    /// bench would query a half-populated store.
    pub async fn await_cognitive_idle(&self) {
        use cognitive::repos::SemanticFactRepo;
        use std::time::{Duration, Instant};
        let repo = SemanticFactRepo::new(self.pool.inner().clone());
        let started = Instant::now();
        let deadline = started + Duration::from_secs(60);
        // Unconditional floor. The previous logic let idle return as soon
        // as count was stable for 3s as long as count > 0 — fine on the
        // first turn but catastrophic on per-turn sync: turn 1's LLM call
        // takes up to ~12s, but turn 0's facts have already been counted
        // for 3s by then, so idle returns BEFORE turn 1 writes anything.
        // 14s covers Kimi's worst-case extraction latency observed in
        // benchmarks (12s tail) with margin.
        let floor = started + Duration::from_secs(14);
        let mut last = -1i64;
        let mut stable = 0u32;
        while Instant::now() < deadline {
            tokio::time::sleep(Duration::from_millis(750)).await;
            let now = repo.count_active().await.unwrap_or(0);
            if now == last {
                if stable >= 4 && Instant::now() >= floor {
                    return;
                }
                stable += 1;
            } else {
                stable = 0;
            }
            last = now;
        }
    }

    /// Diagnostic: dump every active semantic fact in the store as
    /// `(subject, predicate, object)` tuples. Used by bench harnesses to
    /// verify what the extraction pipeline actually persisted.
    pub async fn dump_facts(&self) -> Vec<(String, String, String)> {
        let pool = self.pool.inner().clone();
        let rows: Vec<(String, String, String)> = sqlx::query_as(
            "SELECT subject, predicate, object FROM semantic_facts \
             WHERE superseded_at IS NULL ORDER BY recorded_at",
        )
        .fetch_all(&pool)
        .await
        .unwrap_or_default();
        rows
    }

    /// Send a user message and block until the agent's full streamed reply is
    /// received. Drains [`AgentEvent::ContentChunk`] events from the chat
    /// stream and returns the concatenated assistant text.
    ///
    /// This is the bench-friendly counterpart to `chat_send`, which is
    /// intentionally non-blocking for the streaming UI.
    pub async fn chat_complete(
        &self,
        content: String,
        session_key: String,
    ) -> common::Result<String> {
        let user_message = content.clone();
        let (_user_msg, mut info) = self
            .app
            .chat_send(content, session_key.clone(), None, None)
            .await
            .map_err(|e| common::KlyntbotError::Storage(format!("chat_send failed: {e}")))?;
        let mut answer = String::new();
        while let Some(ev) = info.event_rx.recv().await {
            if let agent::AgentEvent::ContentChunk { data } = ev {
                answer.push_str(&data);
            }
        }
        // The bench harness drains `event_rx` directly, bypassing
        // `relay_chat_stream` — the function that normally publishes
        // `ChatTurnCompleted` to the cognitive pipeline. Without this
        // event, `IngestionConsumer` never fires and `semantic_facts`
        // stays empty even though the agent itself answered correctly
        // from session history. Publish it manually here to wire the
        // memory pipeline back up for benchmarks.
        //
        // Defensive: if the bus has been dropped or has no live
        // consumers, the publish becomes a no-op rather than a hard
        // error. Observed during n=500 LoCoMo runs where late
        // cognitive tasks could outlive the consumer in some teardown
        // races. Losing one ChatTurnCompleted event for a single QA
        // is not worth aborting the bench over.
        if let Ok(bus) = self.app.domain_event_bus() {
            if let Err(e) = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                bus.publish(bus::DomainEvent::ChatTurnCompleted {
                    session_key,
                    user_message: Some(user_message),
                });
            })) {
                tracing::warn!(
                    ?e,
                    "ChatTurnCompleted publish failed (bus likely shutting down)"
                );
            }
        }
        Ok(answer)
    }
}

#[derive(Debug, Default, Clone)]
pub struct ReplayMeasurements {
    pub turns_replayed: u32,
    pub turn_latencies_ms: Vec<u64>,
}

impl ReplayMeasurements {
    pub fn p95_ms(&self) -> u64 {
        if self.turn_latencies_ms.is_empty() {
            return 0;
        }
        let mut s = self.turn_latencies_ms.clone();
        s.sort();
        let idx = (s.len() as f64 * 0.95).ceil() as usize - 1;
        s[idx.min(s.len() - 1)]
    }
}
