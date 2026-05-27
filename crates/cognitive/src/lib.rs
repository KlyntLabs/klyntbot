//! Facade for the cognitive subsystem.
//!
//! Re-exports the whole public surface of `cognitive-memory` (the core: semantic
//! facts, episodics, retrieval, pipeline, reforge, mirror, …) so existing
//! `cognitive::*` paths used across the workspace keep resolving unchanged. The
//! concern crates — cognitive-memory, -graph, -learning, -schema (and, later,
//! -reforge / -mirror) — own the implementations behind their own interfaces.

pub use cognitive_memory::*;

// `reforge` was lifted into its own crate (it depends on memory's repos/types).
// Rebuild the `services` module so `cognitive::services::reforge::*` — used by
// agent, app-core, and the integration tests — keeps resolving. The explicit
// module shadows the glob-imported `services` above without conflict.
pub mod services {
    pub use cognitive_memory::services::*;
    pub use cognitive_reforge as reforge;
}
