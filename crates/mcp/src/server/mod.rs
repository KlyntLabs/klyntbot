//! MCP server: exposes klyntbot tools to external AI agents.

pub mod handler;
pub mod security;

pub use handler::McpServerRunner;
