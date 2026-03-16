# Launcher Caching & Search Quality Implementation Plan

> **For agentic workers:** REQUIRED: Use superpowers:subagent-driven-development (if subagents available) or superpowers:executing-plans to implement this plan. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Make launcher search instant by pre-indexing all sources into memory, refreshing in the background, caching live subprocess results, and normalizing all scoring to nucleo fuzzy matching.

**Architecture:** Four changes: (1) `BackgroundRefresher` polls sources on intervals, (2) `SourceFileWatcher` watches files for bookmarks/SSH/scripts, (3) all search algorithms normalized to nucleo fuzzy, (4) query-level TTL cache for mdfind/rg results. `RunningAppsSource` and `ContactsSource` are refactored from live-query to pre-loaded index model.

**Tech Stack:** Rust (MSRV 1.75), nucleo-matcher (fuzzy), notify + notify-debouncer-mini (file watching), dashmap (concurrent cache), tokio + tokio-util (async, CancellationToken), parking_lot (RwLock)

**Important notes for implementers:**
- The `SearchSource::name()` trait method returns `&str` but all implementations return string literals. Task 1.1 changes the return type to `&'static str` so the cache key avoids cloning.
- `init_launcher()` needs a new `shutdown_token: CancellationToken` parameter (matching the pattern of `init_productivity`, `init_coaching`, `init_cognitive`). The call site in `init/mod.rs` passes the existing `shutdown_token` local.
- Blocking refresh calls (brew, git repos) must use `tokio::task::spawn_blocking` to avoid stalling the async runtime.

---

## Chunk 1: Foundation — Query Cache + Trait Changes

### Task 1.1: Add dependencies and `cache_ttl()` trait method

**Files:**
- Modify: `crates/feature-launcher/Cargo.toml`
- Modify: `crates/feature-launcher/src/search/mod.rs`

- [ ] **Step 1: Add dependencies to Cargo.toml**

Add under `[dependencies]`:
```toml
dashmap.workspace = true
notify.workspace = true
notify-debouncer-mini.workspace = true
tokio-util.workspace = true
```

- [ ] **Step 2: Change `name()` return type to `&'static str`**

In `search/mod.rs`, change the `SearchSource` trait's `name()` signature:

```rust
    fn name(&self) -> &'static str;
```

This is required so the cache key `(&'static str, String)` compiles. All existing implementations already return string literals, so no impl changes needed.

- [ ] **Step 3: Add `cache_ttl()` to SearchSource trait**

In `search/mod.rs`, add to the `SearchSource` trait after the `refresh()` method:

```rust
    /// Optional TTL for query result caching. Sources that return `Some(duration)`
    /// have their search results cached to avoid repeated subprocess calls.
    /// Only needed for sources that can't pre-index (mdfind, rg).
    fn cache_ttl(&self) -> Option<std::time::Duration> {
        None
    }
```

- [ ] **Step 3: Build to verify**

Run: `cargo build -p feature-launcher`
Expected: Compiles (default trait method, non-breaking)

- [ ] **Step 4: Commit**

```bash
git add crates/feature-launcher/
git commit -m "feat(launcher): add cache_ttl() trait method and caching dependencies"
```

### Task 1.2: Add query cache to SourceRegistry

**Files:**
- Modify: `crates/feature-launcher/src/search/mod.rs`

- [ ] **Step 1: Add CachedResult struct and update SourceRegistry**

Add above `SourceRegistry`:

```rust
use dashmap::DashMap;
use std::time::{Duration, Instant};

struct CachedResult {
    results: Vec<LauncherItem>,
    created_at: Instant,
}
```

Update `SourceRegistry` struct:

```rust
pub struct SourceRegistry {
    sources: Vec<Arc<dyn SearchSource>>,
    query_cache: Arc<DashMap<(&'static str, String), CachedResult>>,
}
```

- [ ] **Step 2: Update constructor**

```rust
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
```

- [ ] **Step 3: Add cache-aware search helper**

Add a private method to `SourceRegistry`:

```rust
    async fn search_source(
        &self,
        source: &Arc<dyn SearchSource>,
        query: &str,
        limit: usize,
    ) -> Vec<LauncherItem> {
        // Check if this source uses caching
        let ttl = match source.cache_ttl() {
            Some(ttl) => ttl,
            None => return source.search(query, limit).await,
        };

        let key = (source.name(), query.to_string());

        // Check cache
        if let Some(entry) = self.query_cache.get(&key) {
            if entry.created_at.elapsed() < ttl {
                return entry.results.clone();
            }
        }

        // Cache miss or expired — call source
        let results = source.search(query, limit).await;

        // Store in cache (evict oldest if at capacity)
        if self.query_cache.len() >= 200 {
            // O(n) scan to find oldest — acceptable for n ≤ 200
            if let Some(oldest_key) = self
                .query_cache
                .iter()
                .min_by_key(|entry| entry.value().created_at)
                .map(|entry| entry.key().clone())
            {
                self.query_cache.remove(&oldest_key);
            }
        }

        self.query_cache.insert(key, CachedResult {
            results: results.clone(),
            created_at: Instant::now(),
        });

        results
    }
```

- [ ] **Step 4: Update fan-out search to use cache-aware helper**

Replace the fan-out section in `SourceRegistry::search()`:

```rust
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
                return self
                    .search_source(source, inner_query.trim(), limit)
                    .await;
            }
        }

        // No prefix match — fan out to all non-prefix sources
        let sources: Vec<_> = self
            .sources
            .iter()
            .filter(|s| s.prefix().is_none())
            .cloned()
            .collect();

        let mut handles = Vec::with_capacity(sources.len());
        for source in &sources {
            let s = Arc::clone(source);
            let q = query.to_string();
            let cache = Arc::clone(&self.query_cache);
            let registry_sources = self.sources.clone();
            // For cached sources, check inline; for non-cached, spawn directly
            handles.push(self.search_source(source, query, limit));
        }

        let results = futures_util::future::join_all(handles).await;
        results.into_iter().flatten().collect()
    }
```

