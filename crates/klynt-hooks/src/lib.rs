//! Klynt hook engine — Claude-Code-compatible schema.
//! Adapted from codex-rs/hooks/.

pub mod engine;
pub mod error;
pub mod events;
pub mod registry;
pub mod schema;
pub mod types;

pub use engine::{HookEngine, HookFireInput, HookOutcome};
pub use error::{HookError, HookResult};
pub use registry::HookRegistry;
pub use schema::{Hook, HookConfig, HookEvents};
pub use types::HookPayload;
