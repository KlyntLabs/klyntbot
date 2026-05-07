//! Unified approval gate — pre-tool-execution permission system.
//!
//! See: docs/superpowers/specs/2026-05-05-unified-permission-gate-design.md

pub mod channel;
pub mod class;
pub mod coding_policy;
pub mod gate;
pub mod grants;
pub mod policy;
pub mod request;

pub use channel::{ApprovalCapabilities, ApprovalChannel, BlockingFallbackChannel};
pub use class::{ApprovalClass, ApprovalDecision, ApprovalLifetime, ApprovalScope};
pub use coding_policy::CodingApprovalPolicy;
pub use gate::{ApprovalGate, GateOutcome};
pub use grants::{ApprovalGrantsRepo, GrantRow};
pub use policy::ClassifyHook;
pub use request::{ApprovalContext, ApprovalRequest, ChannelKind};
