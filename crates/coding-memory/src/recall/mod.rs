//! Recall service — the one engine behind passive injection and MCP tools.
//!
//! Phase 1: types + stub methods returning `NotImplemented`. Phase 4 wires
//! `QueryPipeline`, `UnifiedMemoryService`, the C3 failure-state probe, and
//! the dead-end check.

use jiff::Timestamp;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

pub mod budget;
pub mod causal_walker;
pub mod change_history;
pub mod dead_end;
pub mod decision_points;
pub mod facts_as_of;
pub mod open_threads;
pub mod fetch_builder;
pub mod index_builder;
pub mod probe;
pub mod renderers;
pub mod scope_resolve;
pub mod telemetry;
pub mod timeline_builder;
pub use causal_walker::CausalWalker;

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

pub mod service;
pub use service::{default_weights, CodingRecallService, CodingRecallServiceConfig};

/// Row in a `recall_facts_as_of` response.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct FactAsOfRow {
    /// Fact id.
    pub id: String,
    /// Subject.
    pub subject: String,
    /// Predicate.
    pub predicate: String,
    /// Object value at `as_of`.
    pub object: String,
    /// `valid_from`.
    pub valid_from: String,
    /// `valid_until` if closed.
    pub valid_until: Option<String>,
    /// Confidence at the time.
    pub confidence: f32,
}

/// Response from `recall_facts_as_of`.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct FactsAsOfResponse {
    /// Subject queried.
    pub subject: String,
    /// Predicate queried.
    pub predicate: String,
    /// `as_of` timestamp.
    pub as_of: Timestamp,
    /// Matching rows.
    pub rows: Vec<FactAsOfRow>,
}

/// One step in a SUPERSEDE chain.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ChangeHistoryStep {
    /// Fact id at this step.
    pub id: String,
    /// Object value.
    pub object: String,
    /// `valid_from`.
    pub valid_from: String,
    /// `valid_until`.
    pub valid_until: Option<String>,
}

/// Response from `recall_change_history`.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ChangeHistoryResponse {
    /// Subject.
    pub subject: String,
    /// Predicate.
    pub predicate: String,
    /// Chain ordered oldest-first.
    pub steps: Vec<ChangeHistoryStep>,
}

/// One decision point row.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct DecisionPointRow {
    /// Episode id.
    pub id: String,
    /// Episode kind.
    pub kind: String,
    /// When.
    pub when: String,
    /// Summary.
    pub summary: String,
    /// Repo scope.
    pub scope: String,
}

/// Response from `recall_decision_points`.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct DecisionPointsResponse {
    /// Domain (always `"code"` here).
    pub domain: String,
    /// Rows ordered newest-first.
    pub rows: Vec<DecisionPointRow>,
}

/// Union accepted by `recall_timeline`.
#[derive(Debug, Clone)]
pub enum RecallQuery {
    /// Pre-selected memory ids.
    Ids(Vec<String>),
    /// Free-text query.
    Text(String),
}
