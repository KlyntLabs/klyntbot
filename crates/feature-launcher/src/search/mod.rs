pub mod app_index;
pub mod background;
pub mod bookmarks;
pub mod brew;
pub mod browser_history;
pub mod calculator;
pub mod contacts;
pub mod content_grep;
pub mod file_search;
pub mod file_watcher;
pub mod git_repos;
pub mod running_apps;
pub mod script_runner;
pub mod ssh_hosts;
pub mod system_commands;
pub mod system_prefs;
pub mod url_navigation;

pub use app_index::{AppEntry, AppIndex};
pub use background::{BackgroundRefresher, RefreshEntry};
pub use bookmarks::BookmarksSource;
pub use brew::BrewSource;
pub use browser_history::BrowserHistorySource;
pub use calculator::Calculator;
pub use contacts::ContactsSource;
pub use content_grep::ContentGrepSource;
pub use file_search::FileSearchSource;
pub use file_watcher::SourceFileWatcher;
pub use git_repos::GitReposSource;
pub use running_apps::RunningAppsSource;
pub use script_runner::ScriptRunner;
pub use ssh_hosts::SshHostsSource;
pub use system_commands::SystemCommands;
pub use system_prefs::SystemPrefsSource;
pub use url_navigation::UrlNavigation;

use crate::types::LauncherItem;
use async_trait::async_trait;
use dashmap::DashMap;
use std::sync::Arc;
use std::time::{Duration, Instant};

/// Trait that all launcher search sources implement.
#[async_trait]
pub trait SearchSource: Send + Sync {
    /// Unique source identifier (e.g., "apps", "files", "brew").
    fn name(&self) -> &'static str;

    /// Optional prefix for direct routing (e.g., '?' for grep, '@' for contacts).
    /// Sources with a prefix are only queried when the user types that prefix.
    fn prefix(&self) -> Option<char> {
        None
    }

    /// Search this source. Returns scored LauncherItems.
    async fn search(&self, query: &str, limit: usize) -> Vec<LauncherItem>;

    /// Re-index this source (e.g., re-scan apps, reload bookmarks).
    /// No-op for always-live sources like mdfind or rg.
    async fn refresh(&self) {}

    /// Optional TTL for query result caching. Sources that return `Some(duration)`
    /// have their search results cached to avoid repeated subprocess calls.
    /// Only needed for sources that can't pre-index (mdfind, rg).
    fn cache_ttl(&self) -> Option<Duration> {
        None
    }
}

/// Cached search result with timestamp for TTL expiry.
pub struct CachedResult {
    /// The cached search results.
    pub results: Vec<LauncherItem>,
    /// When this cache entry was created.
    pub created_at: Instant,
}

/// Registry of enabled search sources. Handles prefix routing, fan-out, and query caching.
pub struct SourceRegistry {
    sources: Vec<Arc<dyn SearchSource>>,
    query_cache: Arc<DashMap<(&'static str, String), CachedResult>>,
}

impl SourceRegistry {
    pub fn new(sources: Vec<Arc<dyn SearchSource>>) -> Self {
        Self {
            sources,
            query_cache: Arc::new(DashMap::new()),
        }
    }

    /// Get a reference to the shared query cache (for BackgroundRefresher).
    pub fn query_cache(&self) -> Arc<DashMap<(&'static str, String), CachedResult>> {
        Arc::clone(&self.query_cache)
    }

    /// Search all sources. Checks for prefix routing first, otherwise fans out to all.
    pub async fn search(&self, query: &str, limit: usize) -> Vec<LauncherItem> {
        let query = query.trim();
        if query.is_empty() {
            return vec![];
        }

        // Check for prefix routing
        let first_char = query.chars().next().unwrap();
        for source in &self.sources {
            if source.prefix() == Some(first_char) {
                let inner_query = &query[first_char.len_utf8()..];
                return cached_search(source, inner_query.trim(), limit, &self.query_cache).await;
            }
        }

        // No prefix match — fan out to all non-prefix sources
        let futures: Vec<_> = self
            .sources
            .iter()
            .filter(|s| s.prefix().is_none())
            .map(|s| cached_search(s, query, limit, &self.query_cache))
            .collect();

        let results = futures_util::future::join_all(futures).await;
        results.into_iter().flatten().collect()
    }

    /// Refresh all sources that support it.
    pub async fn refresh_all(&self) {
        let futures: Vec<_> = self.sources.iter().map(|s| s.refresh()).collect();
        futures_util::future::join_all(futures).await;
    }

