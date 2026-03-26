# Launcher File Search Redesign — Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Replace the slow `mdfind` file search with a pre-indexed, instant fuzzy-match file search scoped to user-configured directories.

**Architecture:** Walk user-configured directories on startup using the `ignore` crate (automatic `.gitignore` respect), store file paths in an in-memory `Vec<FileEntry>` behind `Arc<RwLock>`, fuzzy-match with `nucleo_matcher` on search (same pattern as `GitReposSource`, `BrewSource`, etc.). Periodic re-scan via `BackgroundRefresher`.

**Tech Stack:** `ignore` crate (directory walker with gitignore support), `nucleo-matcher` (fuzzy matching), `parking_lot::RwLock` (concurrent index access), `shellexpand` (tilde expansion).

**Spec:** `docs/superpowers/specs/2026-03-26-launcher-file-search-design.md`

---

### Task 1: Add `ignore` crate dependency

**Files:**
- Modify: `Cargo.toml` (workspace root, `[workspace.dependencies]` section)
- Modify: `crates/feature-launcher/Cargo.toml`

- [ ] **Step 1: Add `ignore` to workspace dependencies**

In the root `Cargo.toml`, add to the `[workspace.dependencies]` section (after line ~107, near `shellexpand`):

```toml
ignore = "0.4"
```

- [ ] **Step 2: Add `ignore` to feature-launcher's dependencies**

In `crates/feature-launcher/Cargo.toml`, add under `[dependencies]`:

```toml
ignore = { workspace = true }
```

- [ ] **Step 3: Verify it compiles**

Run: `cargo check -p feature-launcher`
Expected: compiles with no errors.

- [ ] **Step 4: Commit**

```bash
git add Cargo.toml crates/feature-launcher/Cargo.toml
git commit -m "chore(launcher): add ignore crate dependency for file indexing"
```

---

### Task 2: Add `FileSearchConfig` to config schema

**Files:**
- Modify: `crates/config/src/schema/launcher.rs`

- [ ] **Step 1: Write the `FileSearchConfig` struct**

Replace `files: SourceToggle` with a new config struct. In `crates/config/src/schema/launcher.rs`:

Add the default function near the top (alongside `default_scan_dirs`, `default_chrome`, etc.):

```rust
fn default_file_scan_dirs() -> Vec<String> {
    vec![
        "~/Projects".to_string(),
        "~/Documents".to_string(),
        "~/Desktop".to_string(),
    ]
}
fn default_120() -> u64 {
    120
}
```

Add the struct (after `ScriptsConfig`):

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct FileSearchConfig {
    pub enabled: bool,
    #[serde(default = "default_file_scan_dirs")]
    pub scan_dirs: Vec<String>,
    #[serde(default = "default_120")]
    pub refresh_interval_secs: u64,
}

impl Default for FileSearchConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            scan_dirs: default_file_scan_dirs(),
            refresh_interval_secs: 120,
        }
    }
}
```

Change the `files` field in `LauncherSourcesConfig` from:

```rust
pub files: SourceToggle,
```

to:

```rust
pub files: FileSearchConfig,
```

- [ ] **Step 2: Verify it compiles**

Run: `cargo check -p config`
Expected: compiles with no errors. Existing `{ "enabled": true }` in user config files will deserialize cleanly because all new fields have `#[serde(default)]`.

- [ ] **Step 3: Commit**

```bash
git add crates/config/src/schema/launcher.rs
git commit -m "feat(config): add FileSearchConfig with scan_dirs and refresh_interval"
```

---

### Task 3: Rewrite `FileSearchSource` with `ignore`-walk index

**Files:**
- Modify: `crates/feature-launcher/src/search/file_search.rs`

This is the core change. Replace the `mdfind` subprocess with an in-memory index that uses the `ignore` crate for directory walking and `nucleo_matcher` for fuzzy search.

- [ ] **Step 1: Write a test for the file walker**

