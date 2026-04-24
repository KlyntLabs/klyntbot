//! `~/.klyntbot/desktop.lock` — lightweight liveness signal.
//!
//! This is not a mutex. Writers touch the file every 30s; readers treat
//! `now - mtime > 60s` as "desktop dead." This lets `klynt-cli` (and any
//! future native source) swap its `MemorySink` impl at event boundaries
//! without explicit coordination with the desktop.

use common::{KlyntbotError, Result};
use std::path::Path;
use std::time::{Duration, SystemTime};

/// Desktop is considered dead if its heartbeat is older than this.
pub const STALENESS_THRESHOLD: Duration = Duration::from_secs(60);

/// Touch (or create) the lock file, refreshing its mtime.
pub async fn write_heartbeat(path: &Path) -> Result<()> {
    if let Some(parent) = path.parent() {
        tokio::fs::create_dir_all(parent).await
            .map_err(|e| KlyntbotError::Storage(format!("heartbeat mkdir: {e}")))?;
    }
    let pid = std::process::id().to_string();
    tokio::fs::write(path, pid.as_bytes()).await
        .map_err(|e| KlyntbotError::Storage(format!("heartbeat write: {e}")))?;
    Ok(())
}

/// Read the lock's mtime; return true if within the staleness threshold.
#[must_use]
pub fn is_desktop_alive(path: &Path) -> bool {
    let Ok(meta) = std::fs::metadata(path) else { return false };
    let Ok(modified) = meta.modified() else { return false };
    SystemTime::now()
        .duration_since(modified)
        .map(|d| d < STALENESS_THRESHOLD)
        .unwrap_or(false)
}
