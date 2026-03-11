//! Handler traits for the feature-tasks crate.
//!
//! Defines trait abstractions for dependency inversion. Traits are defined here
//! (Layer 4) and implemented in the agent crate (Layer 5). Injected as `Arc<dyn Trait>`.

pub mod decomposition;
pub mod embedding;
pub mod enrichment;
pub mod execution;
pub mod forecast;
pub mod planning;
pub mod proactive;
pub mod progress;
pub mod suggestion_applier;

pub use decomposition::DecompositionHandler;
pub use embedding::EmbeddingHandler;
pub use enrichment::{EnrichmentHandler, EnrichmentResult, EnrichmentSuggestion};
pub use execution::TaskExecutionHandler;
pub use forecast::ForecastHandler;
pub use planning::DayPlanningHandler;
pub use proactive::ProactiveHandler;
pub use progress::ProgressHandler;
pub use suggestion_applier::SuggestionApplier;
