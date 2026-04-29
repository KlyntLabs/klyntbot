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
            let _resp = self
                .app
                .chat_send(turn.user.clone(), session_key.to_string(), None)
                .await
                .map_err(|e| common::KlyntbotError::Storage(format!("chat_send failed: {e}")))?;
            let elapsed = started.elapsed().as_millis() as u64;
            self.turn_latencies_ms.push(elapsed);
            measurements.turn_latencies_ms.push(elapsed);
            tokio::time::sleep(std::time::Duration::from_millis(100)).await;
            measurements.turns_replayed += 1;
        }
        Ok(measurements)
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
