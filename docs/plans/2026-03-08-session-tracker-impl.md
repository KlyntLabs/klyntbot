# Session Tracker & Brainstorm Assistant — Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Build a real-time Claude Code session tracker with context-aware AI brainstorming, integrated into the klyntbot desktop app.

**Architecture:** New `feature-session-tracker` crate (L4) watches Claude Code JSONL session files via FSEvents, parses messages, indexes sessions in SQLite, builds context summaries, and optionally injects messages back. Brainstorming uses existing `providers` crate for direct model mode and `agent` runtime for agent mode. Desktop UI adds session sidebar, mirror view, and brainstorm panel.

**Tech Stack:** Rust (notify, sqlx, serde, tokio, futures-util), React/TypeScript (Tauri events, existing hooks), Tailwind v4 CSS tokens.

**Design Doc:** `docs/plans/2026-03-08-session-tracker-design.md`

---

## Batch 1: Crate Skeleton & Types (Tasks 1-3)

### Task 1: Create feature-session-tracker crate

**Files:**
- Create: `crates/feature-session-tracker/Cargo.toml`
- Create: `crates/feature-session-tracker/src/lib.rs`
- Modify: `Cargo.toml` (root workspace)

**Step 1: Create the crate directory**

```bash
mkdir -p crates/feature-session-tracker/src
mkdir -p crates/feature-session-tracker/migrations
```

**Step 2: Write Cargo.toml**

Create `crates/feature-session-tracker/Cargo.toml`:

```toml
[package]
name = "feature-session-tracker"
version.workspace = true
edition.workspace = true
license.workspace = true

[dependencies]
common.workspace = true
storage.workspace = true
tools-core.workspace = true
providers.workspace = true

async-trait.workspace = true
chrono = { workspace = true, features = ["serde"] }
futures-util.workspace = true
notify = "8"
serde = { workspace = true, features = ["derive"] }
serde_json.workspace = true
sqlx.workspace = true
thiserror.workspace = true
tokio = { workspace = true, features = ["sync", "fs", "io-util", "time"] }
tracing.workspace = true
uuid.workspace = true
```

**Step 3: Register in root workspace**

Modify `Cargo.toml` (root):
- Add `"crates/feature-session-tracker"` to `[workspace] members` list (alphabetically near other feature-* entries)
- Add `feature-session-tracker = { path = "crates/feature-session-tracker" }` to `[workspace.dependencies]`

**Step 4: Write minimal lib.rs**

Create `crates/feature-session-tracker/src/lib.rs`:

```rust
pub mod types;
pub mod parser;

use async_trait::async_trait;
use common::Result;
use serde_json::Value;
use tools_core::{DynTool, FeatureMigration, FeaturePackage, HealthStatus};

pub struct SessionTrackerFeature;

impl SessionTrackerFeature {
    pub fn migration_sql() -> &'static str {
        include_str!("../migrations/001_create_session_tracker.sql")
    }
}

#[async_trait]
impl FeaturePackage for SessionTrackerFeature {
    fn name(&self) -> &str {
        "session_tracker"
    }

    fn tools(&self) -> Vec<DynTool> {
        vec![]
    }

    fn migrations(&self) -> Vec<FeatureMigration> {
        vec![FeatureMigration {
            feature_name: "session_tracker".to_string(),
            version: 1,
            description: "Create session tracker tables".to_string(),
            sql: Self::migration_sql().to_string(),
        }]
    }

    fn config_key(&self) -> &str {
        "sessionTracker"
    }

    fn default_config(&self) -> Value {
        serde_json::json!({
            "enabled": true,
            "claudeDir": "~/.claude",
            "windowSize": 30,
            "chunkSize": 50
        })
    }

    async fn health_check(&self) -> Result<HealthStatus> {
        Ok(HealthStatus::Healthy)
    }
}
```

**Step 5: Verify it compiles**

```bash
cargo build -p feature-session-tracker
```

