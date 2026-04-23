//! MCP (Model Context Protocol) client and server integration.
//!
//! Provides:
//! - `McpManager`: connects to external MCP servers, discovers tools
//! - `McpTool`: adapts MCP server tools to `tools_core::Tool`

pub mod client;
pub mod dispatch;
pub mod server;

// Re-export the types consumers actually need.
pub use config::McpConfig;

pub use client::events::McpStartupEvent;
pub use client::handler::SamplingDelegate;
pub use client::manager::{McpClientOptions, McpManager};
pub use client::sanitize;
pub use dispatch::{dispatch_entity_update, EntityUpdate};
pub use server::security;
