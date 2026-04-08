//! Unified cognitive pipeline: Collectors -> Consolidator -> Writers.

pub mod atom_collector;
pub mod coaching_collector;
pub mod collector;
pub mod consolidator;
pub mod recall_collector;
pub mod session_collector;
pub mod signal;
pub mod writer;

pub use atom_collector::AtomCollector;
pub use coaching_collector::CoachingCollector;
pub use collector::{signal_queue, SignalReceiver, SignalSender};
pub use consolidator::{group_signals, heuristic_promote, KnowledgeCluster, PromotionOp};
pub use recall_collector::RecallCollector;
pub use session_collector::SessionCollector;
pub use signal::{CognitiveSignal, SignalContext, SignalSource};
pub use writer::execute_promotions;
