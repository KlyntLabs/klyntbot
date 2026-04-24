//! `HookClient` — the hook binary's public write API. Tries the Unix socket
//! first; on any failure, falls back to appending into the file buffer and
//! emits a rate-limited stderr warning.

use crate::event::AgentEvent;
use crate::transport::{FileBufferFallback, IngestSocket, UnixIngestSocket};
use crate::warn::WarnLimiter;
use common::Result;
use std::path::PathBuf;

/// The hook binary's dispatcher.
#[derive(Debug, Clone)]
pub struct HookClient {
    socket: UnixIngestSocket,
    buffer: FileBufferFallback,
    warn: WarnLimiter,
}

impl HookClient {
    /// Construct with absolute paths (socket / buffer file / touch-stamp).
    #[must_use]
    pub fn new(socket_path: PathBuf, buffer_path: PathBuf, warn_stamp: PathBuf) -> Self {
        Self {
            socket: UnixIngestSocket::new(socket_path),
            buffer: FileBufferFallback::new(buffer_path),
            warn: WarnLimiter::new(warn_stamp),
        }
    }

    /// Try socket; on failure buffer the event + maybe warn.
    pub async fn send(&self, event: &AgentEvent) -> Result<()> {
        match self.socket.send(event).await {
            Ok(()) => Ok(()),
            Err(socket_err) => {
                self.buffer.send(event).await?;
                if self.warn.should_warn() {
                    eprintln!(
                        "klyntbot-hook: desktop unreachable — buffering events to disk ({socket_err})"
                    );
                }
                Ok(())
            }
        }
    }
}