Add a `#[cfg(test)] mod tests` at the bottom of `file_search.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;
    use std::fs;

    #[tokio::test]
    async fn test_index_and_search() {
        let dir = TempDir::new().unwrap();
        let base = dir.path();

        // Create test files
        fs::write(base.join("hello.rs"), "fn main() {}").unwrap();
        fs::write(base.join("world.txt"), "hello world").unwrap();
        fs::create_dir_all(base.join("sub")).unwrap();
        fs::write(base.join("sub/nested.rs"), "mod nested;").unwrap();

        // Create a node_modules dir that should be ignored
        fs::create_dir_all(base.join("node_modules")).unwrap();
        fs::write(base.join("node_modules/junk.js"), "junk").unwrap();

        let source = FileSearchSource::new(vec![base.to_string_lossy().to_string()]);
        source.refresh().await;

        // Search for "hello"
        let results = source.search("hello", 10).await;
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].title, "hello.rs");

        // Search for ".rs" files
        let results = source.search("rs", 10).await;
        assert!(results.len() >= 2); // hello.rs and nested.rs

        // node_modules/junk.js should NOT appear
        let results = source.search("junk", 10).await;
        assert!(results.is_empty());
    }

    #[tokio::test]
    async fn test_gitignore_respected() {
        let dir = TempDir::new().unwrap();
        let base = dir.path();

        // Create a .gitignore that ignores *.log files
        fs::write(base.join(".gitignore"), "*.log\nbuild/\n").unwrap();
        fs::write(base.join("app.rs"), "fn main() {}").unwrap();
        fs::write(base.join("debug.log"), "log output").unwrap();
        fs::create_dir_all(base.join("build")).unwrap();
        fs::write(base.join("build/output.bin"), "binary").unwrap();

        let source = FileSearchSource::new(vec![base.to_string_lossy().to_string()]);
        source.refresh().await;

        // app.rs should be found
        let results = source.search("app", 10).await;
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].title, "app.rs");

        // debug.log should NOT be found (gitignored)
        let results = source.search("debug", 10).await;
        assert!(results.is_empty());

        // build/output.bin should NOT be found (gitignored dir)
        let results = source.search("output", 10).await;
        assert!(results.is_empty());
    }

    #[tokio::test]
    async fn test_nonexistent_dir_skipped() {
        let source = FileSearchSource::new(vec!["/nonexistent/path/12345".to_string()]);
        source.refresh().await;
        let results = source.search("anything", 10).await;
        assert!(results.is_empty());
    }

    #[tokio::test]
    async fn test_empty_query_returns_nothing() {
        let source = FileSearchSource::new(vec![]);
        let results = source.search("", 10).await;
        assert!(results.is_empty());
    }
}
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo nextest run -p feature-launcher -E 'test(file_search)'`
Expected: FAIL — `FileSearchSource::new` doesn't accept `Vec<String>` yet.

- [ ] **Step 3: Rewrite `FileSearchSource` implementation**

Replace the entire contents of `crates/feature-launcher/src/search/file_search.rs` with:

