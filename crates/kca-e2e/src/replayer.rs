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
        }
        // Cognitive extraction runs on a 3s background batch window. The
        // streaming reply finishes long before facts are persisted, so a
        // bench query fired immediately would race against the extractor.
        // Wait until the fact store stops growing before returning.
        self.await_cognitive_idle().await;
        Ok(measurements)
    }

    /// Block until the cognitive fact store stops growing, indicating that
    /// the background extraction pipeline has caught up. Polls every 500ms,
    /// returns when the count is unchanged across two consecutive polls or
    /// the safety timeout (45s) elapses.
    pub async fn await_cognitive_idle(&self) {
        use cognitive::repos::SemanticFactRepo;
        use std::time::{Duration, Instant};
        let repo = SemanticFactRepo::new(self.pool.inner().clone());
        let deadline = Instant::now() + Duration::from_secs(45);
        let mut last = -1i64;
        let mut stable = 0u32;
        while Instant::now() < deadline {
            tokio::time::sleep(Duration::from_millis(500)).await;
            let now = repo.count_active().await.unwrap_or(0);
            if now == last {
                stable += 1;
                if stable >= 2 {
                    return;
                }
            } else {
                stable = 0;
            }
            last = now;
        }
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
        let (_user_msg, mut info) = self
            .app
            .chat_send(content, session_key, None)
            .await
            .map_err(|e| common::KlyntbotError::Storage(format!("chat_send failed: {e}")))?;
        let mut answer = String::new();
        while let Some(ev) = info.event_rx.recv().await {
            if let agent::AgentEvent::ContentChunk { data } = ev {
                answer.push_str(&data);
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
