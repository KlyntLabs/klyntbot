//! Confidence context source — confidence evaluation instructions.

use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::Arc;

use async_trait::async_trait;
use context_engine::source::{ContextSource, SourceContext};

/// Provides confidence evaluation instructions with an adaptive threshold.
///
/// The threshold is stored as an `AtomicU32` (bit-cast f32) so it can be
/// updated lock-free by the learning service subscriber.
pub struct ConfidenceSource {
    threshold_bits: Arc<AtomicU32>,
}

impl ConfidenceSource {
    pub fn new(threshold: f32) -> Self {
        Self {
            threshold_bits: Arc::new(AtomicU32::new(threshold.to_bits())),
        }
    }

    /// Get a shared handle to the threshold for external updates.
    ///
    /// The learning service subscriber uses this to update the threshold
    /// when `LearningEvent::ThresholdChanged` is received.
    pub fn threshold_handle(&self) -> Arc<AtomicU32> {
        Arc::clone(&self.threshold_bits)
    }

    /// Read the current threshold value.
    pub fn threshold(&self) -> f32 {
        f32::from_bits(self.threshold_bits.load(Ordering::Relaxed))
    }

    /// Update the threshold value.
    pub fn set_threshold(&self, value: f32) {
        self.threshold_bits
            .store(value.to_bits(), Ordering::Relaxed);
    }
}

#[async_trait]
impl ContextSource for ConfidenceSource {
    fn name(&self) -> &str {
        "confidence"
    }

    fn priority(&self) -> u8 {
        50
    }

    async fn provide(&self, _ctx: &SourceContext) -> Option<String> {
        let threshold = self.threshold();
        Some(crate::confidence::prompt::confidence_prompt(threshold))
    }
}