Expected: Compiles (will warn about missing migration file — that's Task 3).

**Step 6: Commit**

```bash
git add crates/feature-session-tracker/ Cargo.toml
git commit -m "feat(session-tracker): scaffold feature-session-tracker crate"
```

---

### Task 2: Define core types

**Files:**
- Create: `crates/feature-session-tracker/src/types.rs`

**Step 1: Write the types module**

Create `crates/feature-session-tracker/src/types.rs`:

```rust
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

// --- Claude Code JSONL types (deserialization) ---

/// Raw JSONL line from Claude Code session files.
/// Uses untagged enum because CC messages use a "type" field with varying schemas.
#[derive(Debug, Clone, Deserialize)]
#[serde(tag = "type")]
pub enum RawSessionLine {
    #[serde(rename = "user")]
    User {
        uuid: String,
        #[serde(rename = "sessionId")]
        session_id: String,
        message: RawMessage,
        #[serde(rename = "isMeta", default)]
        is_meta: bool,
        #[serde(rename = "gitBranch")]
        git_branch: Option<String>,
        timestamp: String,
        cwd: Option<String>,
    },
    #[serde(rename = "assistant")]
    Assistant {
        uuid: String,
        #[serde(rename = "sessionId")]
        session_id: Option<String>,
        message: RawMessage,
        timestamp: String,
    },
    #[serde(rename = "system")]
    System {
        uuid: String,
        subtype: Option<String>,
        content: String,
        level: Option<String>,
        timestamp: String,
    },
    #[serde(rename = "progress")]
    Progress {
        uuid: String,
        data: serde_json::Value,
        timestamp: String,
        #[serde(rename = "toolUseID")]
        tool_use_id: Option<String>,
    },
    #[serde(rename = "queue-operation")]
    QueueOperation {
        operation: String,
        content: Option<String>,
        timestamp: String,
        #[serde(rename = "sessionId")]
        session_id: Option<String>,
    },
    #[serde(rename = "file-history-snapshot")]
    FileHistorySnapshot {
        #[serde(rename = "messageId")]
        message_id: String,
    },
    #[serde(rename = "last-prompt")]
    LastPrompt {
        #[serde(rename = "lastPrompt")]
        last_prompt: Option<String>,
        #[serde(rename = "sessionId")]
        session_id: Option<String>,
    },
}

#[derive(Debug, Clone, Deserialize)]
pub struct RawMessage {
    pub role: Option<String>,
    pub content: RawContent,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(untagged)]
pub enum RawContent {
    Text(String),
    Blocks(Vec<ContentBlock>),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum ContentBlock {
    #[serde(rename = "text")]
    Text { text: String },
    #[serde(rename = "tool_use")]
    ToolUse {
        id: String,
        name: String,
        input: serde_json::Value,
    },
    #[serde(rename = "tool_result")]
    ToolResult {
        tool_use_id: String,
        content: serde_json::Value,
        #[serde(default)]
        is_error: bool,
    },
}

// --- Application types (serialized to frontend) ---

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", tag = "type")]
pub enum SessionMessage {
    User {
        uuid: String,
        text: String,
        timestamp: DateTime<Utc>,
        is_meta: bool,
    },
    Assistant {
        uuid: String,
        content: Vec<ContentBlock>,
        timestamp: DateTime<Utc>,
    },
    System {
        uuid: String,
        subtype: Option<String>,
        content: String,
        timestamp: DateTime<Utc>,
    },
    Progress {
        uuid: String,
        data: serde_json::Value,
        tool_use_id: Option<String>,
        timestamp: DateTime<Utc>,
    },
    QueueOperation {
        operation: String,
        content: Option<String>,
        timestamp: DateTime<Utc>,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub enum SessionStatus {
    Active,
    Idle,
    Completed,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TrackedSession {
    pub session_id: String,
    pub project_path: String,
    pub project_name: String,
    pub jsonl_path: String,
    pub status: SessionStatus,
    pub first_message_preview: Option<String>,
    pub message_count: i64,
    pub git_branch: Option<String>,
    pub last_activity: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PinnedMessage {
    pub id: i64,
    pub session_id: String,
    pub message_uuid: String,
    pub message_content: String,
    pub message_role: String,
    pub pin_order: i64,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BrainstormConversation {
    pub id: String,
    pub session_id: String,
    pub title: Option<String>,
    pub mode: BrainstormMode,
    pub model_key: Option<String>,
    pub agent_profile: Option<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub enum BrainstormMode {
    DirectModel,
    Agent,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BrainstormMessage {
    pub id: String,
    pub conversation_id: String,
    pub role: String,
    pub content: String,
    pub is_result_block: bool,
    pub edited_content: Option<String>,
    pub sent_to_cc: bool,
    pub created_at: DateTime<Utc>,
}

// --- History JSONL types ---

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HistoryEntry {
    pub display: String,
    pub timestamp: i64,
    pub project: String,
    pub session_id: String,
}

// --- Tauri event payloads ---

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SessionMessagePayload {
    pub session_id: String,
    pub message: SessionMessage,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SessionStatusPayload {
    pub session_id: String,
    pub status: SessionStatus,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BrainstormTokenPayload {
    pub conversation_id: String,
    pub token: String,
}

// --- Context builder types ---

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SessionContext {
    pub rolling_summary: String,
    pub pinned_messages: Vec<PinnedMessage>,
    pub recent_messages: Vec<SessionMessage>,
    pub total_messages: i64,
    pub estimated_tokens: usize,
}

#[derive(Debug, Clone)]
pub struct ChunkSummary {
    pub session_id: String,
    pub chunk_start: i64,
    pub chunk_end: i64,
    pub summary: String,
    pub files_touched: Vec<String>,
    pub key_decisions: Vec<String>,
    pub rolling_summary: String,
}
```

**Step 2: Verify it compiles**

```bash
cargo build -p feature-session-tracker
```

**Step 3: Commit**

```bash
git add crates/feature-session-tracker/src/types.rs
git commit -m "feat(session-tracker): define core types for session tracking and brainstorming"
```

---

### Task 3: Write migration SQL

**Files:**
- Create: `crates/feature-session-tracker/migrations/001_create_session_tracker.sql`

**Step 1: Write migration**

Create `crates/feature-session-tracker/migrations/001_create_session_tracker.sql`:

```sql
-- Session tracker feature tables

CREATE TABLE IF NOT EXISTS tracked_sessions (
    session_id TEXT PRIMARY KEY,
    project_path TEXT NOT NULL,
    project_name TEXT NOT NULL,
    jsonl_path TEXT NOT NULL,
    status TEXT NOT NULL DEFAULT 'idle',
    first_message_preview TEXT,
    message_count INTEGER NOT NULL DEFAULT 0,
    git_branch TEXT,
    last_activity TIMESTAMP,
    file_offset INTEGER NOT NULL DEFAULT 0,
    created_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP
);

CREATE INDEX IF NOT EXISTS idx_tracked_sessions_project
    ON tracked_sessions(project_name);

CREATE INDEX IF NOT EXISTS idx_tracked_sessions_status
    ON tracked_sessions(status);

CREATE INDEX IF NOT EXISTS idx_tracked_sessions_last_activity
    ON tracked_sessions(last_activity);

CREATE TABLE IF NOT EXISTS pinned_messages (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    session_id TEXT NOT NULL REFERENCES tracked_sessions(session_id) ON DELETE CASCADE,
    message_uuid TEXT NOT NULL,
    message_content TEXT NOT NULL,
    message_role TEXT NOT NULL,
    pin_order INTEGER NOT NULL,
    created_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP,
    UNIQUE(session_id, message_uuid)
);

CREATE INDEX IF NOT EXISTS idx_pinned_messages_session
    ON pinned_messages(session_id);

CREATE TABLE IF NOT EXISTS brainstorm_conversations (
    id TEXT PRIMARY KEY,
    session_id TEXT NOT NULL REFERENCES tracked_sessions(session_id) ON DELETE CASCADE,
    title TEXT,
    mode TEXT NOT NULL,
    model_key TEXT,
    agent_profile TEXT,
    created_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP,
    updated_at TIMESTAMP
);

CREATE INDEX IF NOT EXISTS idx_brainstorm_conversations_session
    ON brainstorm_conversations(session_id);

CREATE TABLE IF NOT EXISTS brainstorm_messages (
    id TEXT PRIMARY KEY,
    conversation_id TEXT NOT NULL REFERENCES brainstorm_conversations(id) ON DELETE CASCADE,
    role TEXT NOT NULL,
    content TEXT NOT NULL,
    is_result_block INTEGER NOT NULL DEFAULT 0,
    edited_content TEXT,
    sent_to_cc INTEGER NOT NULL DEFAULT 0,
    created_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP
);

CREATE INDEX IF NOT EXISTS idx_brainstorm_messages_conversation
    ON brainstorm_messages(conversation_id);

CREATE TABLE IF NOT EXISTS session_summaries (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    session_id TEXT NOT NULL REFERENCES tracked_sessions(session_id) ON DELETE CASCADE,
    chunk_start INTEGER NOT NULL,
    chunk_end INTEGER NOT NULL,
    summary TEXT NOT NULL,
    files_touched TEXT,
    key_decisions TEXT,
    rolling_summary TEXT,
    created_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP
);

CREATE INDEX IF NOT EXISTS idx_session_summaries_session
    ON session_summaries(session_id);
```

**Step 2: Verify crate compiles with migration**

```bash
cargo build -p feature-session-tracker
```

**Step 3: Commit**

```bash
git add crates/feature-session-tracker/migrations/
git commit -m "feat(session-tracker): add migration for session tracker tables"
```

---

## Batch 2: JSONL Parser (Tasks 4-5)

### Task 4: Implement JSONL parser

**Files:**
- Create: `crates/feature-session-tracker/src/parser.rs`
- Modify: `crates/feature-session-tracker/src/lib.rs` (add module)

**Step 1: Write parser tests first**

Create `crates/feature-session-tracker/src/parser.rs`:

```rust
use crate::types::{ContentBlock, RawContent, RawSessionLine, SessionMessage};
use chrono::{DateTime, Utc};
use tracing::warn;

/// Parse a single JSONL line into a SessionMessage.
/// Returns None for lines that should be filtered out (snapshots, last-prompt, unparseable).
pub fn parse_line(line: &str) -> Option<SessionMessage> {
    let line = line.trim();
    if line.is_empty() {
        return None;
    }

    let raw: RawSessionLine = match serde_json::from_str(line) {
        Ok(r) => r,
        Err(e) => {
            warn!("Failed to parse session line: {e}");
            return None;
        }
    };

    convert_raw(raw)
}

/// Parse all lines from a JSONL string, returning only valid SessionMessages.
pub fn parse_lines(content: &str) -> Vec<SessionMessage> {
    content.lines().filter_map(parse_line).collect()
}

fn convert_raw(raw: RawSessionLine) -> Option<SessionMessage> {
    match raw {
        RawSessionLine::User {
            uuid,
            message,
            is_meta,
            timestamp,
            ..
        } => {
            let ts = parse_timestamp(&timestamp)?;
            let text = extract_text(&message.content);
            Some(SessionMessage::User {
                uuid,
                text,
                timestamp: ts,
                is_meta,
            })
        }
        RawSessionLine::Assistant {
            uuid,
            message,
            timestamp,
            ..
        } => {
            let ts = parse_timestamp(&timestamp)?;
            let content = match message.content {
                RawContent::Text(t) => vec![ContentBlock::Text { text: t }],
                RawContent::Blocks(blocks) => blocks,
            };
            Some(SessionMessage::Assistant {
                uuid,
                content,
                timestamp: ts,
            })
        }
        RawSessionLine::System {
            uuid,
            subtype,
            content,
            timestamp,
            ..
        } => {
            let ts = parse_timestamp(&timestamp)?;
            Some(SessionMessage::System {
                uuid,
                subtype,
                content,
                timestamp: ts,
            })
        }
        RawSessionLine::Progress {
            uuid,
            data,
            timestamp,
            tool_use_id,
        } => {
            let ts = parse_timestamp(&timestamp)?;
            Some(SessionMessage::Progress {
                uuid,
                data,
                tool_use_id,
                timestamp: ts,
            })
        }
        RawSessionLine::QueueOperation {
            operation,
            content,
            timestamp,
            ..
        } => {
            let ts = parse_timestamp(&timestamp)?;
            Some(SessionMessage::QueueOperation {
                operation,
                content,
                timestamp: ts,
            })
        }
        // Filter out non-conversational types
        RawSessionLine::FileHistorySnapshot { .. } | RawSessionLine::LastPrompt { .. } => None,
    }
}

fn extract_text(content: &RawContent) -> String {
    match content {
        RawContent::Text(t) => t.clone(),
        RawContent::Blocks(blocks) => blocks
            .iter()
            .filter_map(|b| match b {
                ContentBlock::Text { text } => Some(text.as_str()),
                _ => None,
            })
            .collect::<Vec<_>>()
            .join("\n"),
    }
}

fn parse_timestamp(ts: &str) -> Option<DateTime<Utc>> {
    ts.parse::<DateTime<Utc>>()
        .map_err(|e| warn!("Failed to parse timestamp '{ts}': {e}"))
        .ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_user_message() {
        let line = r#"{"parentUuid":null,"isSidechain":false,"userType":"external","cwd":"/home/user/project","sessionId":"abc-123","version":"2.1.71","gitBranch":"main","type":"user","message":{"role":"human","content":[{"type":"text","text":"Hello world"}]},"isMeta":false,"uuid":"msg-001","timestamp":"2026-03-08T10:00:00.000Z"}"#;

        let msg = parse_line(line).unwrap();
        match msg {
            SessionMessage::User {
                uuid,
                text,
                is_meta,
                ..
            } => {
                assert_eq!(uuid, "msg-001");
                assert_eq!(text, "Hello world");
                assert!(!is_meta);
            }
            _ => panic!("Expected User message"),
        }
    }

    #[test]
    fn parse_assistant_with_tool_use() {
        let line = r#"{"parentUuid":"msg-001","isSidechain":false,"userType":"external","message":{"role":"assistant","content":[{"type":"text","text":"Let me read that file."},{"type":"tool_use","id":"tool-1","name":"Read","input":{"file_path":"/src/lib.rs"}}]},"type":"assistant","uuid":"msg-002","timestamp":"2026-03-08T10:00:01.000Z"}"#;

        let msg = parse_line(line).unwrap();
        match msg {
            SessionMessage::Assistant { uuid, content, .. } => {
                assert_eq!(uuid, "msg-002");
                assert_eq!(content.len(), 2);
                assert!(matches!(&content[0], ContentBlock::Text { text } if text == "Let me read that file."));
                assert!(matches!(&content[1], ContentBlock::ToolUse { name, .. } if name == "Read"));
            }
            _ => panic!("Expected Assistant message"),
        }
    }

    #[test]
    fn parse_queue_operation() {
        let line = r#"{"type":"queue-operation","operation":"enqueue","timestamp":"2026-03-08T10:00:02.000Z","sessionId":"abc-123","content":"Please implement this feature"}"#;

        let msg = parse_line(line).unwrap();
        match msg {
            SessionMessage::QueueOperation {
                operation,
                content,
                ..
            } => {
                assert_eq!(operation, "enqueue");
                assert_eq!(content.unwrap(), "Please implement this feature");
            }
            _ => panic!("Expected QueueOperation"),
        }
    }

    #[test]
    fn parse_file_history_snapshot_returns_none() {
        let line = r#"{"type":"file-history-snapshot","messageId":"snap-001","snapshot":{},"isSnapshotUpdate":false}"#;
        assert!(parse_line(line).is_none());
    }

    #[test]
    fn parse_empty_line_returns_none() {
        assert!(parse_line("").is_none());
        assert!(parse_line("   ").is_none());
    }

    #[test]
    fn parse_invalid_json_returns_none() {
        assert!(parse_line("{not valid json}").is_none());
    }

    #[test]
    fn parse_user_string_content() {
        let line = r#"{"type":"user","uuid":"msg-003","sessionId":"abc","message":{"role":"human","content":"simple string message"},"isMeta":false,"timestamp":"2026-03-08T10:00:00.000Z"}"#;

        let msg = parse_line(line).unwrap();
        match msg {
            SessionMessage::User { text, .. } => {
                assert_eq!(text, "simple string message");
            }
            _ => panic!("Expected User message"),
        }
    }

    #[test]
    fn parse_progress_message() {
        let line = r#"{"type":"progress","uuid":"prog-001","data":{"type":"hook_progress","hookEvent":"SessionStart","hookName":"clear"},"timestamp":"2026-03-08T10:00:00.000Z","toolUseID":"tool-99"}"#;

        let msg = parse_line(line).unwrap();
        match msg {
            SessionMessage::Progress {
                uuid,
                tool_use_id,
                ..
            } => {
                assert_eq!(uuid, "prog-001");
                assert_eq!(tool_use_id.unwrap(), "tool-99");
            }
            _ => panic!("Expected Progress message"),
        }
    }

    #[test]
    fn parse_lines_filters_correctly() {
        let content = r#"{"type":"file-history-snapshot","messageId":"snap-001","snapshot":{},"isSnapshotUpdate":false}
{"type":"user","uuid":"msg-001","sessionId":"abc","message":{"role":"human","content":"Hello"},"isMeta":false,"timestamp":"2026-03-08T10:00:00.000Z"}
{"type":"assistant","uuid":"msg-002","message":{"role":"assistant","content":[{"type":"text","text":"Hi there"}]},"timestamp":"2026-03-08T10:00:01.000Z"}
invalid json line
{"type":"last-prompt","lastPrompt":"Hello","sessionId":"abc"}"#;

        let messages = parse_lines(content);
        assert_eq!(messages.len(), 2); // Only user + assistant
    }
}
```

**Step 2: Run tests to verify they pass**

```bash
cargo nextest run -p feature-session-tracker
```

Expected: All 8 tests pass.

**Step 3: Commit**

```bash
git add crates/feature-session-tracker/src/parser.rs crates/feature-session-tracker/src/lib.rs
git commit -m "feat(session-tracker): implement JSONL parser with tests"
```

---

### Task 5: Implement session discovery and history parsing

**Files:**
- Create: `crates/feature-session-tracker/src/discovery.rs`
- Modify: `crates/feature-session-tracker/src/lib.rs` (add module)

**Step 1: Write discovery module**

Create `crates/feature-session-tracker/src/discovery.rs`:

```rust
use crate::types::{HistoryEntry, SessionStatus, TrackedSession};
use chrono::{DateTime, TimeZone, Utc};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use tokio::fs;
use tracing::{debug, warn};

/// Default Claude Code data directory
pub fn default_claude_dir() -> PathBuf {
    dirs::home_dir()
        .unwrap_or_else(|| PathBuf::from("~"))
        .join(".claude")
}

/// Discover all sessions across all projects in the Claude directory.
pub async fn discover_sessions(claude_dir: &Path) -> Vec<TrackedSession> {
    let projects_dir = claude_dir.join("projects");
    let history = load_history(claude_dir).await;

    let mut sessions = Vec::new();

    let mut entries = match fs::read_dir(&projects_dir).await {
        Ok(e) => e,
        Err(e) => {
            warn!("Cannot read Claude projects dir {}: {e}", projects_dir.display());
            return sessions;
        }
    };

    while let Ok(Some(project_entry)) = entries.next_entry().await {
        let project_dir = project_entry.path();
        if !project_dir.is_dir() {
            continue;
        }

        let project_name = decode_project_name(
            project_dir.file_name().unwrap_or_default().to_str().unwrap_or(""),
        );

        let mut jsonl_entries = match fs::read_dir(&project_dir).await {
            Ok(e) => e,
            Err(_) => continue,
        };

        while let Ok(Some(file_entry)) = jsonl_entries.next_entry().await {
            let path = file_entry.path();
            if path.extension().and_then(|e| e.to_str()) != Some("jsonl") {
                continue;
            }

            let session_id = path
                .file_stem()
                .and_then(|s| s.to_str())
                .unwrap_or("")
                .to_string();

            if session_id.is_empty() {
                continue;
            }

            let metadata = file_entry.metadata().await.ok();
            let last_modified = metadata
                .as_ref()
                .and_then(|m| m.modified().ok())
                .and_then(|t| DateTime::<Utc>::from(t).into());

            let preview = history.get(&session_id).map(|h| h.display.clone());

            let status = match &last_modified {
                Some(t) => {
                    let age = Utc::now() - *t;
                    if age.num_seconds() < 60 {
                        SessionStatus::Active
                    } else if age.num_minutes() < 30 {
                        SessionStatus::Idle
                    } else {
                        SessionStatus::Completed
                    }
                }
                None => SessionStatus::Completed,
            };

            sessions.push(TrackedSession {
                session_id,
                project_path: project_name.clone(),
                project_name: extract_short_name(&project_name),
                jsonl_path: path.to_string_lossy().to_string(),
                status,
                first_message_preview: preview,
                message_count: 0,
                git_branch: None,
                last_activity: last_modified,
                created_at: last_modified.unwrap_or_else(Utc::now),
            });
        }
    }

    // Sort by last activity (most recent first)
    sessions.sort_by(|a, b| b.last_activity.cmp(&a.last_activity));
    sessions
}

/// Load history.jsonl to map session IDs to their display prompts.
async fn load_history(claude_dir: &Path) -> HashMap<String, HistoryEntry> {
    let history_path = claude_dir.join("history.jsonl");
    let content = match fs::read_to_string(&history_path).await {
        Ok(c) => c,
        Err(_) => return HashMap::new(),
    };

    let mut map = HashMap::new();
    for line in content.lines() {
        if let Ok(entry) = serde_json::from_str::<HistoryEntry>(line) {
            map.insert(entry.session_id.clone(), entry);
        }
    }
    map
}

/// Decode project directory name back to a path.
/// Claude uses `-` as separator: "-Users-jayden-Projects-foo" → "/Users/jayden/Projects/foo"
fn decode_project_name(encoded: &str) -> String {
    if encoded.starts_with('-') {
        encoded.replacen('-', "/", 1).replace('-', "/")
    } else {
        encoded.replace('-', "/")
    }
}

/// Extract the last path component as a short project name.
fn extract_short_name(project_path: &str) -> String {
    project_path
        .trim_end_matches('/')
        .rsplit('/')
        .next()
        .unwrap_or(project_path)
        .to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_decode_project_name() {
        assert_eq!(
            decode_project_name("-Users-jayden-Projects-Klynt-nanobot-klyntbot"),
            "/Users/jayden/Projects/Klynt/nanobot/klyntbot"
        );
    }

    #[test]
    fn test_extract_short_name() {
        assert_eq!(
            extract_short_name("/Users/jayden/Projects/Klynt/nanobot/klyntbot"),
            "klyntbot"
        );
        assert_eq!(extract_short_name("simple"), "simple");
    }
}
```

**Step 2: Add `dirs` dependency to Cargo.toml**

Add to `crates/feature-session-tracker/Cargo.toml` under `[dependencies]`:

```toml
dirs = "6"
```

**Step 3: Add module to lib.rs**

Add `pub mod discovery;` to `lib.rs`.

**Step 4: Run tests**

```bash
cargo nextest run -p feature-session-tracker
```

Expected: All tests pass.

**Step 5: Commit**

```bash
git add crates/feature-session-tracker/
git commit -m "feat(session-tracker): implement session discovery and history parsing"
```

---

## Batch 3: Session Watcher (Tasks 6-7)

### Task 6: Implement FSEvent-based session watcher

**Files:**
- Create: `crates/feature-session-tracker/src/watcher.rs`
- Modify: `crates/feature-session-tracker/src/lib.rs` (add module)

**Step 1: Write the watcher**

Create `crates/feature-session-tracker/src/watcher.rs`:

```rust
use crate::parser;
use crate::types::SessionMessage;
use notify::{Config, Event, EventKind, RecommendedWatcher, RecursiveMode, Watcher};
use std::collections::HashMap;
use std::io::{BufRead, BufReader, Seek, SeekFrom};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use tokio::sync::{mpsc, Mutex};
use tracing::{debug, error, info, warn};

/// Messages emitted by the SessionWatcher.
#[derive(Debug, Clone)]
pub enum WatchEvent {
    /// New message parsed from a session file.
    NewMessage {
        session_id: String,
        message: SessionMessage,
    },
    /// A new session file was discovered.
    NewSession {
        session_id: String,
        jsonl_path: PathBuf,
    },
    /// A session file was modified (for status tracking).
    FileModified {
        session_id: String,
    },
}

/// Watches Claude Code session JSONL files for real-time updates.
pub struct SessionWatcher {
    _watcher: RecommendedWatcher,
    offsets: Arc<Mutex<HashMap<PathBuf, u64>>>,
}

impl SessionWatcher {
    /// Start watching a Claude projects directory.
    /// Returns the watcher and a receiver for watch events.
    pub fn start(
        claude_projects_dir: &Path,
        event_tx: mpsc::UnboundedSender<WatchEvent>,
    ) -> Result<Self, notify::Error> {
        let offsets: Arc<Mutex<HashMap<PathBuf, u64>>> = Arc::new(Mutex::new(HashMap::new()));
        let offsets_clone = offsets.clone();
        let tx = event_tx.clone();

        let mut watcher = RecommendedWatcher::new(
            move |result: Result<Event, notify::Error>| {
                let event = match result {
                    Ok(e) => e,
                    Err(e) => {
                        error!("Watch error: {e}");
                        return;
                    }
                };

                // Only care about modifications and creations
                match event.kind {
                    EventKind::Modify(_) | EventKind::Create(_) => {}
                    _ => return,
                }

                for path in &event.paths {
                    if path.extension().and_then(|e| e.to_str()) != Some("jsonl") {
                        continue;
                    }

                    let session_id = match path.file_stem().and_then(|s| s.to_str()) {
                        Some(s) => s.to_string(),
                        None => continue,
                    };

                    if matches!(event.kind, EventKind::Create(_)) {
                        let _ = tx.send(WatchEvent::NewSession {
                            session_id: session_id.clone(),
                            jsonl_path: path.clone(),
                        });
                    }

                    let _ = tx.send(WatchEvent::FileModified {
                        session_id: session_id.clone(),
                    });

                    // Read new lines from the file
                    let offsets = offsets_clone.clone();
                    let path = path.clone();
                    let tx = tx.clone();
                    // Note: notify callbacks are sync, so we do blocking file reads here.
                    // The files are small (line reads) so this is acceptable.
                    std::thread::spawn(move || {
                        let rt = tokio::runtime::Handle::try_current();
                        let mut offsets_guard = if let Ok(rt) = rt {
                            rt.block_on(offsets.lock())
                        } else {
                            // Fallback: spin-lock (shouldn't happen in practice)
                            loop {
                                if let Ok(g) = offsets.try_lock() {
                                    break g;
                                }
                                std::thread::yield_now();
                            }
                        };
                        if let Some(messages) =
                            read_new_lines(&path, &mut offsets_guard)
                        {
                            for msg in messages {
                                let _ = tx.send(WatchEvent::NewMessage {
                                    session_id: session_id.clone(),
                                    message: msg,
                                });
                            }
                        }
                    });
                }
            },
            Config::default(),
        )?;

        watcher.watch(claude_projects_dir, RecursiveMode::Recursive)?;

        info!(
            "Session watcher started on {}",
            claude_projects_dir.display()
        );

        Ok(Self {
            _watcher: watcher,
            offsets,
        })
    }

    /// Initialize offsets for already-known session files (seek to end).
    /// Call this after discovery to avoid replaying existing content.
    pub async fn init_offsets(&self, jsonl_paths: &[PathBuf]) {
        let mut offsets = self.offsets.lock().await;
        for path in jsonl_paths {
            if let Ok(metadata) = std::fs::metadata(path) {
                offsets.insert(path.clone(), metadata.len());
            }
        }
    }
}

/// Read new lines from a JSONL file starting at the stored offset.
fn read_new_lines(
    path: &Path,
    offsets: &mut HashMap<PathBuf, u64>,
) -> Option<Vec<SessionMessage>> {
    let file = std::fs::File::open(path).ok()?;
    let file_len = file.metadata().ok()?.len();

    let offset = offsets.get(path).copied().unwrap_or(0);

    if file_len <= offset {
        return None;
    }

    let mut reader = BufReader::new(file);
    reader.seek(SeekFrom::Start(offset)).ok()?;

    let mut messages = Vec::new();
    let mut new_offset = offset;

    let mut line = String::new();
    loop {
        line.clear();
        match reader.read_line(&mut line) {
            Ok(0) => break,
            Ok(n) => {
                new_offset += n as u64;
                if let Some(msg) = parser::parse_line(&line) {
                    messages.push(msg);
                }
            }
            Err(e) => {
                warn!("Error reading {}: {e}", path.display());
                break;
            }
        }
    }

    offsets.insert(path.to_path_buf(), new_offset);

    if messages.is_empty() {
        None
    } else {
        Some(messages)
    }
}
```

**Step 2: Add module to lib.rs**

Add `pub mod watcher;` to `lib.rs`.

**Step 3: Verify it compiles**

```bash
cargo build -p feature-session-tracker
```

**Step 4: Commit**

```bash
git add crates/feature-session-tracker/src/watcher.rs crates/feature-session-tracker/src/lib.rs
git commit -m "feat(session-tracker): implement FSEvent-based session watcher"
```

---

### Task 7: Implement session injector (write-back to Claude Code)

**Files:**
- Create: `crates/feature-session-tracker/src/injector.rs`
- Modify: `crates/feature-session-tracker/src/lib.rs` (add module)

**Step 1: Write injector with tests**

Create `crates/feature-session-tracker/src/injector.rs`:

```rust
use chrono::Utc;
use serde_json::json;
use std::path::Path;
use tokio::fs::OpenOptions;
use tokio::io::AsyncWriteExt;
use tracing::info;

/// Inject a message into a Claude Code session by writing a queue-operation enqueue.
pub async fn send_to_session(
    jsonl_path: &Path,
    session_id: &str,
    content: &str,
) -> std::io::Result<()> {
    let entry = json!({
        "type": "queue-operation",
        "operation": "enqueue",
        "timestamp": Utc::now().to_rfc3339(),
        "sessionId": session_id,
        "content": content
    });

    let mut line = serde_json::to_string(&entry)?;
    line.push('\n');

    let mut file = OpenOptions::new()
        .create(false)
        .append(true)
        .open(jsonl_path)
        .await?;

    file.write_all(line.as_bytes()).await?;
    file.flush().await?;

    info!(
        "Injected message into session {session_id} at {}",
        jsonl_path.display()
    );

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::NamedTempFile;
    use tokio::fs;

    #[tokio::test]
    async fn test_send_to_session() {
        let tmp = NamedTempFile::new().unwrap();
        let path = tmp.path().to_path_buf();

        // Write an initial line so the file exists with content
        fs::write(&path, "{\"type\":\"user\"}\n").await.unwrap();

        send_to_session(&path, "test-session-123", "Hello from klyntbot")
            .await
            .unwrap();

        let content = fs::read_to_string(&path).await.unwrap();
        let lines: Vec<&str> = content.lines().collect();
        assert_eq!(lines.len(), 2);

        let injected: serde_json::Value = serde_json::from_str(lines[1]).unwrap();
        assert_eq!(injected["type"], "queue-operation");
        assert_eq!(injected["operation"], "enqueue");
        assert_eq!(injected["sessionId"], "test-session-123");
        assert_eq!(injected["content"], "Hello from klyntbot");
    }

    #[tokio::test]
    async fn test_send_to_nonexistent_file_fails() {
        let result = send_to_session(
            Path::new("/tmp/nonexistent-session-tracker-test.jsonl"),
            "test",
            "hello",
        )
        .await;
        assert!(result.is_err());
    }
}
```

**Step 2: Add `tempfile` as a dev dependency**

Add to `crates/feature-session-tracker/Cargo.toml`:

```toml
[dev-dependencies]
tempfile = "3"
tokio = { workspace = true, features = ["rt", "macros"] }
```

**Step 3: Add module to lib.rs**

Add `pub mod injector;` to `lib.rs`.

**Step 4: Run tests**

```bash
cargo nextest run -p feature-session-tracker
```

Expected: All tests pass.

**Step 5: Commit**

```bash
git add crates/feature-session-tracker/
git commit -m "feat(session-tracker): implement session injector for Claude Code write-back"
```

---

## Batch 4: Repository Layer (Tasks 8-9)

### Task 8: Implement session tracker repos

**Files:**
- Create: `crates/feature-session-tracker/src/repos.rs`
- Modify: `crates/feature-session-tracker/src/lib.rs` (add module)

**Step 1: Write repo implementations**

Create `crates/feature-session-tracker/src/repos.rs`:

```rust
use crate::types::{
    BrainstormConversation, BrainstormMessage, BrainstormMode, PinnedMessage, SessionStatus,
    TrackedSession,
};
use chrono::{DateTime, Utc};
use sqlx::SqlitePool;
use storage::StorageError;

type Result<T> = std::result::Result<T, StorageError>;

#[derive(Clone)]
pub struct SessionTrackerRepos {
    pool: SqlitePool,
}

impl SessionTrackerRepos {
    pub fn new(pool: SqlitePool) -> Self {
        Self { pool }
    }

    // --- Tracked Sessions ---

    pub async fn upsert_session(&self, session: &TrackedSession) -> Result<()> {
        let status = serde_json::to_string(&session.status).unwrap_or_default();
        let status = status.trim_matches('"');

        sqlx::query(
            r#"INSERT INTO tracked_sessions (session_id, project_path, project_name, jsonl_path, status, first_message_preview, message_count, git_branch, last_activity, created_at)
               VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)
               ON CONFLICT(session_id) DO UPDATE SET
                 status = ?5,
                 first_message_preview = COALESCE(?6, tracked_sessions.first_message_preview),
                 message_count = ?7,
                 git_branch = COALESCE(?8, tracked_sessions.git_branch),
                 last_activity = ?9"#,
        )
        .bind(&session.session_id)
        .bind(&session.project_path)
        .bind(&session.project_name)
        .bind(&session.jsonl_path)
        .bind(status)
        .bind(&session.first_message_preview)
        .bind(session.message_count)
        .bind(&session.git_branch)
        .bind(session.last_activity)
        .bind(session.created_at)
        .execute(&self.pool)
        .await
        .map_err(StorageError::from)?;

        Ok(())
    }

    pub async fn list_sessions(&self) -> Result<Vec<TrackedSession>> {
        let rows = sqlx::query_as::<_, TrackedSessionRow>(
            "SELECT * FROM tracked_sessions ORDER BY last_activity DESC",
        )
        .fetch_all(&self.pool)
        .await
        .map_err(StorageError::from)?;

        Ok(rows.into_iter().map(Into::into).collect())
    }

    pub async fn get_session(&self, session_id: &str) -> Result<Option<TrackedSession>> {
        let row = sqlx::query_as::<_, TrackedSessionRow>(
            "SELECT * FROM tracked_sessions WHERE session_id = ?",
        )
        .bind(session_id)
        .fetch_optional(&self.pool)
        .await
        .map_err(StorageError::from)?;

        Ok(row.map(Into::into))
    }

    pub async fn update_session_status(
        &self,
        session_id: &str,
        status: &SessionStatus,
    ) -> Result<()> {
        let status_str = serde_json::to_string(status).unwrap_or_default();
        let status_str = status_str.trim_matches('"');

        sqlx::query("UPDATE tracked_sessions SET status = ?, last_activity = ? WHERE session_id = ?")
            .bind(status_str)
            .bind(Utc::now())
            .bind(session_id)
            .execute(&self.pool)
            .await
            .map_err(StorageError::from)?;

        Ok(())
    }

    pub async fn update_file_offset(&self, session_id: &str, offset: i64) -> Result<()> {
        sqlx::query("UPDATE tracked_sessions SET file_offset = ? WHERE session_id = ?")
            .bind(offset)
            .bind(session_id)
            .execute(&self.pool)
            .await
            .map_err(StorageError::from)?;

        Ok(())
    }

    pub async fn increment_message_count(&self, session_id: &str) -> Result<()> {
        sqlx::query(
            "UPDATE tracked_sessions SET message_count = message_count + 1, last_activity = ? WHERE session_id = ?",
        )
        .bind(Utc::now())
        .bind(session_id)
        .execute(&self.pool)
        .await
        .map_err(StorageError::from)?;

        Ok(())
    }

    // --- Pinned Messages ---

    pub async fn pin_message(&self, pin: &PinnedMessage) -> Result<()> {
        sqlx::query(
            r#"INSERT INTO pinned_messages (session_id, message_uuid, message_content, message_role, pin_order)
               VALUES (?1, ?2, ?3, ?4, ?5)
               ON CONFLICT(session_id, message_uuid) DO UPDATE SET pin_order = ?5"#,
        )
        .bind(&pin.session_id)
        .bind(&pin.message_uuid)
        .bind(&pin.message_content)
        .bind(&pin.message_role)
        .bind(pin.pin_order)
        .execute(&self.pool)
        .await
        .map_err(StorageError::from)?;

        Ok(())
    }

    pub async fn unpin_message(&self, id: i64) -> Result<()> {
        sqlx::query("DELETE FROM pinned_messages WHERE id = ?")
            .bind(id)
            .execute(&self.pool)
            .await
            .map_err(StorageError::from)?;

        Ok(())
    }

    pub async fn list_pins(&self, session_id: &str) -> Result<Vec<PinnedMessage>> {
        let rows = sqlx::query_as::<_, PinnedMessageRow>(
            "SELECT * FROM pinned_messages WHERE session_id = ? ORDER BY pin_order",
        )
        .bind(session_id)
        .fetch_all(&self.pool)
        .await
        .map_err(StorageError::from)?;

        Ok(rows.into_iter().map(Into::into).collect())
    }

    // --- Brainstorm Conversations ---

    pub async fn create_conversation(&self, conv: &BrainstormConversation) -> Result<()> {
        let mode = match conv.mode {
            BrainstormMode::DirectModel => "direct_model",
            BrainstormMode::Agent => "agent",
        };

        sqlx::query(
            r#"INSERT INTO brainstorm_conversations (id, session_id, title, mode, model_key, agent_profile, created_at)
               VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)"#,
        )
        .bind(&conv.id)
        .bind(&conv.session_id)
        .bind(&conv.title)
        .bind(mode)
        .bind(&conv.model_key)
        .bind(&conv.agent_profile)
        .bind(conv.created_at)
        .execute(&self.pool)
        .await
        .map_err(StorageError::from)?;

        Ok(())
    }

    pub async fn list_conversations(&self, session_id: &str) -> Result<Vec<BrainstormConversation>> {
        let rows = sqlx::query_as::<_, BrainstormConversationRow>(
            "SELECT * FROM brainstorm_conversations WHERE session_id = ? ORDER BY created_at DESC",
        )
        .bind(session_id)
        .fetch_all(&self.pool)
        .await
        .map_err(StorageError::from)?;

        Ok(rows.into_iter().map(Into::into).collect())
    }

    pub async fn get_conversation(&self, id: &str) -> Result<Option<BrainstormConversation>> {
        let row = sqlx::query_as::<_, BrainstormConversationRow>(
            "SELECT * FROM brainstorm_conversations WHERE id = ?",
        )
        .bind(id)
        .fetch_optional(&self.pool)
        .await
        .map_err(StorageError::from)?;

        Ok(row.map(Into::into))
    }

    // --- Brainstorm Messages ---

    pub async fn add_brainstorm_message(&self, msg: &BrainstormMessage) -> Result<()> {
        sqlx::query(
            r#"INSERT INTO brainstorm_messages (id, conversation_id, role, content, is_result_block, edited_content, sent_to_cc, created_at)
               VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)"#,
        )
        .bind(&msg.id)
        .bind(&msg.conversation_id)
        .bind(&msg.role)
        .bind(&msg.content)
        .bind(msg.is_result_block)
        .bind(&msg.edited_content)
        .bind(msg.sent_to_cc)
        .bind(msg.created_at)
        .execute(&self.pool)
        .await
        .map_err(StorageError::from)?;

        // Update conversation timestamp
        sqlx::query("UPDATE brainstorm_conversations SET updated_at = ? WHERE id = ?")
            .bind(Utc::now())
            .bind(&msg.conversation_id)
            .execute(&self.pool)
            .await
            .map_err(StorageError::from)?;

        Ok(())
    }

    pub async fn list_brainstorm_messages(&self, conversation_id: &str) -> Result<Vec<BrainstormMessage>> {
        let rows = sqlx::query_as::<_, BrainstormMessageRow>(
            "SELECT * FROM brainstorm_messages WHERE conversation_id = ? ORDER BY created_at",
        )
        .bind(conversation_id)
        .fetch_all(&self.pool)
        .await
        .map_err(StorageError::from)?;

        Ok(rows.into_iter().map(Into::into).collect())
    }

    pub async fn mark_sent_to_cc(&self, message_id: &str) -> Result<()> {
        sqlx::query("UPDATE brainstorm_messages SET sent_to_cc = 1 WHERE id = ?")
            .bind(message_id)
            .execute(&self.pool)
            .await
            .map_err(StorageError::from)?;

        Ok(())
    }

    // --- Session Summaries ---

    pub async fn save_summary(&self, summary: &crate::types::ChunkSummary) -> Result<()> {
        sqlx::query(
            r#"INSERT INTO session_summaries (session_id, chunk_start, chunk_end, summary, files_touched, key_decisions, rolling_summary)
               VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)"#,
        )
        .bind(&summary.session_id)
        .bind(summary.chunk_start)
        .bind(summary.chunk_end)
        .bind(&summary.summary)
        .bind(serde_json::to_string(&summary.files_touched).ok())
        .bind(serde_json::to_string(&summary.key_decisions).ok())
        .bind(&summary.rolling_summary)
        .execute(&self.pool)
        .await
        .map_err(StorageError::from)?;

        Ok(())
    }

    pub async fn get_latest_summary(&self, session_id: &str) -> Result<Option<String>> {
        let row: Option<(String,)> = sqlx::query_as(
            "SELECT rolling_summary FROM session_summaries WHERE session_id = ? ORDER BY id DESC LIMIT 1",
        )
        .bind(session_id)
        .fetch_optional(&self.pool)
        .await
        .map_err(StorageError::from)?;

        Ok(row.map(|r| r.0))
    }
}

// --- SQLx row types ---

#[derive(sqlx::FromRow)]
struct TrackedSessionRow {
    session_id: String,
    project_path: String,
    project_name: String,
    jsonl_path: String,
    status: String,
    first_message_preview: Option<String>,
    message_count: i64,
    git_branch: Option<String>,
    last_activity: Option<DateTime<Utc>>,
    #[allow(dead_code)]
    file_offset: i64,
    created_at: DateTime<Utc>,
}

impl From<TrackedSessionRow> for TrackedSession {
    fn from(row: TrackedSessionRow) -> Self {
        let status = match row.status.as_str() {
            "active" | "\"active\"" => SessionStatus::Active,
            "idle" | "\"idle\"" => SessionStatus::Idle,
            _ => SessionStatus::Completed,
        };

        Self {
            session_id: row.session_id,
            project_path: row.project_path,
            project_name: row.project_name,
            jsonl_path: row.jsonl_path,
            status,
            first_message_preview: row.first_message_preview,
            message_count: row.message_count,
            git_branch: row.git_branch,
            last_activity: row.last_activity,
            created_at: row.created_at,
        }
    }
}

#[derive(sqlx::FromRow)]
struct PinnedMessageRow {
    id: i64,
    session_id: String,
    message_uuid: String,
    message_content: String,
    message_role: String,
    pin_order: i64,
    created_at: DateTime<Utc>,
}

impl From<PinnedMessageRow> for PinnedMessage {
    fn from(row: PinnedMessageRow) -> Self {
        Self {
            id: row.id,
            session_id: row.session_id,
            message_uuid: row.message_uuid,
            message_content: row.message_content,
            message_role: row.message_role,
            pin_order: row.pin_order,
            created_at: row.created_at,
        }
    }
}

#[derive(sqlx::FromRow)]
struct BrainstormConversationRow {
    id: String,
    session_id: String,
    title: Option<String>,
    mode: String,
    model_key: Option<String>,
    agent_profile: Option<String>,
    created_at: DateTime<Utc>,
    updated_at: Option<DateTime<Utc>>,
}

impl From<BrainstormConversationRow> for BrainstormConversation {
    fn from(row: BrainstormConversationRow) -> Self {
        let mode = match row.mode.as_str() {
            "agent" => BrainstormMode::Agent,
            _ => BrainstormMode::DirectModel,
        };

        Self {
            id: row.id,
            session_id: row.session_id,
            title: row.title,
            mode,
            model_key: row.model_key,
            agent_profile: row.agent_profile,
            created_at: row.created_at,
            updated_at: row.updated_at,
        }
    }
}

#[derive(sqlx::FromRow)]
struct BrainstormMessageRow {
    id: String,
    conversation_id: String,
    role: String,
    content: String,
    is_result_block: bool,
    edited_content: Option<String>,
    sent_to_cc: bool,
    created_at: DateTime<Utc>,
}

impl From<BrainstormMessageRow> for BrainstormMessage {
    fn from(row: BrainstormMessageRow) -> Self {
        Self {
            id: row.id,
            conversation_id: row.conversation_id,
            role: row.role,
            content: row.content,
            is_result_block: row.is_result_block,
            edited_content: row.edited_content,
            sent_to_cc: row.sent_to_cc,
            created_at: row.created_at,
        }
    }
}
```

**Step 2: Add module to lib.rs**

Add `pub mod repos;` to `lib.rs`.

**Step 3: Verify it compiles**

```bash
cargo build -p feature-session-tracker
```

**Step 4: Commit**

```bash
git add crates/feature-session-tracker/src/repos.rs crates/feature-session-tracker/src/lib.rs
git commit -m "feat(session-tracker): implement SQLite repos for sessions, pins, brainstorms"
```

---

### Task 9: Write repo integration tests

**Files:**
- Create: `crates/feature-session-tracker/tests/repo_tests.rs`

**Step 1: Write integration tests**

Create `crates/feature-session-tracker/tests/repo_tests.rs`:

```rust
use chrono::Utc;
use feature_session_tracker::repos::SessionTrackerRepos;
use feature_session_tracker::types::*;
use feature_session_tracker::SessionTrackerFeature;
use storage::StoragePool;

async fn setup() -> SessionTrackerRepos {
    let pool = StoragePool::connect_in_memory().await.unwrap();
    pool.run_feature_migrations(&SessionTrackerFeature.migrations())
        .await
        .unwrap();
    SessionTrackerRepos::new(pool.inner().clone())
}

#[tokio::test]
async fn test_upsert_and_list_sessions() {
    let repos = setup().await;

    let session = TrackedSession {
        session_id: "sess-001".to_string(),
        project_path: "/Users/test/project".to_string(),
        project_name: "project".to_string(),
        jsonl_path: "/home/.claude/projects/test/sess-001.jsonl".to_string(),
        status: SessionStatus::Active,
        first_message_preview: Some("Hello".to_string()),
        message_count: 5,
        git_branch: Some("main".to_string()),
        last_activity: Some(Utc::now()),
        created_at: Utc::now(),
    };

    repos.upsert_session(&session).await.unwrap();

    let sessions = repos.list_sessions().await.unwrap();
    assert_eq!(sessions.len(), 1);
    assert_eq!(sessions[0].session_id, "sess-001");
    assert_eq!(sessions[0].status, SessionStatus::Active);
    assert_eq!(sessions[0].project_name, "project");
}

#[tokio::test]
async fn test_pin_and_unpin_messages() {
    let repos = setup().await;

    // Create a session first
    let session = TrackedSession {
        session_id: "sess-002".to_string(),
        project_path: "/test".to_string(),
        project_name: "test".to_string(),
        jsonl_path: "/test.jsonl".to_string(),
        status: SessionStatus::Active,
        first_message_preview: None,
        message_count: 0,
        git_branch: None,
        last_activity: Some(Utc::now()),
        created_at: Utc::now(),
    };
    repos.upsert_session(&session).await.unwrap();

    let pin = PinnedMessage {
        id: 0, // auto-increment
        session_id: "sess-002".to_string(),
        message_uuid: "msg-42".to_string(),
        message_content: "Important decision".to_string(),
        message_role: "assistant".to_string(),
        pin_order: 1,
        created_at: Utc::now(),
    };
    repos.pin_message(&pin).await.unwrap();

    let pins = repos.list_pins("sess-002").await.unwrap();
    assert_eq!(pins.len(), 1);
    assert_eq!(pins[0].message_uuid, "msg-42");

    repos.unpin_message(pins[0].id).await.unwrap();
    let pins = repos.list_pins("sess-002").await.unwrap();
    assert!(pins.is_empty());
}

#[tokio::test]
async fn test_brainstorm_conversation_and_messages() {
    let repos = setup().await;

    // Create session
    let session = TrackedSession {
        session_id: "sess-003".to_string(),
        project_path: "/test".to_string(),
        project_name: "test".to_string(),
        jsonl_path: "/test.jsonl".to_string(),
        status: SessionStatus::Active,
        first_message_preview: None,
        message_count: 0,
        git_branch: None,
        last_activity: Some(Utc::now()),
        created_at: Utc::now(),
    };
    repos.upsert_session(&session).await.unwrap();

    // Create conversation
    let conv = BrainstormConversation {
        id: "conv-001".to_string(),
        session_id: "sess-003".to_string(),
        title: Some("Debug auth".to_string()),
        mode: BrainstormMode::DirectModel,
        model_key: Some("gpt-4o".to_string()),
        agent_profile: None,
        created_at: Utc::now(),
        updated_at: None,
    };
    repos.create_conversation(&conv).await.unwrap();

    let convs = repos.list_conversations("sess-003").await.unwrap();
    assert_eq!(convs.len(), 1);
    assert_eq!(convs[0].title.as_deref(), Some("Debug auth"));

    // Add messages
    let msg = BrainstormMessage {
        id: "bmsg-001".to_string(),
        conversation_id: "conv-001".to_string(),
        role: "user".to_string(),
        content: "What's wrong with this auth approach?".to_string(),
        is_result_block: false,
        edited_content: None,
        sent_to_cc: false,
        created_at: Utc::now(),
    };
    repos.add_brainstorm_message(&msg).await.unwrap();

    let messages = repos.list_brainstorm_messages("conv-001").await.unwrap();
    assert_eq!(messages.len(), 1);

    // Mark as sent
    repos.mark_sent_to_cc("bmsg-001").await.unwrap();
    let messages = repos.list_brainstorm_messages("conv-001").await.unwrap();
    assert!(messages[0].sent_to_cc);
}
```

**Step 2: Run tests**

```bash
cargo nextest run -p feature-session-tracker
```

Expected: All tests pass. Note: This requires `StoragePool::run_feature_migrations` to accept `&[FeatureMigration]`. If the API differs, adjust to match the pattern used in `init.rs` for other feature crates.

**Step 3: Commit**

```bash
git add crates/feature-session-tracker/tests/
git commit -m "test(session-tracker): add repo integration tests"
```

---

## Batch 5: App-Core Integration (Tasks 10-12)

### Task 10: Add session tracker to AppCore state

**Files:**
- Modify: `crates/app-core/Cargo.toml` (add dependency)
- Modify: `crates/app-core/src/state.rs` (add field)
- Modify: `crates/app-core/src/init.rs` (wire up)

**Step 1: Add dependency**

In `crates/app-core/Cargo.toml`, add to `[dependencies]`:

```toml
feature-session-tracker.workspace = true
```

**Step 2: Add field to AppCore**

In `crates/app-core/src/state.rs`, add:

```rust
use feature_session_tracker::repos::SessionTrackerRepos;
```

Add field to `AppCore` struct:

```rust
pub session_tracker_repos: Option<SessionTrackerRepos>,
```

Add accessor method:

```rust
pub fn session_tracker_repos(&self) -> Result<&SessionTrackerRepos, ApiError> {
    self.session_tracker_repos
        .as_ref()
        .ok_or_else(|| ApiError::internal("Session tracker feature not enabled"))
}
```

**Step 3: Wire in init.rs**

In `crates/app-core/src/init.rs`, near where other feature migrations run:

```rust
use feature_session_tracker::SessionTrackerFeature;

// Run migrations (near other feature migration calls)
StoragePool::run_feature_migrations(&pool, &SessionTrackerFeature.migrations()).await?;

// Build repos (near other feature repo construction)
let session_tracker_repos = Some(SessionTrackerRepos::new(pool.inner().clone()));
```

Set the field on the `AppCore` struct being constructed:

```rust
session_tracker_repos,
```

**Step 4: Verify it compiles**

```bash
cargo build -p app-core
```

**Step 5: Commit**

```bash
git add crates/app-core/
git commit -m "feat(session-tracker): integrate session tracker repos into AppCore"
```

---

### Task 11: Implement session tracker handlers

**Files:**
- Create: `crates/app-core/src/handlers/session_tracker.rs`
- Modify: `crates/app-core/src/handlers/mod.rs` (add module)

**Step 1: Write handlers**

Create `crates/app-core/src/handlers/session_tracker.rs`. These are `impl AppCore` methods following the existing handler pattern. The exact content should follow the patterns from `handlers/tasks.rs`:

- Read-only handlers return `Result<T, ApiError>`
- Mutating handlers return `HandlerResult<T>` (with `EntityUpdate` vec)
- Access repos via `self.session_tracker_repos()?`

Handlers needed:

```rust
impl AppCore {
    // Session tracking
    pub async fn get_tracked_sessions(&self) -> Result<Vec<TrackedSession>, ApiError>;
    pub async fn get_session_messages(&self, session_id: &str, limit: Option<usize>) -> Result<Vec<SessionMessage>, ApiError>;
    pub async fn sync_sessions(&self) -> Result<Vec<TrackedSession>, ApiError>;

    // Pinning
    pub async fn pin_session_message(&self, params: PinMessageParams) -> Result<(), ApiError>;
    pub async fn unpin_session_message(&self, pin_id: i64) -> Result<(), ApiError>;
    pub async fn get_pinned_messages(&self, session_id: &str) -> Result<Vec<PinnedMessage>, ApiError>;

    // Send to Claude Code
    pub async fn send_to_claude_code(&self, session_id: &str, content: &str) -> Result<(), ApiError>;

    // Brainstorming
    pub async fn create_brainstorm(&self, params: CreateBrainstormParams) -> Result<BrainstormConversation, ApiError>;
    pub async fn list_brainstorms(&self, session_id: &str) -> Result<Vec<BrainstormConversation>, ApiError>;
    pub async fn get_brainstorm_messages(&self, conversation_id: &str) -> Result<Vec<BrainstormMessage>, ApiError>;
    pub async fn send_brainstorm_message(&self, params: SendBrainstormParams) -> Result<BrainstormMessage, ApiError>;

    // Context
    pub async fn get_session_context(&self, session_id: &str) -> Result<SessionContext, ApiError>;
}
```

The `get_session_messages` handler reads the JSONL file directly via `parser::parse_lines()` (messages are not stored in SQLite — only metadata is). The `sync_sessions` handler runs `discovery::discover_sessions()` and upserts results. The `send_brainstorm_message` handler calls the LLM provider (direct mode) or agent runtime (agent mode) with context from `ContextBuilder`.

**Step 2: Add module to handlers/mod.rs**

Add `pub mod session_tracker;` to `handlers/mod.rs`.

**Step 3: Verify it compiles**

```bash
cargo build -p app-core
```

**Step 4: Commit**

```bash
git add crates/app-core/src/handlers/
git commit -m "feat(session-tracker): implement session tracker handlers in app-core"
```

---

### Task 12: Add Tauri commands

**Files:**
- Create: `crates/desktop/src/commands/session_tracker.rs`
- Modify: `crates/desktop/src/commands/mod.rs` (add module)
- Modify: `crates/desktop/src/main.rs` (register commands)

**Step 1: Write thin adapter commands**

Create `crates/desktop/src/commands/session_tracker.rs` following the exact pattern from `commands/tasks.rs`:

Each command:
- `#[tauri::command]`
- Receives `state: State<'_, Arc<AppCore>>` and optionally `app: tauri::AppHandle`
- Delegates to `state.handler_method().await?`
- Returns `Result<T, ApiError>`

Commands to register:

```rust
#[tauri::command] pub async fn get_tracked_sessions(...)
#[tauri::command] pub async fn get_session_messages(...)
#[tauri::command] pub async fn sync_sessions(...)
#[tauri::command] pub async fn pin_session_message(...)
#[tauri::command] pub async fn unpin_session_message(...)
#[tauri::command] pub async fn get_pinned_messages(...)
#[tauri::command] pub async fn send_to_claude_code(...)
#[tauri::command] pub async fn create_brainstorm(...)
#[tauri::command] pub async fn list_brainstorms(...)
#[tauri::command] pub async fn get_brainstorm_messages(...)
#[tauri::command] pub async fn send_brainstorm_message(...)
#[tauri::command] pub async fn get_session_context(...)
```

**Step 2: Register in mod.rs and main.rs**

Add module to `commands/mod.rs`. Register all commands in the Tauri builder's `.invoke_handler(tauri::generate_handler![...])` call in `main.rs`.

**Step 3: Verify it compiles**

```bash
cargo build -p desktop
```

**Step 4: Commit**

```bash
git add crates/desktop/src/commands/ crates/desktop/src/main.rs
git commit -m "feat(session-tracker): add Tauri commands for session tracker"
```

---

## Batch 6: Context Builder (Task 13)

### Task 13: Implement context builder with incremental summarization

**Files:**
- Create: `crates/feature-session-tracker/src/context_builder.rs`
- Modify: `crates/feature-session-tracker/src/lib.rs` (add module)

**Step 1: Write context builder**

The `ContextBuilder` implements the three-layer strategy:
1. Rolling summary (generated by LLM, stored in `session_summaries` table)
2. Pinned messages (from `pinned_messages` table)
3. Recent window (last N messages from JSONL file)

Key methods:

```rust
pub struct ContextBuilder {
    repos: SessionTrackerRepos,
    window_size: usize,     // default 30
    chunk_size: usize,      // default 50
}

impl ContextBuilder {
    /// Build the full context for a brainstorming session.
    /// Returns messages ready for LLM injection.
    pub async fn build_context(
        &self,
        session_id: &str,
        all_messages: &[SessionMessage],
    ) -> Result<SessionContext>;

    /// Generate a summary for a chunk of messages using an LLM.
    pub async fn summarize_chunk(
        &self,
        provider: &DynProvider,
        messages: &[SessionMessage],
        previous_summary: Option<&str>,
    ) -> Result<ChunkSummary>;

    /// Format context as a system message for LLM injection.
    pub fn format_as_system_message(context: &SessionContext) -> String;
}
```

The `format_as_system_message` produces:

```
## Active Development Session Context

### Session Summary
{rolling_summary}

### Pinned Messages
[Pin 1] (assistant): {content}
[Pin 2] (user): {content}

### Recent Conversation (last {N} messages)
[user]: {text}
[assistant]: {text with tool calls summarized}
...
```

The summarization prompt:

```
Summarize this development session chunk concisely. Extract:
- Current task/goal
- Key decisions made
- Files created/modified
- Problems encountered
- Current state/blocker (if any)

Merge with previous summary if provided. Keep under 200 words. Be factual.
```

**Step 2: Add module to lib.rs**

Add `pub mod context_builder;` to `lib.rs`.

**Step 3: Verify it compiles**

```bash
cargo build -p feature-session-tracker
```

**Step 4: Commit**

```bash
git add crates/feature-session-tracker/src/context_builder.rs crates/feature-session-tracker/src/lib.rs
git commit -m "feat(session-tracker): implement context builder with incremental summarization"
```

---

## Batch 7: Desktop UI — Session Views (Tasks 14-17)

### Task 14: Add routes and TypeScript types

**Files:**
- Modify: `desktop-ui/src/App.tsx` (add routes)
- Create: `desktop-ui/src/lib/session-types.ts`

**Step 1: Define TypeScript types**

Create `desktop-ui/src/lib/session-types.ts` mirroring the Rust types with camelCase:

```typescript
export interface TrackedSession {
  sessionId: string;
  projectPath: string;
  projectName: string;
  jsonlPath: string;
  status: "active" | "idle" | "completed";
  firstMessagePreview: string | null;
  messageCount: number;
  gitBranch: string | null;
  lastActivity: string | null;
  createdAt: string;
}

export interface SessionMessage {
  type: "user" | "assistant" | "system" | "progress" | "queueOperation";
  uuid: string;
  timestamp: string;
  // Type-specific fields
  text?: string;       // user
  content?: ContentBlock[]; // assistant
  isMeta?: boolean;    // user
  subtype?: string;    // system
  data?: unknown;      // progress
  operation?: string;  // queueOperation
  toolUseId?: string;  // progress
}

export interface ContentBlock {
  type: "text" | "tool_use" | "tool_result";
  text?: string;
  id?: string;
  name?: string;
  input?: unknown;
  toolUseId?: string;
  content?: unknown;
  isError?: boolean;
}

export interface PinnedMessage {
  id: number;
  sessionId: string;
  messageUuid: string;
  messageContent: string;
  messageRole: string;
  pinOrder: number;
  createdAt: string;
}

export interface BrainstormConversation {
  id: string;
  sessionId: string;
  title: string | null;
  mode: "directModel" | "agent";
  modelKey: string | null;
  agentProfile: string | null;
  createdAt: string;
  updatedAt: string | null;
}

export interface BrainstormMessage {
  id: string;
  conversationId: string;
  role: string;
  content: string;
  isResultBlock: boolean;
  editedContent: string | null;
  sentToCc: boolean;
  createdAt: string;
}

export interface SessionContext {
  rollingSummary: string;
  pinnedMessages: PinnedMessage[];
  recentMessages: SessionMessage[];
  totalMessages: number;
  estimatedTokens: number;
}
```

**Step 2: Add lazy-loaded routes to App.tsx**

In `desktop-ui/src/App.tsx`, add inside the AppShell children routes:

```tsx
{
  path: "sessions",
  lazy: () => import("./components/views/Sessions").then(m => ({ Component: m.Sessions })),
},
{
  path: "sessions/:sessionId",
  lazy: () => import("./components/views/SessionDetail").then(m => ({ Component: m.SessionDetail })),
},
{
  path: "sessions/:sessionId/brainstorm/:brainstormId",
  lazy: () => import("./components/views/BrainstormChat").then(m => ({ Component: m.BrainstormChat })),
},
```

**Step 3: Commit**

```bash
git add desktop-ui/src/lib/session-types.ts desktop-ui/src/App.tsx
git commit -m "feat(desktop-ui): add session tracker routes and TypeScript types"
```

---

### Task 15: Build SessionSidebar component

**Files:**
- Create: `desktop-ui/src/components/sessions/SessionSidebar.tsx`

The sidebar shows sessions grouped by project with status indicators. Uses `useQuery("get_tracked_sessions")` and `useEvent("session:status")` for real-time updates. Follows the existing sidebar patterns and uses CSS tokens (`glass-sidebar`, `text-primary`, `text-muted`, `bg-surface-base`).

Key features:
- Project grouping with collapsible headers
- Status dot: green (active), yellow (idle), gray (completed)
- Auto-follow indicator showing which session is currently tracked
- Click to navigate to `/sessions/:sessionId`
- Last message preview + relative time

**Commit:**

```bash
git add desktop-ui/src/components/sessions/
git commit -m "feat(desktop-ui): implement SessionSidebar component"
```

---

### Task 16: Build SessionMirrorView component

**Files:**
- Create: `desktop-ui/src/components/sessions/SessionMirrorView.tsx`
- Create: `desktop-ui/src/components/sessions/SessionMessageItem.tsx`
- Create: `desktop-ui/src/components/sessions/CollapsibleToolBlock.tsx`
- Create: `desktop-ui/src/hooks/useSessionStream.ts`

The mirror view is the core of the feature. It displays the Claude Code conversation in real time.

**`useSessionStream` hook:**
- Uses `useQuery("get_session_messages", { sessionId })` for initial load
- Uses `useEvent("session:message", handler)` for real-time appends
- Returns `{ messages, isLive, pinMessage }`

**`SessionMirrorView`:**
- Renders messages as a chat stream
- User messages: `glass-bubble-user` styled
- Assistant messages: `glass-bubble` styled with text content inline
- Tool calls: `CollapsibleToolBlock` — collapsed by default showing tool name + icon, expandable to show full input/output
- Progress events: thin inline badges
- Auto-scrolls to bottom when live

**`CollapsibleToolBlock`:**
- Collapsed: `[icon] Read src/lib.rs` or `[icon] Bash: cargo test`
- Expanded: shows full input JSON and result content
- Uses `glass-card` styling

**Commit:**

```bash
git add desktop-ui/src/components/sessions/ desktop-ui/src/hooks/useSessionStream.ts
git commit -m "feat(desktop-ui): implement SessionMirrorView with collapsible tool blocks"
```

---

### Task 17: Build BrainstormPanel and action bar

**Files:**
- Create: `desktop-ui/src/components/sessions/BrainstormPanel.tsx`
- Create: `desktop-ui/src/components/sessions/ActionBar.tsx`
- Create: `desktop-ui/src/components/sessions/SendConfirmDialog.tsx`
- Create: `desktop-ui/src/components/sessions/PinnedContextStrip.tsx`
- Create: `desktop-ui/src/hooks/useBrainstormStream.ts`

**`useBrainstormStream` hook:**
- Similar pattern to `useAgentStream` but for brainstorm events
- Listens to `brainstorm:token` and `brainstorm:complete` events
- Returns `{ segments, isStreaming, send }`

**`BrainstormPanel`:**
- Top: model/mode selector (dropdown: list of configured models + "Agent Mode" option)
- Middle: `PinnedContextStrip` showing pinned messages as small cards
- Main: chat message list with streaming support
- Bottom: `ChatInput` (reuse existing) + send handler

**`ActionBar`:**
- Appears on assistant messages marked as result blocks
- Three buttons: Copy (clipboard API), Edit (toggle textarea), Send (opens dialog)
- Edit mode: content becomes editable textarea, Edit button → "Done"
- Uses `glass-button` styling

**`SendConfirmDialog`:**
- Modal with session info (name, branch, status)
- Message preview
- Cancel + Send buttons
- Calls `ipc("send_to_claude_code", { sessionId, content })`

**`PinnedContextStrip`:**
- Horizontal strip of small cards above the brainstorm input
- Each card shows role + truncated content + unpin X button
- Uses `glass-badge` styling

**Commit:**

```bash
git add desktop-ui/src/components/sessions/ desktop-ui/src/hooks/useBrainstormStream.ts
git commit -m "feat(desktop-ui): implement BrainstormPanel with action bar and send confirmation"
```

---

## Batch 8: View Composition & Navigation (Tasks 18-19)

### Task 18: Build Sessions and SessionDetail view pages

**Files:**
- Create: `desktop-ui/src/components/views/Sessions.tsx`
- Create: `desktop-ui/src/components/views/SessionDetail.tsx`
- Create: `desktop-ui/src/components/views/BrainstormChat.tsx`

**`Sessions` page (`/sessions`):**
- Split layout: `SessionSidebar` on left (280px) + `SessionMirrorView` on right
- Auto-selects the active session on mount via `useQuery("get_tracked_sessions")` → find first with `status: "active"` → navigate to `/sessions/:id`

**`SessionDetail` page (`/sessions/:sessionId`):**
- `SessionMirrorView` for the selected session
- Floating "New Brainstorm" button (bottom-right) → creates conversation and navigates to brainstorm view
- Pin button on each message (pin icon appears on hover)

**`BrainstormChat` page (`/sessions/:sessionId/brainstorm/:brainstormId`):**
- Split: `SessionMirrorView` on left (50%), `BrainstormPanel` on right (50%)
- Resizable split via drag handle
- Both panels scroll independently

**Commit:**

```bash
git add desktop-ui/src/components/views/
git commit -m "feat(desktop-ui): implement Sessions, SessionDetail, and BrainstormChat view pages"
```

---

### Task 19: Add Sessions to AppShell navigation

**Files:**
- Modify: `desktop-ui/src/components/layout/Sidebar.tsx` (add nav icon)

**Step 1: Add sessions icon to sidebar**

Add a new nav item to the sidebar (using an appropriate icon like a terminal or eye icon) that navigates to `/sessions`. Position it below existing nav items.

Follow the existing pattern in `Sidebar.tsx` — each nav item is an icon button with tooltip that uses `useNavigate()`.

**Step 2: Verify it renders**

```bash
cd desktop-ui && bun run dev
```

Navigate to `localhost:1420` and verify the sessions icon appears in the sidebar.

**Step 3: Commit**

```bash
git add desktop-ui/src/components/layout/Sidebar.tsx
git commit -m "feat(desktop-ui): add Sessions navigation to sidebar"
```

---

## Batch 9: Watcher Integration & Events (Task 20)

### Task 20: Wire SessionWatcher into AppCore lifecycle with Tauri events

**Files:**
- Modify: `crates/app-core/src/init.rs` (start watcher)
- Modify: `crates/app-core/src/state.rs` (store watcher handle)
- Modify: `crates/desktop/src/main.rs` (emit events from watcher)
- Create: `crates/desktop-shared/src/session_events.rs` (event constants)

**Step 1: Add event constants**

In `desktop-shared`, add session tracker event constants:

```rust
pub const SESSION_NEW: &str = "session:new";
pub const SESSION_MESSAGE: &str = "session:message";
pub const SESSION_STATUS: &str = "session:status";
pub const BRAINSTORM_TOKEN: &str = "brainstorm:token";
pub const BRAINSTORM_COMPLETE: &str = "brainstorm:complete";
```

**Step 2: Start watcher in init.rs**

In `AppCore::init()`, after session tracker repos are created:

```rust
// Start session watcher
let (watch_tx, mut watch_rx) = tokio::sync::mpsc::unbounded_channel();
let claude_dir = feature_session_tracker::discovery::default_claude_dir();
let projects_dir = claude_dir.join("projects");

if projects_dir.exists() {
    // Discover existing sessions and init offsets
    let existing = feature_session_tracker::discovery::discover_sessions(&claude_dir).await;
    for s in &existing {
        repos.session_tracker_repos().upsert_session(s).await.ok();
    }

    let watcher = feature_session_tracker::watcher::SessionWatcher::start(&projects_dir, watch_tx).ok();
    if let Some(ref w) = watcher {
        let paths: Vec<_> = existing.iter().map(|s| PathBuf::from(&s.jsonl_path)).collect();
        w.init_offsets(&paths).await;
    }
    // Store watcher in AppCore to keep it alive
}
```

**Step 3: Forward watch events to Tauri in main.rs**

Spawn a tokio task that drains `watch_rx` and emits Tauri events:

```rust
// In the Tauri setup callback, after AppCore is created:
let app_handle = app.handle().clone();
tokio::spawn(async move {
    while let Some(event) = watch_rx.recv().await {
        match event {
            WatchEvent::NewMessage { session_id, message } => {
                let _ = app_handle.emit(SESSION_MESSAGE, SessionMessagePayload { session_id, message });
            }
            WatchEvent::NewSession { session_id, .. } => {
                let _ = app_handle.emit(SESSION_NEW, session_id);
            }
            WatchEvent::FileModified { session_id } => {
                // Update status in DB, emit if changed
            }
        }
    }
});
```

**Step 4: Verify it compiles**

```bash
cargo build -p desktop
```

**Step 5: Commit**

```bash
git add crates/app-core/ crates/desktop/ crates/desktop-shared/
git commit -m "feat(session-tracker): wire SessionWatcher into AppCore lifecycle with Tauri events"
```

---

## Batch 10: End-to-End Polish (Tasks 21-22)

### Task 21: Implement brainstorm LLM integration (direct model mode)

**Files:**
- Modify: `crates/app-core/src/handlers/session_tracker.rs` (implement send_brainstorm_message)

**Step 1: Implement the brainstorm message handler**

The `send_brainstorm_message` handler:
1. Gets the brainstorm conversation from DB
2. Loads session context via `ContextBuilder`
3. Builds message array: system message (context) + conversation history + new user message
4. If direct model mode: calls `provider.chat_stream()` with streaming callbacks
5. Emits `brainstorm:token` events for each chunk
6. On completion: saves assistant message to DB, emits `brainstorm:complete`
7. Returns the saved message

Uses the existing `providers` crate — `create_provider()` with the model key from the conversation config.

**Step 2: Verify it compiles**

```bash
cargo build -p app-core
```

**Step 3: Commit**

```bash
git add crates/app-core/src/handlers/session_tracker.rs
git commit -m "feat(session-tracker): implement brainstorm LLM integration with streaming"
```

---

### Task 22: Add desktop-shared types for Tauri serialization

**Files:**
- Modify: `crates/desktop-shared/src/commands.rs` (add param/response types)

**Step 1: Add serializable types**

Add to `desktop-shared/src/commands.rs`:

```rust
#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PinMessageParams {
    pub session_id: String,
    pub message_uuid: String,
    pub message_content: String,
    pub message_role: String,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CreateBrainstormParams {
    pub session_id: String,
    pub mode: String,  // "directModel" | "agent"
    pub model_key: Option<String>,
    pub agent_profile: Option<String>,
    pub title: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SendBrainstormParams {
    pub conversation_id: String,
    pub content: String,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SendToClaudeCodeParams {
    pub session_id: String,
    pub content: String,
}
```

**Step 2: Verify it compiles**

```bash
cargo build -p desktop-shared
```

**Step 3: Commit**

```bash
git add crates/desktop-shared/
git commit -m "feat(session-tracker): add desktop-shared types for session tracker commands"
```

---

## Summary

| Batch | Tasks | Description |
|-------|-------|-------------|
| 1 | 1-3 | Crate skeleton, types, migration SQL |
| 2 | 4-5 | JSONL parser + session discovery |
| 3 | 6-7 | FSEvent watcher + session injector |
| 4 | 8-9 | SQLite repos + integration tests |
| 5 | 10-12 | AppCore state, handlers, Tauri commands |
| 6 | 13 | Context builder with incremental summarization |
| 7 | 14-17 | Desktop UI: routes, sidebar, mirror view, brainstorm panel |
| 8 | 18-19 | View composition, navigation |
| 9 | 20 | Watcher lifecycle + Tauri event wiring |
| 10 | 21-22 | Brainstorm LLM integration + shared types |

**Parallelization:** Batches 1-4 (Rust backend) can be implemented before Batches 7-8 (UI). Batch 6 (context builder) is independent of Batch 5 (app-core integration). Within the UI batches, Task 14 (types/routes) must come first, then 15-17 can be parallelized.
