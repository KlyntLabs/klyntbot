//! Scope partitioning, provenance metadata, anchored symbols, causal edges.
//!
//! These types appear in the `metadata` JSON column of `semantic_facts` and
//! `episodic_memories`, and in the `memory_causal_edges` table.

use jiff::Timestamp;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use uuid::Uuid;

pub use coding_ingest::RepoScope;

/// Privacy tier — every memory carries exactly one.
#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq, Default)]
#[serde(rename_all = "snake_case")]
pub enum Sensitivity {
    /// Default. Normal retrieval + eligible for externalization.
    #[default]
    Normal,
    /// Retrieved normally but never written to rule artifacts on disk.
    High,
    /// Hidden from retrieval unless `include_excluded: true`.
    Excluded,
}

/// Provenance chain attached to every memory write.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct ProvenanceMetadata {
    /// `ingest_event_log.id` rows that produced this memory.
    pub source_events: Vec<Uuid>,
    /// Session id at distillation time.
    pub session_id: String,
    /// Turn id at distillation time.
    pub turn_id: Option<String>,
    /// When distillation ran.
    pub distilled_at: Timestamp,
    /// Which model produced it (model id string).
    pub distiller_model: String,
    /// Pipeline that wrote this fact.
    pub source_kind: ProvenanceKind,
}

/// Which pipeline produced a given fact.
#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ProvenanceKind {
    /// Phase-A extractive pass.
    DistillerExtractive,
    /// Phase-B LLM synthesis.
    DistillerLlm,
    /// User explicitly edited/promoted the fact.
    UserCorrected,
    /// Reforge synthesis phase.
    ReforgeSynthesis,
}

/// Anchored symbol — link from a memory to a tree-sitter-extracted code symbol.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct AnchoredSymbol {
    /// Absolute file path.
    pub file_path: PathBuf,
    /// Symbol name.
    pub symbol: String,
    /// Symbol kind (function, method, struct, enum, const).
    pub kind: String,
    /// Commit at which the symbol was anchored.
    pub git_hash: String,
    /// Optional byte span for precise invalidation.
    pub byte_span: Option<(u32, u32)>,
}

/// Causal edge kinds — MAGMA-style. Closed enum.
#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum CausalEdgeKind {
    /// `from` fix caused `to` failure.
    Broke,
    /// `from` change was fixed by `to`.
    FixedBy,
    /// `from` test pass flipped to fail at `to`.
    FlippedToFail,
    /// `from` failure shares root cause with `to` failure.
    SharesRootCause,
    /// `from` refactor enabled `to` subsequent work.
    Enabled,
}

/// Causal edge row — backed by `memory_causal_edges` table.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct CausalEdge {
    /// Edge id.
    pub id: Uuid,
    /// Source memory id (semantic or episodic).
    pub from_id: Uuid,
    /// Target memory id.
    pub to_id: Uuid,
    /// Edge kind.
    pub edge_kind: CausalEdgeKind,
    /// Confidence (0.0 – 1.0).
    pub confidence: f32,
    /// When the edge was inferred.
    pub inferred_at: Timestamp,
}
