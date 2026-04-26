//! Cross-process event bridge between the desktop process and a child
//! `klyntbot mcp serve --stdio` process.
//!
//! See `docs/superpowers/plans/2026-04-26-realtime-data-layer-phase-3.md`
//! for the full design rationale and protocol specification.

pub mod client;
pub mod emitter;
pub mod protocol;
pub mod server;

pub use client::BridgeClient;
pub use emitter::SocketBridgeEmitter;
pub use protocol::{BridgeFrame, FrameError, read_frame, write_frame};
pub use server::{BridgeServer, BridgeServerHandle};

use std::path::PathBuf;

/// Resolve the bridge socket path. Both the desktop server and MCP child
/// must agree, so they share this single helper.
///
/// Path: `${KLYNTBOT_HOME or ~/.klyntbot}/mcp-events.sock`.
///
/// Returns `None` if the home directory cannot be determined (very rare —
/// only when `HOME` is unset on Unix and `KLYNTBOT_HOME` is also unset).
pub fn bridge_socket_path() -> Option<PathBuf> {
    config::loader::config_dir()
        .ok()
        .map(|d| d.join("mcp-events.sock"))
}
