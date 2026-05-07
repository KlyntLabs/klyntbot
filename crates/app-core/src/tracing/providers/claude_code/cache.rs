//! `(file_path, mtime)`-keyed `list_sessions` cache for Claude Code.

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Mutex;
use std::time::SystemTime;

use crate::tracing::types::SessionSummary;

#[derive(Default)]
pub struct SummaryCache {
    inner: Mutex<HashMap<PathBuf, Entry>>,
}

struct Entry {
    mtime: SystemTime,
    summary: SessionSummary,
}

impl SummaryCache {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn get(&self, path: &PathBuf, mtime: SystemTime) -> Option<SessionSummary> {
        let lock = self.inner.lock().ok()?;
        let e = lock.get(path)?;
        if e.mtime == mtime {
            Some(e.summary.clone())
        } else {
            None
        }
    }

    pub fn put(&self, path: PathBuf, mtime: SystemTime, summary: SessionSummary) {
        if let Ok(mut lock) = self.inner.lock() {
            lock.insert(path, Entry { mtime, summary });
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use jiff::Timestamp;
    use std::path::PathBuf;
    use std::time::Duration;

    fn fixture(id: &str) -> SessionSummary {
        SessionSummary {
            session_id: id.into(),
            provider_id: "claudeCode".into(),
            source_dir: PathBuf::from("/"),
            cwd: None,
            project_basename: None,
            custom_title: None,
            started_at: Timestamp::UNIX_EPOCH,
            last_event_at: Timestamp::UNIX_EPOCH,
            size_bytes: 0,
            turn_count: 0,
            step_count: 0,
            tool_call_count: 0,
            error_count: 0,
            subagent_count: 0,
            has_wire: true,
            has_context: false,
            imported: false,
            work_dir_hash: String::new(),
            has_state: false,
            wire_size: 0,
            context_size: 0,
            state_size: 0,
            total_size: 0,
            metadata: None,
        }
    }

    #[test]
    fn hit_when_mtime_unchanged() {
        let c = SummaryCache::new();
        let p = PathBuf::from("/x.jsonl");
        let t = SystemTime::UNIX_EPOCH;
        c.put(p.clone(), t, fixture("a"));
        assert_eq!(c.get(&p, t).unwrap().session_id, "a");
    }

    #[test]
    fn miss_when_mtime_bumps() {
        let c = SummaryCache::new();
        let p = PathBuf::from("/x.jsonl");
        let t = SystemTime::UNIX_EPOCH;
        c.put(p.clone(), t, fixture("a"));
        let later = t + Duration::from_secs(1);
        assert!(c.get(&p, later).is_none());
    }
}
