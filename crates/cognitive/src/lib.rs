//! Facade for the cognitive subsystem.
//!
//! Re-exports the whole public surface of `cognitive-memory` (the core: semantic
//! facts, episodics, retrieval, pipeline, reforge, mirror, …) so existing
//! `cognitive::*` paths used across the workspace keep resolving unchanged. The
//! concern crates — cognitive-memory, -graph, -learning, -schema (and, later,
//! -reforge / -mirror) — own the implementations behind their own interfaces.

pub use cognitive_memory::*;
