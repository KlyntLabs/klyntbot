//! The LLM tool schema the Distiller exposes.
//!
//! The model is asked to call `record_observation` zero or more times, each
//! call producing one structured observation. The tool enum admits exactly
//! the 5 Distiller-writable `CodingKind` values — the 3 Reforge-only kinds
//! (`problem_solution_pattern`, `project_understanding`, `user_habit`) are
//! rejected at decode time.

use super::error::DistillerError;
use crate::facts::CodingKind;

/// Tool name the model must use.
pub const RECORD_OBSERVATION_TOOL_NAME: &str = "record_observation";

/// Scope the observation applies to.
#[derive(serde::Serialize, serde::Deserialize, Clone, Copy, Debug, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ObservationScope {
    /// Applies everywhere (user-level).
    Global,
    /// Applies to the current repo only.
    Repo,
}

/// One decoded observation the model emitted.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Observation {
    /// Kind — one of the 5 Distiller-writable kinds.
    pub kind: CodingKind,
    /// Subject (e.g. `"user"`, `"repo:<id>"`).
    pub subject: String,
    /// Predicate (e.g. `"prefers"`, `"framework"`, `"fixed"`).
    pub predicate: String,
    /// Object / value.
    pub object: String,
    /// 0.0–1.0 confidence, clamped on decode.
    pub confidence: f32,
    /// Scope partitioning.
    pub scope: ObservationScope,
    /// Free-text justification — stored in metadata, never user-surfaced.
    pub reasoning: String,
}

/// Build the `ToolDefinition` for the Distiller's Phase B LLM call.
#[must_use]
pub fn record_observation_tool_def() -> serde_json::Value {
    serde_json::json!({
        "name": RECORD_OBSERVATION_TOOL_NAME,
        "description": "Record one structured coding-memory observation. Emit zero or more calls per turn; emit nothing if nothing significant happened. Each call must use one of the 5 allowed kinds — NEVER problem_solution_pattern / project_understanding / user_habit (those are Reforge-only).",
        "input_schema": {
            "type": "object",
            "required": ["kind", "subject", "predicate", "object", "confidence", "scope", "reasoning"],
            "properties": {
                "kind": {
                    "type": "string",
                    "enum": ["fix_attempt", "style_preference", "workflow_pattern", "repo_context", "failure_pattern"]
                },
                "subject": { "type": "string" },
                "predicate": { "type": "string" },
                "object": { "type": "string" },
                "confidence": { "type": "number", "minimum": 0.0, "maximum": 1.0 },
                "scope": { "type": "string", "enum": ["global", "repo"] },
                "reasoning": { "type": "string" }
            },
            "additionalProperties": false
        }
    })
}

/// Decode a batch of tool-call arg-objects into `Observation`s.
/// `kind` values outside the 5-value `CodingKind` enum produce `DistillerError::LlmMalformed`.
pub fn decode_observations(
    raw: &[serde_json::Value],
) -> Result<Vec<Observation>, DistillerError> {
    let mut out = Vec::with_capacity(raw.len());
    for v in raw {
        let mut obs: Observation = serde_json::from_value(v.clone())
            .map_err(|e| DistillerError::LlmMalformed { detail: format!("observation decode: {e}") })?;
        obs.confidence = obs.confidence.clamp(0.0, 1.0);
        out.push(obs);
    }
    Ok(out)
}

/// Filter a list of provider tool calls down to the observations the Distiller cares about.
pub fn observations_from_tool_calls(
    calls: &[providers::types::ToolCall],
) -> Result<Vec<Observation>, DistillerError> {
    let args: Vec<serde_json::Value> = calls
        .iter()
        .filter(|c| c.name == RECORD_OBSERVATION_TOOL_NAME)
        .map(|c| c.arguments.clone())
        .collect();
    decode_observations(&args)
}
