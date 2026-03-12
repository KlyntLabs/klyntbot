//! Klyntbot MCP server — library interface.
//!
//! Used by the `klyntbot-mcp` binary (standalone) and by the desktop crate
//! (embedded HTTP server sharing AppCore).

pub mod bridge;
pub mod cli;
pub mod handler;
pub mod logging;

pub use handler::KlyntbotServerHandler;
