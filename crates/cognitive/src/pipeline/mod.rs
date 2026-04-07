//! Unified cognitive pipeline: Collectors -> Consolidator -> Writers.

pub mod collector;
pub mod signal;

pub use collector::{signal_queue, SignalReceiver, SignalSender};
pub use signal::{CognitiveSignal, SignalContext, SignalSource};
