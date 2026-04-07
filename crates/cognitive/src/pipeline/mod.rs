//! Unified cognitive pipeline: Collectors -> Consolidator -> Writers.

pub mod collector;
pub mod session_collector;
pub mod signal;

pub use collector::{signal_queue, SignalReceiver, SignalSender};
pub use session_collector::SessionCollector;
pub use signal::{CognitiveSignal, SignalContext, SignalSource};
