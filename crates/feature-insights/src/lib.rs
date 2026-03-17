//! Feature crate for versioned Insight Reviews with learning progress tracking.

pub mod progress_repo;
pub mod prompts;
pub mod repo;
pub mod service;
pub mod traits;
pub mod types;

// Re-exports
pub use progress_repo::InsightProgressRepo;
pub use repo::InsightReviewRepo;
pub use service::InsightService;
pub use traits::*;
pub use types::*;
