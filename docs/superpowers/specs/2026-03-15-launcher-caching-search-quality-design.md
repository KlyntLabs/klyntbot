# Launcher Caching & Search Quality Design

**Goal:** Make launcher search feel instant by pre-indexing everything into memory, refreshing in the background, caching live subprocess results, and normalizing all sources to nucleo fuzzy matching for consistent scoring.

**Principle:** Never spawn a subprocess in the search hot path. Every search should be a pure in-memory fuzzy match against pre-built indices.

---

## 1. BackgroundRefresher

A single background task that owns refresh scheduling for all cached sources.

**Location:** `crates/feature-launcher/src/search/background.rs`

```rust
pub struct BackgroundRefresher {
    entries: Vec<RefreshEntry>,
    query_cache: Arc<DashMap<(&'static str, String), CachedResult>>,
    shutdown: CancellationToken,
}

struct RefreshEntry {
    source: Arc<dyn SearchSource>,
    interval: Duration,
    last_refreshed: Instant,
}
```

**Behavior:**
- Spawned at init via `tokio::spawn`, shutdown via `CancellationToken`
- Main loop ticks every 1s, checks which sources are due based on `last_refreshed + interval`
- Dispatches blocking refreshes (brew, git repos) via `tokio::task::spawn_blocking` to avoid stalling the async runtime. Async-safe refreshes (running apps, contacts, browser history) are called directly.
- Also evicts expired query cache entries every 60s
- `last_refreshed` is initialized to `Instant::now() - interval` so every source gets an immediate first refresh on startup

**Refresh intervals:**

| Source | Interval | Rationale |
|---|---|---|
| Running apps | 3s | Active context changes frequently |
| Contacts | 30s | Changes rarely |
| Browser history | 2min | Moderate change frequency |
| Brew packages | 5min | Only changes on install/uninstall |
| Git repos | 5min | New repos are rare |

Sources with file watchers (bookmarks, SSH, scripts) are NOT in the refresher.
Sources that are startup-only (apps, system prefs) or stateless (calculator, system commands) are excluded.

### Converting RunningAppsSource and ContactsSource to Pre-Loaded Index

Both `RunningAppsSource` and `ContactsSource` currently spawn subprocesses inside `search()`. This violates the "never subprocess in the hot path" principle. Both must be refactored to the pre-loaded index model:

**RunningAppsSource:** Add `apps: Arc<RwLock<Vec<(String, u32, PathBuf)>>>` field. The `refresh()` method calls osascript to populate the list. The `search()` method does a pure in-memory nucleo fuzzy match against the cached list. Refresh interval: 3s.

**ContactsSource:** Add `contacts: Arc<RwLock<Vec<ContactEntry>>>` field. The `refresh()` method calls osascript/JXA to load all contacts (no query filter — fetch all, limit to ~500). The `search()` method does a pure in-memory nucleo fuzzy match against the cached list. Refresh interval: 30s. The `@` prefix is retained — it still routes to this source, but the source searches its in-memory cache instead of spawning osascript.

This means the query cache (Section 4) is only needed for `FileSearchSource` (mdfind) and `ContentGrepSource` (rg) — the two sources that genuinely can't pre-index.

---

## 2. SourceFileWatcher

Event-driven refresh for sources backed by specific files that change infrequently.

**Location:** `crates/feature-launcher/src/search/file_watcher.rs`

```rust
pub struct SourceFileWatcher {
    _debouncer: notify_debouncer_mini::Debouncer<notify::RecommendedWatcher>,
}
```

**Watched paths:**

| Path | Source | Trigger |
|---|---|---|
| Browser bookmarks file (resolved dynamically from config) | BookmarksSource | `refresh()` |
| `~/.ssh/config` | SshHostsSource | `refresh()` |
| Scripts dir (from config) | ScriptRunner | re-discover + `set_scripts()` |

The bookmarks path is resolved at watcher setup time by calling `BookmarksSource::browser_bookmarks_path(&config.browser)`. This correctly handles Chrome, Arc, Brave, and Edge paths based on user config.

