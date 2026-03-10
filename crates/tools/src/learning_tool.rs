//! LearningTool — Tool interface for learning system insights with dependency inversion.
//!
//! Defined in Layer 3 (tools). Implemented by LearningHandlerImpl in Layer 5 (agent).
//! Follows the same dependency inversion pattern as GoalTool and PlanTool.

use async_trait::async_trait;
use common::Result;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::HashMap;
use std::sync::Arc;

use crate::{RoutingContext, Tool};
use common::ToolError;

/// Per-tool statistics summary exposed to the LLM via LearningTool.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolSummary {
    pub total_calls: i64,
    pub success_count: i64,
    pub avg_duration_ms: i64,
}

/// High-level learning status returned to the LLM.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LearningStatus {
    pub current_threshold: f32,
    pub total_strategy_records: i64,
    pub strategy_accuracy: f64,
    pub avg_response_time_ms: i64,
    pub avg_satisfaction: Option<f64>,
    pub suggested_threshold: f32,
    pub per_tool: HashMap<String, ToolSummary>,
}

/// A single threshold change record exposed to the LLM.
///
/// Mirrors `agent::learning::types::ThresholdChange` but defined here at Layer 3
/// to preserve the dependency inversion boundary (tools cannot import from agent).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ThresholdEntry {
    pub from: f32,
    pub to: f32,
    pub reason: String,
    pub timestamp: chrono::DateTime<chrono::Utc>,
}

/// LearningHandler trait for dependency inversion.
/// Implemented by LearningHandlerImpl in agent crate (Layer 5).
/// Defined here in tools crate (Layer 3) to break the circular dependency.
#[async_trait]
pub trait LearningHandler: Send + Sync {
    /// Get the current learning status. Returns None if no outcome data exists yet.
    async fn get_status(&self) -> Result<Option<LearningStatus>>;

    /// Trigger immediate analysis and return the current status.
    async fn analyze_now(&self) -> Result<LearningStatus>;

    /// Return the last `limit` adaptive threshold change records (oldest-first).
    async fn get_threshold_history(&self, limit: usize) -> Result<Vec<ThresholdEntry>>;
}

/// LearningTool — exposes learning system insights to the LLM agent.
pub struct LearningTool {
    handler: Option<Arc<dyn LearningHandler>>,
}

impl LearningTool {
    pub fn new(handler: Option<Arc<dyn LearningHandler>>) -> Self {
        Self { handler }
    }
}

#[async_trait]
impl Tool for LearningTool {
    fn name(&self) -> &str {
        "learning"
    }

    fn description(&self) -> &str {
        "Query learning system insights: current confidence threshold, per-tool success rates, \
         outcome statistics, and adaptive threshold history. \
         Actions: status (current data), analyze (fresh analysis), history (last N threshold changes)."
    }

    fn metadata(&self) -> tools_core::ToolMetadata {
        tools_core::ToolMetadata {
            category: tools_core::ToolCategory::Memory,
            tags: vec!["learn".into(), "preference".into(), "behavior".into()],
            cost_hint: tools_core::CostHint::Free,
            ..Default::default()
        }
    }

    fn parameters(&self) -> Value {
        serde_json::json!({
            "type": "object",
            "properties": {
                "action": {
                    "type": "string",
                    "enum": ["status", "analyze", "history"],
                    "description": "status: current learning data; analyze: trigger fresh analysis; \
                                    history: last N adaptive threshold changes"
                },
                "limit": {
                    "type": "integer",
                    "minimum": 1,
                    "maximum": 100,
                    "description": "Number of history entries to return (default: 10, only used by 'history' action)"
                }
            },
            "required": ["action"]
        })
    }

    async fn execute(&self, args: Value, _ctx: &RoutingContext) -> Result<String> {
        let handler = self
            .handler
            .as_ref()
            .ok_or_else(|| ToolError::ExecutionFailed("LearningHandler not configured".into()))?;

        let action = args
            .get("action")
            .and_then(|v| v.as_str())
            .ok_or_else(|| ToolError::InvalidParams("Missing action".into()))?;

        match action {
            "status" => {
                let status = handler.get_status().await?;
                match status {
                    Some(s) => Ok(serde_json::to_string_pretty(&s)?),
                    None => Ok(r#"{"message": "No learning data available yet. More tool calls are needed to build outcome history."}"#.to_string()),
                }
            }
            "analyze" => {
                let status = handler.analyze_now().await?;
                Ok(serde_json::to_string_pretty(&status)?)
            }
            "history" => {
                let limit = args.get("limit").and_then(|v| v.as_u64()).unwrap_or(10) as usize;
                let history = handler.get_threshold_history(limit).await?;
                Ok(serde_json::to_string_pretty(&history)?)
            }
            _ => Err(ToolError::InvalidParams(format!("Unknown action: {}", action)).into()),
        }
    }
}
