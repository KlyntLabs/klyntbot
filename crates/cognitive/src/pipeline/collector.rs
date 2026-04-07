//! Signal queue types for the unified pipeline.

use super::signal::CognitiveSignal;
use tokio::sync::mpsc;

pub type SignalSender = mpsc::Sender<CognitiveSignal>;
pub type SignalReceiver = mpsc::Receiver<CognitiveSignal>;

pub fn signal_queue(capacity: usize) -> (SignalSender, SignalReceiver) {
    mpsc::channel(capacity)
}