**Behavior:**
- Uses `notify-debouncer-mini` with 500ms debounce
- Maps changed path → source via `HashMap<PathBuf, Arc<dyn SearchSource>>`
- Spawned during `init_launcher()`
- Non-existent paths at startup are silently skipped (no error, logged at debug level)
- Permission errors logged once, then ignored

**Shutdown:** The `notify-debouncer-mini` debouncer runs an OS-level watcher thread, not a tokio task. Shutdown is achieved by dropping the `SourceFileWatcher` struct (which drops the `Debouncer`, joining the watcher thread). The `SourceFileWatcher` must be stored in a field on `LauncherSearchEngine` or `AppCore` so it is dropped during app shutdown — `CancellationToken` alone does not stop the OS thread.

---

## 3. Unified Nucleo Fuzzy Scoring

Replace all substring matching with nucleo fuzzy for consistent cross-source ranking.

**Sources that need algorithm changes:**

| Source | Current algorithm | New algorithm |
|---|---|---|
| SSH hosts | `contains()` on host + hostname | Nucleo on host, bonus if hostname matches |
| Browser history | `contains()` on title + url | Nucleo on title, bonus if url matches |
| Scripts | `contains()` on name + description | Nucleo on name, bonus if description matches |
| System commands | `contains()` on title + keywords | Nucleo on title, keywords as tiebreaker |

**Sources that already use nucleo** (only need weight update): Apps, bookmarks, brew, git repos, system prefs, running apps. These already use the `(score as f64) / 1000.0 * weight` formula — the change is updating the weight constant only.

**Score normalization formula:** `(nucleo_score as f64) / 1000.0 * source_weight`

All sources use the same formula. Source weights by priority tier:

| Tier | Weight | Sources |
|---|---|---|
| Boosted | 1.2 | Running apps |
| High | 1.0 | Apps, system commands |
| Medium | 0.8 | Files, git repos, bookmarks |
| Normal | 0.6 | SSH, contacts, system prefs, scripts |
| Low | 0.4 | Brew, browser history, clipboard |

Calculator remains fixed at `2.0` (always ranks highest when matched). Frequency boosts from `apply_frequency_boosts` still apply additively on top.

**Note:** Browser history results currently use a flat `score: 0.5` with no per-result ranking. After migrating to nucleo, results will be ranked by fuzzy match quality, which changes visible result order (better matches surface first). This is an intentional improvement.

---

## 4. Query Cache for Live Sources

TTL-based result cache in `SourceRegistry` for the two sources that genuinely can't pre-index: `FileSearchSource` (mdfind) and `ContentGrepSource` (rg).

**Location:** Cache state lives in `SourceRegistry`, keyed by `(source_name, query)`.

```rust
struct CachedResult {
    results: Vec<LauncherItem>,
    created_at: Instant,
}

// Inside SourceRegistry:
pub struct SourceRegistry {
    sources: Vec<Arc<dyn SearchSource>>,
    query_cache: Arc<DashMap<(&'static str, String), CachedResult>>,
}
```

Cache key uses `&'static str` for source name (all `SearchSource::name()` impls return `&'static str`) to avoid unnecessary string cloning on every lookup.

**New trait method on SearchSource:**

```rust
fn cache_ttl(&self) -> Option<Duration> { None }
```

Sources that return `Some(duration)` get their results cached. In-memory sources return `None` (default).

**Cache TTLs per source:**

| Source | TTL | Rationale |
|---|---|---|
| FileSearchSource (mdfind) | 5s | Filesystem changes infrequently during typing |
| ContentGrepSource (rg) | 5s | Same |

**Cache behavior:**
- Before calling `source.search()`, check cache for `(source.name(), query)`
- Hit + not expired → return cached results
- Miss or expired → call `source.search()`, store result, return
- Max 200 entries; on overflow, do an O(n) scan to evict the oldest entry by `created_at`. This is acceptable: n ≤ 200, the scan is rare (only at capacity), and adding a dependency like `moka` for a 200-element cache is not justified.
- Lazy eviction of expired entries every 60s (done by `BackgroundRefresher`)
- Cache is shared via `Arc<DashMap>` between `SourceRegistry` and `BackgroundRefresher`

