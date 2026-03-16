# Launcher Search Sources Implementation Plan

> **For agentic workers:** REQUIRED: Use superpowers:subagent-driven-development (if subagents available) or superpowers:executing-plans to implement this plan. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Expand the launcher to search 12+ configurable sources (apps, files, bookmarks, contacts, git repos, brew packages, SSH hosts, system prefs, browser history, running apps, content grep) through a unified `SearchSource` trait and `SourceRegistry`.

**Architecture:** Each source implements the `SearchSource` trait. A `SourceRegistry` holds only enabled sources (configured via `LauncherConfig` in `config.json`). The engine fans out queries via `futures::future::join_all`, applies frequency boosts, ranks, and returns top 20 results. Shell-out sources (mdfind, rg, contacts) support cancellation via `CancellationToken`.

**Tech Stack:** Rust (MSRV 1.75), nucleo-matcher (fuzzy), objc2 (macOS native), tokio (async), serde (config), sqlx (browser history), futures (join_all)

---

## Chunk 1: Foundation — Trait, Registry, Config, Types

### Task 1.1: Add new LauncherItemKind variants and FileKind enum

**Files:**
- Modify: `crates/feature-launcher/src/types.rs`

- [ ] **Step 1: Add FileKind enum**

Add above `LauncherItemKind`:

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum FileKind {
    File,
    Folder,
    Image,
    Document,
    Code,
    Archive,
}
```

- [ ] **Step 2: Add new variants to LauncherItemKind**

Add these variants to the existing `LauncherItemKind` enum (after `AiChat`):

```rust
    File {
        path: PathBuf,
        kind: FileKind,
    },
    ContentMatch {
        path: PathBuf,
        line: u32,
        preview: String,
    },
    Contact {
        name: String,
        email: Option<String>,
        phone: Option<String>,
    },
    SystemPref {
        pane_id: String,
    },
    RunningApp {
        pid: u32,
        path: PathBuf,
    },
    Bookmark {
        url: String,
        browser: String,
    },
    BrowserHistory {
        url: String,
        visited_at: String,
    },
    BrewPackage {
        name: String,
        is_cask: bool,
    },
    SshHost {
        host: String,
        user: Option<String>,
    },
    GitRepo {
        path: PathBuf,
    },
```

- [ ] **Step 3: Build to verify**

Run: `cargo build -p feature-launcher`
Expected: Compiles (new variants are additive). There will be non-exhaustive match warnings in `app-core` — that's expected and fixed in Task 1.5.

- [ ] **Step 4: Commit**

```bash
git add crates/feature-launcher/src/types.rs
git commit -m "feat(launcher): add LauncherItemKind variants for all new search sources"
```

### Task 1.2: Create LauncherConfig in config crate

**Files:**
- Create: `crates/config/src/schema/launcher.rs`
- Modify: `crates/config/src/schema/mod.rs`
- Modify: `crates/config/src/schema/core.rs`

- [ ] **Step 1: Create launcher.rs config module**

```rust
use serde::{Deserialize, Serialize};