Wait — this won't work because `search_source` borrows `self`. Let me restructure. The fan-out needs to be self-contained. Better approach: make `search_source` a free function that takes the cache:

Replace the entire `search_source` and `search` approach:

```rust
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
```

And make `cached_search` a free async function outside the impl:

```rust
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

    // Evict oldest if at capacity
    if cache.len() >= 200 {
        if let Some(oldest_key) = cache
            .iter()
            .min_by_key(|entry| entry.value().created_at)
            .map(|entry| entry.key().clone())
        {
            cache.remove(&oldest_key);
        }
    }

    cache.insert(key, CachedResult {
        results: results.clone(),
        created_at: Instant::now(),
    });

    results
}
```

- [ ] **Step 5: Build to verify**

Run: `cargo build -p feature-launcher`
Expected: Compiles

- [ ] **Step 6: Commit**

```bash
git add crates/feature-launcher/src/search/mod.rs
git commit -m "feat(launcher): add query cache to SourceRegistry with TTL support"
```

### Task 1.3: Add cache_ttl to FileSearchSource and ContentGrepSource

**Files:**
- Modify: `crates/feature-launcher/src/search/file_search.rs`
- Modify: `crates/feature-launcher/src/search/content_grep.rs`

- [ ] **Step 1: Add cache_ttl to FileSearchSource**

In `file_search.rs`, add to the `SearchSource` impl:

```rust
    fn cache_ttl(&self) -> Option<std::time::Duration> {
        Some(std::time::Duration::from_secs(5))
    }
```

- [ ] **Step 2: Add cache_ttl to ContentGrepSource**

In `content_grep.rs`, add to the `SearchSource` impl:

```rust
    fn cache_ttl(&self) -> Option<std::time::Duration> {
        Some(std::time::Duration::from_secs(5))
    }
```

- [ ] **Step 3: Build and test**

Run: `cargo build -p feature-launcher && cargo nextest run -p feature-launcher`
Expected: Compiles, all tests pass

- [ ] **Step 4: Commit**

```bash
git add crates/feature-launcher/src/search/
git commit -m "feat(launcher): enable 5s query cache for mdfind and rg sources"
```

---

## Chunk 2: Normalize All Sources to Nucleo Fuzzy

### Task 2.1: Convert SSH hosts to nucleo fuzzy

**Files:**
- Modify: `crates/feature-launcher/src/search/ssh_hosts.rs`

- [ ] **Step 1: Replace search method**

Replace the `search` method in the `SearchSource` impl with:

```rust
    async fn search(&self, query: &str, limit: usize) -> Vec<LauncherItem> {
        use nucleo_matcher::{
            pattern::{CaseMatching, Normalization, Pattern},
            Config, Matcher,
        };

        let hosts = self.hosts.read();
        if hosts.is_empty() || query.is_empty() {
            return vec![];
        }

        let mut matcher = Matcher::new(Config::DEFAULT);
        let pattern = Pattern::parse(query, CaseMatching::Ignore, Normalization::Smart);

        let mut scored: Vec<(u32, &SshEntry)> = hosts
            .iter()
            .filter_map(|h| {
                let mut buf = Vec::new();
                let host_score = pattern.score(
                    nucleo_matcher::Utf32Str::new(&h.host, &mut buf),
                    &mut matcher,
                );
                // Also check hostname for a bonus
                let hostname_score = h.hostname.as_ref().and_then(|hn| {
                    let mut buf2 = Vec::new();
                    pattern.score(
                        nucleo_matcher::Utf32Str::new(hn, &mut buf2),
                        &mut matcher,
                    )
                });
                let best = match (host_score, hostname_score) {
                    (Some(a), Some(b)) => Some(a.max(b)),
                    (Some(a), None) | (None, Some(a)) => Some(a),
                    (None, None) => None,
                };
                best.map(|score| (score, h))
            })
            .collect();

        scored.sort_by(|a, b| b.0.cmp(&a.0));
        scored.truncate(limit);

        scored
            .into_iter()
            .map(|(score, h)| {
                let subtitle = match (&h.user, &h.hostname) {
                    (Some(u), Some(hn)) => format!("{u}@{hn}"),
                    (None, Some(hn)) => hn.clone(),
                    (Some(u), None) => format!("{u}@{}", h.host),
                    (None, None) => h.host.clone(),
                };
                LauncherItem {
                    id: format!("ssh:{}", h.host),
                    title: h.host.clone(),
                    subtitle: Some(subtitle),
                    icon: Some("terminal".to_string()),
                    kind: LauncherItemKind::SshHost {
                        host: h.host.clone(),
                        user: h.user.clone(),
                    },
                    score: (score as f64) / 1000.0 * 0.6,
                }
            })
            .collect()
    }
```

- [ ] **Step 2: Build and run tests**

Run: `cargo build -p feature-launcher && cargo nextest run -p feature-launcher`
Expected: Compiles, existing SSH test still passes

- [ ] **Step 3: Commit**

```bash
git add crates/feature-launcher/src/search/ssh_hosts.rs
git commit -m "feat(launcher): convert SSH hosts to nucleo fuzzy with 0.6 weight"
```

### Task 2.2: Convert browser history to nucleo fuzzy

**Files:**
- Modify: `crates/feature-launcher/src/search/browser_history.rs`

- [ ] **Step 1: Replace search method**

Replace the `search` method in the `SearchSource` impl with:

