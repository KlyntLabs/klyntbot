//! Per-(wire-path, mtime) memo for `list_sessions` summary rows.

use crate::tracing::types::SessionSummary;
use std::collections::HashMap;
use std::path::PathBuf;
use std::time::SystemTime;
use tokio::sync::RwLock;

#[derive(Debug, Clone)]
pub struct CacheEntry {
    pub mtime: SystemTime,
    pub summary: SessionSummary,
}

#[derive(Default)]
pub struct SessionCache {
    map: RwLock<HashMap<PathBuf, CacheEntry>>,
}

impl SessionCache {
    pub fn new() -> Self {
        Self::default()
    }

    pub async fn get(&self, key: &PathBuf, mtime: SystemTime) -> Option<SessionSummary> {
        let g = self.map.read().await;
        g.get(key)
            .filter(|e| e.mtime == mtime)
            .map(|e| e.summary.clone())
    }

    pub async fn put(&self, key: PathBuf, mtime: SystemTime, summary: SessionSummary) {
        let mut g = self.map.write().await;
        g.insert(key, CacheEntry { mtime, summary });
    }

    pub async fn clear(&self) {
        self.map.write().await.clear();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tracing::types::*;
    use jiff::Timestamp;

    fn dummy(s: &str) -> SessionSummary {
        SessionSummary {
            session_id: s.into(),
            provider_id: "kimi".into(),
            source_dir: PathBuf::new(),
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
        }
    }

    #[tokio::test]
    async fn miss_then_hit_then_invalidate() {
        let c = SessionCache::new();
        let key = PathBuf::from("/x");
        let mtime = SystemTime::now();
        assert!(c.get(&key, mtime).await.is_none());
        c.put(key.clone(), mtime, dummy("a")).await;
        assert!(c.get(&key, mtime).await.is_some());
        // Different mtime treated as miss.
        let later = mtime + std::time::Duration::from_secs(1);
        assert!(c.get(&key, later).await.is_none());
    }
}
