//! Facade for the cognitive subsystem.
//!
//! Re-exports the whole public surface of `cognitive-memory` (the core: semantic
//! facts, episodics, retrieval, pipeline, …) so existing `cognitive::*` paths
//! used across the workspace keep resolving unchanged. The concern crates —
//! cognitive-memory, -graph, -learning, -schema, -reforge, -mirror — own the
//! implementations behind their own interfaces.

pub use cognitive_memory::*;

// `mirror` was lifted into its own crate (it depends on memory's repos/types/
// embedder). Re-export it under the original `cognitive::mirror` path; the glob
// above no longer provides it since it left cognitive-memory.
pub use cognitive_mirror as mirror;

// `reforge` was lifted into its own crate (it depends on memory's repos/types).
// Rebuild the `services` module so `cognitive::services::reforge::*` — used by
// agent, app-core, and the integration tests — keeps resolving. The explicit
// module shadows the glob-imported `services` above without conflict.
pub mod services {
    pub use cognitive_memory::services::*;
    pub use cognitive_reforge as reforge;
}