```rust
    async fn search(&self, query: &str, limit: usize) -> Vec<LauncherItem> {
        use nucleo_matcher::{
            pattern::{CaseMatching, Normalization, Pattern},
            Config, Matcher,
        };

        let entries = self.entries.read();
        if entries.is_empty() || query.is_empty() {
            return vec![];
        }

        let mut matcher = Matcher::new(Config::DEFAULT);
        let pattern = Pattern::parse(query, CaseMatching::Ignore, Normalization::Smart);

        let mut scored: Vec<(u32, &HistoryEntry)> = entries
            .iter()
            .filter_map(|e| {
                let mut buf = Vec::new();
                let title_score = pattern.score(
                    nucleo_matcher::Utf32Str::new(&e.title, &mut buf),
                    &mut matcher,
                );
                let mut buf2 = Vec::new();
                let url_score = pattern.score(
                    nucleo_matcher::Utf32Str::new(&e.url, &mut buf2),
                    &mut matcher,
                );
                let best = match (title_score, url_score) {
                    (Some(a), Some(b)) => Some(a.max(b)),
                    (Some(a), None) | (None, Some(a)) => Some(a),
                    (None, None) => None,
                };
                best.map(|score| (score, e))
            })
            .collect();

        scored.sort_by(|a, b| b.0.cmp(&a.0));
        scored.truncate(limit);

        scored
            .into_iter()
            .map(|(score, e)| LauncherItem {
                id: format!("history:{}", e.url),
                title: e.title.clone(),
                subtitle: Some(e.url.clone()),
                icon: Some("globe".to_string()),
                kind: LauncherItemKind::BrowserHistory {
                    url: e.url.clone(),
                    visited_at: e.last_visit.clone(),
                },
                score: (score as f64) / 1000.0 * 0.4,
            })
            .collect()
    }
```

- [ ] **Step 2: Build to verify**

Run: `cargo build -p feature-launcher`
Expected: Compiles

- [ ] **Step 3: Commit**

```bash
git add crates/feature-launcher/src/search/browser_history.rs
git commit -m "feat(launcher): convert browser history to nucleo fuzzy with 0.4 weight"
```

### Task 2.3: Convert scripts to nucleo fuzzy

**Files:**
- Modify: `crates/feature-launcher/src/search/script_runner.rs`

- [ ] **Step 1: Replace the inherent `search` method**

Replace the `pub fn search(...)` method in `impl ScriptRunner`:

```rust
    pub fn search(&self, query: &str, limit: usize) -> Vec<LauncherItem> {
        use nucleo_matcher::{
            pattern::{CaseMatching, Normalization, Pattern},
            Config, Matcher,
        };

        let scripts = self.scripts.read();
        if scripts.is_empty() || query.is_empty() {
            return vec![];
        }

        let mut matcher = Matcher::new(Config::DEFAULT);
        let pattern = Pattern::parse(query, CaseMatching::Ignore, Normalization::Smart);

        let mut scored: Vec<(u32, &ScriptEntry)> = scripts
            .iter()
            .filter_map(|s| {
                let mut buf = Vec::new();
                let name_score = pattern.score(
                    nucleo_matcher::Utf32Str::new(&s.name, &mut buf),
                    &mut matcher,
                );
                let desc_score = s.description.as_ref().and_then(|d| {
                    let mut buf2 = Vec::new();
                    pattern.score(
                        nucleo_matcher::Utf32Str::new(d, &mut buf2),
                        &mut matcher,
                    )
                });
                let best = match (name_score, desc_score) {
                    (Some(a), Some(b)) => Some(a.max(b)),
                    (Some(a), None) | (None, Some(a)) => Some(a),
                    (None, None) => None,
                };
                best.map(|score| (score, s))
            })
            .collect();

        scored.sort_by(|a, b| b.0.cmp(&a.0));
        scored.truncate(limit);

        scored
            .into_iter()
            .map(|(score, s)| LauncherItem {
                id: format!("script:{}", s.path.display()),
                title: s.name.clone(),
                subtitle: s.description.clone(),
                icon: s.icon.clone().or_else(|| Some("file-code".to_string())),
                kind: LauncherItemKind::Script {
                    path: s.path.clone(),
                    name: s.name.clone(),
                },
                score: (score as f64) / 1000.0 * 0.6,
            })
            .collect()
    }
```

- [ ] **Step 2: Build and run tests**

Run: `cargo build -p feature-launcher && cargo nextest run -p feature-launcher`
Expected: Compiles, script_runner tests pass (the `test_search_scripts` test searches "deploy" which will still match "Deploy Staging" via fuzzy)

- [ ] **Step 3: Commit**

```bash
git add crates/feature-launcher/src/search/script_runner.rs
git commit -m "feat(launcher): convert scripts to nucleo fuzzy with 0.6 weight"
```

### Task 2.4: Convert system commands to nucleo fuzzy

**Files:**
- Modify: `crates/feature-launcher/src/search/system_commands.rs`

- [ ] **Step 1: Replace the inherent `search` method**

Replace the `pub fn search(query: &str) -> Vec<LauncherItem>` method:

```rust
    pub fn search(query: &str) -> Vec<LauncherItem> {
        use nucleo_matcher::{
            pattern::{CaseMatching, Normalization, Pattern},
            Config, Matcher,
        };

        if query.is_empty() {
            // Return all commands with base score when no query
            return COMMANDS
                .iter()
                .map(|cmd| LauncherItem {
                    id: format!("system:{:?}", cmd.action),
                    title: cmd.title.to_string(),
                    subtitle: Some(cmd.subtitle.to_string()),
                    icon: Some("terminal".to_string()),
                    kind: LauncherItemKind::SystemCommand {
                        action: cmd.action.clone(),
                    },
                    score: 0.5,
                })
                .collect();
        }

        let mut matcher = Matcher::new(Config::DEFAULT);
        let pattern = Pattern::parse(query, CaseMatching::Ignore, Normalization::Smart);

        let mut scored: Vec<(u32, &CommandDef)> = COMMANDS
            .iter()
            .filter_map(|cmd| {
                let mut buf = Vec::new();
                let title_score = pattern.score(
                    nucleo_matcher::Utf32Str::new(cmd.title, &mut buf),
                    &mut matcher,
                );
                // Also check keywords
                let keyword_score = cmd.keywords.iter().filter_map(|kw| {
                    let mut buf2 = Vec::new();
                    pattern.score(
                        nucleo_matcher::Utf32Str::new(kw, &mut buf2),
                        &mut matcher,
                    )
                }).max();
                let best = match (title_score, keyword_score) {
                    (Some(a), Some(b)) => Some(a.max(b)),
                    (Some(a), None) | (None, Some(a)) => Some(a),
                    (None, None) => None,
                };
                best.map(|score| (score, cmd))
            })
            .collect();

        scored.sort_by(|a, b| b.0.cmp(&a.0));

        scored
            .into_iter()
            .map(|(score, cmd)| LauncherItem {
                id: format!("system:{:?}", cmd.action),
                title: cmd.title.to_string(),
                subtitle: Some(cmd.subtitle.to_string()),
                icon: Some("terminal".to_string()),
                kind: LauncherItemKind::SystemCommand {
                    action: cmd.action.clone(),
                },
                score: (score as f64) / 1000.0 * 1.0,
            })
            .collect()
    }
```

