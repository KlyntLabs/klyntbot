use ai_core::{AiSignal, SignalConsumer};
use async_trait::async_trait;
use tokio::sync::mpsc;

/// Forwards every `coaching_signal`-flagged `AiSignal` into the accumulator's
/// processing channel. The receiving end is driven by `CoachingService`.
pub struct CoachingSignalConsumer {
    tx: mpsc::Sender<AiSignal>,
}

impl CoachingSignalConsumer {
    pub fn new(tx: mpsc::Sender<AiSignal>) -> Self {
        Self { tx }
    }
}

#[async_trait]
impl SignalConsumer for CoachingSignalConsumer {
    fn name(&self) -> &'static str {
        "coaching"
    }

    async fn consume(&self, signal: &AiSignal) -> common::Result<()> {
        if !signal.coaching_signal {
            return Ok(());
        }
        let _ = self.tx.send(signal.clone()).await; // drop on full — coaching is best-effort
        Ok(())
    }
}