fn default_true() -> bool {
    true
}
fn default_scan_dirs() -> Vec<String> {
    vec!["~/Projects".to_string(), "~/Developer".to_string()]
}
fn default_chrome() -> String {
    "chrome".to_string()
}
fn default_30() -> i64 {
    30
}
fn default_dot() -> String {
    ".".to_string()
}
fn default_1000() -> i64 {
    1000
}
fn default_scripts_dir() -> String {
    "~/.klyntbot/scripts".to_string()
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct LauncherConfig {
    pub enabled: bool,
    pub sources: LauncherSourcesConfig,
}

impl Default for LauncherConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            sources: LauncherSourcesConfig::default(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
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
    pub clipboard: ClipboardSourceConfig,
}

impl Default for LauncherSourcesConfig {
    fn default() -> Self {
        Self {
            apps: SourceToggle::default(),
            system_prefs: SourceToggle::default(),
            brew: SourceToggle::default(),
            ssh_hosts: SourceToggle::default(),
            git_repos: GitReposConfig::default(),
            scripts: ScriptsConfig::default(),
            files: SourceToggle::default(),
            content_grep: ContentGrepConfig::default(),
            contacts: SourceToggle::default(),
            running_apps: SourceToggle::default(),
            bookmarks: BrowserSourceConfig::default(),
            browser_history: BrowserHistoryConfig::default(),
            tasks: SourceToggle::default(),
            notes: SourceToggle::default(),
            clipboard: ClipboardSourceConfig::default(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct SourceToggle {
    pub enabled: bool,
}

impl Default for SourceToggle {
    fn default() -> Self {
        Self { enabled: true }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct GitReposConfig {
    pub enabled: bool,
    #[serde(default = "default_scan_dirs")]
    pub scan_dirs: Vec<String>,
}

impl Default for GitReposConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            scan_dirs: default_scan_dirs(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct ScriptsConfig {
    pub enabled: bool,
    #[serde(default = "default_scripts_dir")]
    pub dir: String,
}

impl Default for ScriptsConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            dir: default_scripts_dir(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct BrowserSourceConfig {
    pub enabled: bool,
    #[serde(default = "default_chrome")]
    pub browser: String,
}

impl Default for BrowserSourceConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            browser: default_chrome(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct BrowserHistoryConfig {
    pub enabled: bool,
    #[serde(default = "default_chrome")]
    pub browser: String,
    #[serde(default = "default_30")]
    pub max_days: i64,
}

impl Default for BrowserHistoryConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            browser: default_chrome(),
            max_days: 30,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct ContentGrepConfig {
    pub enabled: bool,
    #[serde(default = "default_dot")]
    pub default_scope: String,
}

impl Default for ContentGrepConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            default_scope: default_dot(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct ClipboardSourceConfig {
    pub enabled: bool,
    #[serde(default = "default_1000")]
    pub max_entries: i64,
}

impl Default for ClipboardSourceConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            max_entries: 1000,
        }
    }
}
```

- [ ] **Step 2: Register in config/schema/mod.rs**

Add `mod launcher;` and `pub use launcher::*;` following the existing pattern.

- [ ] **Step 3: Add launcher field to Config struct in core.rs**

Add to the `Config` struct:

```rust
    #[serde(default)]
    pub launcher: LauncherConfig,
```

- [ ] **Step 4: Build to verify**

Run: `cargo build -p config`
Expected: Compiles

- [ ] **Step 5: Commit**

```bash
git add crates/config/src/schema/
git commit -m "feat(config): add LauncherConfig with per-source toggles"
```

### Task 1.3: Create SearchSource trait and SourceRegistry

**Files:**
- Modify: `crates/feature-launcher/src/search/mod.rs`
- Modify: `crates/feature-launcher/Cargo.toml`

- [ ] **Step 1: Add futures-util dependency**

Add to `crates/feature-launcher/Cargo.toml` under `[dependencies]`:
```toml
futures-util.workspace = true
```

- [ ] **Step 2: Define SearchSource trait and SourceRegistry in search/mod.rs**

Replace the contents of `search/mod.rs` with:

```rust
pub mod app_index;
pub mod calculator;
pub mod script_runner;
pub mod system_commands;

pub use app_index::{AppEntry, AppIndex};
pub use calculator::Calculator;
pub use script_runner::ScriptRunner;
pub use system_commands::SystemCommands;

use crate::types::LauncherItem;
use async_trait::async_trait;
use std::sync::Arc;

/// Trait that all launcher search sources implement.
#[async_trait]
pub trait SearchSource: Send + Sync {
    /// Unique source identifier (e.g., "apps", "files", "brew").
    fn name(&self) -> &str;

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
}

/// Registry of enabled search sources. Handles prefix routing and fan-out.
pub struct SourceRegistry {
    sources: Vec<Arc<dyn SearchSource>>,
}

impl SourceRegistry {
    pub fn new(sources: Vec<Arc<dyn SearchSource>>) -> Self {
        Self { sources }
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
                return source.search(inner_query.trim(), limit).await;
            }
        }

        // No prefix match — fan out to all non-prefix sources
        let futures: Vec<_> = self
            .sources
            .iter()
            .filter(|s| s.prefix().is_none())
            .map(|s| s.search(query, limit))
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
```

- [ ] **Step 3: Build to verify**

Run: `cargo build -p feature-launcher`
Expected: Compiles

- [ ] **Step 4: Commit**

```bash
git add crates/feature-launcher/
git commit -m "feat(launcher): add SearchSource trait and SourceRegistry"
```

### Task 1.4: Refactor existing sources to implement SearchSource

**Files:**
- Modify: `crates/feature-launcher/src/search/app_index.rs`
- Modify: `crates/feature-launcher/src/search/system_commands.rs`
- Modify: `crates/feature-launcher/src/search/script_runner.rs`

- [ ] **Step 1: Add SearchSource impl for AppIndex**

Add at the bottom of `app_index.rs` (before `#[cfg(test)]`):

```rust
#[async_trait::async_trait]
impl super::SearchSource for AppIndex {
    fn name(&self) -> &str {
        "apps"
    }

    async fn search(&self, query: &str, limit: usize) -> Vec<LauncherItem> {
        // Delegate to existing sync search
        self.search(query, limit)
    }

    async fn refresh(&self) {
        self.index_applications().await;
    }
}
```

Note: This wraps the sync `search()` in an async fn. The sync method is fast (in-memory fuzzy match) so no blocking concern.

- [ ] **Step 2: Add SearchSource impl for SystemCommands**

Refactor `SystemCommands` into a struct instance (currently it's a zero-sized type with static methods). Add at the bottom of `system_commands.rs`:

```rust
#[async_trait::async_trait]
impl super::SearchSource for SystemCommands {
    fn name(&self) -> &str {
        "system_commands"
    }

    fn prefix(&self) -> Option<char> {
        Some('>')
    }

    async fn search(&self, query: &str, limit: usize) -> Vec<LauncherItem> {
        let mut results = Self::search(query);
        results.truncate(limit);
        results
    }
}
```

Note: `SystemCommands::search` is an inherent method, the trait method calls it. There's a name conflict — rename the inherent method to `search_all` if needed, or qualify with `Self::search`.

**IMPORTANT:** The inherent method `SystemCommands::search(query)` and the trait method `SearchSource::search(&self, query, limit)` have different signatures, so they don't conflict. The trait impl calls the inherent static method.

- [ ] **Step 3: Add SearchSource impl for ScriptRunner**

Add at the bottom of `script_runner.rs`:

```rust
#[async_trait::async_trait]
impl super::SearchSource for ScriptRunner {
    fn name(&self) -> &str {
        "scripts"
    }

    fn prefix(&self) -> Option<char> {
        Some('/')
    }

    async fn search(&self, query: &str, limit: usize) -> Vec<LauncherItem> {
        self.search(query, limit)
    }

    async fn refresh(&self) {
        // Scripts are discovered at init time, no dynamic refresh
    }
}
```

- [ ] **Step 4: Build and run existing tests**

Run: `cargo build -p feature-launcher && cargo nextest run -p feature-launcher`
Expected: All existing tests still pass

- [ ] **Step 5: Commit**

```bash
git add crates/feature-launcher/src/search/
git commit -m "feat(launcher): impl SearchSource for AppIndex, SystemCommands, ScriptRunner"
```

### Task 1.5: Refactor LauncherSearchEngine to use SourceRegistry

**Files:**
- Modify: `crates/app-core/src/handlers/launcher/search_engine.rs`
- Modify: `crates/app-core/src/handlers/launcher/handlers.rs`
- Modify: `crates/app-core/src/init/launcher.rs`
- Modify: `crates/app-core/src/state.rs`
- Modify: `crates/app-core/Cargo.toml`

- [ ] **Step 1: Add config dependency to app-core**

In `crates/app-core/Cargo.toml`, verify `config.workspace = true` is present (it should be already).

- [ ] **Step 2: Refactor LauncherSearchEngine struct**

Replace the struct definition in `search_engine.rs`:

```rust
use feature_launcher::{
    Calculator, ClipboardRepo, FrequencyRepo, LauncherItem, LauncherItemKind, SourceRegistry,
};

pub struct LauncherSearchEngine {
    pub registry: SourceRegistry,
    pub frequency_repo: FrequencyRepo,
    pub clipboard_repo: ClipboardRepo,
}
```

- [ ] **Step 3: Refactor the search method**

Replace the `search()` method body:

```rust
    pub async fn search(
        &self,
        query: &str,
        repos: &Repos,
        note_repo: &NoteRepo,
    ) -> Result<Vec<LauncherItem>, ApiError> {
        let query = query.trim();
        if query.is_empty() {
            return Ok(vec![]);
        }

        // Calculator handles both prefix (=) and universal
        let calc_results = Calculator::try_eval(query)
            .map(|r| vec![LauncherItem {
                id: format!("calc:{}", r.expression),
                title: format!("{}", r.result),
                subtitle: Some(r.expression.clone()),
                icon: Some("calculator".to_string()),
                kind: LauncherItemKind::Calculator {
                    expression: r.expression,
                    result: r.result,
                },
                score: 2.0,
            }])
            .unwrap_or_default();

        // Registry handles prefix routing + fan-out
        let mut results = self.registry.search(query, 10).await;

        // Add DB-backed sources (tasks, notes) — these aren't in registry
        // because they need external repos
        let (tasks, notes) = tokio::join!(
            self.search_tasks(query, repos),
            self.search_notes(query, note_repo),
        );
        results.extend(tasks.unwrap_or_default());
        results.extend(notes.unwrap_or_default());

        // Add calculator results
        results.extend(calc_results);

        // Apply frequency boosts
        self.apply_frequency_boosts(&mut results).await;

        // Sort by score descending
        results.sort_by(|a, b| {
            b.score
                .partial_cmp(&a.score)
                .unwrap_or(std::cmp::Ordering::Equal)
        });
        results.truncate(20);

        // Always add AI chat fallback at the end
        results.push(LauncherItem {
            id: format!("ai:{}", query),
            title: format!("Ask AI: {}", query),
            subtitle: Some("Chat with your AI assistant".to_string()),
            icon: Some("message-circle".to_string()),
            kind: LauncherItemKind::AiChat {
                query: query.to_string(),
            },
            score: 0.0,
        });

        Ok(results)
    }
```

Keep `search_tasks`, `search_notes`, `record_execution` methods as-is. Remove `search_apps`, `search_clipboard`, `search_calculator` methods (these are now handled by the registry or inline).

- [ ] **Step 4: Update apply_frequency_boosts with default arm**

Replace the match in `apply_frequency_boosts`:

```rust
            let kind_str = match &item.kind {
                LauncherItemKind::Application { .. } => "app",
                LauncherItemKind::Task { .. } => "task",
                LauncherItemKind::Note { .. } => "note",
                LauncherItemKind::ClipboardEntry { .. } => "clip",
                LauncherItemKind::SystemCommand { .. } => "system",
                LauncherItemKind::Script { .. } => "script",
                LauncherItemKind::Calculator { .. } | LauncherItemKind::AiChat { .. } => {
                    return None
                }
                LauncherItemKind::Calendar { .. } => "calendar",
                LauncherItemKind::File { .. } => "file",
                LauncherItemKind::ContentMatch { .. } => "grep",
                LauncherItemKind::Contact { .. } => "contact",
                LauncherItemKind::SystemPref { .. } => "pref",
                LauncherItemKind::RunningApp { .. } => "running_app",
                LauncherItemKind::Bookmark { .. } => "bookmark",
                LauncherItemKind::BrowserHistory { .. } => "history",
                LauncherItemKind::BrewPackage { .. } => "brew",
                LauncherItemKind::SshHost { .. } => "ssh",
                LauncherItemKind::GitRepo { .. } => "repo",
            };
```

- [ ] **Step 5: Update init/launcher.rs**

Refactor `init_launcher()` to build a `SourceRegistry`:

```rust
use feature_launcher::{AppIndex, ClipboardRepo, FrequencyRepo, ScriptRunner, SourceRegistry};

pub(super) struct LauncherResult {
    pub launcher_engine: Option<Arc<LauncherSearchEngine>>,
}

pub(super) async fn init_launcher(
    config: &config::Config,
    storage_pool: &StoragePool,
) -> LauncherResult {
    let pool = storage_pool.inner().clone();

    if let Err(e) = StoragePool::run_feature_migrations(
        &pool,
        &feature_launcher::LauncherFeature::migrations_static(),
    )
    .await
    {
        error!("launcher migration failed — feature disabled: {e}");
        return LauncherResult {
            launcher_engine: None,
        };
    }

    let launcher_config = &config.launcher;
    let frequency_repo = FrequencyRepo::new(pool.clone());
    let clipboard_repo = ClipboardRepo::new(pool);

    let mut sources: Vec<Arc<dyn feature_launcher::SearchSource>> = Vec::new();

    // Apps source
    if launcher_config.sources.apps.enabled {
        let icon_cache_dir = config.data_dir_path().join("cache").join("app-icons");
        let app_index = Arc::new(AppIndex::with_cache_dir(icon_cache_dir));
        let idx = Arc::clone(&app_index);
        tokio::spawn(async move { idx.index_applications().await });
        sources.push(app_index);
    }

    // Scripts source
    if launcher_config.sources.scripts.enabled {
        let scripts_dir = shellexpand::tilde(&launcher_config.sources.scripts.dir).to_string();
        let scripts_path = std::path::Path::new(&scripts_dir);
        let script_runner = Arc::new(ScriptRunner::new());
        if scripts_path.exists() {
            let scripts = ScriptRunner::discover(scripts_path);
            info!("discovered {} launcher scripts", scripts.len());
            script_runner.set_scripts(scripts);
        }
        sources.push(script_runner);
    }

    // System commands (always enabled — no config toggle, it's lightweight)
    sources.push(Arc::new(feature_launcher::SystemCommands));

    // Clipboard source
    if launcher_config.sources.clipboard.enabled {
        sources.push(Arc::new(clipboard_repo.clone()));
    }

    let registry = SourceRegistry::new(sources);

    let engine = Arc::new(LauncherSearchEngine {
        registry,
        frequency_repo,
        clipboard_repo,
    });

    info!("launcher feature initialized");
    LauncherResult {
        launcher_engine: Some(engine),
    }
}
```

- [ ] **Step 6: Update state.rs — remove launcher_clipboard_repo field**

Remove the `launcher_clipboard_repo` field from `AppCore`. Update `launcher_clipboard_repo()` accessor to go through the engine:

```rust
    pub fn launcher_clipboard_repo(&self) -> Result<&feature_launcher::ClipboardRepo, ApiError> {
        self.launcher_engine
            .as_ref()
            .map(|e| &e.clipboard_repo)
            .ok_or_else(|| ApiError::new("FEATURE_DISABLED", "launcher feature is not enabled"))
    }
```

Update `init/mod.rs` to remove the `launcher_clipboard_repo` field from the AppCore construction.

- [ ] **Step 7: Add SearchSource impl for ClipboardRepo**

In `crates/feature-launcher/src/repos/clipboard.rs`, add:

```rust
#[async_trait::async_trait]
impl crate::search::SearchSource for ClipboardRepo {
    fn name(&self) -> &str {
        "clipboard"
    }

    async fn search(&self, query: &str, limit: usize) -> Vec<crate::LauncherItem> {
        let entries = match self.search(query, limit as i64).await {
            Ok(e) => e,
            Err(_) => return vec![],
        };
        entries
            .into_iter()
            .map(|e| {
                let content_type = match e.content_type.as_str() {
                    "image" => crate::ClipboardContentType::Image,
                    "file" => crate::ClipboardContentType::File,
                    _ => crate::ClipboardContentType::Text,
                };
                let preview: String = e
                    .preview
                    .clone()
                    .unwrap_or_else(|| e.content.chars().take(80).collect());
                crate::LauncherItem {
                    id: format!("clip:{}", e.id),
                    title: preview,
                    subtitle: e.source_app.clone(),
                    icon: Some("clipboard".to_string()),
                    kind: crate::LauncherItemKind::ClipboardEntry {
                        entry_id: e.id,
                        content_type,
                    },
                    score: 0.5,
                }
            })
            .collect()
    }
}
```

- [ ] **Step 8: Update exports in feature-launcher/src/lib.rs**

Add `pub use search::{SearchSource, SourceRegistry};` to the re-exports.

- [ ] **Step 9: Build workspace and run tests**

Run: `cargo build --workspace && cargo nextest run --workspace`
Expected: All tests pass. The refactor is behavior-preserving — same sources, same routing, same scoring.

- [ ] **Step 10: Commit**

```bash
git add crates/feature-launcher/ crates/app-core/ crates/config/
git commit -m "feat(launcher): refactor engine to use SourceRegistry with SearchSource trait"
```

---

## Chunk 2: Tier 1 In-Memory Sources (new)

### Task 2.1: System Preferences source

**Files:**
- Create: `crates/feature-launcher/src/search/system_prefs.rs`
- Modify: `crates/feature-launcher/src/search/mod.rs`

- [ ] **Step 1: Create system_prefs.rs**

```rust
use crate::types::*;
use parking_lot::RwLock;
use std::path::{Path, PathBuf};
use std::sync::Arc;

#[derive(Debug, Clone)]
struct PrefPane {
    name: String,
    bundle_id: String,
    path: PathBuf,
}

#[derive(Clone)]
pub struct SystemPrefsSource {
    panes: Arc<RwLock<Vec<PrefPane>>>,
}

impl SystemPrefsSource {
    pub fn new() -> Self {
        Self {
            panes: Arc::new(RwLock::new(Vec::new())),
        }
    }

    #[cfg(target_os = "macos")]
    fn scan_panes() -> Vec<PrefPane> {
        use std::process::Command;

        let dirs = [
            Path::new("/System/Library/PreferencePanes"),
            Path::new("/Library/PreferencePanes"),
        ];
        let home = std::env::var("HOME").unwrap_or_default();
        let user_dir = PathBuf::from(&home).join("Library/PreferencePanes");

        let mut panes = Vec::new();
        for dir in dirs.iter().chain(std::iter::once(&user_dir.as_path())) {
            let entries = match std::fs::read_dir(dir) {
                Ok(e) => e,
                Err(_) => continue,
            };
            for entry in entries.flatten() {
                let path = entry.path();
                if path.extension().map_or(true, |e| e != "prefPane") {
                    continue;
                }
                let plist = path.join("Contents/Info.plist");
                if !plist.exists() {
                    continue;
                }
                // Read bundle identifier
                let bundle_out = Command::new("/usr/libexec/PlistBuddy")
                    .args(["-c", "Print :CFBundleIdentifier", &plist.to_string_lossy()])
                    .output();
                let bundle_id = match bundle_out {
                    Ok(o) if o.status.success() => {
                        String::from_utf8_lossy(&o.stdout).trim().to_string()
                    }
                    _ => continue,
                };
                // Read display name (try NSPrefPaneIconLabel first, then CFBundleName)
                let name = Command::new("/usr/libexec/PlistBuddy")
                    .args(["-c", "Print :NSPrefPaneIconLabel", &plist.to_string_lossy()])
                    .output()
                    .ok()
                    .filter(|o| o.status.success())
                    .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string())
                    .or_else(|| {
                        Command::new("/usr/libexec/PlistBuddy")
                            .args(["-c", "Print :CFBundleName", &plist.to_string_lossy()])
                            .output()
                            .ok()
                            .filter(|o| o.status.success())
                            .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string())
                    })
                    .unwrap_or_else(|| {
                        path.file_stem()
                            .unwrap_or_default()
                            .to_string_lossy()
                            .to_string()
                    });

                panes.push(PrefPane {
                    name,
                    bundle_id,
                    path,
                });
            }
        }
        panes
    }

    #[cfg(not(target_os = "macos"))]
    fn scan_panes() -> Vec<PrefPane> {
        vec![]
    }
}

#[async_trait::async_trait]
impl super::SearchSource for SystemPrefsSource {
    fn name(&self) -> &str {
        "system_prefs"
    }

    async fn search(&self, query: &str, limit: usize) -> Vec<LauncherItem> {
        use nucleo_matcher::{
            pattern::{CaseMatching, Normalization, Pattern},
            Config, Matcher,
        };

        let panes = self.panes.read();
        if panes.is_empty() || query.is_empty() {
            return vec![];
        }

        let mut matcher = Matcher::new(Config::DEFAULT);
        let pattern = Pattern::parse(query, CaseMatching::Ignore, Normalization::Smart);

        let mut scored: Vec<(u32, &PrefPane)> = panes
            .iter()
            .filter_map(|p| {
                let mut buf = Vec::new();
                let score = pattern.score(
                    nucleo_matcher::Utf32Str::new(&p.name, &mut buf),
                    &mut matcher,
                )?;
                Some((score, p))
            })
            .collect();

        scored.sort_by(|a, b| b.0.cmp(&a.0));
        scored.truncate(limit);

        scored
            .into_iter()
            .map(|(score, p)| LauncherItem {
                id: format!("pref:{}", p.bundle_id),
                title: p.name.clone(),
                subtitle: Some("System Settings".to_string()),
                icon: Some("settings".to_string()),
                kind: LauncherItemKind::SystemPref {
                    pane_id: p.bundle_id.clone(),
                },
                score: (score as f64) / 1000.0 * 0.6,
            })
            .collect()
    }

    async fn refresh(&self) {
        let panes = Self::scan_panes();
        tracing::info!("Indexed {} system preference panes", panes.len());
        *self.panes.write() = panes;
    }
}
```

- [ ] **Step 2: Register module in search/mod.rs**

Add `pub mod system_prefs;` and `pub use system_prefs::SystemPrefsSource;`.

- [ ] **Step 3: Build**

Run: `cargo build -p feature-launcher`
Expected: Compiles

- [ ] **Step 4: Commit**

```bash
git add crates/feature-launcher/src/search/
git commit -m "feat(launcher): add System Preferences search source"
```

### Task 2.2: Brew Packages source

**Files:**
- Create: `crates/feature-launcher/src/search/brew.rs`
- Modify: `crates/feature-launcher/src/search/mod.rs`

- [ ] **Step 1: Create brew.rs**

```rust
use crate::types::*;
use parking_lot::RwLock;
use std::sync::Arc;

#[derive(Debug, Clone)]
struct BrewEntry {
    name: String,
    is_cask: bool,
}

#[derive(Clone)]
pub struct BrewSource {
    packages: Arc<RwLock<Vec<BrewEntry>>>,
}

impl BrewSource {
    pub fn new() -> Self {
        Self {
            packages: Arc::new(RwLock::new(Vec::new())),
        }
    }

    fn scan_packages() -> Vec<BrewEntry> {
        let mut entries = Vec::new();

        // Check if brew is installed
        let brew_path = which::which("brew");
        if brew_path.is_err() {
            tracing::info!("brew not found — BrewSource disabled");
            return entries;
        }

        // Formulae
        if let Ok(output) = std::process::Command::new("brew")
            .args(["list", "--formula", "-1"])
            .output()
        {
            if output.status.success() {
                for line in String::from_utf8_lossy(&output.stdout).lines() {
                    let name = line.trim();
                    if !name.is_empty() {
                        entries.push(BrewEntry {
                            name: name.to_string(),
                            is_cask: false,
                        });
                    }
                }
            }
        }

        // Casks
        if let Ok(output) = std::process::Command::new("brew")
            .args(["list", "--cask", "-1"])
            .output()
        {
            if output.status.success() {
                for line in String::from_utf8_lossy(&output.stdout).lines() {
                    let name = line.trim();
                    if !name.is_empty() {
                        entries.push(BrewEntry {
                            name: name.to_string(),
                            is_cask: true,
                        });
                    }
                }
            }
        }

        entries
    }
}

#[async_trait::async_trait]
impl super::SearchSource for BrewSource {
    fn name(&self) -> &str {
        "brew"
    }

    async fn search(&self, query: &str, limit: usize) -> Vec<LauncherItem> {
        use nucleo_matcher::{
            pattern::{CaseMatching, Normalization, Pattern},
            Config, Matcher,
        };

        let packages = self.packages.read();
        if packages.is_empty() || query.is_empty() {
            return vec![];
        }

        let mut matcher = Matcher::new(Config::DEFAULT);
        let pattern = Pattern::parse(query, CaseMatching::Ignore, Normalization::Smart);

        let mut scored: Vec<(u32, &BrewEntry)> = packages
            .iter()
            .filter_map(|p| {
                let mut buf = Vec::new();
                let score = pattern.score(
                    nucleo_matcher::Utf32Str::new(&p.name, &mut buf),
                    &mut matcher,
                )?;
                Some((score, p))
            })
            .collect();

        scored.sort_by(|a, b| b.0.cmp(&a.0));
        scored.truncate(limit);

        scored
            .into_iter()
            .map(|(score, p)| {
                let kind_label = if p.is_cask { "Cask" } else { "Formula" };
                LauncherItem {
                    id: format!("brew:{}", p.name),
                    title: p.name.clone(),
                    subtitle: Some(format!("Homebrew {kind_label}")),
                    icon: Some("package".to_string()),
                    kind: LauncherItemKind::BrewPackage {
                        name: p.name.clone(),
                        is_cask: p.is_cask,
                    },
                    score: (score as f64) / 1000.0 * 0.4,
                }
            })
            .collect()
    }

    async fn refresh(&self) {
        let packages = Self::scan_packages();
        tracing::info!("Indexed {} brew packages", packages.len());
        *self.packages.write() = packages;
    }
}
```

- [ ] **Step 2: Add `which` dependency to feature-launcher Cargo.toml**

```toml
which.workspace = true
```

- [ ] **Step 3: Register module, build, commit**

Add `pub mod brew;` and `pub use brew::BrewSource;` to `search/mod.rs`.

Run: `cargo build -p feature-launcher`

```bash
git add crates/feature-launcher/
git commit -m "feat(launcher): add Brew packages search source"
```

### Task 2.3: SSH Hosts source

**Files:**
- Create: `crates/feature-launcher/src/search/ssh_hosts.rs`
- Modify: `crates/feature-launcher/src/search/mod.rs`

- [ ] **Step 1: Create ssh_hosts.rs**

```rust
use crate::types::*;
use parking_lot::RwLock;
use std::path::Path;
use std::sync::Arc;

#[derive(Debug, Clone)]
struct SshEntry {
    host: String,
    user: Option<String>,
    hostname: Option<String>,
}

#[derive(Clone)]
pub struct SshHostsSource {
    hosts: Arc<RwLock<Vec<SshEntry>>>,
}

impl SshHostsSource {
    pub fn new() -> Self {
        Self {
            hosts: Arc::new(RwLock::new(Vec::new())),
        }
    }

    fn parse_ssh_config(path: &Path) -> Vec<SshEntry> {
        let content = match std::fs::read_to_string(path) {
            Ok(c) => c,
            Err(_) => return vec![],
        };

        let mut entries = Vec::new();
        let mut current_host: Option<String> = None;
        let mut current_user: Option<String> = None;
        let mut current_hostname: Option<String> = None;

        for line in content.lines() {
            let line = line.trim();
            if line.is_empty() || line.starts_with('#') {
                continue;
            }

            let parts: Vec<&str> = line.splitn(2, char::is_whitespace).collect();
            if parts.len() < 2 {
                continue;
            }
            let key = parts[0].to_lowercase();
            let value = parts[1].trim();

            if key == "host" {
                // Save previous entry
                if let Some(host) = current_host.take() {
                    if host != "*" && !host.contains('*') && !host.contains('?') {
                        entries.push(SshEntry {
                            host,
                            user: current_user.take(),
                            hostname: current_hostname.take(),
                        });
                    }
                }
                current_host = Some(value.to_string());
                current_user = None;
                current_hostname = None;
            } else if key == "user" {
                current_user = Some(value.to_string());
            } else if key == "hostname" {
                current_hostname = Some(value.to_string());
            }
        }

        // Save last entry
        if let Some(host) = current_host {
            if host != "*" && !host.contains('*') && !host.contains('?') {
                entries.push(SshEntry {
                    host,
                    user: current_user,
                    hostname: current_hostname,
                });
            }
        }

        entries
    }
}

#[async_trait::async_trait]
impl super::SearchSource for SshHostsSource {
    fn name(&self) -> &str {
        "ssh_hosts"
    }

    async fn search(&self, query: &str, limit: usize) -> Vec<LauncherItem> {
        let hosts = self.hosts.read();
        let query_lower = query.to_lowercase();

        let mut results: Vec<LauncherItem> = hosts
            .iter()
            .filter(|h| {
                query.is_empty()
                    || h.host.to_lowercase().contains(&query_lower)
                    || h.hostname
                        .as_ref()
                        .map_or(false, |hn| hn.to_lowercase().contains(&query_lower))
            })
            .map(|h| {
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
                    score: if h.host.to_lowercase().starts_with(&query_lower) {
                        0.7
                    } else {
                        0.5
                    },
                }
            })
            .collect();

        results.truncate(limit);
        results
    }

    async fn refresh(&self) {
        let home = std::env::var("HOME").unwrap_or_default();
        let ssh_config = Path::new(&home).join(".ssh/config");
        let hosts = Self::parse_ssh_config(&ssh_config);
        tracing::info!("Indexed {} SSH hosts", hosts.len());
        *self.hosts.write() = hosts;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    #[test]
    fn test_parse_ssh_config() {
        let dir = tempfile::TempDir::new().unwrap();
        let config_path = dir.path().join("config");
        let mut f = std::fs::File::create(&config_path).unwrap();
        writeln!(
            f,
            "Host production\n  HostName prod.example.com\n  User deploy\n\nHost staging\n  HostName staging.example.com\n\nHost *\n  ServerAliveInterval 60"
        )
        .unwrap();

        let entries = SshHostsSource::parse_ssh_config(&config_path);
        assert_eq!(entries.len(), 2);
        assert_eq!(entries[0].host, "production");
        assert_eq!(entries[0].user.as_deref(), Some("deploy"));
        assert_eq!(entries[1].host, "staging");
        assert!(entries[1].user.is_none());
    }
}
```

- [ ] **Step 2: Register module, build, test, commit**

Add `pub mod ssh_hosts;` and `pub use ssh_hosts::SshHostsSource;` to `search/mod.rs`.

Run: `cargo build -p feature-launcher && cargo nextest run -p feature-launcher`

```bash
git add crates/feature-launcher/src/search/
git commit -m "feat(launcher): add SSH hosts search source"
```

### Task 2.4: Git Repos source

**Files:**
- Create: `crates/feature-launcher/src/search/git_repos.rs`
- Modify: `crates/feature-launcher/src/search/mod.rs`

- [ ] **Step 1: Create git_repos.rs**

```rust
use crate::types::*;
use parking_lot::RwLock;
use std::path::{Path, PathBuf};
use std::sync::Arc;

#[derive(Debug, Clone)]
struct RepoEntry {
    name: String,
    path: PathBuf,
}

#[derive(Clone)]
pub struct GitReposSource {
    repos: Arc<RwLock<Vec<RepoEntry>>>,
    scan_dirs: Vec<String>,
}

impl GitReposSource {
    pub fn new(scan_dirs: Vec<String>) -> Self {
        Self {
            repos: Arc::new(RwLock::new(Vec::new())),
            scan_dirs,
        }
    }

    fn scan_repos(dirs: &[String], max_depth: usize) -> Vec<RepoEntry> {
        let mut repos = Vec::new();
        for dir in dirs {
            let expanded = shellexpand::tilde(dir).to_string();
            let path = Path::new(&expanded);
            if path.exists() {
                Self::walk_for_repos(path, max_depth, &mut repos);
            }
        }
        repos
    }

    fn walk_for_repos(dir: &Path, depth: usize, repos: &mut Vec<RepoEntry>) {
        if depth == 0 {
            return;
        }
        let entries = match std::fs::read_dir(dir) {
            Ok(e) => e,
            Err(_) => return,
        };
        for entry in entries.flatten() {
            let path = entry.path();
            if !path.is_dir() {
                continue;
            }
            // Check if this directory is a git repo
            if path.join(".git").exists() {
                let name = path
                    .file_name()
                    .unwrap_or_default()
                    .to_string_lossy()
                    .to_string();
                repos.push(RepoEntry { name, path });
                // Don't recurse into git repos
                continue;
            }
            // Skip common non-project dirs
            let dir_name = path.file_name().unwrap_or_default().to_string_lossy();
            if dir_name.starts_with('.') || dir_name == "node_modules" || dir_name == "target" {
                continue;
            }
            Self::walk_for_repos(&path, depth - 1, repos);
        }
    }
}

#[async_trait::async_trait]
impl super::SearchSource for GitReposSource {
    fn name(&self) -> &str {
        "git_repos"
    }

    async fn search(&self, query: &str, limit: usize) -> Vec<LauncherItem> {
        use nucleo_matcher::{
            pattern::{CaseMatching, Normalization, Pattern},
            Config, Matcher,
        };

        let repos = self.repos.read();
        if repos.is_empty() || query.is_empty() {
            return vec![];
        }

        let mut matcher = Matcher::new(Config::DEFAULT);
        let pattern = Pattern::parse(query, CaseMatching::Ignore, Normalization::Smart);

        let mut scored: Vec<(u32, &RepoEntry)> = repos
            .iter()
            .filter_map(|r| {
                let mut buf = Vec::new();
                let score = pattern.score(
                    nucleo_matcher::Utf32Str::new(&r.name, &mut buf),
                    &mut matcher,
                )?;
                Some((score, r))
            })
            .collect();

        scored.sort_by(|a, b| b.0.cmp(&a.0));
        scored.truncate(limit);

        scored
            .into_iter()
            .map(|(score, r)| LauncherItem {
                id: format!("repo:{}", r.path.display()),
                title: r.name.clone(),
                subtitle: Some(r.path.display().to_string()),
                icon: Some("git-branch".to_string()),
                kind: LauncherItemKind::GitRepo {
                    path: r.path.clone(),
                },
                score: (score as f64) / 1000.0 * 0.8,
            })
            .collect()
    }

    async fn refresh(&self) {
        let repos = Self::scan_repos(&self.scan_dirs, 3);
        tracing::info!("Indexed {} git repos", repos.len());
        *self.repos.write() = repos;
    }
}
```

- [ ] **Step 2: Add `shellexpand` dependency to feature-launcher Cargo.toml**

```toml
shellexpand.workspace = true
```

- [ ] **Step 3: Register module, build, commit**

Add `pub mod git_repos;` and `pub use git_repos::GitReposSource;` to `search/mod.rs`.

Run: `cargo build -p feature-launcher`

```bash
git add crates/feature-launcher/
git commit -m "feat(launcher): add Git repos search source"
```

---

## Chunk 3: Tier 2 Shell-Out Sources

### Task 3.1: File search via mdfind

**Files:**
- Create: `crates/feature-launcher/src/search/file_search.rs`
- Modify: `crates/feature-launcher/src/search/mod.rs`

- [ ] **Step 1: Create file_search.rs**

```rust
use crate::types::*;
use std::path::PathBuf;

pub struct FileSearchSource;

impl FileSearchSource {
    pub fn new() -> Self {
        Self
    }

    fn classify_file(path: &std::path::Path) -> FileKind {
        if path.is_dir() {
            return FileKind::Folder;
        }
        match path.extension().and_then(|e| e.to_str()) {
            Some("png" | "jpg" | "jpeg" | "gif" | "svg" | "webp" | "ico" | "heic") => {
                FileKind::Image
            }
            Some("pdf" | "doc" | "docx" | "txt" | "md" | "rtf" | "pages" | "odt") => {
                FileKind::Document
            }
            Some(
                "rs" | "ts" | "tsx" | "js" | "jsx" | "py" | "go" | "rb" | "java" | "c" | "cpp"
                | "h" | "swift" | "kt" | "sh" | "toml" | "yaml" | "json" | "html" | "css",
            ) => FileKind::Code,
            Some("zip" | "tar" | "gz" | "bz2" | "xz" | "7z" | "rar" | "dmg") => {
                FileKind::Archive
            }
            _ => FileKind::File,
        }
    }
}

#[async_trait::async_trait]
impl super::SearchSource for FileSearchSource {
    fn name(&self) -> &str {
        "files"
    }

    async fn search(&self, query: &str, limit: usize) -> Vec<LauncherItem> {
        if query.is_empty() {
            return vec![];
        }

        #[cfg(target_os = "macos")]
        {
            let output = tokio::time::timeout(
                std::time::Duration::from_secs(1),
                tokio::process::Command::new("mdfind")
                    .args(["-name", query, "-limit", &limit.to_string()])
                    .output(),
            )
            .await;

            let output = match output {
                Ok(Ok(o)) if o.status.success() => o,
                _ => return vec![],
            };

            String::from_utf8_lossy(&output.stdout)
                .lines()
                .filter(|line| !line.trim().is_empty())
                .map(|line| {
                    let path = PathBuf::from(line.trim());
                    let name = path
                        .file_name()
                        .unwrap_or_default()
                        .to_string_lossy()
                        .to_string();
                    let kind = Self::classify_file(&path);
                    LauncherItem {
                        id: format!("file:{}", path.display()),
                        title: name,
                        subtitle: Some(path.display().to_string()),
                        icon: Some("file".to_string()),
                        kind: LauncherItemKind::File {
                            path,
                            kind,
                        },
                        score: 0.8,
                    }
                })
                .collect()
        }

        #[cfg(not(target_os = "macos"))]
        {
            let _ = (query, limit);
            vec![]
        }
    }
}
```

- [ ] **Step 2: Register module, build, commit**

Add `pub mod file_search;` and `pub use file_search::FileSearchSource;` to `search/mod.rs`.

Run: `cargo build -p feature-launcher`

```bash
git add crates/feature-launcher/src/search/
git commit -m "feat(launcher): add file search source via mdfind"
```

### Task 3.2: Content grep via rg

**Files:**
- Create: `crates/feature-launcher/src/search/content_grep.rs`
- Modify: `crates/feature-launcher/src/search/mod.rs`

- [ ] **Step 1: Create content_grep.rs**

```rust
use crate::types::*;
use std::path::PathBuf;

pub struct ContentGrepSource {
    default_scope: String,
}

impl ContentGrepSource {
    pub fn new(default_scope: String) -> Self {
        Self { default_scope }
    }
}

#[async_trait::async_trait]
impl super::SearchSource for ContentGrepSource {
    fn name(&self) -> &str {
        "content_grep"
    }

    fn prefix(&self) -> Option<char> {
        Some('?')
    }

    async fn search(&self, query: &str, limit: usize) -> Vec<LauncherItem> {
        if query.is_empty() {
            return vec![];
        }

        // Check if rg is available
        if which::which("rg").is_err() {
            tracing::info!("rg not found — content grep disabled");
            return vec![];
        }

        let scope = shellexpand::tilde(&self.default_scope).to_string();

        let output = tokio::time::timeout(
            std::time::Duration::from_secs(2),
            tokio::process::Command::new("rg")
                .args(["--json", "-m", "1", "--max-count", "1", query, &scope])
                .output(),
        )
        .await;

        let output = match output {
            Ok(Ok(o)) => o,
            _ => return vec![],
        };

        let stdout = String::from_utf8_lossy(&output.stdout);
        let mut results = Vec::new();
        let mut seen_files = std::collections::HashSet::new();

        for line in stdout.lines() {
            if results.len() >= limit {
                break;
            }
            let Ok(json) = serde_json::from_str::<serde_json::Value>(line) else {
                continue;
            };
            if json.get("type").and_then(|t| t.as_str()) != Some("match") {
                continue;
            }
            let Some(data) = json.get("data") else {
                continue;
            };
            let Some(path_str) = data.get("path").and_then(|p| p.get("text")).and_then(|t| t.as_str()) else {
                continue;
            };
            // Deduplicate by file
            if !seen_files.insert(path_str.to_string()) {
                continue;
            }
            let line_num = data
                .get("line_number")
                .and_then(|n| n.as_u64())
                .unwrap_or(0) as u32;
            let preview = data
                .get("lines")
                .and_then(|l| l.get("text"))
                .and_then(|t| t.as_str())
                .unwrap_or("")
                .trim()
                .chars()
                .take(100)
                .collect::<String>();
            let path = PathBuf::from(path_str);
            let file_name = path
                .file_name()
                .unwrap_or_default()
                .to_string_lossy()
                .to_string();

            results.push(LauncherItem {
                id: format!("grep:{}:{}", path.display(), line_num),
                title: file_name,
                subtitle: Some(preview.clone()),
                icon: Some("search".to_string()),
                kind: LauncherItemKind::ContentMatch {
                    path,
                    line: line_num,
                    preview,
                },
                score: 0.7,
            });
        }

        results
    }
}
```

- [ ] **Step 2: Register module, build, commit**

Add `pub mod content_grep;` and `pub use content_grep::ContentGrepSource;` to `search/mod.rs`.

Run: `cargo build -p feature-launcher`

```bash
git add crates/feature-launcher/src/search/
git commit -m "feat(launcher): add content grep source via rg"
```

### Task 3.3: Contacts source via JXA

**Files:**
- Create: `crates/feature-launcher/src/search/contacts.rs`
- Modify: `crates/feature-launcher/src/search/mod.rs`

- [ ] **Step 1: Create contacts.rs**

```rust
use crate::types::*;
use std::sync::atomic::{AtomicBool, Ordering};

pub struct ContactsSource {
    permission_warned: AtomicBool,
}

impl ContactsSource {
    pub fn new() -> Self {
        Self {
            permission_warned: AtomicBool::new(false),
        }
    }

    #[cfg(target_os = "macos")]
    const JXA_SCRIPT: &'static str = r#"
        var app = Application("Contacts");
        var query = "$QUERY";
        var people = app.people.whose({name: {_contains: query}});
        var results = [];
        var limit = Math.min(people.length, 10);
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
}

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

        #[cfg(target_os = "macos")]
        {
            // Escape query for JXA
            let safe_query = query.replace('\\', "\\\\").replace('"', "\\\"");
            let script = Self::JXA_SCRIPT.replace("$QUERY", &safe_query);

            let output = tokio::time::timeout(
                std::time::Duration::from_secs(1),
                tokio::process::Command::new("osascript")
                    .args(["-l", "JavaScript", "-e", &script])
                    .output(),
            )
            .await;

            let output = match output {
                Ok(Ok(o)) if o.status.success() => o,
                Ok(Ok(o)) => {
                    // Permission denied — warn once
                    if !self.permission_warned.swap(true, Ordering::Relaxed) {
                        let stderr = String::from_utf8_lossy(&o.stderr);
                        if stderr.contains("not allowed") || stderr.contains("denied") {
                            tracing::warn!(
                                "Contacts access denied. Grant access in System Settings > Privacy > Contacts."
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
                .take(limit)
                .filter_map(|c| {
                    let name = c.get("name")?.as_str()?.to_string();
                    let email = c
                        .get("email")
                        .and_then(|e| e.as_str())
                        .map(|s| s.to_string());
                    let phone = c
                        .get("phone")
                        .and_then(|p| p.as_str())
                        .map(|s| s.to_string());
                    let subtitle = email
                        .clone()
                        .or_else(|| phone.clone())
                        .unwrap_or_default();
                    Some(LauncherItem {
                        id: format!("contact:{}", name),
                        title: name.clone(),
                        subtitle: Some(subtitle),
                        icon: Some("user".to_string()),
                        kind: LauncherItemKind::Contact {
                            name,
                            email,
                            phone,
                        },
                        score: 0.6,
                    })
                })
                .collect()
        }

        #[cfg(not(target_os = "macos"))]
        {
            let _ = (query, limit);
            vec![]
        }
    }
}
```

- [ ] **Step 2: Register module, build, commit**

Add `pub mod contacts;` and `pub use contacts::ContactsSource;` to `search/mod.rs`.

Run: `cargo build -p feature-launcher`

```bash
git add crates/feature-launcher/src/search/
git commit -m "feat(launcher): add contacts search source via JXA"
```

---

## Chunk 4: Tier 3 Native + DB Sources

### Task 4.1: Running Apps source

**Files:**
- Create: `crates/feature-launcher/src/search/running_apps.rs`
- Modify: `crates/feature-launcher/src/search/mod.rs`
- Modify: `crates/feature-launcher/Cargo.toml`

- [ ] **Step 1: Add objc2 dependencies to Cargo.toml**

Add under `[target.'cfg(target_os = "macos")'.dependencies]` (create this section if it doesn't exist):

```toml
[target.'cfg(target_os = "macos")'.dependencies]
objc2 = "0.5"
objc2-app-kit = { version = "0.2", features = ["NSWorkspace", "NSRunningApplication"] }
objc2-foundation = { version = "0.2", features = ["NSString"] }
```

- [ ] **Step 2: Create running_apps.rs**

```rust
use crate::types::*;

pub struct RunningAppsSource;

impl RunningAppsSource {
    pub fn new() -> Self {
        Self
    }
}

#[async_trait::async_trait]
impl super::SearchSource for RunningAppsSource {
    fn name(&self) -> &str {
        "running_apps"
    }

    async fn search(&self, query: &str, limit: usize) -> Vec<LauncherItem> {
        if query.is_empty() {
            return vec![];
        }

        #[cfg(target_os = "macos")]
        {
            use nucleo_matcher::{
                pattern::{CaseMatching, Normalization, Pattern},
                Config, Matcher,
            };

            let apps = Self::get_running_apps();
            let mut matcher = Matcher::new(Config::DEFAULT);
            let pattern = Pattern::parse(query, CaseMatching::Ignore, Normalization::Smart);

            let mut scored: Vec<(u32, String, u32, std::path::PathBuf)> = apps
                .into_iter()
                .filter_map(|(name, pid, path)| {
                    let mut buf = Vec::new();
                    let score = pattern.score(
                        nucleo_matcher::Utf32Str::new(&name, &mut buf),
                        &mut matcher,
                    )?;
                    Some((score, name, pid, path))
                })
                .collect();

            scored.sort_by(|a, b| b.0.cmp(&a.0));
            scored.truncate(limit);

            scored
                .into_iter()
                .map(|(score, name, pid, path)| LauncherItem {
                    id: format!("running:{pid}"),
                    title: name,
                    subtitle: Some("Running".to_string()),
                    icon: Some("activity".to_string()),
                    kind: LauncherItemKind::RunningApp { pid, path },
                    score: (score as f64) / 1000.0 * 1.2, // Boost running apps
                })
                .collect()
        }

        #[cfg(not(target_os = "macos"))]
        {
            let _ = (query, limit);
            vec![]
        }
    }
}

#[cfg(target_os = "macos")]
impl RunningAppsSource {
    fn get_running_apps() -> Vec<(String, u32, std::path::PathBuf)> {
        use objc2_app_kit::NSWorkspace;

        let mut apps = Vec::new();
        unsafe {
            let workspace = NSWorkspace::sharedWorkspace();
            let running = workspace.runningApplications();
            for app in running.iter() {
                let name = match app.localizedName() {
                    Some(n) => n.to_string(),
                    None => continue,
                };
                let pid = app.processIdentifier() as u32;
                let path = app
                    .bundleURL()
                    .and_then(|u| u.path().map(|p| std::path::PathBuf::from(p.to_string())))
                    .unwrap_or_default();

                // Skip background/system processes
                if app.activationPolicy()
                    == objc2_app_kit::NSApplicationActivationPolicy::Prohibited
                {
                    continue;
                }

                apps.push((name, pid, path));
            }
        }
        apps
    }
}
```

**Note:** The exact `objc2-app-kit` API may differ slightly from what's shown. During implementation, check the actual crate docs for method signatures. The pattern follows `crates/feature-productivity/src/tracker/macos.rs`.

- [ ] **Step 3: Register module, build, commit**

Add `pub mod running_apps;` and `pub use running_apps::RunningAppsSource;` to `search/mod.rs`.

Run: `cargo build -p feature-launcher`

```bash
git add crates/feature-launcher/
git commit -m "feat(launcher): add running apps search source via NSWorkspace"
```

### Task 4.2: Browser Bookmarks source

**Files:**
- Create: `crates/feature-launcher/src/search/bookmarks.rs`
- Modify: `crates/feature-launcher/src/search/mod.rs`

- [ ] **Step 1: Create bookmarks.rs**

```rust
use crate::types::*;
use parking_lot::RwLock;
use std::path::{Path, PathBuf};
use std::sync::Arc;

#[derive(Debug, Clone)]
struct BookmarkEntry {
    title: String,
    url: String,
}

#[derive(Clone)]
pub struct BookmarksSource {
    bookmarks: Arc<RwLock<Vec<BookmarkEntry>>>,
    browser: String,
}

impl BookmarksSource {
    pub fn new(browser: String) -> Self {
        Self {
            bookmarks: Arc::new(RwLock::new(Vec::new())),
            browser,
        }
    }

    fn browser_bookmarks_path(browser: &str) -> Option<PathBuf> {
        let home = std::env::var("HOME").ok()?;
        let app_support = PathBuf::from(&home).join("Library/Application Support");
        match browser {
            "chrome" => Some(app_support.join("Google/Chrome/Default/Bookmarks")),
            "arc" => Some(app_support.join("Arc/User Data/Default/Bookmarks")),
            "brave" => Some(app_support.join("BraveSoftware/Brave-Browser/Default/Bookmarks")),
            "edge" => Some(app_support.join("Microsoft Edge/Default/Bookmarks")),
            _ => None,
        }
    }

    fn parse_chromium_bookmarks(path: &Path) -> Vec<BookmarkEntry> {
        let content = match std::fs::read_to_string(path) {
            Ok(c) => c,
            Err(_) => return vec![],
        };
        let json: serde_json::Value = match serde_json::from_str(&content) {
            Ok(j) => j,
            Err(_) => return vec![],
        };

        let mut entries = Vec::new();
        if let Some(roots) = json.get("roots") {
            for (_key, folder) in roots.as_object().into_iter().flatten() {
                Self::collect_bookmarks(folder, &mut entries);
            }
        }
        entries
    }

    fn collect_bookmarks(node: &serde_json::Value, entries: &mut Vec<BookmarkEntry>) {
        match node.get("type").and_then(|t| t.as_str()) {
            Some("url") => {
                if let (Some(title), Some(url)) = (
                    node.get("name").and_then(|n| n.as_str()),
                    node.get("url").and_then(|u| u.as_str()),
                ) {
                    entries.push(BookmarkEntry {
                        title: title.to_string(),
                        url: url.to_string(),
                    });
                }
            }
            Some("folder") => {
                if let Some(children) = node.get("children").and_then(|c| c.as_array()) {
                    for child in children {
                        Self::collect_bookmarks(child, entries);
                    }
                }
            }
            _ => {}
        }
    }
}

#[async_trait::async_trait]
impl super::SearchSource for BookmarksSource {
    fn name(&self) -> &str {
        "bookmarks"
    }

    async fn search(&self, query: &str, limit: usize) -> Vec<LauncherItem> {
        use nucleo_matcher::{
            pattern::{CaseMatching, Normalization, Pattern},
            Config, Matcher,
        };

        let bookmarks = self.bookmarks.read();
        if bookmarks.is_empty() || query.is_empty() {
            return vec![];
        }

        let mut matcher = Matcher::new(Config::DEFAULT);
        let pattern = Pattern::parse(query, CaseMatching::Ignore, Normalization::Smart);

        let mut scored: Vec<(u32, &BookmarkEntry)> = bookmarks
            .iter()
            .filter_map(|b| {
                let mut buf = Vec::new();
                let score = pattern.score(
                    nucleo_matcher::Utf32Str::new(&b.title, &mut buf),
                    &mut matcher,
                )?;
                Some((score, b))
            })
            .collect();

        scored.sort_by(|a, b| b.0.cmp(&a.0));
        scored.truncate(limit);

        scored
            .into_iter()
            .map(|(score, b)| LauncherItem {
                id: format!("bookmark:{}", b.url),
                title: b.title.clone(),
                subtitle: Some(b.url.clone()),
                icon: Some("bookmark".to_string()),
                kind: LauncherItemKind::Bookmark {
                    url: b.url.clone(),
                    browser: self.browser.clone(),
                },
                score: (score as f64) / 1000.0 * 0.7,
            })
            .collect()
    }

    async fn refresh(&self) {
        let path = match Self::browser_bookmarks_path(&self.browser) {
            Some(p) if p.exists() => p,
            _ => {
                tracing::debug!("Bookmarks file not found for browser: {}", self.browser);
                return;
            }
        };
        let bookmarks = Self::parse_chromium_bookmarks(&path);
        tracing::info!("Indexed {} bookmarks from {}", bookmarks.len(), self.browser);
        *self.bookmarks.write() = bookmarks;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_chromium_bookmarks() {
        let json = r#"{
            "roots": {
                "bookmark_bar": {
                    "type": "folder",
                    "children": [
                        { "type": "url", "name": "Rust", "url": "https://rust-lang.org" },
                        { "type": "folder", "children": [
                            { "type": "url", "name": "Tokio", "url": "https://tokio.rs" }
                        ], "type": "folder" }
                    ]
                }
            }
        }"#;

        let dir = tempfile::TempDir::new().unwrap();
        let path = dir.path().join("Bookmarks");
        std::fs::write(&path, json).unwrap();

        let entries = BookmarksSource::parse_chromium_bookmarks(&path);
        assert_eq!(entries.len(), 2);
        assert_eq!(entries[0].title, "Rust");
        assert_eq!(entries[1].title, "Tokio");
    }
}
```

- [ ] **Step 2: Register module, build, test, commit**

Add `pub mod bookmarks;` and `pub use bookmarks::BookmarksSource;` to `search/mod.rs`.

Run: `cargo build -p feature-launcher && cargo nextest run -p feature-launcher`

```bash
git add crates/feature-launcher/src/search/
git commit -m "feat(launcher): add browser bookmarks search source"
```

### Task 4.3: Browser History source

**Files:**
- Create: `crates/feature-launcher/src/search/browser_history.rs`
- Modify: `crates/feature-launcher/src/search/mod.rs`

- [ ] **Step 1: Create browser_history.rs**

```rust
use crate::types::*;
use parking_lot::RwLock;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

#[derive(Debug, Clone)]
struct HistoryEntry {
    title: String,
    url: String,
    last_visit: String,
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

    fn history_db_path(browser: &str) -> Option<PathBuf> {
        let home = std::env::var("HOME").ok()?;
        let app_support = PathBuf::from(&home).join("Library/Application Support");
        match browser {
            "chrome" => Some(app_support.join("Google/Chrome/Default/History")),
            "arc" => Some(app_support.join("Arc/User Data/Default/History")),
            "brave" => Some(app_support.join("BraveSoftware/Brave-Browser/Default/History")),
            "edge" => Some(app_support.join("Microsoft Edge/Default/History")),
            _ => None,
        }
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
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e.to_string()))?;

        // Chrome stores last_visit_time as microseconds since Jan 1, 1601
        // We compute a cutoff based on max_days
        let cutoff_us = (chrono::Utc::now() - chrono::Duration::days(max_days))
            .timestamp_micros()
            + 11_644_473_600_000_000; // Chromium epoch offset

        let rows = sqlx::query_as::<_, (String, String, i64)>(
            "SELECT COALESCE(title, ''), url, last_visit_time FROM urls \
             WHERE last_visit_time > ? AND url NOT LIKE 'chrome%' \
             ORDER BY last_visit_time DESC LIMIT 500",
        )
        .bind(cutoff_us)
        .fetch_all(&pool)
        .await
        .unwrap_or_default();

        let _ = std::fs::remove_file(&temp_db);

        Ok(rows
            .into_iter()
            .filter(|(title, _, _)| !title.is_empty())
            .map(|(title, url, visit_time)| {
                let ts_secs = (visit_time - 11_644_473_600_000_000) / 1_000_000;
                let visited_at = chrono::DateTime::from_timestamp(ts_secs, 0)
                    .map(|dt| dt.to_rfc3339())
                    .unwrap_or_default();
                HistoryEntry {
                    title,
                    url,
                    last_visit: visited_at,
                }
            })
            .collect())
    }
}

#[async_trait::async_trait]
impl super::SearchSource for BrowserHistorySource {
    fn name(&self) -> &str {
        "browser_history"
    }

    async fn search(&self, query: &str, limit: usize) -> Vec<LauncherItem> {
        let entries = self.entries.read();
        if entries.is_empty() || query.is_empty() {
            return vec![];
        }

        let query_lower = query.to_lowercase();
        entries
            .iter()
            .filter(|e| {
                e.title.to_lowercase().contains(&query_lower)
                    || e.url.to_lowercase().contains(&query_lower)
            })
            .take(limit)
            .map(|e| LauncherItem {
                id: format!("history:{}", e.url),
                title: e.title.clone(),
                subtitle: Some(e.url.clone()),
                icon: Some("globe".to_string()),
                kind: LauncherItemKind::BrowserHistory {
                    url: e.url.clone(),
                    visited_at: e.last_visit.clone(),
                },
                score: 0.5,
            })
            .collect()
    }

    async fn refresh(&self) {
        match Self::load_history(&self.browser, self.max_days).await {
            Ok(entries) => {
                tracing::info!("Loaded {} browser history entries", entries.len());
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
```

- [ ] **Step 2: Register module, build, commit**

Add `pub mod browser_history;` and `pub use browser_history::BrowserHistorySource;` to `search/mod.rs`.

Run: `cargo build -p feature-launcher`

```bash
git add crates/feature-launcher/src/search/
git commit -m "feat(launcher): add browser history search source"
```

---

## Chunk 5: Wire All Sources into Init + Frontend

### Task 5.1: Register all new sources in init_launcher

**Files:**
- Modify: `crates/app-core/src/init/launcher.rs`

- [ ] **Step 1: Add source registration for all new sources**

After the existing sources in `init_launcher()`, add:

```rust
    // System preferences
    if launcher_config.sources.system_prefs.enabled {
        let source = Arc::new(feature_launcher::SystemPrefsSource::new());
        let s = Arc::clone(&source);
        tokio::spawn(async move { s.refresh().await });
        sources.push(source);
    }

    // Brew packages
    if launcher_config.sources.brew.enabled {
        let source = Arc::new(feature_launcher::BrewSource::new());
        let s = Arc::clone(&source);
        tokio::spawn(async move { s.refresh().await });
        sources.push(source);
    }

    // SSH hosts
    if launcher_config.sources.ssh_hosts.enabled {
        let source = Arc::new(feature_launcher::SshHostsSource::new());
        let s = Arc::clone(&source);
        tokio::spawn(async move { s.refresh().await });
        sources.push(source);
    }

    // Git repos
    if launcher_config.sources.git_repos.enabled {
        let source = Arc::new(feature_launcher::GitReposSource::new(
            launcher_config.sources.git_repos.scan_dirs.clone(),
        ));
        let s = Arc::clone(&source);
        tokio::spawn(async move { s.refresh().await });
        sources.push(source);
    }

    // File search (mdfind)
    if launcher_config.sources.files.enabled {
        sources.push(Arc::new(feature_launcher::FileSearchSource::new()));
    }

    // Content grep (rg) — prefix ?
    if launcher_config.sources.content_grep.enabled {
        sources.push(Arc::new(feature_launcher::ContentGrepSource::new(
            launcher_config.sources.content_grep.default_scope.clone(),
        )));
    }

    // Contacts — prefix @
    if launcher_config.sources.contacts.enabled {
        sources.push(Arc::new(feature_launcher::ContactsSource::new()));
    }

    // Running apps
    if launcher_config.sources.running_apps.enabled {
        sources.push(Arc::new(feature_launcher::RunningAppsSource::new()));
    }

    // Browser bookmarks
    if launcher_config.sources.bookmarks.enabled {
        let source = Arc::new(feature_launcher::BookmarksSource::new(
            launcher_config.sources.bookmarks.browser.clone(),
        ));
        let s = Arc::clone(&source);
        tokio::spawn(async move { s.refresh().await });
        sources.push(source);
    }

    // Browser history
    if launcher_config.sources.browser_history.enabled {
        let source = Arc::new(feature_launcher::BrowserHistorySource::new(
            launcher_config.sources.browser_history.browser.clone(),
            launcher_config.sources.browser_history.max_days,
        ));
        let s = Arc::clone(&source);
        tokio::spawn(async move { s.refresh().await });
        sources.push(source);
    }
```

- [ ] **Step 2: Update feature-launcher lib.rs exports**

Ensure all new sources are re-exported from `lib.rs`:

```rust
pub use search::{
    AppEntry, AppIndex, BookmarksSource, BrewSource, Calculator, ContactsSource,
    ContentGrepSource, FileSearchSource, GitReposSource, RunningAppsSource, ScriptRunner,
    SearchSource, SshHostsSource, SourceRegistry, SystemCommands, SystemPrefsSource,
    BrowserHistorySource,
};
```

- [ ] **Step 3: Build workspace**

Run: `cargo build --workspace`
Expected: Compiles

- [ ] **Step 4: Commit**

```bash
git add crates/
git commit -m "feat(launcher): wire all search sources into init with config toggles"
```

### Task 5.2: Update frontend types

**Files:**
- Modify: `desktop-ui/src/features/launcher/types.ts`
- Modify: `desktop-ui/src/features/launcher/components/ResultsList.tsx`

- [ ] **Step 1: Add new LauncherItemKind variants to TypeScript**

In `types.ts`, add to the `LauncherItemKind` union:

```typescript
  | { type: "file"; path: string; kind: "file" | "folder" | "image" | "document" | "code" | "archive" }
  | { type: "contentMatch"; path: string; line: number; preview: string }
  | { type: "contact"; name: string; email: string | null; phone: string | null }
  | { type: "systemPref"; paneId: string }
  | { type: "runningApp"; pid: number; path: string }
  | { type: "bookmark"; url: string; browser: string }
  | { type: "browserHistory"; url: string; visitedAt: string }
  | { type: "brewPackage"; name: string; isCask: boolean }
  | { type: "sshHost"; host: string; user: string | null }
  | { type: "gitRepo"; path: string }
```

- [ ] **Step 2: Update ICON_MAP and KIND_LABELS in ResultsList.tsx**

Add entries for new kinds:

```typescript
const ICON_MAP: Record<string, string> = {
  application: "\uD83E\uDEDF",
  task: "\u2713",
  note: "\uD83D\uDCDD",
  clipboardEntry: "\uD83D\uDCCB",
  systemCommand: "\u2699\uFE0F",
  script: "\u25B6",
  calculator: "\uD83D\uDD22",
  calendar: "\uD83D\uDCC5",
  aiChat: "\u2728",
  file: "\uD83D\uDCC4",
  contentMatch: "\uD83D\uDD0D",
  contact: "\uD83D\uDC64",
  systemPref: "\u2699\uFE0F",
  runningApp: "\uD83D\uDFE2",
  bookmark: "\uD83D\uDD16",
  browserHistory: "\uD83C\uDF10",
  brewPackage: "\uD83C\uDF7A",
  sshHost: "\uD83D\uDD11",
  gitRepo: "\uD83D\uDCC2",
};

const KIND_LABELS: Record<string, string> = {
  application: "App",
  task: "Task",
  note: "Note",
  clipboardEntry: "Clip",
  systemCommand: "Cmd",
  script: "Script",
  calculator: "Calc",
  calendar: "Event",
  aiChat: "AI",
  file: "File",
  contentMatch: "Match",
  contact: "Contact",
  systemPref: "Pref",
  runningApp: "Running",
  bookmark: "Bookmark",
  browserHistory: "History",
  brewPackage: "Brew",
  sshHost: "SSH",
  gitRepo: "Repo",
};
```

- [ ] **Step 3: Run frontend build and lint**

Run: `cd desktop-ui && bun run build && bunx biome check src/features/launcher/`

- [ ] **Step 4: Commit**

```bash
git add desktop-ui/src/features/launcher/
git commit -m "feat(launcher): add frontend types and icons for all new search sources"
```

### Task 5.3: Integration verification

- [ ] **Step 1: Run full workspace build**

Run: `cargo build --workspace`
Expected: Compiles

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
Expected: Builds

- [ ] **Step 6: Commit any fixes**

```bash
git add .
git commit -m "fix(launcher): address lint and build issues from search sources integration"
```
