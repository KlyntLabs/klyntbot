//! Feature crate for versioned Insight Reviews with learning progress tracking.

pub mod merge;
pub mod progress;
pub mod progress_repo;
pub mod prompt_builder;
pub mod prompts;
pub mod repo;
pub mod scope;
pub mod service;
pub mod traits;
pub mod types;

// Re-exports
pub use merge::SmartMergeEngine;
pub use progress::{generate_change_note, ProgressComputer};
pub use progress_repo::InsightProgressRepo;
pub use prompt_builder::{InsightContext, PromptBuilder};
pub use repo::InsightReviewRepo;
pub use scope::{NoopScopeResolver, ScopeResolver};
pub use service::InsightService;
pub use traits::*;
pub use types::*;
