//! MCP tool dispatchers — Phase 4 wires 7 active tools; `trace_causes` stays Phase-6.
//!
//! Each call decodes args, invokes `CodingRecallService`, and serializes the response.
//! `CodingMemoryToolset` is a `Send + Sync` handle the MCP server registers.

use crate::recall::{CodingRecallService, RecallQuery};
use jiff::Timestamp;
use std::sync::Arc;

/// Individual MCP tool wrapper for a coding-memory recall surface.
#[derive(Clone)]
pub struct CodingMemoryMcpTool {
    name: &'static str,
    description: &'static str,
    parameters: serde_json::Value,
    toolset: CodingMemoryToolset,
}

impl CodingMemoryMcpTool {
    /// Construct a tool wrapper.
    #[must_use]
    pub fn new(
        name: &'static str,
        description: &'static str,
        parameters: serde_json::Value,
        toolset: CodingMemoryToolset,
    ) -> Self {
        Self {
            name,
            description,
            parameters,
            toolset,
        }
    }
}

#[async_trait::async_trait]
impl tools_core::Tool for CodingMemoryMcpTool {
    fn name(&self) -> &str {
        self.name
    }

    fn description(&self) -> &str {
        self.description
    }

    fn parameters(&self) -> serde_json::Value {
        self.parameters.clone()
    }

    async fn execute(
        &self,
        args: serde_json::Value,
        _ctx: &tools_core::RoutingContext,
    ) -> common::Result<String> {
        let val = self.toolset.dispatch(self.name, args).await?;
        serde_json::to_string_pretty(&val).map_err(|e| {
            common::KlyntbotError::Tool(common::ToolError::ExecutionFailed(format!(
                "Failed to serialize response: {e}"
            )))
        })
    }

    fn metadata(&self) -> tools_core::ToolMetadata {
        tools_core::ToolMetadata {
            category: tools_core::ToolCategory::Memory,
            tags: vec!["coding-memory".into(), "recall".into()],
            cost_hint: tools_core::CostHint::Free,
            ..Default::default()
        }
    }
}

/// Public tool names — must match `EXPLICIT_TOOL_ALLOWLIST` in config.
pub const CODING_MEMORY_MCP_TOOLS: &[&str] = &[
    "recall_index",
    "recall_timeline",
    "recall_fetch",
    "trace_causes",
    "check_dead_ends",
    "recall_facts_as_of",
    "recall_change_history",
    "recall_decision_points",
];

/// Toolset handle.
#[derive(Clone)]
pub struct CodingMemoryToolset {
    svc: Arc<CodingRecallService>,
}

impl std::fmt::Debug for CodingMemoryToolset {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("CodingMemoryToolset").finish()
    }
}

impl CodingMemoryToolset {
    /// Construct.
    #[must_use]
    pub fn new(svc: Arc<CodingRecallService>) -> Self {
        Self { svc }
    }

