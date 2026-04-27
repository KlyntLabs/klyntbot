//! Causal edge repo + auto-detection (Phase 6).

pub mod detector;
pub mod problem_hash_lookup;
pub mod repo;

pub use detector::CausalEdgeDetector;
pub use repo::{CausalEdgeRepo, ProblemHashGroup};