**Memory bounds:** ~1MB max worst case (200 entries, some with long URLs/paths). Negligible.

---

## Integration Points

**init_launcher.rs changes:**
1. Create `Arc<DashMap>` for query cache, pass to both `SourceRegistry` and `BackgroundRefresher`
2. Spawn `BackgroundRefresher::start()` with the appropriate sources and intervals
3. Create `SourceFileWatcher::start()` with bookmarks/SSH/scripts paths mapped to their sources
4. Store the `SourceFileWatcher` in `LauncherSearchEngine` (so it's dropped on shutdown)
5. `BackgroundRefresher` uses `shutdown_token` from `AppCore` for clean shutdown

**SearchSource trait changes:**
- Add `fn cache_ttl(&self) -> Option<Duration> { None }` with default impl (non-breaking)

**SourceRegistry changes:**
- Add `query_cache: Arc<DashMap<...>>` field
- `search()` method checks cache before calling sources with a `cache_ttl`
- Constructor takes the shared cache

**RunningAppsSource changes:**
- Add `apps: Arc<RwLock<Vec<...>>>` field for pre-loaded index
- `refresh()` populates via osascript
- `search()` does in-memory nucleo fuzzy match only

**ContactsSource changes:**
- Add `contacts: Arc<RwLock<Vec<ContactEntry>>>` field
- `refresh()` loads all contacts via JXA (no query filter, limit ~500)
- `search()` does in-memory nucleo fuzzy match only

**No frontend changes needed.** The caching and refresh are entirely backend — the existing 100ms debounce + search API contract remain unchanged.

---

## Files to Create

| File | Purpose |
|---|---|
| `crates/feature-launcher/src/search/background.rs` | BackgroundRefresher |
| `crates/feature-launcher/src/search/file_watcher.rs` | SourceFileWatcher |

## Files to Modify

| File | Changes |
|---|---|
| `crates/feature-launcher/src/search/mod.rs` | Add modules, re-exports, `cache_ttl()` to trait, cache in registry |
| `crates/feature-launcher/src/search/ssh_hosts.rs` | Nucleo fuzzy search |
| `crates/feature-launcher/src/search/browser_history.rs` | Nucleo fuzzy search |
| `crates/feature-launcher/src/search/script_runner.rs` | Nucleo fuzzy search |
| `crates/feature-launcher/src/search/system_commands.rs` | Nucleo fuzzy search |
| `crates/feature-launcher/src/search/running_apps.rs` | Convert to pre-loaded index, nucleo search, normalize score |
| `crates/feature-launcher/src/search/contacts.rs` | Convert to pre-loaded index, nucleo search, normalize score |
| `crates/feature-launcher/src/search/file_search.rs` | Add `cache_ttl()`, normalize score |
| `crates/feature-launcher/src/search/content_grep.rs` | Add `cache_ttl()`, normalize score |
| `crates/feature-launcher/src/search/app_index.rs` | Normalize score weight (already nucleo) |
| `crates/feature-launcher/src/search/bookmarks.rs` | Normalize score weight (already nucleo) |
| `crates/feature-launcher/src/search/brew.rs` | Normalize score weight (already nucleo) |
| `crates/feature-launcher/src/search/git_repos.rs` | Normalize score weight (already nucleo) |
| `crates/feature-launcher/src/search/system_prefs.rs` | Normalize score weight (already nucleo) |
| `crates/feature-launcher/Cargo.toml` | Add `dashmap`, `notify`, `notify-debouncer-mini` deps |
| `crates/app-core/src/init/launcher.rs` | Spawn refresher + file watcher, store watcher, pass shared cache |
| `crates/feature-launcher/src/lib.rs` | Re-export new types |

## Dependencies to Add

| Crate | Where | Why |
|---|---|---|
| `dashmap.workspace = true` | `feature-launcher/Cargo.toml` | Concurrent query cache |
| `notify.workspace = true` | `feature-launcher/Cargo.toml` | File watching (OS-level FSEvents) |
| `notify-debouncer-mini.workspace = true` | `feature-launcher/Cargo.toml` | Debounced file events |
