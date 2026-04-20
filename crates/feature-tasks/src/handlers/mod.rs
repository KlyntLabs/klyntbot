//! Handler traits for the feature-tasks crate.
//!
//! Defines trait abstractions for dependency inversion. Traits are defined here
//! (Layer 4) and implemented in the agent crate (Layer 5). Injected as `Arc<dyn Trait>`.

pub mod embedding;
pub mod enrichment;
pub mod progress;

pub use embedding::EmbeddingHandler;
pub use enrichment::{EnrichmentHandler, EnrichmentResult, EnrichmentSuggestion};
pub use progress::ProgressHandler;
