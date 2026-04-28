//! SQLite schema shapes for opencode's `message` and `part` tables.
//!
//! Opencode (a Drizzle/Bun project) uses singular table names. The `message`
//! table stores the envelope (role, model, cost, tokens, cwd) as a JSON blob
//! in `data`. The `part` table stores content blocks (text, tool calls,
//! step-finish markers) keyed by `message_id`, also as JSON in `data`.

use serde::Deserialize;

/// One row from opencode's `message` table.
#[derive(Debug, Deserialize, sqlx::FromRow)]
pub struct MessageRow {
    /// Message id (UUID-like text).
    pub id: String,
    /// Session identifier (UUID-like text).
    pub session_id: String,
    /// Created-at unix milliseconds.
    pub time_created: i64,
    /// Updated-at unix milliseconds.
    pub time_updated: i64,
    /// JSON-encoded envelope. See [`MessageData`] for the parsed shape.
    pub data: String,
}

/// One row from opencode's `part` table.
#[derive(Debug, Deserialize, sqlx::FromRow)]
pub struct PartRow {
    /// Part id.
    pub id: String,
    /// Foreign key to `message.id`.
    pub message_id: String,
    /// Session id (denormalised on the row for query speed).
    pub session_id: String,
    /// Created-at unix milliseconds.
    pub time_created: i64,
    /// Updated-at unix milliseconds.
    pub time_updated: i64,
    /// JSON-encoded part body. See [`PartData`] for the discriminated shape.
    pub data: String,
}

/// Decoded shape of `message.data`. We keep this loose: only the fields we
/// need are required, everything else is ignored via serde defaults.
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MessageData {
    /// `user` | `assistant` | `system` | `tool`
    #[serde(default)]
    pub role: String,
    /// Optional working directory (only present on assistant turns).
    #[serde(default)]
    pub path: Option<MessagePath>,
    /// Provider model id (e.g. `k2p6`).
    #[serde(default, rename = "modelID")]
    pub model_id: Option<String>,
    /// Token usage breakdown for assistant turns.
    #[serde(default)]
    pub tokens: Option<MessageTokens>,
}

/// Path metadata attached to assistant turns.
#[derive(Debug, Deserialize)]
pub struct MessagePath {
    /// Working directory when the turn ran.
    #[serde(default)]
    pub cwd: String,
    /// Repository root (often equal to cwd).
    #[serde(default)]
    pub root: String,
}

/// Token-usage summary for a single assistant turn.
#[derive(Debug, Deserialize)]
pub struct MessageTokens {
    /// Prompt tokens.
    #[serde(default)]
    pub input: u32,
    /// Completion tokens.
    #[serde(default)]
    pub output: u32,
    /// Cache breakdown (anthropic-style read/write).
    #[serde(default)]
    pub cache: Option<MessageCache>,
}

/// Cache hit/miss breakdown.
#[derive(Debug, Deserialize)]
pub struct MessageCache {
    /// Cache read tokens.
    #[serde(default)]
    pub read: u32,
    /// Cache write tokens.
    #[serde(default)]
    pub write: u32,
}

/// Discriminated shape of `part.data`. We map a small subset of part types
/// onto our internal `EventKind`s; unknown types are skipped.
#[derive(Debug, Deserialize)]
#[serde(tag = "type", rename_all = "kebab-case")]
pub enum PartData {
    /// Plain text block — used for both user prompts and assistant messages
    /// depending on the parent message's role.
    Text {
        /// Text content of the block.
        #[serde(default)]
        text: String,
    },
    /// A tool invocation (call + result).
    Tool {
        /// Tool name.
        #[serde(default)]
        tool: String,
        /// Arbitrary tool state JSON (input + output).
        #[serde(default)]
        state: serde_json::Value,
    },
    /// Step-finish marker emitted at the end of an assistant turn.
    StepFinish {
        /// Stop reason (e.g. `"stop"`, `"tool_use"`).
        #[serde(default)]
        reason: String,
    },
    /// Anything else — skipped by the normaliser.
    #[serde(other)]
    Other,
}