    /// Return all coding-memory recall tools as `DynTool` handles for MCP registration.
    #[must_use]
    pub fn mcp_tools(&self) -> Vec<tools_core::DynTool> {
        use serde_json::json;
        vec![
            Arc::new(CodingMemoryMcpTool::new(
                "recall_index",
                "Retrieve a ranked index of relevant coding memories for a query. Returns compact entries with kind, title, scope, confidence, and token cost.",
                json!({
                    "type": "object",
                    "required": ["query"],
                    "properties": {
                        "query": { "type": "string", "description": "Search query" },
                        "repo": { "type": "string", "description": "Optional repo scope filter" },
                        "kinds": { "type": "array", "items": { "type": "string" }, "description": "Optional memory kind filter" },
                        "days": { "type": "integer", "description": "Lookback window in days" },
                        "limit": { "type": "integer", "description": "Max results (default 10)" }
                    }
                }),
                self.clone(),
            )),
            Arc::new(CodingMemoryMcpTool::new(
                "recall_timeline",
                "Retrieve a chronological timeline of coding memories. Accepts either explicit memory ids or a free-text query.",
                json!({
                    "type": "object",
                    "properties": {
                        "ids": { "type": "array", "items": { "type": "string" }, "description": "Explicit memory ids" },
                        "query": { "type": "string", "description": "Free-text query" },
                        "repo": { "type": "string", "description": "Optional repo scope filter" },
                        "days": { "type": "integer", "description": "Lookback window in days (default 30)" }
                    }
                }),
                self.clone(),
            )),
            Arc::new(CodingMemoryMcpTool::new(
                "recall_fetch",
                "Fetch full structured content for specific memory ids, including provenance and causal edges.",
                json!({
                    "type": "object",
                    "required": ["ids"],
                    "properties": {
                        "ids": { "type": "array", "items": { "type": "string" }, "description": "Memory ids to fetch" },
                        "includeProvenance": { "type": "boolean", "description": "Include provenance metadata (default true)" },
                        "includeCausalGraph": { "type": "boolean", "description": "Include causal edges (default false)" }
                    }
                }),
                self.clone(),
            )),
            Arc::new(CodingMemoryMcpTool::new(
                "check_dead_ends",
                "Check whether a proposed approach matches known failed attempts (counterfactual memories). Returns aggregate confidence and matching attempts.",
                json!({
                    "type": "object",
                    "required": ["approach"],
                    "properties": {
                        "approach": { "type": "string", "description": "Proposed approach text" },
                        "repo": { "type": "string", "description": "Optional repo scope filter" }
                    }
                }),
                self.clone(),
            )),
            Arc::new(CodingMemoryMcpTool::new(
                "recall_facts_as_of",
                "Bi-temporal query: return the fact value for a subject+predicate as of a specific timestamp.",
                json!({
                    "type": "object",
                    "required": ["subject", "predicate", "asOf"],
                    "properties": {
                        "subject": { "type": "string", "description": "Fact subject" },
                        "predicate": { "type": "string", "description": "Fact predicate" },
                        "asOf": { "type": "string", "description": "ISO-8601 timestamp" }
                    }
                }),
                self.clone(),
            )),
            Arc::new(CodingMemoryMcpTool::new(
                "recall_change_history",
                "Walk the SUPERSEDE chain for a subject+predicate, returning historical values oldest-first.",
                json!({
                    "type": "object",
                    "required": ["subject", "predicate"],
                    "properties": {
                        "subject": { "type": "string", "description": "Fact subject" },
                        "predicate": { "type": "string", "description": "Fact predicate" },
                        "repo": { "type": "string", "description": "Optional repo scope filter" }
                    }
                }),
                self.clone(),
            )),
            Arc::new(CodingMemoryMcpTool::new(
                "recall_decision_points",
                "List significant decision-point episodes (fix attempts, dead ends, refactors) for a repo.",
                json!({
                    "type": "object",
                    "properties": {
                        "domain": { "type": "string", "description": "Domain filter" },
                        "repo": { "type": "string", "description": "Optional repo scope filter" },
                        "limit": { "type": "integer", "description": "Max results (default 50)" }
                    }
                }),
                self.clone(),
            )),
            Arc::new(CodingMemoryMcpTool::new(
                "trace_causes",
                "Trace causal ancestors and descendants of a memory (stub — lands in Phase 6).",
                json!({
                    "type": "object",
                    "required": ["subject"],
                    "properties": {
                        "subject": { "type": "string", "description": "Memory id to trace" }
                    }
                }),
                self.clone(),
            )),
        ]
    }

    /// Dispatch.
    pub async fn dispatch(
        &self,
        tool: &str,
        args: serde_json::Value,
    ) -> common::Result<serde_json::Value> {
        match tool {
            "recall_index" => self.recall_index(args).await,
            "recall_timeline" => self.recall_timeline(args).await,
            "recall_fetch" => self.recall_fetch(args).await,
            "check_dead_ends" => self.check_dead_ends(args).await,
            "recall_facts_as_of" => self.recall_facts_as_of(args).await,
            "recall_change_history" => self.recall_change_history(args).await,
            "recall_decision_points" => self.recall_decision_points(args).await,
            "trace_causes" => Err(common::KlyntbotError::Storage(
                "trace_causes lands in Phase 6".into(),
            )),
            other => Err(common::KlyntbotError::Storage(format!(
                "unknown coding-memory tool: {other}"
            ))),
        }
    }

    async fn recall_index(&self, args: serde_json::Value) -> common::Result<serde_json::Value> {
        #[derive(serde::Deserialize)]
        #[serde(rename_all = "camelCase")]
        struct A {
            query: String,
            repo: Option<String>,
            #[serde(default)]
            kinds: Option<Vec<String>>,
            days: Option<u32>,
            #[serde(default = "default_limit")]
            limit: u32,
        }
        fn default_limit() -> u32 {
            10
        }
        let a: A = serde_json::from_value(args).map_err(decode_err)?;
        let kinds_owned = a.kinds.clone().unwrap_or_default();
        let kinds_borrow: Vec<&str> = kinds_owned.iter().map(|s| s.as_str()).collect();
        let kinds_opt: Option<&[&str]> = if kinds_borrow.is_empty() {
            None
        } else {
            Some(&kinds_borrow)
        };
        let resp = self
            .svc
            .recall_index(&a.query, a.repo.as_deref(), kinds_opt, a.days, a.limit)
            .await?;
        serde_json::to_value(resp).map_err(encode_err)
    }

