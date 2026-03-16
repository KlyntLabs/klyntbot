# Launcher Search Sources — Design Spec

## Overview

Expand the launcher from app-only search to a comprehensive search system with 12+ configurable sources. Users search apps, files, bookmarks, contacts, running apps, git repos, SSH hosts, brew packages, system preferences, browser history, and file contents — all from a single input.

## Architecture

### SearchSource Trait

All sources implement a common trait, replacing the current ad-hoc struct fields:

```rust
#[async_trait]
pub trait SearchSource: Send + Sync {
    /// Unique source identifier (e.g., "apps", "files", "brew")
    fn name(&self) -> &str;

    /// Optional prefix for direct routing (e.g., '?' for grep, '@' for contacts)
    fn prefix(&self) -> Option<char> { None }

    /// Search this source. Returns scored LauncherItems.
    async fn search(&self, query: &str, limit: usize) -> Vec<LauncherItem>;

    /// Re-index this source. No-op for always-live sources.
    async fn refresh(&self) {}
}
```

### SourceRegistry

Holds all enabled sources. Replaces individual fields on `LauncherSearchEngine`.

```rust
pub struct SourceRegistry {
    sources: Vec<Arc<dyn SearchSource>>,
}

impl SourceRegistry {
    pub fn search(&self, query: &str) -> impl Future<Output = Vec<LauncherItem>> {
        // 1. Check prefixes — if match, route to that single source
        // 2. Otherwise fan out to all sources via futures::future::join_all
        //    (tokio::join! requires static arity; join_all handles Vec<Future>)
        // 3. Return merged results (unsorted — engine handles ranking)
    }
}
```

### Engine Changes

```rust
pub struct LauncherSearchEngine {
    registry: SourceRegistry,
    frequency_repo: FrequencyRepo,
    clipboard_repo: ClipboardRepo, // kept separate for Tauri command access
}
```

The engine's `search()` method:
1. Delegates to `registry.search(query)`
2. Applies frequency boosts via batch query — the `apply_frequency_boosts` match must add a default arm or explicit arms for all new `LauncherItemKind` variants, mapping each to a `kind` string for the frequency table
3. Sorts by score descending, truncates to 20
4. Appends AI chat fallback

**`clipboard_repo` ownership:** Remove `AppCore::launcher_clipboard_repo` field. The clipboard source is registered in the `SourceRegistry` like any other source. The Tauri clipboard commands (`launcher_clipboard_paste`, `launcher_clipboard_delete`, `launcher_clipboard_pin`) access the repo through `LauncherSearchEngine` which exposes a `clipboard_repo()` accessor. Calculator stays standalone — called for every query (prefix + universal), not routed through the registry.

### Query Routing

| Prefix | Source | Example |
|--------|--------|---------|
| `=` | Calculator | `=sqrt(144)` |
| `>` | System commands | `>lock` |
| `/` | Scripts | `/deploy` |
| `?` | Content grep (rg) | `?TODO fixme` |
| `@` | Contacts | `@john` |
| *(none)* | All universal sources | `safari` |

## Sources

### Tier 1 — In-Memory Indexes

Built at startup, held in `Arc<RwLock<Vec<T>>>`, searched instantly via `nucleo-matcher` fuzzy matching.

#### Apps (exists)
- **Scans:** `/Applications`, `/System/Applications`, `~/Applications`
- **Refresh:** Startup. Icon cache at `{data_dir}/cache/app-icons/` with mtime invalidation.
- **Search:** nucleo fuzzy on app name
- **Action:** `open -a <path>`

#### System Preferences (new)
- **Scans:** `/System/Library/PreferencePanes/*.prefPane`, `~/Library/PreferencePanes/`
- **pane_id extraction:** Read `CFBundleIdentifier` from each `.prefPane/Contents/Info.plist` via `PlistBuddy`. Display name from `NSPrefPaneIconLabel` or `CFBundleName`. Map bundle ID to URL scheme fragment (e.g., `com.apple.preference.security` → `x-apple.systempreferences:com.apple.preference.security`).
- **Refresh:** Startup only (~30 static items)
- **Search:** nucleo fuzzy on pane name
- **Action:** `open x-apple.systempreferences:<bundle_id>`

#### Brew Packages (new)
- **Scans:** `brew list --formula -1` + `brew list --cask -1`
- **Refresh:** Startup + manual
- **Search:** nucleo fuzzy on package name
- **Action:** Show info / open cask app
- **Graceful degradation:** Skip silently if `brew` not installed

