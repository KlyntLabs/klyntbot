//! Klyntbot Bus - Async message bus for channel↔agent communication
//!
//! This crate provides the message bus infrastructure for inbound and outbound messages.

pub mod context_updates;
pub mod domain_events;
pub mod event_domain;
pub mod events;
pub mod learning_events;
pub mod queue;

pub use context_updates::{ContextUpdate, ContextUpdateQueue, ContextUpdateReason, UpdatePriority};
pub use domain_events::{CodingMemoryKind, CorrectionKind, DomainEvent, DomainEventBus, FeedbackResponse};
pub use event_domain::EventDomain;
pub use events::{InboundMessage, MessageKind, OutboundMessage};
pub use learning_events::{LearningEvent, LearningEventBus};
pub use queue::MessageBus;
