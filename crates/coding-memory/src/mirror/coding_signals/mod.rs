//! Coding-specific Mirror signal sources.
//!
//! These sources implement `MirrorSignalSource` and are registered alongside
//! the built-in cognitive sources via `register_coding_sources`.

pub mod approval_history;
pub mod recall_coverage;
pub mod skill_effectiveness;

pub use approval_history::ApprovalHistorySignal;
pub use recall_coverage::RecallCoverageSignal;
pub use skill_effectiveness::SkillEffectivenessSignal;