#### SSH Hosts (new)
- **Scans:** `~/.ssh/config` — parse `Host` entries
- **Refresh:** Startup + file watch
- **Search:** Substring match on host name
- **Action:** Copy `ssh <host>` to clipboard or open terminal

#### Git Repos (new)
- **Scans:** Configurable directories (default: `~/Projects`, `~/Developer`). Walk max depth 3 for `.git/` directories.
- **Refresh:** Startup + periodic (5 min)
- **Search:** nucleo fuzzy on repo directory name
- **Action:** Open in editor or terminal

#### Scripts (exists)
- **Scans:** `{data_dir}/scripts/` for `.sh`, `.applescript`, `.scpt`
- **Refresh:** Startup
- **Search:** Substring match on name + description
- **Prefix:** `/`

### Tier 2 — Shell-Out Sources

Execute a command per search query. Always fresh, slightly slower. Debounced at 150ms.

#### Files via mdfind (new)
- **Command:** `mdfind -name "<query>" -limit 10`
- **Search:** Delegated to macOS Spotlight index
- **Result mapping:** Parse output lines as file paths. Detect file kind from extension.
- **Action:** `open <path>`
- **Timeout:** 1 second

#### Content Grep via rg (new)
- **Prefix:** `?`
- **Command:** `rg --json -m 5 "<query>" <scope>` where scope is configured (default: `.`). Note: no `-l` flag — we need per-line match data for previews.
- **Search:** On-demand ripgrep. Shows file path + matching line preview.
- **Result mapping:** Parse JSON output for file, line number, match text. Deduplicate by file (show first match per file).
- **Action:** Open file at line in editor
- **Timeout:** 2 seconds
- **Graceful degradation:** Skip silently if `rg` not installed. Fallback to `grep -rn` if available.

#### Contacts (new)
- **Prefix:** `@`
- **Method:** JXA (JavaScript for Automation) via `osascript -l JavaScript`. No `contacts` CLI exists on stock macOS. The JXA script queries `Application("Contacts").people.whose({name: {_contains: query}})` and extracts name, emails, phones.
- **Search:** Name matching via Contacts framework through JXA bridge
- **Result mapping:** Parse JSON output from JXA script (name, email, phone)
- **Action:** Copy email, open in Contacts.app, compose email
- **Graceful degradation:** If Contacts access is denied (TCC), returns empty with a one-time log suggesting the user grant access in System Settings > Privacy > Contacts.

### Tier 3 — Native API + DB-Backed

#### Running Apps (new)
- **API:** `NSWorkspace.sharedWorkspace().runningApplications` via `objc2` crate. Same FFI pattern as `crates/feature-productivity/src/tracker/macos.rs`.
- **Dependency:** `objc2` + `objc2-app-kit` added to `feature-launcher/Cargo.toml` under `[target.'cfg(target_os = "macos")'.dependencies]` (same as clipboard monitor).
- **Search:** nucleo fuzzy on app name. Per-query (fast native call, no caching needed).
- **Action:** `NSRunningApplication.activateWithOptions()` — bring to front
- **Score boost:** Running apps score higher than non-running app index matches

#### Browser Bookmarks (new)
- **Reads:** Browser-specific bookmark files:
  - Chrome/Arc/Brave/Edge: `~/Library/Application Support/<Browser>/Default/Bookmarks` (JSON)
  - Safari: `~/Library/Safari/Bookmarks.plist` (binary plist)
- **Refresh:** Startup + file watch on bookmark file
- **Search:** nucleo fuzzy on bookmark title
- **Action:** `open <url>`
- **Config:** `browser` field selects which browser to read from

#### Browser History (new)
- **Reads:** Chrome History SQLite at `~/Library/Application Support/Google/Chrome/Default/History`
- **Strategy:** Copy file to temp location (browser holds write lock), query `urls` table with `LIKE` on title + URL, limit to last N days.
- **TCC entitlement:** Reading Chrome's data directory requires Full Disk Access on macOS 10.15+. If the copy fails with `PermissionDenied`, log a one-time message: "Browser history requires Full Disk Access — grant in System Settings > Privacy > Full Disk Access". No entitlement changes needed in `tauri.conf.json` — this is a user-level system permission, not an app sandbox entitlement.
- **Refresh:** Periodic (5 min, copy + query recent)
- **Search:** Substring match on title + URL
- **Action:** `open <url>`
- **Config:** `maxDays` limits how far back to search (default: 30)
- **V1 scope:** Chrome only. The `browser` config field accepts `"chrome"` | `"arc"` | `"brave"` | `"edge"` (all Chromium, same SQLite schema, different paths). Safari uses a different schema and is deferred to v2.

