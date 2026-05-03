//! MCP (Model Context Protocol) client and server integration.
//!
//! Provides:
//! - `McpManager`: connects to external MCP servers, discovers tools
//! - `McpTool`: adapts MCP server tools to `tools_core::Tool`

pub mod allowlist;
pub mod client;
pub mod dispatch;
pub mod server;

// Re-export the types consumers actually need.
pub use config::McpConfig;

pub use client::events::McpStartupEvent;
pub use client::handler::SamplingDelegate;
pub use client::manager::{McpClientOptions, McpManager};
pub use client::sanitize;
pub use dispatch::{EntityUpdate, dispatch_entity_update};
pub use server::security;
