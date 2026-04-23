//! Proactive intelligence engine — signal accumulation, pattern detection,
//! LLM-powered coaching, intervention routing, and closed-loop feedback.

pub mod consumer;
pub mod events;
pub mod feedback;
pub mod learning_templates;
pub mod pattern_detector;
pub mod reasoner;
pub mod router;
pub mod service;
pub mod signal_accumulator;

pub use consumer::CoachingSignalConsumer;
pub use feedback::{FeedbackTracker, PendingBehavioral};
pub use pattern_detector::PatternDetector;
pub use reasoner::{CoachingDecision, CoachingReasonerHandler};
pub use router::{InterventionChannel, InterventionRouter};
pub use service::CoachingService;
pub use signal_accumulator::{SignalAccumulator, TriggerCondition};