    async fn recall_timeline(&self, args: serde_json::Value) -> common::Result<serde_json::Value> {
        #[derive(serde::Deserialize)]
        #[serde(rename_all = "camelCase")]
        struct A {
            ids: Option<Vec<String>>,
            query: Option<String>,
            repo: Option<String>,
            #[serde(default = "default_days")]
            days: u32,
        }
        fn default_days() -> u32 {
            30
        }
        let a: A = serde_json::from_value(args).map_err(decode_err)?;
        let q = match (a.ids, a.query) {
            (Some(ids), _) => RecallQuery::Ids(ids),
            (_, Some(q)) => RecallQuery::Text(q),
            _ => {
                return Err(common::KlyntbotError::Storage(
                    "missing ids or query".into(),
                ))
            }
        };
        let resp = self
            .svc
            .recall_timeline(q, a.repo.as_deref(), a.days)
            .await?;
        serde_json::to_value(resp).map_err(encode_err)
    }

    async fn recall_fetch(&self, args: serde_json::Value) -> common::Result<serde_json::Value> {
        #[derive(serde::Deserialize)]
        #[serde(rename_all = "camelCase")]
        struct A {
            ids: Vec<String>,
            #[serde(default = "default_true")]
            include_provenance: bool,
            #[serde(default)]
            include_causal_graph: bool,
        }
        fn default_true() -> bool {
            true
        }
        let a: A = serde_json::from_value(args).map_err(decode_err)?;
        let resp = self
            .svc
            .recall_fetch(&a.ids, a.include_provenance, a.include_causal_graph)
            .await?;
        serde_json::to_value(resp).map_err(encode_err)
    }

    async fn check_dead_ends(&self, args: serde_json::Value) -> common::Result<serde_json::Value> {
        #[derive(serde::Deserialize)]
        #[serde(rename_all = "camelCase")]
        struct A {
            approach: String,
            repo: Option<String>,
        }
        let a: A = serde_json::from_value(args).map_err(decode_err)?;
        let resp = self
            .svc
            .check_dead_ends(&a.approach, a.repo.as_deref())
            .await?;
        serde_json::to_value(resp).map_err(encode_err)
    }

    async fn recall_facts_as_of(
        &self,
        args: serde_json::Value,
    ) -> common::Result<serde_json::Value> {
        #[derive(serde::Deserialize)]
        #[serde(rename_all = "camelCase")]
        struct A {
            subject: String,
            predicate: String,
            as_of: Timestamp,
        }
        let a: A = serde_json::from_value(args).map_err(decode_err)?;
        let resp = self
            .svc
            .recall_facts_as_of(&a.subject, &a.predicate, a.as_of)
            .await?;
        serde_json::to_value(resp).map_err(encode_err)
    }

    async fn recall_change_history(
        &self,
        args: serde_json::Value,
    ) -> common::Result<serde_json::Value> {
        #[derive(serde::Deserialize)]
        #[serde(rename_all = "camelCase")]
        struct A {
            subject: String,
            predicate: String,
            repo: Option<String>,
        }
        let a: A = serde_json::from_value(args).map_err(decode_err)?;
        let resp = self
            .svc
            .recall_change_history(&a.subject, &a.predicate, a.repo.as_deref())
            .await?;
        serde_json::to_value(resp).map_err(encode_err)
    }

    async fn recall_decision_points(
        &self,
        args: serde_json::Value,
    ) -> common::Result<serde_json::Value> {
        #[derive(serde::Deserialize)]
        #[serde(rename_all = "camelCase")]
        struct A {
            #[serde(default)]
            domain: Option<String>,
            repo: Option<String>,
            #[serde(default = "default_dp_limit")]
            limit: i64,
        }
        fn default_dp_limit() -> i64 {
            50
        }
        let a: A = serde_json::from_value(args).map_err(decode_err)?;
        let resp = self
            .svc
            .recall_decision_points(a.repo.as_deref(), a.limit)
            .await?;
        let _ = a.domain;
        serde_json::to_value(resp).map_err(encode_err)
    }
}

fn decode_err<E: std::fmt::Display>(e: E) -> common::KlyntbotError {
    common::KlyntbotError::Storage(format!("decode args: {e}"))
}

fn encode_err<E: std::fmt::Display>(e: E) -> common::KlyntbotError {
    common::KlyntbotError::Storage(format!("encode response: {e}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn allowlist_matches_design() {
        let expected = [
            "recall_index",
            "recall_timeline",
            "recall_fetch",
            "trace_causes",
            "check_dead_ends",
            "recall_facts_as_of",
            "recall_change_history",
            "recall_decision_points",
        ];
        assert_eq!(CODING_MEMORY_MCP_TOOLS, expected);
    }
}
