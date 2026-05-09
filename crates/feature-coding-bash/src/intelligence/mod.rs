//! Phase 2.3b — Execution Intelligence layer.
//!
//! Spec: `docs/superpowers/specs/2026-05-09-coding-bash-execution-intelligence-design.md`

pub mod diff;
pub mod normalize;
pub mod verification_match;

pub use diff::{diff_against_prior, ExtractedDiff, JobDiff, KindTransition, Location};
pub use normalize::command_key;
pub use verification_match::{classify as classify_verification, VerificationVerb};
