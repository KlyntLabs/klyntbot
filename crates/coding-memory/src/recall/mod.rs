//! Recall service — the one engine behind passive injection and MCP tools.
//!
//! Phase 1: types + stub methods returning `NotImplemented`. Phase 4 wires
//! `QueryPipeline`, `UnifiedMemoryService`, the C3 failure-state probe, and
//! the dead-end check.

use crate::error::NotImplementedInPhase;
use common::{KlyntbotError, Result};
use jiff::Timestamp;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

pub mod budget;
pub mod renderers;
pub mod telemetry;

/// One recall "level" for progressive disclosure.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RecallLayer {
    /// Compact index — `recall_index`.
    Index,
    /// Chronological framing — `recall_timeline`.
    Timeline,
    /// Full structured content + provenance — `recall_fetch`.
    Fetch,
}

/// Layer-1 entry — used by `recall_index`.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct IndexEntry {
    /// Memory id.
    pub id: Uuid,
    /// `"fix_attempt" | "style_preference" | ...`
    pub kind: String,
    /// Short human-readable title.
    pub title: String,
    /// When recorded.
    pub when: Timestamp,
    /// `"global"` | `"repo:<id>"`.
    pub scope: String,
    /// Confidence (0.0 – 1.0).
    pub confidence: f32,
    /// Estimated token cost if fetched at layer 3.
    pub token_cost: u32,
}

/// Layer-2 entry — used by `recall_timeline`.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct TimelineEntry {
    /// Memory id.
    pub id: Uuid,
    /// Kind.
    pub kind: String,
    /// When.
    pub when: Timestamp,
    /// Short snippet.
    pub snippet: String,
    /// Related memory ids (for expansion).
    pub related_ids: Vec<Uuid>,
}

/// Layer-3 entry — used by `recall_fetch`.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct FullEntry {
    /// Memory id.
    pub id: Uuid,
    /// Kind.
    pub kind: String,
    /// Full structured content as JSON value.
    pub content: serde_json::Value,
    /// Full `metadata` column JSON.
    pub metadata: serde_json::Value,
    /// Causal edges involving this memory (optional).
    pub causal_edges: Vec<crate::scope::CausalEdge>,
    /// Ancestor memory in SUPERSEDE chain.
    pub supersedes: Option<Uuid>,
    /// Descendant memory in SUPERSEDE chain.
    pub superseded_by: Option<Uuid>,
}

/// Response from `recall_index`.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct RecallIndexResponse {
    /// Ranked results.
    pub results: Vec<IndexEntry>,
    /// C3 coverage score.
    pub coverage_score: f32,
    /// Whether the caller can request escalation.
    pub escalation_available: bool,
}

/// Response from `check_dead_ends`.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct DeadEndResponse {
    /// Prior failed attempts matching the approach.
    pub matches: Vec<DeadEndMatch>,
    /// Aggregate confidence that the approach is a dead end.
    pub aggregate_confidence: f32,
}

/// One dead-end match row.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct DeadEndMatch {
    /// Source fix-attempt episode id.
    pub attempt_id: Uuid,
    /// Problem hash.
    pub problem_hash: String,
    /// What was tried.
    pub approach: String,
    /// Why it failed.
    pub reason: String,
    /// When.
    pub when: Timestamp,
}

/// Response from `trace_causes`.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct CausalTraceResponse {
    /// Subject id requested.
    pub subject: Uuid,
    /// Ancestors walked.
    pub ancestors: Vec<crate::scope::CausalEdge>,
    /// Descendants walked.
    pub descendants: Vec<crate::scope::CausalEdge>,
    /// Depth used.
    pub depth: u32,
}

/// The single service both passive injection and MCP tools call.
#[derive(Debug)]
pub struct CodingRecallService {
    /// Phase-4 wiring will carry `UnifiedMemoryService`, `QueryPipeline`.
    _phase_stub: (),
}

impl CodingRecallService {
    /// Construct. Phase 1 stub.
    #[must_use]
    pub fn new() -> Self {
        Self { _phase_stub: () }
    }

    /// Layer-1 compact index. Phase 4.
    pub async fn recall_index(
        &self,
        _query: &str,
        _repo: Option<&str>,
        _kinds: Option<&[&str]>,
        _days: Option<u32>,
        _limit: u32,
    ) -> Result<RecallIndexResponse> {
        Err(phase(4))
    }

    /// Layer-2 timeline. Phase 4.
    pub async fn recall_timeline(
        &self,
        _ids_or_query: RecallQuery,
        _repo: Option<&str>,
        _days: u32,
    ) -> Result<Vec<TimelineEntry>> {
        Err(phase(4))
    }

    /// Layer-3 full fetch. Phase 4.
    pub async fn recall_fetch(
        &self,
        _ids: &[Uuid],
        _include_provenance: bool,
        _include_causal_graph: bool,
    ) -> Result<Vec<FullEntry>> {
        Err(phase(4))
    }

    /// Counterfactual check. Phase 4.
    pub async fn check_dead_ends(
        &self,
        _approach: &str,
        _repo: Option<&str>,
    ) -> Result<DeadEndResponse> {
        Err(phase(4))
    }

    /// Causal graph walk. Phase 6.
    pub async fn trace_causes(
        &self,
        _subject: Uuid,
        _repo: Option<&str>,
        _depth: u32,
    ) -> Result<CausalTraceResponse> {
        Err(phase(6))
    }
}

impl Default for CodingRecallService {
    fn default() -> Self {
        Self::new()
    }
}

/// Union accepted by `recall_timeline`.
#[derive(Debug, Clone)]
pub enum RecallQuery {
    /// Pre-selected memory ids.
    Ids(Vec<Uuid>),
    /// Free-text query.
    Text(String),
}

fn phase(p: u8) -> KlyntbotError {
    KlyntbotError::NotImplemented(format!("{:?}", NotImplementedInPhase::new(p)))
}