#### Tasks, Notes, Clipboard (exists)
- Unchanged. SQLite queries via existing repos.

## New Types

### LauncherItemKind Additions

```rust
pub enum LauncherItemKind {
    // ... existing variants unchanged ...
    File { path: PathBuf, kind: FileKind },
    ContentMatch { path: PathBuf, line: u32, preview: String },
    Contact { name: String, email: Option<String>, phone: Option<String> },
    SystemPref { pane_id: String },
    RunningApp { pid: u32, path: PathBuf },
    Bookmark { url: String, browser: String },
    BrowserHistory { url: String, visited_at: String },
    BrewPackage { name: String, is_cask: bool },
    SshHost { host: String, user: Option<String> },
    GitRepo { path: PathBuf },
}

pub enum FileKind {
    File, Folder, Image, Document, Code, Archive,
}
```

### Frontend Types

Add corresponding TypeScript variants to `LauncherItemKind` union in `desktop-ui/src/features/launcher/types.ts`.

## Config Schema

Located in `crates/config/src/schema/launcher.rs`. Type-safe struct with serde defaults.

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LauncherConfig {
    #[serde(default = "default_true")]
    pub enabled: bool,
    #[serde(default)]
    pub sources: LauncherSourcesConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase", default)]
pub struct LauncherSourcesConfig {
    pub apps: SourceToggle,
    pub system_prefs: SourceToggle,
    pub brew: SourceToggle,
    pub ssh_hosts: SourceToggle,
    pub git_repos: GitReposConfig,
    pub scripts: ScriptsConfig,
    pub files: SourceToggle,
    pub content_grep: ContentGrepConfig,
    pub contacts: SourceToggle,
    pub running_apps: SourceToggle,
    pub bookmarks: BrowserSourceConfig,
    pub browser_history: BrowserHistoryConfig,
    pub tasks: SourceToggle,
    pub notes: SourceToggle,
    pub clipboard: ClipboardConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SourceToggle {
    #[serde(default = "default_true")]
    pub enabled: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GitReposConfig {
    #[serde(default = "default_true")]
    pub enabled: bool,
    #[serde(default = "default_scan_dirs")]
    pub scan_dirs: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BrowserSourceConfig {
    #[serde(default = "default_true")]
    pub enabled: bool,
    #[serde(default = "default_chrome")]
    pub browser: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BrowserHistoryConfig {
    #[serde(default = "default_true")]
    pub enabled: bool,
    #[serde(default = "default_chrome")]
    pub browser: String,
    #[serde(default = "default_30")]
    pub max_days: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ContentGrepConfig {
    #[serde(default = "default_true")]
    pub enabled: bool,
    #[serde(default = "default_dot")]
    pub default_scope: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ClipboardConfig {
    #[serde(default = "default_true")]
    pub enabled: bool,
    #[serde(default = "default_1000")]
    pub max_entries: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ScriptsConfig {
    #[serde(default = "default_true")]
    pub enabled: bool,
    #[serde(default = "default_scripts_dir")]
    pub dir: String,
}
```

JSON example in `config.json`:
```json
{
  "launcher": {
    "enabled": true,
    "sources": {
      "apps": { "enabled": true },
      "systemPrefs": { "enabled": true },
      "brew": { "enabled": true },
      "sshHosts": { "enabled": true },
      "gitRepos": { "enabled": true, "scanDirs": ["~/Projects", "~/Developer"] },
      "scripts": { "enabled": true, "dir": "~/.klyntbot/scripts" },
      "files": { "enabled": true },
      "contentGrep": { "enabled": true, "defaultScope": "." },
      "contacts": { "enabled": true },
      "runningApps": { "enabled": true },
      "bookmarks": { "enabled": true, "browser": "chrome" },
      "browserHistory": { "enabled": true, "browser": "chrome", "maxDays": 30 },
      "tasks": { "enabled": true },
      "notes": { "enabled": true },
      "clipboard": { "enabled": true, "maxEntries": 1000 }
    }
  }
}
```

## File Structure

```
crates/feature-launcher/src/
├── search/
│   ├── mod.rs              — SearchSource trait + SourceRegistry
│   ├── app_index.rs        — refactor: add SearchSource impl (wraps sync search in async)
│   ├── calculator.rs       — stays standalone (called in both prefix + universal, not in registry)
│   ├── system_commands.rs  — refactor to impl SearchSource
│   ├── script_runner.rs    — refactor to impl SearchSource
│   ├── file_search.rs      — NEW: mdfind wrapper
│   ├── content_grep.rs     — NEW: rg wrapper (prefix ?)
│   ├── contacts.rs         — NEW: contacts CLI wrapper (prefix @)
│   ├── system_prefs.rs     — NEW: prefpane scanner
│   ├── running_apps.rs     — NEW: NSWorkspace query
│   ├── bookmarks.rs        — NEW: Chrome/Arc/Safari bookmark reader
│   ├── browser_history.rs  — NEW: Chrome History SQLite reader
│   ├── brew.rs             — NEW: brew list parser
│   ├── ssh_hosts.rs        — NEW: ~/.ssh/config parser
│   └── git_repos.rs        — NEW: .git/ directory scanner
├── repos/                  — unchanged
├── clipboard/              — unchanged
├── window_mgmt/            — unchanged
├── types.rs                — add new LauncherItemKind variants + FileKind
└── lib.rs                  — add LauncherConfig, update FeaturePackage

crates/config/src/schema/
└── launcher.rs             — NEW: LauncherConfig + source sub-configs

crates/app-core/src/
├── handlers/launcher/
│   └── search_engine.rs    — refactor to use SourceRegistry
└── init/
    └── launcher.rs         — read config, register enabled sources
```

## Score Ranges

Consistent scoring across all sources for predictable ranking:

| Source | Base Score | Notes |
|--------|-----------|-------|
| Calculator | 2.0 | Highest — exact math result |
| Running Apps | 1.2 | Active context, likely what user wants |
| Apps (nucleo) | 0.5–1.0 | Normalized from nucleo score |
| System Commands | 0.7–1.0 | Keyword vs prefix match |
| Files (mdfind) | 0.8 | High intent — user searching for files |
| Git Repos | 0.8 | Developer context |
| Content Grep | 0.7 | Specific content match |
| Bookmarks | 0.7 | Saved = intentional |
| Scripts | 0.7 | User-created automation |
| Tasks | 0.7–0.9 | Higher for "doing" status |
| SSH Hosts | 0.7 | Exact match patterns |
| System Prefs | 0.6 | Utility |
| Notes | 0.6 | Content search |
| Contacts | 0.6 | People lookup |
| Browser History | 0.5 | Recency-weighted |
| Clipboard | 0.5 | Recent context |
| Brew Packages | 0.4 | Rarely searched |
| AI Chat | 0.0 | Always-last fallback |

Frequency boosts add `log2(count + 1) * 0.1` on top of base scores.

## Graceful Degradation

Sources that depend on external tools handle missing dependencies gracefully:
- **brew not installed:** `BrewSource` returns empty on `refresh()`, logs info once
- **rg not installed:** `ContentGrepSource` returns empty, logs suggestion to install
- **Contacts permission denied:** Returns empty, logs permission hint
- **Browser not installed:** Bookmark/history source detects missing path, returns empty
- **Non-macOS:** All native sources (running apps, contacts, mdfind) return empty via `#[cfg]` stubs

No source failure should prevent the launcher from working. Each source is independently failable.

## Debouncing

Shell-out sources (mdfind, rg, contacts) are debounced at 150ms in the frontend — the `useLauncherSearch` hook already has debouncing. No backend debouncing needed.

Shell-out commands have per-source timeouts:
- mdfind: 1s
- rg: 2s
- contacts: 1s

If a source times out, its results are simply omitted from that search cycle.

**Cancellation:** When a new query arrives while shell-out sources are still running, the previous `tokio::process::Command` children should be killed via `Child::kill()`. The `SourceRegistry::search()` method accepts a `CancellationToken` and passes it to each source. Shell-out sources check the token before spawning and abort early if cancelled.

## Non-Goals

- **Full-text file indexing** — we use mdfind (Spotlight) and rg instead of building our own
- **Cross-platform support** — macOS-only sources get `#[cfg]` stubs, not alternative implementations
- **Real-time file watching for mdfind** — Spotlight handles its own index updates
- **Bookmark sync across browsers** — one browser at a time per config