```rust
use crate::types::*;
use ignore::WalkBuilder;
use parking_lot::RwLock;
use std::path::PathBuf;
use std::sync::Arc;

/// Hardcoded directory names to skip (applied globally, even outside git repos).
const SKIP_DIRS: &[&str] = &[
    "node_modules",
    "target",
    "__pycache__",
    ".cache",
    ".Trash",
    "Library",
];

/// Hardcoded file patterns to skip.
const SKIP_EXTENSIONS: &[&str] = &["pyc", "pyo", "class", "o", "obj", "dylib", "so"];

#[derive(Debug, Clone)]
struct FileEntry {
    name: String,
    path: String,
    extension: String,
    dir_index: usize,
}

pub struct FileSearchSource {
    entries: Arc<RwLock<Vec<FileEntry>>>,
    scan_dirs: Vec<String>,
}

impl FileSearchSource {
    pub fn new(scan_dirs: Vec<String>) -> Self {
        Self {
            entries: Arc::new(RwLock::new(Vec::new())),
            scan_dirs,
        }
    }

    fn walk_dirs(scan_dirs: &[String]) -> Vec<FileEntry> {
        let mut entries = Vec::new();

        for (dir_index, dir) in scan_dirs.iter().enumerate() {
            let expanded = shellexpand::tilde(dir).to_string();
            let path = std::path::Path::new(&expanded);
            if !path.exists() {
                tracing::warn!("file search: scan dir does not exist: {dir}");
                continue;
            }

            let walker = WalkBuilder::new(path)
                .hidden(true) // skip hidden files/dirs
                .git_ignore(true) // respect .gitignore
                .git_global(true) // respect global gitignore
                .git_exclude(true) // respect .git/info/exclude
                .build();

            for entry in walker {
                let entry = match entry {
                    Ok(e) => e,
                    Err(_) => continue, // permission error, skip
                };

                // Skip directories themselves — we only index files
                if entry.file_type().map_or(true, |ft| ft.is_dir()) {
                    continue;
                }

                let entry_path = entry.path();

                // Skip hardcoded dir patterns (for non-git dirs without .gitignore)
                if entry_path.components().any(|c| {
                    let s = c.as_os_str().to_string_lossy();
                    SKIP_DIRS.iter().any(|skip| s == *skip)
                }) {
                    continue;
                }

                // Skip hardcoded extensions
                let ext = entry_path
                    .extension()
                    .unwrap_or_default()
                    .to_string_lossy()
                    .to_string();
                if SKIP_EXTENSIONS.iter().any(|skip| ext == *skip) {
                    continue;
                }

                let name = entry_path
                    .file_name()
                    .unwrap_or_default()
                    .to_string_lossy()
                    .to_string();

                // Skip .DS_Store
                if name == ".DS_Store" {
                    continue;
                }

                entries.push(FileEntry {
                    name,
                    path: entry_path.to_string_lossy().to_string(),
                    extension: ext,
                    dir_index,
                });
            }
        }

        entries
    }

    fn classify_extension(ext: &str) -> FileKind {
        match ext {
            "png" | "jpg" | "jpeg" | "gif" | "svg" | "webp" | "ico" | "heic" => FileKind::Image,
            "pdf" | "doc" | "docx" | "txt" | "md" | "rtf" | "pages" | "odt" => FileKind::Document,
            "rs" | "ts" | "tsx" | "js" | "jsx" | "py" | "go" | "rb" | "java" | "c" | "cpp"
            | "h" | "swift" | "kt" | "sh" | "toml" | "yaml" | "json" | "html" | "css" => {
                FileKind::Code
            }
            "zip" | "tar" | "gz" | "bz2" | "xz" | "7z" | "rar" | "dmg" => FileKind::Archive,
            _ => FileKind::File,
        }
    }
}

#[async_trait::async_trait]
impl super::SearchSource for FileSearchSource {
    fn name(&self) -> &'static str {
        "files"
    }

    async fn search(&self, query: &str, limit: usize) -> Vec<LauncherItem> {
        if query.is_empty() {
            return vec![];
        }

        let entries = self.entries.read();
        let scored = super::fuzzy_match(query, &entries, |e| &e.name, limit);

        scored
            .into_iter()
            .map(|(score, entry)| {
                let kind = Self::classify_extension(&entry.extension);
                // Base score from nucleo (normalized to 0-1 range), boosted by dir priority
                let dir_boost = 1.0 - (entry.dir_index as f64 * 0.05).min(0.3);
                let final_score = (score as f64) / 1000.0 * 0.85 * dir_boost;

                LauncherItem {
                    id: format!("file:{}", entry.path),
                    title: entry.name.clone(),
                    subtitle: Some(entry.path.clone()),
                    icon: Some("file".to_string()),
                    kind: LauncherItemKind::File {
                        path: PathBuf::from(&entry.path),
                        kind,
                    },
                    score: final_score,
                }
            })
            .collect()
    }

    async fn refresh(&self) {
        let dirs = self.scan_dirs.clone();
        let entries = tokio::task::spawn_blocking(move || Self::walk_dirs(&dirs))
            .await
            .unwrap_or_default();
        tracing::info!("file search: indexed {} files", entries.len());
        *self.entries.write() = entries;
    }
}
```

- [ ] **Step 4: Run tests to verify they pass**

Run: `cargo nextest run -p feature-launcher -E 'test(file_search)'`
Expected: all 4 tests PASS.

- [ ] **Step 5: Commit**

```bash
git add crates/feature-launcher/src/search/file_search.rs
git commit -m "feat(launcher): replace mdfind with ignore-walk indexed file search"
```

---

### Task 4: Wire `FileSearchConfig` into `init_launcher()`