- [ ] **Step 2: Build and run tests**

Run: `cargo build -p feature-launcher && cargo nextest run -p feature-launcher`
Expected: Compiles. `test_search_exact` ("lock") finds Lock Screen. `test_search_fuzzy` ("dark") finds Toggle Dark Mode. `test_search_empty_returns_all` returns all 8.

- [ ] **Step 3: Commit**

```bash
git add crates/feature-launcher/src/search/system_commands.rs
git commit -m "feat(launcher): convert system commands to nucleo fuzzy with 1.0 weight"
```

### Task 2.5: Normalize score weights for existing nucleo sources

**Files:**
- Modify: `crates/feature-launcher/src/search/app_index.rs`
- Modify: `crates/feature-launcher/src/search/bookmarks.rs`
- Modify: `crates/feature-launcher/src/search/brew.rs`
- Modify: `crates/feature-launcher/src/search/git_repos.rs`
- Modify: `crates/feature-launcher/src/search/system_prefs.rs`
- Modify: `crates/feature-launcher/src/search/file_search.rs`
- Modify: `crates/feature-launcher/src/search/content_grep.rs`

- [ ] **Step 1: Update score formulas**

For each file, find the score line and update to use the normalized weight:

| File | Current score | New score |
|---|---|---|
| `app_index.rs` | `(score as f64) / 1000.0` | `(score as f64) / 1000.0 * 1.0` |
| `bookmarks.rs` | `(score as f64) / 1000.0 * 0.7` | `(score as f64) / 1000.0 * 0.8` |
| `brew.rs` | `(score as f64) / 1000.0 * 0.4` | `(score as f64) / 1000.0 * 0.4` (no change) |
| `git_repos.rs` | `(score as f64) / 1000.0 * 0.8` | `(score as f64) / 1000.0 * 0.8` (no change) |
| `system_prefs.rs` | `(score as f64) / 1000.0 * 0.6` | `(score as f64) / 1000.0 * 0.6` (no change) |
| `file_search.rs` | `score: 0.8` (fixed) | `score: 0.8` (no change — mdfind has no per-result ranking) |
| `content_grep.rs` | `score: 0.7` (fixed) | `score: 0.7` (no change — rg results are relevance-ordered) |

Only `app_index.rs` and `bookmarks.rs` need actual changes.

In `app_index.rs`, the score line is `score: (score as f64) / 1000.0` — change to `score: (score as f64) / 1000.0 * 1.0` (explicit weight, or leave as-is since `* 1.0` is identity).

In `bookmarks.rs`, change `score: (score as f64) / 1000.0 * 0.7` to `score: (score as f64) / 1000.0 * 0.8`.

- [ ] **Step 2: Build and test**

Run: `cargo build -p feature-launcher && cargo nextest run -p feature-launcher`
Expected: All tests pass

- [ ] **Step 3: Commit**

```bash
git add crates/feature-launcher/src/search/
git commit -m "feat(launcher): normalize score weights across all sources"
```

---

## Chunk 3: Convert RunningApps + Contacts to Pre-Loaded Index

### Task 3.1: Refactor RunningAppsSource to pre-loaded index

**Files:**
- Modify: `crates/feature-launcher/src/search/running_apps.rs`

- [ ] **Step 1: Add in-memory index field**

Replace the struct and constructor:

```rust
use crate::types::*;
use parking_lot::RwLock;
use std::sync::Arc;

#[derive(Clone)]
pub struct RunningAppsSource {
    apps: Arc<RwLock<Vec<(String, u32, std::path::PathBuf)>>>,
}

impl RunningAppsSource {
    pub fn new() -> Self {
        Self {
            apps: Arc::new(RwLock::new(Vec::new())),
        }
    }
}

impl Default for RunningAppsSource {
    fn default() -> Self {
        Self::new()
    }
}
```

- [ ] **Step 2: Move osascript call to refresh()**

Update the `SearchSource` impl:

