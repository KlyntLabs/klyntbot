//! MCP startup events for observability.
//!
//! Emitted during `McpManager::connect_all` so consumers (agent loop,
//! desktop UI) can track server connection progress in real time.

/// Events emitted during MCP server startup.
#[derive(Debug, Clone)]
pub enum McpStartupEvent {
    /// A server connection attempt is starting.
    Starting { server_name: String },
    /// A server connected successfully and discovered tools.
    Ready {
        server_name: String,
        tool_count: usize,
    },
    /// A server connection failed.
    Failed {
        server_name: String,
        error: String,
    },
    /// A server was skipped (disabled in config).
    Skipped { server_name: String },
    /// All server connections have completed.
    Complete {
        ready: usize,
        failed: usize,
        skipped: usize,
    },
}
