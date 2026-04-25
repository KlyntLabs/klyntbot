//! Touch-file rate-limited stderr warnings for the hook binary.

use common::Result;
use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime};

/// Interval between warnings sharing the same touch-file.
pub const WARN_INTERVAL: Duration = Duration::from_secs(5 * 60);

/// Rate-limiter using filesystem mtime as persisted state.
#[derive(Debug, Clone)]
pub struct WarnLimiter {
    /// Path to the touch file (usually `~/.klyntbot/.hook-warn.stamp`).
    pub stamp_path: PathBuf,
}

impl WarnLimiter {
    /// Construct.
    #[must_use]
    pub fn new(stamp_path: PathBuf) -> Self {
        Self { stamp_path }
    }

    /// Should we emit the warning now? Touches the file if yes.
    pub fn should_warn(&self) -> bool {
        let now = SystemTime::now();
        let due = std::fs::metadata(&self.stamp_path)
            .and_then(|m| m.modified())
            .map(|t| {
                now.duration_since(t)
                    .map(|d| d >= WARN_INTERVAL)
                    .unwrap_or(true)
            })
            .unwrap_or(true);
        if due {
            let _ = touch(&self.stamp_path);
        }
        due
    }
}

fn touch(p: &Path) -> Result<()> {
    if let Some(parent) = p.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    let _ = std::fs::File::create(p)
        .map_err(|e| common::KlyntbotError::Storage(format!("warn stamp: {e}")))?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn first_call_warns_then_suppresses() {
        let dir = TempDir::new().unwrap();
        let l = WarnLimiter::new(dir.path().join(".stamp"));
        assert!(l.should_warn());
        assert!(!l.should_warn());
    }

    #[test]
    fn warns_again_after_interval_simulated_by_backdating() {
        let dir = TempDir::new().unwrap();
        let stamp = dir.path().join(".stamp");
        let l = WarnLimiter::new(stamp.clone());
        assert!(l.should_warn());
        // Backdate mtime beyond the interval.
        let past = std::time::SystemTime::now() - WARN_INTERVAL - Duration::from_secs(1);
        let ft = filetime::FileTime::from_system_time(past);
        filetime::set_file_mtime(&stamp, ft).unwrap();
        assert!(l.should_warn());
    }
}
