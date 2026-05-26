use crate::repos::MetricRepo;
use ai_core::{AiSignal, SignalConsumer};
use async_trait::async_trait;

/// Consumes `AiSignal::metric_samples` and persists them to `ai_metric_samples`.
pub struct MetricHarvestConsumer {
    repo: MetricRepo,
}

impl MetricHarvestConsumer {
    pub fn new(repo: MetricRepo) -> Self {
        Self { repo }
    }
}

#[async_trait]
impl SignalConsumer for MetricHarvestConsumer {
    fn name(&self) -> &'static str {
        "metric_harvest"
    }

    async fn consume(&self, signal: &AiSignal) -> common::Result<()> {
        if signal.metric_samples.is_empty() {
            return Ok(());
        }

        let now = signal.timestamp.as_second();
        let domain = signal.domain.as_str();
        let event_kind = signal.event_kind;

        for sample in &signal.metric_samples {
            self.repo
                .insert_sample(sample.name, sample.value, now, domain, event_kind)
                .await?;
        }

        Ok(())
    }
}
