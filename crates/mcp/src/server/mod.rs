//! MCP server: exposes klyntbot tools to external AI agents.

pub mod handler;
pub mod handlers;
pub mod security;

pub use handler::McpServerRunner;
pub use handlers::MCP_EXPOSED_TOOLS;
