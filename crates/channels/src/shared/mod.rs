//! Shared utilities for channel implementations.

pub mod interaction;
pub mod typing;

pub use interaction::{InteractionTracker, PendingCallback};
pub use typing::TypingManager;