**Files:**
- Modify: `crates/app-core/src/init/launcher.rs`

- [ ] **Step 1: Update the `FileSearchSource` construction**

In `crates/app-core/src/init/launcher.rs`, replace the file search source block (lines 108-111):

```rust
    // File search (mdfind) — live query, cached by SourceRegistry
    if launcher_config.sources.files.enabled {
        sources.push(Arc::new(feature_launcher::FileSearchSource::new()));
    }
```

with:

```rust
    // File search — pre-indexed with ignore-walk, refreshed by BackgroundRefresher
    if launcher_config.sources.files.enabled {
        let source = Arc::new(feature_launcher::FileSearchSource::new(
            launcher_config.sources.files.scan_dirs.clone(),
        ));
        let s = Arc::clone(&source);
        tokio::spawn(async move { s.refresh().await });
        sources.push(source);
    }
```

- [ ] **Step 2: Register files with `BackgroundRefresher`**

In the same file, add a refresh entry for the files source. After the `git_repos` refresh entry block (around line 187), add:

```rust
    if let Some(s) = find_source("files") {
        let interval_secs = launcher_config.sources.files.refresh_interval_secs;
        refresh_entries.push(feature_launcher::RefreshEntry {
            source: s,
            interval: std::time::Duration::from_secs(interval_secs),
        });
    }
```

- [ ] **Step 3: Verify it compiles**

Run: `cargo check -p app-core`
Expected: compiles with no errors.

- [ ] **Step 4: Commit**

```bash
git add crates/app-core/src/init/launcher.rs
git commit -m "feat(launcher): wire FileSearchConfig into init_launcher with BackgroundRefresher"
```

---

### Task 5: Update settings UI

**Files:**
- Modify: `desktop-ui/src/features/settings/pages/LauncherSettings.tsx`

- [ ] **Step 1: Update the `LauncherData` type**

In `LauncherSettings.tsx`, change the `files` type in the `LauncherData` interface (line 21) from:

```ts
files?: { enabled?: boolean };
```

to:

```ts
files?: { enabled?: boolean; scanDirs?: string[]; refreshIntervalSecs?: number };
```

- [ ] **Step 2: Update the `SOURCE_DEFS` entry**

Change the Files entry in `SOURCE_DEFS` (line 64) from:

```ts
{ key: "files", label: "Files" },
```

to:

```ts
{
  key: "files",
  label: "Files",
  extraFields: [
    { key: "scanDirs", label: "Search directories", type: "dirs", placeholder: "~/Projects, ~/Documents, ~/Desktop" },
    { key: "refreshIntervalSecs", label: "Refresh interval (seconds)", type: "number" },
  ],
},
```

- [ ] **Step 3: Verify lint passes**

Run: `cd desktop-ui && bun run lint`
Expected: no new errors (existing warnings are fine).

- [ ] **Step 4: Commit**

```bash
git add desktop-ui/src/features/settings/pages/LauncherSettings.tsx
git commit -m "feat(ui): add file search directory and refresh interval settings"
```

---

### Task 6: Remove stale `cache_ttl` and verify full build

**Files:**
- Verify: `crates/feature-launcher/src/search/file_search.rs` (no `cache_ttl` override)
- Verify: full workspace build

The new `FileSearchSource` no longer overrides `cache_ttl()` (it returns `None` from the default trait impl), so the `SourceRegistry` won't cache its results — search goes directly to the in-memory index every time. This is correct since in-memory fuzzy match is microseconds.

- [ ] **Step 1: Verify `cache_ttl` is not present in the new code**

Read `crates/feature-launcher/src/search/file_search.rs` and confirm there is no `cache_ttl` method. The old `mdfind` version had `Some(Duration::from_secs(5))` — this should be gone.

- [ ] **Step 2: Full workspace build**

Run: `cargo build --workspace`
Expected: builds with no errors.

- [ ] **Step 3: Run all launcher tests**

Run: `cargo nextest run -p feature-launcher`
Expected: all tests pass.

- [ ] **Step 4: Run clippy**

Run: `cargo clippy -p feature-launcher -p config -p app-core --all-targets`
Expected: no warnings.

- [ ] **Step 5: Commit (if any fixups needed)**

Only commit if clippy or tests required changes. Otherwise skip this step.
