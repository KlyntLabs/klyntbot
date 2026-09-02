//! TemporalTool — read-only temporal reasoning queries over the knowledge graph.

use common::Result;
use tools_core::{tool_actions, ActionParams};

use cognitive::services::temporal::TemporalService;

// ---------------------------------------------------------------------------
// Action param structs
// ---------------------------------------------------------------------------

#[derive(Debug, ActionParams)]
pub struct FactsAsOfParams {
    /// The subject of the fact (e.g., "user")
    pub subject: String,
    /// The predicate of the fact (e.g., "peak_hours")
    pub predicate: String,
    /// ISO-8601 date or datetime to query at (e.g., "2026-03-15")
    pub as_of: String,
}

#[derive(Debug, ActionParams)]
pub struct FirstMentionParams {
    /// The subject of the fact (e.g., "user")
    pub subject: String,
    /// The predicate of the fact (e.g., "occupation")
    pub predicate: String,
}

#[derive(Debug, ActionParams)]
pub struct ChangeHistoryParams {
    /// The subject of the fact (e.g., "user")
    pub subject: String,
    /// The predicate of the fact (e.g., "peak_hours")
    pub predicate: String,
}

#[derive(Debug, ActionParams)]
pub struct CompetingTruthsParams {
    /// The subject of the fact (e.g., "user")
    pub subject: String,
    /// The predicate of the fact (e.g., "favorite_language")
    pub predicate: String,
}

#[derive(Debug, ActionParams)]
pub struct KnowledgeDiffParams {
    /// Start of the period (ISO-8601, e.g., "2026-03-01T00:00:00Z")
    pub from: String,
    /// End of the period (ISO-8601, e.g., "2026-04-01T00:00:00Z")
    pub to: String,
    /// Optional domain filter (e.g., "work", "finance")
    pub domain: Option<String>,
}

#[derive(Debug, ActionParams)]
pub struct DecisionPointsParams {
    /// Optional domain filter
    pub domain: Option<String>,
    /// Maximum number of decision points to return (default: 10)
    pub limit: Option<i64>,
}

// ---------------------------------------------------------------------------
// TemporalTool
// ---------------------------------------------------------------------------

pub struct TemporalTool {
    service: TemporalService,
}

impl TemporalTool {
    pub fn new(service: TemporalService) -> Self {
        Self { service }
    }
}

#[tool_actions(
    ctx = "()",
    name = "temporal",
    description = "Time-oriented queries over the knowledge graph. Query fact history, find when something was first mentioned, compare knowledge states across time, and discover decision points where beliefs changed.",
    category = "Memory",
    tags = "temporal,history,memory,facts,timeline,change",
    cost = "Free",
    mcp_exposure = "default"
)]
impl TemporalTool {
    /// Return the state of a fact at a specific point in time.
    #[action(name = "facts_as_of")]
    async fn facts_as_of(&self, params: FactsAsOfParams, _ctx: ()) -> Result<String> {
        let result = self
            .service
            .facts_as_of(&params.subject, &params.predicate, &params.as_of)
            .await
            .map_err(|e| tool_err(format!("facts_as_of failed: {e}")))?;

        match result {
            Some(version) => serde_json::to_string_pretty(&version)
                .map_err(|e| tool_err(format!("serialize: {e}"))),
            None => Ok(format!(
                "No fact found for {}.{} as of {}",
                params.subject, params.predicate, params.as_of
            )),
        }
    }

    /// Find when a subject+predicate was first mentioned.
    #[action(name = "first_mention")]
    async fn first_mention(&self, params: FirstMentionParams, _ctx: ()) -> Result<String> {
        let result = self
            .service
            .first_mention(&params.subject, &params.predicate)
            .await
            .map_err(|e| tool_err(format!("first_mention failed: {e}")))?;

        match result {
            Some(date) => Ok(format!(
                "{}.{} was first mentioned on {}",
                params.subject, params.predicate, date
            )),
            None => Ok(format!(
                "No records found for {}.{}",
                params.subject, params.predicate
            )),
        }
    }

    /// Return full version history of a fact (newest first).
    #[action(name = "change_history")]
    async fn change_history(&self, params: ChangeHistoryParams, _ctx: ()) -> Result<String> {
        let history = self
            .service
            .get_fact_history(&params.subject, &params.predicate)
            .await
            .map_err(|e| tool_err(format!("change_history failed: {e}")))?;

        if history.is_empty() {
            return Ok(format!(
                "No history found for {}.{}",
                params.subject, params.predicate
            ));
        }

        serde_json::to_string_pretty(&history).map_err(|e| tool_err(format!("serialize: {e}")))
    }

    /// Find competing truths — active facts with the same key but different values.
    #[action(name = "competing_truths")]
    async fn competing_truths(&self, params: CompetingTruthsParams, _ctx: ()) -> Result<String> {
        let truths = self
            .service
            .competing_truths(&params.subject, &params.predicate)
            .await
            .map_err(|e| tool_err(format!("competing_truths failed: {e}")))?;

        if truths.len() <= 1 {
            return Ok(format!(
                "No competing truths for {}.{} ({})",
                params.subject,
                params.predicate,
                if truths.is_empty() {
                    "no facts"
                } else {
                    "single value"
                }
            ));
        }

        serde_json::to_string_pretty(&truths).map_err(|e| tool_err(format!("serialize: {e}")))
    }

    /// Compute a knowledge diff between two timestamps.
    #[action(name = "knowledge_diff")]
    async fn knowledge_diff(&self, params: KnowledgeDiffParams, _ctx: ()) -> Result<String> {
        let domains_vec: Option<Vec<&str>> = params.domain.as_ref().map(|d| vec![d.as_str()]);
        let diff = self
            .service
            .knowledge_diff(&params.from, &params.to, domains_vec.as_deref())
            .await
            .map_err(|e| tool_err(format!("knowledge_diff failed: {e}")))?;

        serde_json::to_string_pretty(&diff).map_err(|e| tool_err(format!("serialize: {e}")))
    }

    /// Find decision points where the user changed their mind.
    #[action(name = "decision_points")]
    async fn decision_points(&self, params: DecisionPointsParams, _ctx: ()) -> Result<String> {
        let limit = params.limit.unwrap_or(10) as usize;
        let points = self
            .service
            .decision_points(params.domain.as_deref(), limit)
            .await
            .map_err(|e| tool_err(format!("decision_points failed: {e}")))?;

        if points.is_empty() {
            return Ok("No decision points found — no facts have been revised yet.".to_string());
        }

        serde_json::to_string_pretty(&points).map_err(|e| tool_err(format!("serialize: {e}")))
    }
}

fn tool_err(msg: String) -> common::KlyntbotError {
    common::KlyntbotError::Tool(common::ToolError::ExecutionFailed(msg))
}