```rust
#[async_trait::async_trait]
impl super::SearchSource for RunningAppsSource {
    fn name(&self) -> &str {
        "running_apps"
    }

    async fn search(&self, query: &str, limit: usize) -> Vec<LauncherItem> {
        if query.is_empty() {
            return vec![];
        }

        use nucleo_matcher::{
            pattern::{CaseMatching, Normalization, Pattern},
            Config, Matcher,
        };

        let apps = self.apps.read();
        if apps.is_empty() {
            return vec![];
        }

        let mut matcher = Matcher::new(Config::DEFAULT);
        let pattern = Pattern::parse(query, CaseMatching::Ignore, Normalization::Smart);

        let mut scored: Vec<(u32, &(String, u32, std::path::PathBuf))> = apps
            .iter()
            .filter_map(|app| {
                let mut buf = Vec::new();
                let score = pattern.score(
                    nucleo_matcher::Utf32Str::new(&app.0, &mut buf),
                    &mut matcher,
                )?;
                Some((score, app))
            })
            .collect();

        scored.sort_by(|a, b| b.0.cmp(&a.0));
        scored.truncate(limit);

        scored
            .into_iter()
            .map(|(score, (name, pid, path))| LauncherItem {
                id: format!("running:{pid}"),
                title: name.clone(),
                subtitle: Some("Running".to_string()),
                icon: Some("activity".to_string()),
                kind: LauncherItemKind::RunningApp {
                    pid: *pid,
                    path: path.clone(),
                },
                score: (score as f64) / 1000.0 * 1.2,
            })
            .collect()
    }

    async fn refresh(&self) {
        let apps = Self::get_running_apps();
        tracing::debug!("Refreshed {} running apps", apps.len());
        *self.apps.write() = apps;
    }
}
```

- [ ] **Step 3: Move get_running_apps to a static method**

