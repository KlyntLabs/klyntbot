//! MCP (Model Context Protocol) client and server integration.
//!
//! Provides:
//! - `McpManager`: connects to external MCP servers, discovers tools
//! - `McpTool`: adapts MCP server tools to `tools_core::Tool`
//! - `McpServerRunner`: exposes klyntbot tools to external AI agents

pub mod client;
pub mod server;

// Re-export the types consumers actually need.
pub use config::McpConfig;

pub use client::events::McpStartupEvent;
pub use client::manager::McpManager;
pub use client::sanitize;
pub use server::McpServerRunner;
