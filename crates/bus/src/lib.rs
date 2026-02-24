//! Klyntbot Bus - Async message bus for channel↔agent communication
//!
//! This crate provides the message bus infrastructure for inbound and outbound messages.

pub mod events;
pub mod learning_events;
pub mod queue;

pub use events::{InboundMessage, MessageKind, OutboundMessage};
pub use learning_events::{LearningEvent, LearningEventBus};
pub use queue::MessageBus;