Keep the existing `#[cfg(target_os = "macos")]` impl block with `get_running_apps()` as-is (it's already a static method returning `Vec<(String, u32, PathBuf)>`).

Add the non-macOS refresh:

```rust
#[cfg(not(target_os = "macos"))]
impl RunningAppsSource {
    fn get_running_apps() -> Vec<(String, u32, std::path::PathBuf)> {
        vec![]
    }
}
```

- [ ] **Step 4: Build and verify**

Run: `cargo build -p feature-launcher`
Expected: Compiles

- [ ] **Step 5: Commit**

```bash
git add crates/feature-launcher/src/search/running_apps.rs
git commit -m "feat(launcher): convert RunningAppsSource to pre-loaded index with refresh()"
```

### Task 3.2: Add refresh() implementation to ScriptRunner

**Files:**
- Modify: `crates/feature-launcher/src/search/script_runner.rs`

- [ ] **Step 1: Add scripts_dir field and update refresh()**

Add a `scripts_dir` field to `ScriptRunner`:

```rust
pub struct ScriptRunner {
    scripts: Arc<RwLock<Vec<ScriptEntry>>>,
    scripts_dir: Option<std::path::PathBuf>,
}
```

Update `new()` and add `with_dir()`:

```rust
impl ScriptRunner {
    pub fn new() -> Self {
        Self {
            scripts: Arc::new(RwLock::new(Vec::new())),
            scripts_dir: None,
        }
    }

    pub fn with_dir(dir: std::path::PathBuf) -> Self {
        Self {
            scripts: Arc::new(RwLock::new(Vec::new())),
            scripts_dir: Some(dir),
        }
    }
```

Update the `SearchSource` impl's `refresh()`:

```rust
    async fn refresh(&self) {
        if let Some(dir) = &self.scripts_dir {
            if dir.exists() {
                let scripts = Self::discover(dir);
                tracing::info!("Re-discovered {} launcher scripts", scripts.len());
                self.set_scripts(scripts);
            }
        }
    }
```

- [ ] **Step 2: Update init_launcher.rs to use `with_dir`**

In `init_launcher.rs`, change the ScriptRunner construction:

```rust
    if launcher_config.sources.scripts.enabled {
        let scripts_dir = shellexpand::tilde(&launcher_config.sources.scripts.dir).to_string();
        let scripts_path = std::path::PathBuf::from(&scripts_dir);
        let script_runner = Arc::new(ScriptRunner::with_dir(scripts_path.clone()));
        if scripts_path.exists() {
            let scripts = ScriptRunner::discover(&scripts_path);
            info!("discovered {} launcher scripts", scripts.len());
            script_runner.set_scripts(scripts);
        }
        sources.push(script_runner);
    }
```

- [ ] **Step 3: Build and test**

Run: `cargo build -p feature-launcher && cargo nextest run -p feature-launcher`
Expected: All tests pass

- [ ] **Step 4: Commit**

```bash
git add crates/feature-launcher/src/search/script_runner.rs crates/app-core/src/init/launcher.rs
git commit -m "feat(launcher): add refresh() to ScriptRunner for file-watcher support"
```

### Task 3.3: Refactor ContactsSource to pre-loaded index

**Files:**
- Modify: `crates/feature-launcher/src/search/contacts.rs`

- [ ] **Step 1: Add ContactEntry struct and in-memory index**

Replace the struct definition and constructor:

```rust
use crate::types::*;
use parking_lot::RwLock;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

#[derive(Debug, Clone)]
struct ContactEntry {
    name: String,
    email: Option<String>,
    phone: Option<String>,
}

#[derive(Clone)]
pub struct ContactsSource {
    contacts: Arc<RwLock<Vec<ContactEntry>>>,
    permission_warned: Arc<AtomicBool>,
}

impl ContactsSource {
    pub fn new() -> Self {
        Self {
            contacts: Arc::new(RwLock::new(Vec::new())),
            permission_warned: Arc::new(AtomicBool::new(false)),
        }
    }
}

impl Default for ContactsSource {
    fn default() -> Self {
        Self::new()
    }
}
```

- [ ] **Step 2: Move JXA to refresh(), search from in-memory cache**

Update the JXA script to fetch ALL contacts (no query filter):

```rust
#[cfg(target_os = "macos")]
impl ContactsSource {
    const JXA_FETCH_ALL: &'static str = r#"
        var app = Application("Contacts");
        var people = app.people();
        var results = [];
        var limit = Math.min(people.length, 500);
        for (var i = 0; i < limit; i++) {
            var p = people[i];
            var emails = p.emails();
            var phones = p.phones();
            results.push({
                name: p.name(),
                email: emails.length > 0 ? emails[0].value() : null,
                phone: phones.length > 0 ? phones[0].value() : null,
            });
        }
        JSON.stringify(results);
    "#;

    async fn load_contacts(&self) -> Vec<ContactEntry> {
        let output = tokio::time::timeout(
            std::time::Duration::from_secs(5),
            tokio::process::Command::new("osascript")
                .args(["-l", "JavaScript", "-e", Self::JXA_FETCH_ALL])
                .output(),
        )
        .await;

        let output = match output {
            Ok(Ok(o)) if o.status.success() => o,
            Ok(Ok(o)) => {
                if !self.permission_warned.swap(true, Ordering::Relaxed) {
                    let stderr = String::from_utf8_lossy(&o.stderr);
                    if stderr.contains("not allowed") || stderr.contains("denied") {
                        tracing::warn!(
                            "Contacts access denied. Grant in System Settings > Privacy > Contacts."
                        );
                    }
                }
                return vec![];
            }
            _ => return vec![],
        };

        let stdout = String::from_utf8_lossy(&output.stdout);
        let contacts: Vec<serde_json::Value> =
            serde_json::from_str(stdout.trim()).unwrap_or_default();

        contacts
            .into_iter()
            .filter_map(|c| {
                let name = c.get("name")?.as_str()?.to_string();
                let email = c.get("email").and_then(|e| e.as_str()).map(|s| s.to_string());
                let phone = c.get("phone").and_then(|p| p.as_str()).map(|s| s.to_string());
                Some(ContactEntry { name, email, phone })
            })
            .collect()
    }
}

#[cfg(not(target_os = "macos"))]
impl ContactsSource {
    async fn load_contacts(&self) -> Vec<ContactEntry> {
        vec![]
    }
}
```

- [ ] **Step 3: Update SearchSource impl**

```rust
#[async_trait::async_trait]
impl super::SearchSource for ContactsSource {
    fn name(&self) -> &str {
        "contacts"
    }

    fn prefix(&self) -> Option<char> {
        Some('@')
    }

    async fn search(&self, query: &str, limit: usize) -> Vec<LauncherItem> {
        if query.is_empty() {
            return vec![];
        }

        use nucleo_matcher::{
            pattern::{CaseMatching, Normalization, Pattern},
            Config, Matcher,
        };

        let contacts = self.contacts.read();
        if contacts.is_empty() {
            return vec![];
        }

        let mut matcher = Matcher::new(Config::DEFAULT);
        let pattern = Pattern::parse(query, CaseMatching::Ignore, Normalization::Smart);

        let mut scored: Vec<(u32, &ContactEntry)> = contacts
            .iter()
            .filter_map(|c| {
                let mut buf = Vec::new();
                let score = pattern.score(
                    nucleo_matcher::Utf32Str::new(&c.name, &mut buf),
                    &mut matcher,
                )?;
                Some((score, c))
            })
            .collect();

        scored.sort_by(|a, b| b.0.cmp(&a.0));
        scored.truncate(limit);

        scored
            .into_iter()
            .map(|(score, c)| {
                let subtitle = c.email.clone()
                    .or_else(|| c.phone.clone())
                    .unwrap_or_default();
                LauncherItem {
                    id: format!("contact:{}", c.name),
                    title: c.name.clone(),
                    subtitle: Some(subtitle),
                    icon: Some("user".to_string()),
                    kind: LauncherItemKind::Contact {
                        name: c.name.clone(),
                        email: c.email.clone(),
                        phone: c.phone.clone(),
                    },
                    score: (score as f64) / 1000.0 * 0.6,
                }
            })
            .collect()
    }

    async fn refresh(&self) {
        let contacts = self.load_contacts().await;
        tracing::info!("Indexed {} contacts", contacts.len());
        *self.contacts.write() = contacts;
    }
}
```

- [ ] **Step 4: Build and verify**

Run: `cargo build -p feature-launcher`
Expected: Compiles

- [ ] **Step 5: Commit**

```bash
git add crates/feature-launcher/src/search/contacts.rs
git commit -m "feat(launcher): convert ContactsSource to pre-loaded index with refresh()"
```

---

## Chunk 4: BackgroundRefresher

### Task 4.1: Create BackgroundRefresher

**Files:**
- Create: `crates/feature-launcher/src/search/background.rs`
- Modify: `crates/feature-launcher/src/search/mod.rs`

- [ ] **Step 1: Create background.rs**

```rust
use super::SearchSource;
use dashmap::DashMap;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio_util::sync::CancellationToken;

use super::CachedResult;

pub struct RefreshEntry {
    pub source: Arc<dyn SearchSource>,
    pub interval: Duration,
}

pub struct BackgroundRefresher {
    entries: Vec<(RefreshEntry, Instant)>,
    query_cache: Arc<DashMap<(&'static str, String), CachedResult>>,
    shutdown: CancellationToken,
    last_cache_eviction: Instant,
}

impl BackgroundRefresher {
    pub fn new(
        entries: Vec<RefreshEntry>,
        query_cache: Arc<DashMap<(&'static str, String), CachedResult>>,
        shutdown: CancellationToken,
    ) -> Self {
        // Initialize last_refreshed to now - interval so first tick triggers refresh
        let entries = entries
            .into_iter()
            .map(|e| {
                let initial = Instant::now() - e.interval;
                (e, initial)
            })
            .collect();

        Self {
            entries,
            query_cache,
            shutdown,
            last_cache_eviction: Instant::now(),
        }
    }

    pub async fn run(mut self) {
        let mut tick = tokio::time::interval(Duration::from_secs(1));

        loop {
            tokio::select! {
                _ = self.shutdown.cancelled() => {
                    tracing::info!("BackgroundRefresher shutting down");
                    break;
                }
                _ = tick.tick() => {
                    self.tick().await;
                }
            }
        }
    }

    async fn tick(&mut self) {
        let now = Instant::now();

        for (entry, last_refreshed) in &mut self.entries {
            if now.duration_since(*last_refreshed) >= entry.interval {
                let source = Arc::clone(&entry.source);
                tracing::debug!("Refreshing source: {}", source.name());
                // Spawn refresh as independent task so slow refreshes don't block tick
                tokio::spawn(async move {
                    source.refresh().await;
                });
                *last_refreshed = Instant::now();
            }
        }

        // Evict expired cache entries every 60s
        if now.duration_since(self.last_cache_eviction) >= Duration::from_secs(60) {
            self.query_cache.retain(|_, v| {
                v.created_at.elapsed() < Duration::from_secs(10)
            });
            self.last_cache_eviction = Instant::now();
        }
    }

    /// Spawn the refresher as a background task. Returns immediately.
    pub fn spawn(self) -> tokio::task::JoinHandle<()> {
        tokio::spawn(self.run())
    }
}
```

- [ ] **Step 2: Register module in mod.rs**

Add `pub mod background;` and `pub use background::{BackgroundRefresher, RefreshEntry};` to `search/mod.rs`.

Also make `CachedResult` public so `background.rs` can reference it:

```rust
pub(crate) struct CachedResult {
```

- [ ] **Step 3: Update lib.rs exports**

Add `BackgroundRefresher` and `RefreshEntry` to the re-exports (they flow through `pub use search::*;`).

- [ ] **Step 4: Build to verify**

Run: `cargo build -p feature-launcher`
Expected: Compiles

- [ ] **Step 5: Commit**

```bash
git add crates/feature-launcher/src/search/
git commit -m "feat(launcher): add BackgroundRefresher with per-source intervals"
```

---

## Chunk 5: SourceFileWatcher

### Task 5.1: Create SourceFileWatcher

**Files:**
- Create: `crates/feature-launcher/src/search/file_watcher.rs`
- Modify: `crates/feature-launcher/src/search/mod.rs`

- [ ] **Step 1: Create file_watcher.rs**

```rust
use super::SearchSource;
use notify_debouncer_mini::{new_debouncer, DebouncedEventKind};
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

pub struct SourceFileWatcher {
    _debouncer: notify_debouncer_mini::Debouncer<notify::RecommendedWatcher>,
}

impl SourceFileWatcher {
    /// Create a file watcher that triggers source refresh on file changes.
    ///
    /// `watches` maps file/directory paths to the source that should be refreshed.
    /// Non-existent paths are silently skipped.
    /// The watcher is dropped (and the OS thread joined) when this struct is dropped.
    pub fn start(
        watches: Vec<(PathBuf, Arc<dyn SearchSource>)>,
    ) -> Result<Self, notify::Error> {
        let source_map: Arc<HashMap<PathBuf, Arc<dyn SearchSource>>> = Arc::new(
            watches
                .iter()
                .filter(|(path, _)| path.exists())
                .map(|(path, source)| (path.clone(), Arc::clone(source)))
                .collect(),
        );

        if source_map.is_empty() {
            tracing::debug!("SourceFileWatcher: no valid paths to watch");
        }

        let map_clone = Arc::clone(&source_map);
        let mut debouncer = new_debouncer(Duration::from_millis(500), move |events: Result<Vec<notify_debouncer_mini::DebouncedEvent>, notify::Error>| {
            let events = match events {
                Ok(e) => e,
                Err(e) => {
                    tracing::debug!("File watcher error: {e}");
                    return;
                }
            };

            for event in events {
                if event.kind != DebouncedEventKind::Any {
                    continue;
                }
                // Check if the changed path matches any watched path
                let changed = &event.path;
                for (watched_path, source) in map_clone.iter() {
                    if changed.starts_with(watched_path) || changed == watched_path {
                        let source = Arc::clone(source);
                        tracing::info!(
                            "File change detected for {}, refreshing {}",
                            watched_path.display(),
                            source.name()
                        );
                        // Fire-and-forget refresh in tokio runtime
                        if let Ok(handle) = tokio::runtime::Handle::try_current() {
                            handle.spawn(async move {
                                source.refresh().await;
                            });
                        }
                        break;
                    }
                }
            }
        })?;

        // Watch all valid paths
        for (path, source) in &*source_map {
            if let Err(e) = debouncer.watcher().watch(path, notify::RecursiveMode::NonRecursive) {
                tracing::warn!(
                    "Failed to watch {} for {}: {e}",
                    path.display(),
                    source.name()
                );
            } else {
                tracing::info!("Watching {} for {} changes", path.display(), source.name());
            }
        }

        Ok(Self {
            _debouncer: debouncer,
        })
    }
}
```

- [ ] **Step 2: Register module in mod.rs**

Add `pub mod file_watcher;` and `pub use file_watcher::SourceFileWatcher;` to `search/mod.rs`.

- [ ] **Step 3: Make BookmarksSource::browser_bookmarks_path public**

In `bookmarks.rs`, change the visibility of `browser_bookmarks_path`:

```rust
    pub fn browser_bookmarks_path(browser: &str) -> Option<PathBuf> {
```

- [ ] **Step 4: Build to verify**

Run: `cargo build -p feature-launcher`
Expected: Compiles

- [ ] **Step 5: Commit**

```bash
git add crates/feature-launcher/src/search/
git commit -m "feat(launcher): add SourceFileWatcher for bookmarks, SSH, and scripts"
```

---

## Chunk 6: Wire Everything into Init

### Task 6.1: Update init_launcher to spawn BackgroundRefresher and SourceFileWatcher

**Files:**
- Modify: `crates/app-core/src/init/launcher.rs`
- Modify: `crates/app-core/src/init/mod.rs`
- Modify: `crates/app-core/src/handlers/launcher/search_engine.rs`

- [ ] **Step 1: Add file_watcher field to LauncherSearchEngine**

In `search_engine.rs`, add the field:

```rust
pub struct LauncherSearchEngine {
    pub registry: SourceRegistry,
    pub frequency_repo: FrequencyRepo,
    pub clipboard_repo: ClipboardRepo,
    // Stored here so it's dropped (and OS watcher thread joined) on shutdown
    pub _file_watcher: Option<feature_launcher::SourceFileWatcher>,
}
```

- [ ] **Step 2: Add shutdown_token parameter to init_launcher**

Change the signature of `init_launcher` to accept a `CancellationToken`:

```rust
pub(super) async fn init_launcher(
    config: &config::Config,
    storage_pool: &StoragePool,
    shutdown_token: &CancellationToken,
) -> LauncherResult {
```

Add `use tokio_util::sync::CancellationToken;` to imports.

Update the call site in `init/mod.rs` to pass `&shutdown_token`:

```rust
        let launcher::LauncherResult { launcher_engine } =
            launcher::init_launcher(&config, &storage_pool, &shutdown_token).await;
```

- [ ] **Step 3: Build refresh entries and file watches BEFORE consuming sources**

After the existing source registration loop but BEFORE `SourceRegistry::new(sources)`, build both refresh entries and file watches from `&sources`:

```rust
    // -- Build refresh entries (before sources are consumed by SourceRegistry) --
    let mut refresh_entries: Vec<feature_launcher::RefreshEntry> = Vec::new();

    // Helper to find a source by name
    let find_source = |name: &str| -> Option<Arc<dyn feature_launcher::SearchSource>> {
        sources.iter().find(|s| s.name() == name).cloned()
    };

    // Running apps: 3s
    if let Some(s) = find_source("running_apps") {
        refresh_entries.push(feature_launcher::RefreshEntry {
            source: s,
            interval: std::time::Duration::from_secs(3),
        });
    }
    // Contacts: 30s
    if let Some(s) = find_source("contacts") {
        refresh_entries.push(feature_launcher::RefreshEntry {
            source: s,
            interval: std::time::Duration::from_secs(30),
        });
    }
    // Browser history: 2min
    if let Some(s) = find_source("browser_history") {
        refresh_entries.push(feature_launcher::RefreshEntry {
            source: s,
            interval: std::time::Duration::from_secs(120),
        });
    }
    // Brew: 5min
    if let Some(s) = find_source("brew") {
        refresh_entries.push(feature_launcher::RefreshEntry {
            source: s,
            interval: std::time::Duration::from_secs(300),
        });
    }
    // Git repos: 5min
    if let Some(s) = find_source("git_repos") {
        refresh_entries.push(feature_launcher::RefreshEntry {
            source: s,
            interval: std::time::Duration::from_secs(300),
        });
    }

    // -- Build file watches --
    let mut watches: Vec<(std::path::PathBuf, Arc<dyn feature_launcher::SearchSource>)> = Vec::new();

    if let Some(path) = feature_launcher::BookmarksSource::browser_bookmarks_path(
        &launcher_config.sources.bookmarks.browser,
    ) {
        if let Some(s) = find_source("bookmarks") {
            watches.push((path, s));
        }
    }
    if launcher_config.sources.ssh_hosts.enabled {
        let home = std::env::var("HOME").unwrap_or_default();
        let ssh_config = std::path::PathBuf::from(&home).join(".ssh/config");
        if let Some(s) = find_source("ssh_hosts") {
            watches.push((ssh_config, s));
        }
    }
    if launcher_config.sources.scripts.enabled {
        let scripts_dir = shellexpand::tilde(&launcher_config.sources.scripts.dir).to_string();
        if let Some(s) = find_source("scripts") {
            watches.push((std::path::PathBuf::from(&scripts_dir), s));
        }
    }
```

- [ ] **Step 4: Create SourceRegistry and spawn BackgroundRefresher**

```rust
    let registry = SourceRegistry::new(sources);
    let refresh_count = refresh_entries.len();

    // Spawn background refresher
    if !refresh_entries.is_empty() {
        let query_cache = registry.query_cache();
        let refresher = feature_launcher::BackgroundRefresher::new(
            refresh_entries,
            query_cache,
            shutdown_token.clone(),
        );
        refresher.spawn();
        info!("BackgroundRefresher started with {refresh_count} sources");
    }
```

- [ ] **Step 5: Create SourceFileWatcher**

```rust
    let file_watcher = if !watches.is_empty() {
        match feature_launcher::SourceFileWatcher::start(watches) {
            Ok(watcher) => {
                info!("SourceFileWatcher started");
                Some(watcher)
            }
            Err(e) => {
                tracing::warn!("Failed to start file watcher: {e}");
                None
            }
        }
    } else {
        None
    };
```

- [ ] **Step 6: Update engine construction**

```rust
    let engine = Arc::new(LauncherSearchEngine {
        registry,
        frequency_repo,
        clipboard_repo,
        _file_watcher: file_watcher,
    });
```

- [ ] **Step 7: Build workspace**

Run: `cargo build --workspace`
Expected: Compiles

- [ ] **Step 8: Run all tests**

Run: `cargo nextest run -p feature-launcher -p app-core`
Expected: All tests pass

- [ ] **Step 9: Commit**

```bash
git add crates/feature-launcher/ crates/app-core/
git commit -m "feat(launcher): wire BackgroundRefresher and SourceFileWatcher into init"
```

---

## Chunk 7: Verification

### Task 7.1: Full build, lint, and test verification

- [ ] **Step 1: Run full workspace build**

Run: `cargo build --workspace`
Expected: Compiles with no errors

- [ ] **Step 2: Run clippy**

Run: `cargo clippy -p feature-launcher -p app-core --all-targets`
Expected: 0 warnings from our crates

- [ ] **Step 3: Run all tests**

Run: `cargo nextest run --workspace`
Expected: All pass

- [ ] **Step 4: Run fmt check**

Run: `cargo fmt --all --check`
Expected: Clean

- [ ] **Step 5: Run frontend build**

Run: `cd desktop-ui && bun run build`
Expected: Builds (no frontend changes, but verify nothing broke)

- [ ] **Step 6: Commit any fixes**

```bash
git add .
git commit -m "fix(launcher): address lint and build issues from caching integration"
```
