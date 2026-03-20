//! Klyntbot Bus - Async message bus for channel↔agent communication
//!
//! This crate provides the message bus infrastructure for inbound and outbound messages.

pub mod domain_events;
pub mod events;
pub mod learning_events;
pub mod queue;

pub use domain_events::{CorrectionKind, DomainEvent, DomainEventBus, FeedbackResponse};
pub use events::{InboundMessage, MessageKind, OutboundMessage};
pub use learning_events::{LearningEvent, LearningEventBus};
pub use queue::MessageBus;
