//! Adapter bridging the cognitive mirror layer to the autotuner orchestrator.
//!
//! For Phase 3, `apply_champion` is a logging stub — full integration requires
//! constructing a `Champion` from JSON, which will be refined incrementally.

use std::sync::Arc;

use async_trait::async_trait;

use agent::autotuner::AutoTunerOrchestrator;

pub struct AppAutotunerBridge {
    orchestrator: Arc<AutoTunerOrchestrator>,
}

impl AppAutotunerBridge {
    pub fn new(orchestrator: Arc<AutoTunerOrchestrator>) -> Self {
        Self { orchestrator }
    }
}

#[async_trait]
impl cognitive::mirror::AutotunerBridge for AppAutotunerBridge {
    async fn apply_champion(
        &self,
        _params: serde_json::Value,
        reason: String,
    ) -> common::Result<()> {
        // Phase 3 stub: log intent. Full integration (constructing Champion from
        // JSON + calling orchestrator.update_champion) will be added when the
        // TrialParams ↔ JSON round-trip is validated end-to-end.
        tracing::info!("AutotunerBridge: would apply params for reason: {reason}");
        Ok(())
    }

    async fn current_champion_params(&self) -> common::Result<serde_json::Value> {
        let params = self.orchestrator.current_champion_params().await;
        match params {
            Some(p) => Ok(serde_json::to_value(p)?),
            None => Ok(serde_json::json!({})),
        }
    }
}
