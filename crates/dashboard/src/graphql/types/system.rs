//! GraphQL types for system status, cron jobs, and sessions.

use async_graphql::{SimpleObject, ID};
use chrono::{DateTime, Utc};

/// Overall system status snapshot.
#[derive(SimpleObject, Clone)]
pub struct GqlSystemStatus {
    /// Whether the agent loop is currently running.
    pub agent_running: bool,
    /// Seconds since the server started.
    pub uptime_secs: i64,
    /// Names of currently connected channels.
    pub connected_channels: Vec<String>,
    /// Whether calendar sync is active.
    pub calendar_sync_active: bool,
    /// Number of active chat sessions.
    pub active_sessions: i32,
    /// Server version string.
    pub version: String,
}

/// A configured cron job.
#[derive(SimpleObject, Clone)]
pub struct GqlCronJob {
    pub id: ID,
    pub name: String,
    pub schedule: String,
    pub enabled: bool,
    pub last_run: Option<DateTime<Utc>>,
    pub next_run: Option<DateTime<Utc>>,
}

/// An active or recent chat session.
#[derive(SimpleObject, Clone)]
pub struct GqlSession {
    pub id: ID,
    pub name: String,
    pub channel: String,
    pub created_at: DateTime<Utc>,
    pub last_active: DateTime<Utc>,
    pub message_count: i32,
}