    /// Get the number of registered sources.
    pub fn source_count(&self) -> usize {
        self.sources.len()
    }
}

/// Search a source, using the query cache if the source declares a `cache_ttl`.
async fn cached_search(
    source: &Arc<dyn SearchSource>,
    query: &str,
    limit: usize,
    cache: &DashMap<(&'static str, String), CachedResult>,
) -> Vec<LauncherItem> {
    let ttl = match source.cache_ttl() {
        Some(ttl) => ttl,
        None => return source.search(query, limit).await,
    };

    let key = (source.name(), query.to_string());

    // Check cache
    if let Some(entry) = cache.get(&key) {
        if entry.created_at.elapsed() < ttl {
            return entry.results.clone();
        }
    }

    // Cache miss or expired
    let results = source.search(query, limit).await;

    // Evict expired entries if at capacity (retain only unexpired)
    if cache.len() >= 200 {
        cache.retain(|_, v| v.created_at.elapsed() < ttl);
    }

    cache.insert(
        key,
        CachedResult {
            results: results.clone(),
            created_at: Instant::now(),
        },
    );

    results
}

// ── Shared fuzzy matching helpers ──────────────────────────────────────

use nucleo_matcher::{
    pattern::{CaseMatching, Normalization, Pattern},
    Config, Matcher, Utf32Str,
};

/// Score items against a query using nucleo fuzzy matching on a single field.
/// Returns `(score, index)` pairs sorted by score descending, truncated to `limit`.
pub fn fuzzy_match<'a, T>(
    query: &str,
    items: &'a [T],
    key_fn: impl Fn(&T) -> &str,
    limit: usize,
) -> Vec<(u32, &'a T)> {
    if items.is_empty() || query.is_empty() {
        return vec![];
    }
    let mut matcher = Matcher::new(Config::DEFAULT);
    let pattern = Pattern::parse(query, CaseMatching::Ignore, Normalization::Smart);

    let mut scored: Vec<(u32, &T)> = items
        .iter()
        .filter_map(|item| {
            let mut buf = Vec::new();
            let score = pattern.score(Utf32Str::new(key_fn(item), &mut buf), &mut matcher)?;
            Some((score, item))
        })
        .collect();

    scored.sort_by(|a, b| b.0.cmp(&a.0));
    scored.truncate(limit);
    scored
}

/// Score items against a query using nucleo fuzzy matching on two fields (take best score).
/// Returns `(score, index)` pairs sorted by score descending, truncated to `limit`.
pub fn fuzzy_match2<'a, T>(
    query: &str,
    items: &'a [T],
    key_fn1: impl Fn(&T) -> &str,
    key_fn2: impl Fn(&T) -> Option<&str>,
    limit: usize,
) -> Vec<(u32, &'a T)> {
    if items.is_empty() || query.is_empty() {
        return vec![];
    }
    let mut matcher = Matcher::new(Config::DEFAULT);
    let pattern = Pattern::parse(query, CaseMatching::Ignore, Normalization::Smart);

    let mut scored: Vec<(u32, &T)> = items
        .iter()
        .filter_map(|item| {
            let mut buf = Vec::new();
            let score1 = pattern.score(Utf32Str::new(key_fn1(item), &mut buf), &mut matcher);
            let score2 = key_fn2(item).and_then(|s| {
                let mut buf2 = Vec::new();
                pattern.score(Utf32Str::new(s, &mut buf2), &mut matcher)
            });
            let best = match (score1, score2) {
                (Some(a), Some(b)) => Some(a.max(b)),
                (Some(a), None) | (None, Some(a)) => Some(a),
                (None, None) => None,
            };
            best.map(|score| (score, item))
        })
        .collect();

    scored.sort_by(|a, b| b.0.cmp(&a.0));
    scored.truncate(limit);
    scored
}

/// Resolve a Chromium-based browser's profile directory.
pub fn chromium_profile_dir(browser: &str) -> Option<std::path::PathBuf> {
    let home = std::env::var("HOME").ok()?;
    let app_support = std::path::PathBuf::from(&home).join("Library/Application Support");
    match browser {
        "chrome" => Some(app_support.join("Google/Chrome/Default")),
        "arc" => Some(app_support.join("Arc/User Data/Default")),
        "brave" => Some(app_support.join("BraveSoftware/Brave-Browser/Default")),
        "edge" => Some(app_support.join("Microsoft Edge/Default")),
        _ => None,
    }
}
