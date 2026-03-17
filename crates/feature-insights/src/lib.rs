//! Feature crate for versioned Insight Reviews with learning progress tracking.

pub mod repo;
pub mod traits;
pub mod types;

// Re-exports
pub use repo::InsightReviewRepo;
pub use traits::*;
pub use types::*;
