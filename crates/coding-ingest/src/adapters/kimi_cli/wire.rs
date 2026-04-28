//! Tier-2 — kimi-cli Wire streaming client (stub; reader loop not yet implemented).

use crate::event::AgentEventV1;
use common::Result;
use std::path::PathBuf;

/// Configuration for the tier-2 Wire client.
#[derive(Debug, Clone)]
pub struct WireConfig {
    /// Unix-domain socket path (typically `~/.config/kimi-cli/wire.sock`).
    pub socket_path: PathBuf,
}

/// Reads newline-delimited `AgentEventV1` payloads from a Unix socket.
#[derive(Debug)]
pub struct WireClient {
    cfg: WireConfig,
}

impl WireClient {
    /// Construct a client bound to `cfg.socket_path`.
    pub fn new(cfg: WireConfig) -> Self {
        Self { cfg }
    }

    /// The configured Unix-domain socket path.
    pub fn socket_path(&self) -> &std::path::Path {
        &self.cfg.socket_path
    }

    /// Decode a single newline-delimited frame. Exposed so the wire-format
    /// invariant can be unit-tested without socket I/O.
    pub fn decode_frame(line: &str) -> Result<AgentEventV1> {
        serde_json::from_str(line.trim_end_matches('\n'))
            .map_err(|e| common::KlyntbotError::Storage(format!("kimi wire decode: {e}")))
    }
}
