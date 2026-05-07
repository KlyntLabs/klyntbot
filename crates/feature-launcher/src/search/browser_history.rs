use crate::types::*;
use parking_lot::RwLock;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

#[derive(Debug, Clone)]
struct HistoryEntry {
    title: String,
    url: String,
    last_visit: jiff::Timestamp,
}

#[derive(Clone)]
pub struct BrowserHistorySource {
    entries: Arc<RwLock<Vec<HistoryEntry>>>,
    browser: String,
    max_days: i64,
    permission_warned: Arc<AtomicBool>,
}

impl BrowserHistorySource {
    pub fn new(browser: String, max_days: i64) -> Self {
        Self {
            entries: Arc::new(RwLock::new(Vec::new())),
            browser,
            max_days,
            permission_warned: Arc::new(AtomicBool::new(false)),
        }
    }

    pub fn history_db_path(browser: &str) -> Option<PathBuf> {
        super::chromium_profile_dir(browser).map(|d| d.join("History"))
    }

    async fn load_history(
        browser: &str,
        max_days: i64,
    ) -> Result<Vec<HistoryEntry>, std::io::Error> {
        let db_path = Self::history_db_path(browser).ok_or_else(|| {
            std::io::Error::new(std::io::ErrorKind::NotFound, "unsupported browser")
        })?;

        if !db_path.exists() {
            return Ok(vec![]);
        }

        // Copy to temp file (browser holds write lock)
        let temp_dir = std::env::temp_dir().join("klyntbot-history");
        let _ = std::fs::create_dir_all(&temp_dir);
        let temp_db = temp_dir.join("History-copy");
        std::fs::copy(&db_path, &temp_db)?;

        // Query with sqlx (in-process SQLite)
        let url = format!("sqlite:{}", temp_db.display());
        let pool = sqlx::SqlitePool::connect(&url)
            .await
            .map_err(|e| std::io::Error::other(e.to_string()))?;

        // Chrome stores last_visit_time as microseconds since Jan 1, 1601
        let cutoff_us = (jiff::Timestamp::now() - jiff::SignedDuration::from_hours(max_days * 24))
            .as_microsecond()
            + 11_644_473_600_000_000; // Chromium epoch offset

        let rows = sqlx::query_as::<_, (String, String, i64)>(
            "SELECT COALESCE(title, ''), url, last_visit_time FROM urls \
             WHERE last_visit_time > ? AND url NOT LIKE 'chrome%' \
             ORDER BY last_visit_time DESC LIMIT 100",
        )
        .bind(cutoff_us)
        .fetch_all(&pool)
        .await
        .unwrap_or_default();

        drop(pool);
        let _ = std::fs::remove_file(&temp_db);

        Ok(rows
            .into_iter()
            .filter(|(title, _, _)| !title.is_empty())
            .map(|(title, url, visit_time)| {
                let ts_secs = (visit_time - 11_644_473_600_000_000) / 1_000_000;
                let last_visit =
                    jiff::Timestamp::from_second(ts_secs).unwrap_or(jiff::Timestamp::now());
                HistoryEntry {
                    title,
                    url,
                    last_visit,
                }
            })
            .collect())
    }
}

#[async_trait::async_trait]
impl super::SearchSource for BrowserHistorySource {
    fn name(&self) -> &'static str {
        "browser_history"
    }

    fn prefix(&self) -> Option<&'static str> {
        Some("h/")
    }

    async fn search(&self, query: &str, limit: usize) -> Vec<LauncherItem> {
        let entries = self.entries.read();
        let scored = super::fuzzy_match2(
            query,
            &entries,
            |e| &e.title,
            |e| Some(e.url.as_str()),
            limit,
        );

        scored
            .into_iter()
            .map(|(score, e)| LauncherItem {
                id: format!("history:{}", e.url),
                title: e.title.clone(),
                subtitle: Some(e.url.clone()),
                icon: Some("globe".to_string()),
                kind: LauncherItemKind::BrowserHistory {
                    url: e.url.clone(),
                    visited_at: e.last_visit,
                },
                score: (score as f64) / 1000.0 * 0.4,
                no_view: false,
                arguments: vec![],
                pinned: false,
            })
            .collect()
    }

    async fn refresh(&self) {
        match Self::load_history(&self.browser, self.max_days).await {
            Ok(entries) => {
                tracing::debug!("Loaded {} browser history entries", entries.len());
                *self.entries.write() = entries;
            }
            Err(e) => {
                if e.kind() == std::io::ErrorKind::PermissionDenied
                    && !self.permission_warned.swap(true, Ordering::Relaxed)
                {
                    tracing::warn!(
                        "Browser history requires Full Disk Access — grant in System Settings > Privacy > Full Disk Access"
                    );
                } else {
                    tracing::debug!("Failed to load browser history: {e}");
                }
            }
        }
    }
}
