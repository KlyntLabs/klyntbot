pub mod config;
pub mod mcp;

// Re-export public helpers used by other crates.
pub use mcp::{
    build_mcp_response, build_transport, embedded_status_from_validation, find_server_mut,
    server_to_response,
};
