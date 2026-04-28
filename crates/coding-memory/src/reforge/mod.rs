//! Reforge coding phases — Phase 2.5 (synthesis), Phase 3.5 (rule artifacts),
//! Phase-6 selective-delete, Phase-6.5 cross-session dedup, plus the
//! session-end light pass that runs on `EventKind::SessionEnd`.
//!
//! Bodies land in Phase 5. The module surface here is what `cognitive::run_reforge`
//! and `app-core::coding_memory::reforge` link against.

pub mod coding_synthesis;
pub mod cross_session_dedup;
pub mod managed_block;
pub mod rule_artifacts;
pub mod selective_delete;
pub mod sensitivity_filter;
pub mod session_end;
pub mod session_summary_repo;
pub mod symbol_validation;
pub mod synth_handler;
pub mod types;
pub mod writer;

pub use coding_synthesis::CodingSynthesisPhase;
pub use cross_session_dedup::CrossSessionDedup;
pub use managed_block::{ManagedBlock, ManagedBlockError};
pub use rule_artifacts::RuleArtifactGenerationPhase;
pub use selective_delete::SelectiveDeleteSignal;
pub use session_end::SessionEndPass;
pub use session_summary_repo::{SessionSummaryRepo, SessionSummaryRow};
pub use symbol_validation::{SymbolValidationOutcome, SymbolValidationPhase};
pub use synth_handler::{CodingSynthesisHandler, RuleArtifactsHandler};
pub use types::{
    CodingPhaseHandlers, CodingSynthesisInput, CodingSynthesisOutput, ManagedBlockSection,
    ProjectSkillSpec, PromoteAction, RepoArtifactPlan, RuleArtifactInput, RuleArtifactOutput,
};
