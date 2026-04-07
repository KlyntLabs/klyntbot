//! Unified cognitive pipeline: Collectors -> Consolidator -> Writers.

pub mod atom_collector;
pub mod coaching_collector;
pub mod collector;
pub mod session_collector;
pub mod signal;

pub use atom_collector::AtomCollector;
pub use coaching_collector::CoachingCollector;
pub use collector::{signal_queue, SignalReceiver, SignalSender};
pub use session_collector::SessionCollector;
pub use signal::{CognitiveSignal, SignalContext, SignalSource};
