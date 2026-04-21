//! Handler traits for the feature-tasks crate.
//!
//! Defines trait abstractions for dependency inversion. Traits are defined here
//! (Layer 4) and implemented in the agent crate (Layer 5). Injected as `Arc<dyn Trait>`.

pub mod embedding;
pub mod progress;

pub use embedding::EmbeddingHandler;
pub use progress::ProgressHandler;
