//! `coding-memory` — local-first coding memory layer built on klyntbot's
//! cognitive crate. Hosts the fact taxonomy, Distiller, recall service,
//! Reforge coding phases, and the `MemorySink` trait used by native
//! in-process consumers like `klynt-cli`.
//!
//! Phase 1 lands the module surface, all public types, MCP tool stubs,
//! and the consolidated schema migration. Methods return
//! `NotImplementedInPhase { required_phase }`.

#![deny(missing_docs)]

/// Distiller — online writer stub.
pub mod distiller;
/// Error surface for phased stubs.
pub mod error;
/// Coding fact taxonomy (`FixAttempt`, `RepoContext`, …).
pub mod facts;
/// MCP tool stubs — registered with `default_exposed_tools()`.
pub mod mcp;
/// Recall service — MCP + passive injection stub.
pub mod recall;
/// Reforge coding phases (2.5, 3.5) stubs.
pub mod reforge_phase;
/// C3 retrieval-skill registry stubs.
pub mod retrieval_skills;
/// Scope partitioning, provenance, anchored symbols, causal edges.
pub mod scope;
/// `MemorySink` trait + `InProcessSink` / `IngestSocketSink` stubs.
pub mod sink;
/// Scope-aware skill store extension + project skill evolution.
pub mod skills;

pub use error::{CodingMemoryError, NotImplementedInPhase};

use tools_core::FeatureMigration;

/// Coding-memory migrations. Caller: `AppCore::init_storage` (app-core crate).
pub fn coding_memory_migrations() -> Vec<FeatureMigration> {
    vec![FeatureMigration {
        feature_name: "coding_memory".to_string(),
        version: 1,
        description: "Consolidated Phase-1 schema: scope_repo_id, metadata, \
                      actor_id columns; memory_causal_edges, memory_utilization, \
                      ingest_event_log, klynt_sessions tables; skill_versions \
                      scope columns."
            .to_string(),
        sql: include_str!("../migrations/001_coding_memory.sql").to_string(),
    }]
}
