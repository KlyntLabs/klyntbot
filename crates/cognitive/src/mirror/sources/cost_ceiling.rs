use std::sync::Arc;
use ai_core::{mirror::MirrorSnapshotSpec, AiSignal, MirrorSignalSource};
use async_trait::async_trait;
use crate::mirror::{snippet_from_alert, MirrorAlert, MirrorRepo};

pub struct CostCeilingSource {
    repo: Arc<MirrorRepo>,
}

impl CostCeilingSource {
    pub fn new(repo: Arc<MirrorRepo>) -> Self {
        Self { repo }
    }

    async fn handle_cost_alert(&self, payload: &str) -> common::Result<()> {
        let parsed: serde_json::Value = serde_json::from_str(payload).unwrap_or(serde_json::Value::Null);
        let session_key = parsed.get("session_key").and_then(|v| v.as_str()).unwrap_or("").to_string();
        let spend_usd = parsed.get("spend_usd").and_then(|v| v.as_f64()).unwrap_or(0.0);
        let ceiling_usd = parsed.get("ceiling_usd").and_then(|v| v.as_f64()).unwrap_or(0.0);
        let percent = parsed.get("percent").and_then(|v| v.as_f64()).unwrap_or(0.0);

        let alert = MirrorAlert::CostThresholdCrossed {
            session_key,
            spend_usd,
            ceiling_usd,
            percent,
        };
        let snippet = snippet_from_alert(&alert);
        self.repo.insert_snippet(&snippet).await?;
        Ok(())
    }
}

#[async_trait]
impl MirrorSignalSource for CostCeilingSource {
    fn spec(&self) -> MirrorSnapshotSpec {
        MirrorSnapshotSpec {
            name: "cost_ceiling",
            subscribed_kinds: &["CodingMirrorAlert"],
            flush_interval_secs: None,
        }
    }

    fn name(&self) -> &'static str {
        "cost-ceiling-source"
    }

    async fn accumulate(&self, signal: &AiSignal) -> common::Result<()> {
        if let Some(bus::DomainEvent::CodingMirrorAlert { kind, payload, .. }) = signal.raw_event.as_ref() {
            if kind == "costThresholdCrossed" {
                if let Err(e) = self.handle_cost_alert(payload).await {
                    tracing::warn!("CostCeilingSource: failed to record alert: {e}");
                }
            }
        }
        Ok(())
    }

    async fn flush(&self) -> common::Result<()> {
        Ok(())
    }
}
