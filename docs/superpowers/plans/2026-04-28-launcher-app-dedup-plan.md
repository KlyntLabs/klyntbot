# Launcher App Deduplication Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Eliminate duplicate app rows in launcher search by making `AppIndex` the sole identity owner and converting `RunningAppsSource` + `AttentionSource` into bundle-ID-keyed signal producers that decorate the unified row.

**Architecture:** Decorator pattern. Two shared `Arc<DashMap<SmolStr, …>>` signal maps (`RunningSignals`, `AttentionSignals`) flow from the wiring layer (`app-core/init/launcher.rs`) into all three sources. `AppIndex.search` joins the signals at query time. `RunningAppsSource.search` returns empty (refresh-only). `AttentionSource` filters `kind=app` rows into signals and emits only `kind=site` items.

**Tech Stack:** Rust 1.93 stable, `dashmap` (already a workspace dep), `smol_str`, `sqlx` (SQLite), `criterion` for benches, `tokio::test` for async tests, `tempfile::TempDir` for filesystem-backed tests, `PlistBuddy` subprocess for `Info.plist` parsing.

---

## File Structure

### New files
- `crates/feature-launcher/src/search/signals.rs` — `RunningSignal`, `AttentionStat`, `RunningSignals`, `AttentionSignals` type aliases
- `crates/feature-launcher/src/migration.rs` — `migrate_app_ids_to_bundle_ids()` one-shot
- `crates/platform-macos/src/apps.rs` — extend with `read_bundle_id(path) -> Option<String>` free function
- `crates/feature-launcher/tests/app_dedup_test.rs` — composition integration test
- `crates/feature-launcher/tests/pin_migration_test.rs` — migration round-trip test
- `crates/feature-launcher/benches/app_index_dedup.rs` — Criterion benchmark suite

### Modified files
- `crates/feature-launcher/src/search/mod.rs` — `pub mod signals;` + re-exports
- `crates/feature-launcher/src/search/app_index.rs` — populate `bundle_id`, dedupe, signal-join in `search`, ID format switch, CoreServices dirs
- `crates/feature-launcher/src/search/running_apps.rs` — replace impl with signal producer
- `crates/feature-launcher/src/search/attention.rs` — push app rows to signals, suppress emission
- `crates/feature-launcher/src/lib.rs` — re-export `RunningSignals`, `AttentionSignals`, `migrate_app_ids_to_bundle_ids`
- `crates/app-core/src/init/launcher.rs:42-50,155-161,355-356` — construct signal maps, share via builder, run migration after first index

---

## Conventions used in this plan

- Test commands assume the workspace root `/Users/jayden/Projects/Klynt/bot/`. All paths are relative to that root unless prefixed with `/`.
- Every code step shows the **complete content** of the section being added/changed (not a fragment).
- Every test step is a separate commit so a regression isolates cleanly.
- Run `cargo nextest run -p feature-launcher` after each task; it must stay green.

---

## Task 1: Create signal type aliases

**Files:**
- Create: `crates/feature-launcher/src/search/signals.rs`
- Modify: `crates/feature-launcher/src/search/mod.rs:1-22`
- Modify: `crates/feature-launcher/src/lib.rs:14-23`

- [ ] **Step 1.1: Write the failing test**

Append to the bottom of the new `signals.rs` file (created next step). Defer to step 1.3 — Rust requires the file to exist for the test module to compile. Skip ahead to 1.3, then return for steps 1.4–1.6.

- [ ] **Step 1.2: Create `signals.rs` with full content**

Create `crates/feature-launcher/src/search/signals.rs`:

```rust
//! Bundle-ID-keyed signal maps shared between launcher sources.
//!
//! `AppIndex` consumes these maps at query time to decorate the unified
//! `Application` row with live "is running" + cumulative attention data.
//! `RunningAppsSource` writes into `RunningSignals` on each refresh.
//! `AttentionSource` writes into `AttentionSignals` from inside `search`.

use dashmap::DashMap;
use smol_str::SmolStr;
use std::path::PathBuf;
use std::sync::Arc;

/// Live "is running" snapshot for a single app.
#[derive(Clone, Debug)]
pub struct RunningSignal {
    pub pid: u32,
    pub path: PathBuf,
}

/// Cumulative time-tracking stats for a single app.
#[derive(Clone, Debug)]
pub struct AttentionStat {
    pub attention_secs: i64,
    pub category: Option<SmolStr>,
    pub last_used_at: jiff::Timestamp,
}

/// Bundle ID → live "is running" snapshot. Refreshed by RunningAppsSource.
pub type RunningSignals = Arc<DashMap<SmolStr, RunningSignal>>;

/// Bundle ID → cumulative time-tracking stats. Updated by AttentionSource.
pub type AttentionSignals = Arc<DashMap<SmolStr, AttentionStat>>;

/// Helper: construct an empty `RunningSignals` map.
pub fn new_running_signals() -> RunningSignals {
    Arc::new(DashMap::new())
}

/// Helper: construct an empty `AttentionSignals` map.
pub fn new_attention_signals() -> AttentionSignals {
    Arc::new(DashMap::new())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn running_signals_round_trip() {
        let signals = new_running_signals();
        let bid = SmolStr::new("com.apple.Safari");
        signals.insert(
            bid.clone(),
            RunningSignal {
                pid: 99,
                path: PathBuf::from("/Applications/Safari.app"),
            },
        );
        let got = signals.get(&bid).expect("expected Safari signal");
        assert_eq!(got.pid, 99);
    }

    #[test]
    fn attention_stat_serializes_timestamp() {
        let stat = AttentionStat {
            attention_secs: 3600,
            category: Some(SmolStr::new("browsing")),
            last_used_at: "2026-04-28T12:00:00Z".parse().unwrap(),
        };
        assert_eq!(stat.attention_secs, 3600);
        assert_eq!(stat.category.as_deref(), Some("browsing"));
    }

    #[test]
    fn running_signals_retain_drops_missing_keys() {
        let signals = new_running_signals();
        signals.insert(
            SmolStr::new("com.apple.Safari"),
            RunningSignal { pid: 1, path: PathBuf::new() },
        );
        signals.insert(
            SmolStr::new("com.apple.Mail"),
            RunningSignal { pid: 2, path: PathBuf::new() },
        );

        let live: std::collections::HashSet<SmolStr> =
            std::iter::once(SmolStr::new("com.apple.Safari")).collect();
        signals.retain(|k, _| live.contains(k));

        assert_eq!(signals.len(), 1);
        assert!(signals.contains_key(&SmolStr::new("com.apple.Safari")));
        assert!(!signals.contains_key(&SmolStr::new("com.apple.Mail")));
    }
}
```

- [ ] **Step 1.3: Register the new module**

Edit `crates/feature-launcher/src/search/mod.rs`. After line 22 (`pub mod window_presets;`), add:

```rust
pub mod signals;
```

After the `pub use ...;` block (around line 44), add:

```rust
pub use signals::{
    new_attention_signals, new_running_signals, AttentionSignals, AttentionStat, RunningSignal,
    RunningSignals,
};
```

- [ ] **Step 1.4: Run the new tests, expect failure (compilation only — they should pass once compiled)**

Run:

```bash
cargo nextest run -p feature-launcher signals::tests
```

Expected: **3 passes** (tests are self-contained data-structure round-trips). If any fail, debug before continuing.

- [ ] **Step 1.5: Re-export from `lib.rs`**

`search::*` already re-exports everything from `search::mod`, so the new types are reachable as `feature_launcher::RunningSignals` etc. Verify:

```bash
cargo check -p feature-launcher
```

Expected: 0 errors, 0 warnings.

- [ ] **Step 1.6: Commit**

```bash
git add crates/feature-launcher/src/search/signals.rs crates/feature-launcher/src/search/mod.rs
git commit -m "$(cat <<'EOF'
feat(launcher): add bundle-id-keyed signal type aliases

RunningSignals + AttentionSignals are Arc<DashMap<SmolStr, ...>> aliases
shared between AppIndex (consumer) and RunningAppsSource +
AttentionSource (producers). Empty constructors keep the wiring layer
free of dashmap/Arc imports.

Co-Authored-By: Claude Opus 4.7 (1M context) <noreply@anthropic.com>
EOF
)"
```

---

## Task 2: Add `read_bundle_id` to platform-macos

**Files:**
- Modify: `crates/platform-macos/src/apps.rs:181` (append after `activate_app` non-macOS shim)

The existing `AppIconCache::resolve_icon_path` already shells out to `PlistBuddy` for `CFBundleIconFile`. We mirror that pattern for `CFBundleIdentifier` to avoid adding a CFBundle/objc2 dependency.

- [ ] **Step 2.1: Write the failing test**

Append to the bottom of `crates/platform-macos/src/apps.rs`:

```rust
#[cfg(test)]
mod bundle_id_tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    fn make_fake_app(dir: &Path, name: &str, bundle_id: Option<&str>) -> PathBuf {
        let app_path = dir.join(format!("{name}.app"));
        let contents = app_path.join("Contents");
        fs::create_dir_all(&contents).unwrap();
        let plist = match bundle_id {
            Some(bid) => format!(
                "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n\
                 <!DOCTYPE plist PUBLIC \"-//Apple//DTD PLIST 1.0//EN\" \"http://www.apple.com/DTDs/PropertyList-1.0.dtd\">\n\
                 <plist version=\"1.0\">\n\
                 <dict>\n\
                 \t<key>CFBundleIdentifier</key>\n\
                 \t<string>{bid}</string>\n\
                 </dict>\n\
                 </plist>\n"
            ),
            None => "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n\
                     <plist version=\"1.0\"><dict></dict></plist>\n".to_string(),
        };
        fs::write(contents.join("Info.plist"), plist).unwrap();
        app_path
    }

    #[test]
    #[cfg(target_os = "macos")]
    fn reads_bundle_id_from_plist() {
        let dir = TempDir::new().unwrap();
        let app = make_fake_app(dir.path(), "Foo", Some("com.example.Foo"));
        assert_eq!(read_bundle_id(&app), Some("com.example.Foo".to_string()));
    }

    #[test]
    #[cfg(target_os = "macos")]
    fn returns_none_when_plist_lacks_identifier() {
        let dir = TempDir::new().unwrap();
        let app = make_fake_app(dir.path(), "NoBundle", None);
        assert_eq!(read_bundle_id(&app), None);
    }

    #[test]
    #[cfg(target_os = "macos")]
    fn returns_none_when_plist_missing() {
        let dir = TempDir::new().unwrap();
        let app = dir.path().join("Empty.app");
        std::fs::create_dir_all(app.join("Contents")).unwrap();
        // No Info.plist written
        assert_eq!(read_bundle_id(&app), None);
    }
}
```

- [ ] **Step 2.2: Verify `tempfile` is a dev-dep of platform-macos, add if missing**

Run:

```bash
grep -n "tempfile" crates/platform-macos/Cargo.toml
```

If absent, add under `[dev-dependencies]`:

```toml
tempfile.workspace = true
```

(`tempfile` is in the root `Cargo.toml` workspace deps — used by `feature-launcher/benches/inverted_index.rs`.)

- [ ] **Step 2.3: Run tests, expect compilation failure**

Run:

```bash
cargo nextest run -p platform-macos bundle_id_tests
```

Expected: **compilation error**, `read_bundle_id` not found.

- [ ] **Step 2.4: Implement `read_bundle_id`**

Append to `crates/platform-macos/src/apps.rs` (before the `#[cfg(test)] mod bundle_id_tests` block):

```rust
/// Read `CFBundleIdentifier` from an app bundle's `Info.plist` via PlistBuddy.
///
/// Returns `None` if:
/// - The app bundle has no `Contents/Info.plist`
/// - The plist exists but has no `CFBundleIdentifier` key
/// - PlistBuddy fails for any other reason (malformed plist, permission, …)
///
/// This mirrors the PlistBuddy approach used by `AppIconCache::resolve_icon_path`
/// — keeps the call out of NSWorkspace / CFBundle Objective-C bridges, so it
/// will not contribute to the IconServices mmap leak.
#[cfg(target_os = "macos")]
pub fn read_bundle_id(app_path: &Path) -> Option<String> {
    use std::process::Command;

    let plist_path = app_path.join("Contents/Info.plist");
    if !plist_path.exists() {
        return None;
    }

    let output = Command::new("/usr/libexec/PlistBuddy")
        .args([
            "-c",
            "Print :CFBundleIdentifier",
            &plist_path.to_string_lossy(),
        ])
        .output()
        .ok()?;

    if !output.status.success() {
        return None;
    }

    let bid = String::from_utf8_lossy(&output.stdout).trim().to_string();
    if bid.is_empty() {
        None
    } else {
        Some(bid)
    }
}

#[cfg(not(target_os = "macos"))]
pub fn read_bundle_id(_app_path: &Path) -> Option<String> {
    None
}
```

- [ ] **Step 2.5: Run tests, expect pass**

Run:

```bash
cargo nextest run -p platform-macos bundle_id_tests
```

Expected: **3 passes** (on macOS). On non-macOS, tests are `#[cfg(target_os = "macos")]`-gated and skipped.

- [ ] **Step 2.6: Commit**

```bash
git add crates/platform-macos/src/apps.rs crates/platform-macos/Cargo.toml
git commit -m "$(cat <<'EOF'
feat(platform-macos): add read_bundle_id via PlistBuddy

Mirrors the AppIconCache PlistBuddy approach for CFBundleIdentifier
extraction. Avoids adding a CFBundle/objc2 dependency and stays out
of the NSWorkspace IconServices mmap path.

Co-Authored-By: Claude Opus 4.7 (1M context) <noreply@anthropic.com>
EOF
)"
```

---

## Task 3: Populate `AppEntry.bundle_id` during indexing + dedupe

**Files:**
- Modify: `crates/feature-launcher/src/search/app_index.rs:6-29` (struct + `from_path`)
- Modify: `crates/feature-launcher/src/search/app_index.rs:99-152` (indexing path)
- Modify: `crates/feature-launcher/src/search/app_index.rs:175-228` (extend tests)

- [ ] **Step 3.1: Write failing tests for bundle_id population + dedup**

Add to the `#[cfg(test)] mod tests` block at the bottom of `crates/feature-launcher/src/search/app_index.rs`:

```rust
    #[test]
    #[cfg(target_os = "macos")]
    fn index_populates_bundle_id_from_plist() {
        use std::fs;
        let dir = tempfile::TempDir::new().unwrap();
        let app_path = dir.path().join("Foo.app");
        let contents = app_path.join("Contents");
        fs::create_dir_all(&contents).unwrap();
        fs::write(
            contents.join("Info.plist"),
            "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n\
             <plist version=\"1.0\"><dict>\n\
             <key>CFBundleIdentifier</key><string>com.example.Foo</string>\n\
             </dict></plist>\n",
        ).unwrap();

        let apps = AppIndex::walk_apps(dir.path(), 3).unwrap();
        let apps_with_bid: Vec<_> = apps.iter().filter_map(|a| {
            let bid = platform_macos::apps::read_bundle_id(&a.path)?;
            Some((a.name.clone(), bid))
        }).collect();
        assert_eq!(apps_with_bid, vec![("Foo".to_string(), "com.example.Foo".to_string())]);
    }

    #[test]
    fn dedupe_by_bundle_id_keeps_user_path() {
        let apps = vec![
            AppEntry {
                name: "Safari".into(),
                path: "/System/Applications/Safari.app".into(),
                bundle_id: Some("com.apple.Safari".into()),
                icon_path: None,
            },
            AppEntry {
                name: "Safari".into(),
                path: "/Applications/Safari.app".into(),
                bundle_id: Some("com.apple.Safari".into()),
                icon_path: None,
            },
        ];
        let deduped = AppIndex::dedupe_by_bundle_id(apps);
        assert_eq!(deduped.len(), 1);
        assert_eq!(
            deduped[0].path,
            std::path::PathBuf::from("/Applications/Safari.app")
        );
    }

    #[test]
    fn dedupe_keeps_path_keyed_entries() {
        let apps = vec![
            AppEntry {
                name: "Foo".into(),
                path: "/Applications/Foo.app".into(),
                bundle_id: None,
                icon_path: None,
            },
            AppEntry {
                name: "Bar".into(),
                path: "/Applications/Bar.app".into(),
                bundle_id: None,
                icon_path: None,
            },
        ];
        let deduped = AppIndex::dedupe_by_bundle_id(apps);
        assert_eq!(deduped.len(), 2, "path-keyed entries cannot dupe by bundle");
    }
```

- [ ] **Step 3.2: Update `AppEntry` struct to use `SmolStr` for bundle_id**

In `crates/feature-launcher/src/search/app_index.rs`, replace lines 6–29 with:

```rust
use crate::types::*;
use parking_lot::RwLock;
use smol_str::SmolStr;
use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::sync::Arc;

#[derive(Debug, Clone)]
pub struct AppEntry {
    pub name: String,
    pub path: PathBuf,
    pub bundle_id: Option<SmolStr>,
    /// Path to the cached 64x64 PNG icon (resolved via sips).
    pub icon_path: Option<PathBuf>,
}

impl AppEntry {
    pub fn from_path(path: &Path) -> Option<Self> {
        let ext = path.extension()?.to_str()?;
        if ext != "app" {
            return None;
        }
        let name = path.file_stem()?.to_string_lossy().to_string();
        Some(Self {
            name,
            path: path.to_path_buf(),
            bundle_id: None,
            icon_path: None,
        })
    }
}
```

The only behavior change: `bundle_id: Option<SmolStr>` instead of `Option<String>`.

- [ ] **Step 3.3: Make `walk_apps` `pub(crate)` and add `dedupe_by_bundle_id`**

In `crates/feature-launcher/src/search/app_index.rs:127`, change:

```rust
    fn walk_apps(dir: &Path, max_depth: usize) -> std::io::Result<Vec<AppEntry>> {
```

to:

```rust
    pub(crate) fn walk_apps(dir: &Path, max_depth: usize) -> std::io::Result<Vec<AppEntry>> {
```

After the `walk_apps` function closing brace (around line 146), insert:

```rust
    /// Dedupe app entries by bundle_id, preferring `/Applications` over system / nested paths.
    /// Apps without a bundle_id are kept as-is (their identity is path-based).
    pub(crate) fn dedupe_by_bundle_id(mut apps: Vec<AppEntry>) -> Vec<AppEntry> {
        apps.sort_by_key(|a| match a.path.starts_with("/Applications") {
            true => 0,
            false => 1,
        });
        let mut seen: HashSet<SmolStr> = HashSet::new();
        apps.retain(|a| match &a.bundle_id {
            Some(b) => seen.insert(b.clone()),
            None => true,
        });
        apps
    }
```

- [ ] **Step 3.4: Populate bundle_id during `index_applications`**

Replace the body of `index_applications` (lines 100–124) with:

```rust
    #[cfg(target_os = "macos")]
    pub async fn index_applications(&self) {
        let dirs = [
            "/Applications",
            "/System/Applications",
            "/System/Library/CoreServices",
            "/System/Library/CoreServices/Applications",
        ];
        let home = std::env::var("HOME").unwrap_or_default();
        let user_apps = format!("{}/Applications", home);

        let mut apps = Vec::new();
        for dir in dirs.iter().chain(std::iter::once(&user_apps.as_str())) {
            // CoreServices nests one level deeper than /Applications.
            let max_depth = if dir.starts_with("/System/Library/CoreServices") {
                4
            } else {
                3
            };
            if let Ok(entries) = Self::walk_apps(Path::new(dir), max_depth) {
                apps.extend(entries);
            }
        }

        // Populate bundle_id from Info.plist for every app.
        for app in &mut apps {
            app.bundle_id = platform_macos::apps::read_bundle_id(&app.path).map(SmolStr::new);
        }

        // Dedupe by bundle_id, preferring /Applications over system locations.
        apps = Self::dedupe_by_bundle_id(apps);

        if let Some(cache) = &self.icon_cache {
            for app in &mut apps {
                app.icon_path = cache.resolve_icon_path(&app.path);
            }
        }
        let icon_count = apps.iter().filter(|a| a.icon_path.is_some()).count();
        let bundle_count = apps.iter().filter(|a| a.bundle_id.is_some()).count();
        tracing::info!(
            "Indexed {} applications ({} with icons, {} with bundle ids)",
            apps.len(),
            icon_count,
            bundle_count
        );
        self.set_apps(apps);
    }
```

- [ ] **Step 3.5: Verify `platform-macos` is already a dep of feature-launcher**

```bash
grep -n "platform-macos\|platform_macos" crates/feature-launcher/Cargo.toml
```

If absent, add under `[dependencies]`:

```toml
platform-macos.workspace = true
```

- [ ] **Step 3.6: Run tests, expect pass**

```bash
cargo nextest run -p feature-launcher app_index::
```

Expected: **all existing tests + 3 new tests pass**. The pre-existing `test_fuzzy_search` and `test_search_empty_returns_none` still pass because they use `Option<SmolStr>` implicitly (via the `bundle_id: None` in the literal).

If `test_fuzzy_search` fails to compile because of `bundle_id: None`, change those test literals (lines 192–209, 219–223) — `None` works for both `Option<String>` and `Option<SmolStr>`, so this should be a no-op. If a literal explicitly uses `"...".to_string()` for bundle_id, change to `SmolStr::new("...")`.

- [ ] **Step 3.7: Commit**

```bash
git add crates/feature-launcher/src/search/app_index.rs crates/feature-launcher/Cargo.toml
git commit -m "$(cat <<'EOF'
feat(launcher): populate AppEntry.bundle_id and dedupe by it

Indexing now reads CFBundleIdentifier via PlistBuddy for every
discovered .app, then dedupes apps that report the same bundle ID
(e.g. Safari at /Applications + /System/Applications), preferring
/Applications. Walker also covers CoreServices for Finder/Dock-style
system apps.

Co-Authored-By: Claude Opus 4.7 (1M context) <noreply@anthropic.com>
EOF
)"
```

---

## Task 4: Add signal map fields + builder methods to `AppIndex`

**Files:**
- Modify: `crates/feature-launcher/src/search/app_index.rs:31-56` (struct + constructors)

- [ ] **Step 4.1: Write failing test for builder + getter**

Add to the `#[cfg(test)] mod tests` block:

```rust
    #[test]
    fn with_signals_attaches_maps() {
        use crate::search::signals::{new_attention_signals, new_running_signals};
        let running = new_running_signals();
        let attention = new_attention_signals();
        let idx = AppIndex::new()
            .with_running_signals(running.clone())
            .with_attention_signals(attention.clone());

        assert!(Arc::ptr_eq(&running, &idx.running_signals_for_test()));
        assert!(Arc::ptr_eq(&attention, &idx.attention_signals_for_test()));
    }
```

- [ ] **Step 4.2: Run, expect compilation failure**

```bash
cargo nextest run -p feature-launcher with_signals_attaches_maps
```

Expected: **errors** — `with_running_signals`, `with_attention_signals`, `*_for_test` not found.

- [ ] **Step 4.3: Add fields and builder methods**

Replace lines 31–56 of `crates/feature-launcher/src/search/app_index.rs` with:

```rust
use crate::search::signals::{
    new_attention_signals, new_running_signals, AttentionSignals, RunningSignals,
};

#[derive(Clone)]
pub struct AppIndex {
    apps: Arc<RwLock<Vec<AppEntry>>>,
    /// Shared icon cache backed by `platform_macos::apps::AppIconCache`.
    icon_cache: Option<Arc<platform_macos::apps::AppIconCache>>,
    /// Bundle-ID-keyed live "is running" map, refreshed by RunningAppsSource.
    running_signals: RunningSignals,
    /// Bundle-ID-keyed cumulative attention stats, written by AttentionSource.
    attention_signals: AttentionSignals,
}

impl AppIndex {
    pub fn new() -> Self {
        Self {
            apps: Arc::new(RwLock::new(Vec::new())),
            icon_cache: None,
            running_signals: new_running_signals(),
            attention_signals: new_attention_signals(),
        }
    }

    pub fn with_cache_dir(cache_dir: PathBuf) -> Self {
        Self {
            apps: Arc::new(RwLock::new(Vec::new())),
            icon_cache: Some(Arc::new(platform_macos::apps::AppIconCache::new(cache_dir))),
            running_signals: new_running_signals(),
            attention_signals: new_attention_signals(),
        }
    }

    /// Builder: attach an externally-owned RunningSignals map (so other sources
    /// can write into it). Without this call, `AppIndex` owns a private empty map.
    pub fn with_running_signals(mut self, signals: RunningSignals) -> Self {
        self.running_signals = signals;
        self
    }

    /// Builder: attach an externally-owned AttentionSignals map.
    pub fn with_attention_signals(mut self, signals: AttentionSignals) -> Self {
        self.attention_signals = signals;
        self
    }

    pub fn icon_cache(&self) -> Option<Arc<platform_macos::apps::AppIconCache>> {
        self.icon_cache.clone()
    }

    pub fn running_signals(&self) -> RunningSignals {
        Arc::clone(&self.running_signals)
    }

    pub fn attention_signals(&self) -> AttentionSignals {
        Arc::clone(&self.attention_signals)
    }

    #[cfg(test)]
    pub fn running_signals_for_test(&self) -> RunningSignals {
        Arc::clone(&self.running_signals)
    }

    #[cfg(test)]
    pub fn attention_signals_for_test(&self) -> AttentionSignals {
        Arc::clone(&self.attention_signals)
    }

    pub fn set_apps(&self, apps: Vec<AppEntry>) {
        *self.apps.write() = apps;
    }
```

(Note: The `set_apps` method existed before — keep it. The block above preserves the original `set_apps` definition at the end.)

- [ ] **Step 4.4: Run tests, expect pass**

```bash
cargo nextest run -p feature-launcher app_index::
```

Expected: all `app_index` tests pass, including the new `with_signals_attaches_maps`.

- [ ] **Step 4.5: Commit**

```bash
git add crates/feature-launcher/src/search/app_index.rs
git commit -m "$(cat <<'EOF'
feat(launcher): add signal map fields and builder methods to AppIndex

Defaults to private empty maps so existing callers (with_cache_dir, new)
keep working. with_running_signals/with_attention_signals let the
wiring layer share maps across sources.

Co-Authored-By: Claude Opus 4.7 (1M context) <noreply@anthropic.com>
EOF
)"
```

---

## Task 5: Modify `AppIndex::search` to join signals and emit unified items

**Files:**
- Modify: `crates/feature-launcher/src/search/app_index.rs:61-97` (the `search` method)

- [ ] **Step 5.1: Write failing tests for signal join + boost + ID switch**

Add to the `#[cfg(test)] mod tests` block:

```rust
    #[test]
    fn search_joins_running_signal_and_marks_running() {
        use crate::search::signals::RunningSignal;
        let idx = AppIndex::new();
        idx.set_apps(vec![AppEntry {
            name: "Safari".into(),
            path: "/Applications/Safari.app".into(),
            bundle_id: Some(SmolStr::new("com.apple.Safari")),
            icon_path: None,
        }]);
        idx.running_signals_for_test().insert(
            SmolStr::new("com.apple.Safari"),
            RunningSignal { pid: 99, path: "/Applications/Safari.app".into() },
        );

        let results = idx.search("safari", 5);
        assert_eq!(results.len(), 1);
        match &results[0].kind {
            LauncherItemKind::Application { running, .. } => assert!(*running),
            other => panic!("expected Application kind, got {other:?}"),
        }
    }

    #[test]
    fn search_emits_bundle_id_keyed_id_when_present() {
        let idx = AppIndex::new();
        idx.set_apps(vec![AppEntry {
            name: "Safari".into(),
            path: "/Applications/Safari.app".into(),
            bundle_id: Some(SmolStr::new("com.apple.Safari")),
            icon_path: None,
        }]);
        let results = idx.search("safari", 5);
        assert_eq!(results[0].id, "app:com.apple.Safari");
    }

    #[test]
    fn search_falls_back_to_path_id_when_bundle_id_missing() {
        let idx = AppIndex::new();
        idx.set_apps(vec![AppEntry {
            name: "WeirdCli".into(),
            path: "/Applications/WeirdCli.app".into(),
            bundle_id: None,
            icon_path: None,
        }]);
        let results = idx.search("weird", 5);
        assert_eq!(results[0].id, "app:/Applications/WeirdCli.app");
    }

    #[test]
    fn search_boost_compounds_when_both_signals_present() {
        use crate::search::signals::{AttentionStat, RunningSignal};
        let idx = AppIndex::new();
        idx.set_apps(vec![AppEntry {
            name: "Safari".into(),
            path: "/Applications/Safari.app".into(),
            bundle_id: Some(SmolStr::new("com.apple.Safari")),
            icon_path: None,
        }]);

        let baseline = idx.search("safari", 5)[0].score;

        idx.running_signals_for_test().insert(
            SmolStr::new("com.apple.Safari"),
            RunningSignal { pid: 1, path: PathBuf::new() },
        );
        let with_running = idx.search("safari", 5)[0].score;

        idx.attention_signals_for_test().insert(
            SmolStr::new("com.apple.Safari"),
            AttentionStat {
                attention_secs: 3600,
                category: Some(SmolStr::new("browsing")),
                last_used_at: jiff::Timestamp::now(),
            },
        );
        let with_both = idx.search("safari", 5)[0].score;

        assert!(with_running > baseline, "running signal must boost score");
        assert!(with_both > with_running, "attention signal must compound");
    }

    #[test]
    fn search_subtitle_includes_running_marker() {
        use crate::search::signals::RunningSignal;
        let idx = AppIndex::new();
        idx.set_apps(vec![AppEntry {
            name: "Safari".into(),
            path: "/Applications/Safari.app".into(),
            bundle_id: Some(SmolStr::new("com.apple.Safari")),
            icon_path: None,
        }]);
        idx.running_signals_for_test().insert(
            SmolStr::new("com.apple.Safari"),
            RunningSignal { pid: 1, path: PathBuf::new() },
        );

        let results = idx.search("safari", 5);
        let subtitle = results[0].subtitle.as_deref().unwrap();
        assert!(subtitle.contains("Running"), "subtitle must contain 'Running', got: {subtitle:?}");
    }
```

- [ ] **Step 5.2: Run, expect failures**

```bash
cargo nextest run -p feature-launcher app_index::
```

Expected: all 5 new tests fail (signals are not joined yet, ID is path-based, no boost applied).

- [ ] **Step 5.3: Replace `AppIndex::search`**

In `crates/feature-launcher/src/search/app_index.rs`, replace the existing `search` method (lines 61–97 of the original; now slightly later due to earlier additions) with:

```rust
    pub fn search(&self, query: &str, limit: usize) -> Vec<LauncherItem> {
        let apps = self.apps.read();
        let scored = super::fuzzy_match(query, &apps, |app| &app.name, limit);

        scored
            .into_iter()
            .map(|(score, app)| {
                let bid = app.bundle_id.as_ref();

                let running = bid.and_then(|b| {
                    self.running_signals.get(b).map(|r| r.value().clone())
                });
                let attention = bid.and_then(|b| {
                    self.attention_signals.get(b).map(|s| s.value().clone())
                });

                let icon = app
                    .icon_path
                    .as_ref()
                    .and_then(|p| {
                        let bytes = std::fs::read(p).ok()?;
                        use base64::Engine;
                        let b64 = base64::engine::general_purpose::STANDARD.encode(&bytes);
                        Some(format!("data:image/png;base64,{b64}"))
                    })
                    .or_else(|| Some("app-window".to_string()));

                let id = match bid {
                    Some(b) => format!("app:{b}"),
                    None => format!("app:{}", app.path.display()),
                };

                let subtitle = compose_subtitle(&app.path, running.as_ref(), attention.as_ref());
                let boost = score_boost(running.as_ref(), attention.as_ref());

                LauncherItem {
                    id,
                    title: app.name.clone(),
                    subtitle: Some(subtitle),
                    icon,
                    kind: LauncherItemKind::Application {
                        path: app.path.clone(),
                        running: running.is_some(),
                    },
                    score: (score as f64 / 1000.0) * boost,
                    no_view: false,
                    arguments: vec![],
                    pinned: false,
                }
            })
            .collect()
    }
}
```

- [ ] **Step 5.4: Add `compose_subtitle` and `score_boost` helpers (free functions in same file)**

Append to `crates/feature-launcher/src/search/app_index.rs`, after the `impl AppIndex` block but before `impl Default for AppIndex`:

```rust
fn compose_subtitle(
    path: &Path,
    running: Option<&crate::search::signals::RunningSignal>,
    attention: Option<&crate::search::signals::AttentionStat>,
) -> String {
    let mut parts: Vec<String> = Vec::with_capacity(3);
    if running.is_some() {
        parts.push("Running".to_string());
    }
    if let Some(a) = attention {
        parts.push(format_attention(a.attention_secs));
        if let Some(cat) = a.category.as_ref() {
            parts.push(cat.to_string());
        }
    }
    if parts.is_empty() {
        path.display().to_string()
    } else {
        // "Running · 1h 23m · browsing"
        parts.join(" · ")
    }
}

fn format_attention(secs: i64) -> String {
    let secs = secs.max(0);
    let hours = secs / 3600;
    let minutes = (secs % 3600) / 60;
    if hours > 0 {
        format!("{hours}h {minutes}m")
    } else {
        format!("{minutes}m")
    }
}

/// Multiplicative score boost. Baseline 1.0 (no signals), +0.4 if running,
/// +0.0..=0.6 from attention (logistic on 0..=10800s = 0..=3h).
fn score_boost(
    running: Option<&crate::search::signals::RunningSignal>,
    attention: Option<&crate::search::signals::AttentionStat>,
) -> f64 {
    let mut boost = 1.0;
    if running.is_some() {
        boost += 0.4;
    }
    if let Some(a) = attention {
        let saturated = (a.attention_secs as f64 / 10_800.0).clamp(0.0, 1.0);
        boost += 0.6 * saturated;
    }
    boost
}
```

- [ ] **Step 5.5: Run tests, expect pass**

```bash
cargo nextest run -p feature-launcher app_index::
```

Expected: all `app_index` tests pass — old + 5 new.

- [ ] **Step 5.6: Run full crate test suite to catch unrelated regressions**

```bash
cargo nextest run -p feature-launcher
```

Expected: green. If `running_apps` tests fail because the source now imports something that doesn't compile, ignore for now — Task 6 fixes those.

If anything else fails, debug before proceeding.

- [ ] **Step 5.7: Commit**

```bash
git add crates/feature-launcher/src/search/app_index.rs
git commit -m "$(cat <<'EOF'
feat(launcher): AppIndex.search joins signals + emits unified item

Per-hit join against RunningSignals/AttentionSignals (DashMap.get +
Arc::clone — O(1)). ID switches to app:{bundle_id} when available,
falls back to app:{path}. Score is multiplicative: baseline 1.0 +
0.4 running + up to 0.6 from logistic attention. Subtitle composes
'Running · 1h 23m · browsing'-style.

Co-Authored-By: Claude Opus 4.7 (1M context) <noreply@anthropic.com>
EOF
)"
```

---

## Task 6: Convert `RunningAppsSource` to signal-only producer

**Files:**
- Modify: `crates/feature-launcher/src/search/running_apps.rs` (entire file rewrite)

- [ ] **Step 6.1: Write failing tests for new behavior**

Replace the (currently empty) `#[cfg(test)] mod tests` block at the bottom of `crates/feature-launcher/src/search/running_apps.rs` (or add one if absent):

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::search::signals::new_running_signals;

    #[tokio::test]
    async fn search_returns_empty() {
        let signals = new_running_signals();
        let src = RunningAppsSource::new(signals);
        let results = src.search("anything", 10).await;
        assert!(results.is_empty(), "RunningAppsSource is signal-only");
    }

    #[test]
    fn refresh_replaces_signal_snapshot() {
        // Drives the synchronous refresh helper, not the async refresh wrapper,
        // because the macOS NSWorkspace call cannot be mocked here. The helper
        // takes the snapshot as input and updates the signal map in-place.
        use platform_macos::apps::RunningApp;

        let signals = new_running_signals();
        // Pre-populate with a stale entry that the next snapshot omits.
        signals.insert(
            smol_str::SmolStr::new("com.stale.App"),
            RunningSignal { pid: 1, path: std::path::PathBuf::new() },
        );

        let snapshot = vec![
            RunningApp {
                name: "Safari".into(),
                bundle_id: Some("com.apple.Safari".into()),
                pid: 99,
                path: Some("/Applications/Safari.app".into()),
            },
            RunningApp {
                name: "NoBundle".into(),
                bundle_id: None,    // must be ignored
                pid: 100,
                path: None,
            },
        ];

        apply_snapshot(&signals, &snapshot);

        assert_eq!(signals.len(), 1, "stale entry dropped, no-bundle entry skipped");
        let safari = signals.get(&smol_str::SmolStr::new("com.apple.Safari")).unwrap();
        assert_eq!(safari.pid, 99);
    }
}
```

- [ ] **Step 6.2: Run, expect compilation failure**

```bash
cargo nextest run -p feature-launcher running_apps::
```

Expected: compile errors — `RunningAppsSource::new(signals)`, `apply_snapshot`, `RunningSignal` re-export.

- [ ] **Step 6.3: Rewrite `running_apps.rs` with signal-only impl**

Replace the entire content of `crates/feature-launcher/src/search/running_apps.rs` with:

```rust
//! Running apps source — pure signal producer.
//!
//! Refresh polls `NSRunningApplication`, updates the shared `RunningSignals`
//! map keyed by bundle ID, and removes stale entries. Search returns empty:
//! AppIndex consumes the signal map and emits the unified `Application` row
//! for each running app.

use crate::search::signals::{RunningSignal, RunningSignals};
use crate::types::*;
use platform_macos::apps::RunningApp;
use smol_str::SmolStr;
use std::collections::HashSet;

#[derive(Clone)]
pub struct RunningAppsSource {
    signals: RunningSignals,
}

impl RunningAppsSource {
    pub fn new(signals: RunningSignals) -> Self {
        Self { signals }
    }
}

#[async_trait::async_trait]
impl super::SearchSource for RunningAppsSource {
    fn name(&self) -> &'static str {
        "running_apps"
    }

    async fn refresh(&self) {
        let snapshot = tokio::task::spawn_blocking(|| {
            platform_macos::apps::running_applications()
        })
        .await
        .unwrap_or_default();

        tracing::debug!("Running apps snapshot: {} entries", snapshot.len());
        apply_snapshot(&self.signals, &snapshot);
    }

    async fn search(&self, _query: &str, _limit: usize) -> Vec<LauncherItem> {
        Vec::new()
    }
}

/// Replace the contents of `signals` with the given snapshot, dropping any
/// stale entries no longer present and skipping snapshot rows without a bundle ID.
pub(crate) fn apply_snapshot(signals: &RunningSignals, snapshot: &[RunningApp]) {
    let live: HashSet<SmolStr> = snapshot
        .iter()
        .filter_map(|a| a.bundle_id.as_deref().map(SmolStr::new))
        .collect();

    signals.retain(|k, _| live.contains(k));

    for app in snapshot {
        if let Some(bid) = app.bundle_id.as_deref() {
            signals.insert(
                SmolStr::new(bid),
                RunningSignal {
                    pid: app.pid as u32,
                    path: app.path.clone().unwrap_or_default(),
                },
            );
        }
    }
}
```

Note: the old `with_icon_cache_dir` constructor is gone. Task 8 updates the wiring caller (`launcher.rs:157-161`).

- [ ] **Step 6.4: Run tests, expect pass**

```bash
cargo nextest run -p feature-launcher running_apps::
```

Expected: 2 passes.

- [ ] **Step 6.5: Run full crate test, expect 1 known failure (the wiring still calls `with_icon_cache_dir`)**

```bash
cargo nextest run -p feature-launcher
```

If `feature-launcher` itself compiles, you're good for this commit. The compilation error in `app-core` will surface when we run a workspace-level check — that's Task 8.

```bash
cargo check -p feature-launcher
```

Expected: 0 errors. (The break in `app-core` won't show up here.)

- [ ] **Step 6.6: Commit**

```bash
git add crates/feature-launcher/src/search/running_apps.rs
git commit -m "$(cat <<'EOF'
feat(launcher): convert RunningAppsSource to signal-only producer

refresh() updates a shared RunningSignals map keyed by bundle ID;
search() returns empty. AppIndex consumes the map and emits the
unified Application row. Eliminates one of the three duplicate
emission paths.

Co-Authored-By: Claude Opus 4.7 (1M context) <noreply@anthropic.com>
EOF
)"
```

---

## Task 7: Modify `AttentionSource` to push app rows into signals

**Files:**
- Modify: `crates/feature-launcher/src/search/attention.rs` (entire file)

- [ ] **Step 7.1: Write failing tests for new behavior**

Add to the `#[cfg(test)] mod tests` block at the bottom of `crates/feature-launcher/src/search/attention.rs`:

```rust
    #[test]
    fn route_app_rows_to_signals_emits_only_sites() {
        use crate::search::signals::new_attention_signals;
        use smol_str::SmolStr;

        let signals = new_attention_signals();
        let rows = vec![
            EntityAttentionRow {
                canonical_id: "com.apple.Safari".into(),
                kind: "app".into(),
                display_name: "Safari".into(),
                attention_secs: 3600,
                last_used_at: "2026-04-28T12:00:00Z".into(),
                icon_hint: None,
                category: Some("browsing".into()),
            },
            EntityAttentionRow {
                canonical_id: "github.com".into(),
                kind: "site".into(),
                display_name: "GitHub".into(),
                attention_secs: 7200,
                last_used_at: "2026-04-28T13:00:00Z".into(),
                icon_hint: None,
                category: Some("coding".into()),
            },
        ];

        let items = route_rows_to_items_and_signals(rows, &signals);

        assert_eq!(items.len(), 1, "only the site row becomes an item");
        assert_eq!(items[0].title, "GitHub");

        assert_eq!(signals.len(), 1, "the app row went into signals");
        let safari = signals.get(&SmolStr::new("com.apple.Safari")).unwrap();
        assert_eq!(safari.attention_secs, 3600);
        assert_eq!(safari.category.as_deref(), Some("browsing"));
    }

    #[test]
    fn site_rows_unchanged_in_format() {
        use crate::search::signals::new_attention_signals;
        let signals = new_attention_signals();
        let rows = vec![EntityAttentionRow {
            canonical_id: "github.com".into(),
            kind: "site".into(),
            display_name: "GitHub".into(),
            attention_secs: 100,
            last_used_at: "2026-04-28T12:00:00Z".into(),
            icon_hint: None,
            category: None,
        }];
        let items = route_rows_to_items_and_signals(rows, &signals);
        assert_eq!(items.len(), 1);
        match &items[0].kind {
            LauncherItemKind::UrlNavigation { url } => assert_eq!(url, "https://github.com"),
            other => panic!("expected UrlNavigation, got {other:?}"),
        }
    }
```

- [ ] **Step 7.2: Run, expect compilation failure**

```bash
cargo nextest run -p feature-launcher attention::
```

Expected: error — `route_rows_to_items_and_signals`, `AttentionSource::new(_, _)`.

- [ ] **Step 7.3: Replace `attention.rs` with signal-aware version**

Replace the entire content of `crates/feature-launcher/src/search/attention.rs` with:

```rust
use std::str::FromStr;
use std::sync::Arc;

use smol_str::SmolStr;

use crate::repos::{EntityAttentionRepo, EntityAttentionRow};
use crate::search::signals::{AttentionSignals, AttentionStat};
use crate::search::SearchSource;
use crate::types::{LauncherItem, LauncherItemKind};

pub struct AttentionSource {
    repo: Arc<EntityAttentionRepo>,
    signals: AttentionSignals,
}

impl AttentionSource {
    pub fn new(repo: Arc<EntityAttentionRepo>, signals: AttentionSignals) -> Self {
        Self { repo, signals }
    }
}

#[async_trait::async_trait]
impl SearchSource for AttentionSource {
    fn name(&self) -> &'static str {
        "attention"
    }

    fn cache_ttl(&self) -> Option<std::time::Duration> {
        Some(std::time::Duration::from_secs(30))
    }

    async fn search(&self, query: &str, limit: usize) -> Vec<LauncherItem> {
        let rows = if query.is_empty() {
            self.repo.top_by_attention(None, limit as i64).await
        } else {
            self.repo.fts_search(query, limit as i64).await
        };

        match rows {
            Ok(rows) => route_rows_to_items_and_signals(rows, &self.signals),
            Err(e) => {
                tracing::warn!(error = %e, "AttentionSource search failed");
                vec![]
            }
        }
    }
}

/// Pure routing function so we can test without a database.
/// - `kind = "site"` rows become `UrlNavigation` items (unchanged behavior).
/// - `kind = "app"` rows are pushed into `signals` and **not emitted** —
///   AppIndex owns the unified Application row.
/// - Other kinds are dropped.
pub(crate) fn route_rows_to_items_and_signals(
    rows: Vec<EntityAttentionRow>,
    signals: &AttentionSignals,
) -> Vec<LauncherItem> {
    rows.into_iter()
        .filter_map(|row| match row.kind.as_str() {
            "site" => Some(into_site_item(row)),
            "app" => {
                if let Ok(ts) = jiff::Timestamp::from_str(&row.last_used_at) {
                    signals.insert(
                        SmolStr::new(&row.canonical_id),
                        AttentionStat {
                            attention_secs: row.attention_secs,
                            category: row.category.map(SmolStr::new),
                            last_used_at: ts,
                        },
                    );
                } else {
                    tracing::warn!(
                        "AttentionSource: skipping app row with bad timestamp: {}",
                        row.last_used_at
                    );
                }
                None
            }
            _ => None,
        })
        .collect()
}

fn into_site_item(row: EntityAttentionRow) -> LauncherItem {
    LauncherItem {
        id: format!("attention:site:{}", row.canonical_id),
        title: row.display_name.clone(),
        subtitle: Some(row.canonical_id.clone()),
        icon: row.icon_hint.or_else(|| Some("globe".to_string())),
        kind: LauncherItemKind::UrlNavigation {
            url: if row.canonical_id.starts_with("http") {
                row.canonical_id
            } else {
                format!("https://{}", row.canonical_id)
            },
        },
        score: row.attention_secs as f64,
        no_view: false,
        arguments: vec![],
        pinned: false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn into_site_item_handles_https_prefix() {
        let row = EntityAttentionRow {
            canonical_id: "https://github.com".into(),
            kind: "site".into(),
            display_name: "GitHub".into(),
            attention_secs: 100,
            last_used_at: "2026-04-28T12:00:00Z".into(),
            icon_hint: None,
            category: None,
        };
        let item = into_site_item(row);
        match item.kind {
            LauncherItemKind::UrlNavigation { url } => assert_eq!(url, "https://github.com"),
            _ => panic!("expected UrlNavigation"),
        }
    }

    // The two tests added in step 7.1 go here:
    // - route_app_rows_to_signals_emits_only_sites
    // - site_rows_unchanged_in_format
}
```

Then move (or re-paste) the two tests from step 7.1 into the `#[cfg(test)] mod tests` block.

- [ ] **Step 7.4: Run tests, expect pass**

```bash
cargo nextest run -p feature-launcher attention::
```

Expected: 3 passes (1 pre-existing + 2 new).

- [ ] **Step 7.5: Verify the old `into_launcher_item` callers are gone**

```bash
grep -rn "into_launcher_item" crates/
```

Expected: no hits. The only previous caller was the now-removed `AttentionSource::search` body.

If you find external callers, mark them in the task notes — but the spec scope is internal.

- [ ] **Step 7.6: Commit**

```bash
git add crates/feature-launcher/src/search/attention.rs
git commit -m "$(cat <<'EOF'
feat(launcher): AttentionSource pushes app rows into signals

route_rows_to_items_and_signals filters kind=app rows into the shared
AttentionSignals map (so AppIndex can decorate the unified row) and
emits only kind=site items as UrlNavigation. Eliminates the second
duplicate emission path. Pure routing function = trivially testable
without a database.

Co-Authored-By: Claude Opus 4.7 (1M context) <noreply@anthropic.com>
EOF
)"
```

---

## Task 8: Wire signals through `app-core/init/launcher.rs`

**Files:**
- Modify: `crates/app-core/src/init/launcher.rs:42-50` (apps source construction)
- Modify: `crates/app-core/src/init/launcher.rs:155-161` (running_apps construction)
- Modify: `crates/app-core/src/init/launcher.rs:354-356` (attention source construction)
- Modify: `crates/app-core/src/init/launcher.rs:3-6` (imports)

- [ ] **Step 8.1: Confirm what currently breaks**

Run:

```bash
cargo check --workspace
```

Expected errors:
- `app-core/src/init/launcher.rs:157`: `RunningAppsSource::with_icon_cache_dir` no longer exists.
- `app-core/src/init/launcher.rs:356`: `AttentionSource::new(repo)` now requires 2 args.

- [ ] **Step 8.2: Update imports**

In `crates/app-core/src/init/launcher.rs`, replace the import block at lines 3-6 with:

```rust
use feature_launcher::{
    new_attention_signals, new_running_signals, AppIndex, AttentionSignals, AttentionSource,
    ClipboardRepo, EntityAttentionRepo, FileSearchSource, FrequencyRepo, FsEventKind,
    RunningSignals, ScriptRunner, SearchSource, SourceRegistry,
};
```

- [ ] **Step 8.3: Construct signal maps before any source**

In `crates/app-core/src/init/launcher.rs`, immediately after the line:

```rust
let mut sources: Vec<Arc<dyn feature_launcher::SearchSource>> = Vec::new();
```

(originally line 42), insert:

```rust
// Shared signal maps. AppIndex consumes both; RunningAppsSource writes the
// running map; AttentionSource writes the attention map.
let running_signals: RunningSignals = new_running_signals();
let attention_signals: AttentionSignals = new_attention_signals();
```

- [ ] **Step 8.4: Wire signals into `AppIndex` construction**

Change lines 46–50 of `crates/app-core/src/init/launcher.rs`:

```rust
    // Apps source
    if launcher_config.sources.apps.enabled {
        let app_index = Arc::new(AppIndex::with_cache_dir(icon_cache_dir.clone()));
        let idx = Arc::clone(&app_index);
        tokio::spawn(async move { idx.index_applications().await });
        sources.push(app_index);
    }
```

to:

```rust
    // Apps source — owns identity for installed apps; consumes signals.
    if launcher_config.sources.apps.enabled {
        let app_index = Arc::new(
            AppIndex::with_cache_dir(icon_cache_dir.clone())
                .with_running_signals(Arc::clone(&running_signals))
                .with_attention_signals(Arc::clone(&attention_signals)),
        );
        let idx = Arc::clone(&app_index);
        tokio::spawn(async move { idx.index_applications().await });
        sources.push(app_index);
    }
```

- [ ] **Step 8.5: Wire signals into `RunningAppsSource` construction**

Replace lines 155–161 with:

```rust
    // Running apps — pure signal producer; refresh updates RunningSignals.
    if launcher_config.sources.running_apps.enabled {
        let source = Arc::new(feature_launcher::RunningAppsSource::new(Arc::clone(
            &running_signals,
        )));
        sources.push(source);
    }
```

(`icon_cache_dir` is no longer needed by this source — `AppIndex` owns the icon cache.)

- [ ] **Step 8.6: Wire signals into `AttentionSource` construction**

Replace line 356:

```rust
    sources.push(Arc::new(AttentionSource::new(Arc::clone(&entity_attention_repo))));
```

with:

```rust
    sources.push(Arc::new(AttentionSource::new(
        Arc::clone(&entity_attention_repo),
        Arc::clone(&attention_signals),
    )));
```

- [ ] **Step 8.7: Build the workspace, expect 0 errors**

```bash
cargo check --workspace
```

Expected: clean build.

- [ ] **Step 8.8: Run all feature-launcher tests + app-core tests**

```bash
cargo nextest run -p feature-launcher -p app-core
```

Expected: green.

- [ ] **Step 8.9: Commit**

```bash
git add crates/app-core/src/init/launcher.rs
git commit -m "$(cat <<'EOF'
feat(app-core): wire shared signal maps into launcher sources

Constructs RunningSignals + AttentionSignals at the wiring layer and
shares them via Arc::clone into AppIndex (consumer), RunningAppsSource
(running producer), and AttentionSource (attention producer). Removes
the now-unused icon_cache_dir from RunningAppsSource construction.

Co-Authored-By: Claude Opus 4.7 (1M context) <noreply@anthropic.com>
EOF
)"
```

---

## Task 9: Add the pin/frequency ID migration

**Files:**
- Create: `crates/feature-launcher/src/migration.rs`
- Modify: `crates/feature-launcher/src/lib.rs:1-23` (module + re-export)
- Modify: `crates/app-core/src/init/launcher.rs:46-55` (run after first index)

- [ ] **Step 9.1: Write failing test for migration**

Create `crates/feature-launcher/tests/pin_migration_test.rs`:

```rust
//! Verifies the one-shot rewrite of pin + frequency IDs from path-based
//! to bundle-id-based after AppIndex first identifies bundle IDs.

use feature_launcher::{launcher_migrations, migrate_app_ids_to_bundle_ids, AppEntry};
use smol_str::SmolStr;
use sqlx::Row;
use std::path::PathBuf;
use storage::StoragePool;

async fn setup_pool() -> sqlx::SqlitePool {
    let pool = StoragePool::connect_in_memory().await.unwrap().inner().clone();
    StoragePool::run_feature_migrations(&pool, &launcher_migrations())
        .await
        .unwrap();
    pool
}

#[tokio::test]
async fn migrates_pins_and_frequency_to_bundle_ids() {
    let pool = setup_pool().await;

    // Seed: pin Safari by path (pre-migration shape).
    sqlx::query(
        "INSERT INTO launcher_pins (item_id, kind, position) VALUES (?1, ?2, ?3)",
    )
    .bind("app:/Applications/Safari.app")
    .bind("application")
    .bind(0)
    .execute(&pool)
    .await
    .unwrap();

    // Seed: usage log entry by path.
    sqlx::query(
        "INSERT INTO launcher_usage_log (item_id, kind, used_at) VALUES (?1, ?2, ?3)",
    )
    .bind("app:/Applications/Safari.app")
    .bind("application")
    .bind("2026-04-28T12:00:00Z")
    .execute(&pool)
    .await
    .unwrap();

    let apps = vec![AppEntry {
        name: "Safari".into(),
        path: PathBuf::from("/Applications/Safari.app"),
        bundle_id: Some(SmolStr::new("com.apple.Safari")),
        icon_path: None,
    }];

    let migrated = migrate_app_ids_to_bundle_ids(&pool, &apps).await.unwrap();
    assert_eq!(migrated, 2, "expected 2 rows updated (1 pin + 1 usage)");

    // Verify pin rewritten.
    let row = sqlx::query("SELECT item_id FROM launcher_pins WHERE kind = 'application'")
        .fetch_one(&pool)
        .await
        .unwrap();
    let item_id: String = row.get("item_id");
    assert_eq!(item_id, "app:com.apple.Safari");

    // Verify usage log rewritten.
    let row = sqlx::query("SELECT item_id FROM launcher_usage_log WHERE kind = 'application'")
        .fetch_one(&pool)
        .await
        .unwrap();
    let item_id: String = row.get("item_id");
    assert_eq!(item_id, "app:com.apple.Safari");
}

#[tokio::test]
async fn idempotent_when_already_migrated() {
    let pool = setup_pool().await;

    sqlx::query("INSERT INTO launcher_pins (item_id, kind, position) VALUES (?1, ?2, ?3)")
        .bind("app:com.apple.Safari")  // already migrated
        .bind("application")
        .bind(0)
        .execute(&pool)
        .await
        .unwrap();

    let apps = vec![AppEntry {
        name: "Safari".into(),
        path: PathBuf::from("/Applications/Safari.app"),
        bundle_id: Some(SmolStr::new("com.apple.Safari")),
        icon_path: None,
    }];

    let migrated = migrate_app_ids_to_bundle_ids(&pool, &apps).await.unwrap();
    assert_eq!(migrated, 0, "no rows to migrate (already done)");
}

#[tokio::test]
async fn skips_apps_without_bundle_id() {
    let pool = setup_pool().await;

    sqlx::query("INSERT INTO launcher_pins (item_id, kind, position) VALUES (?1, ?2, ?3)")
        .bind("app:/Applications/Weird.app")
        .bind("application")
        .bind(0)
        .execute(&pool)
        .await
        .unwrap();

    let apps = vec![AppEntry {
        name: "Weird".into(),
        path: PathBuf::from("/Applications/Weird.app"),
        bundle_id: None,
        icon_path: None,
    }];

    let migrated = migrate_app_ids_to_bundle_ids(&pool, &apps).await.unwrap();
    assert_eq!(migrated, 0, "apps without bundle_id are not migrated");

    // The pin row is unchanged — still path-keyed (correct fallback).
    let row = sqlx::query("SELECT item_id FROM launcher_pins WHERE kind = 'application'")
        .fetch_one(&pool)
        .await
        .unwrap();
    let item_id: String = row.get("item_id");
    assert_eq!(item_id, "app:/Applications/Weird.app");
}
```

- [ ] **Step 9.2: Run, expect compile failure**

```bash
cargo nextest run -p feature-launcher --test pin_migration_test
```

Expected: errors — `migrate_app_ids_to_bundle_ids` not exported.

- [ ] **Step 9.3: Implement the migration module**

Create `crates/feature-launcher/src/migration.rs`:

```rust
//! One-shot ID migrations for the launcher.
//!
//! After AppIndex first resolves bundle IDs from Info.plist, any pre-existing
//! pin or usage-log rows keyed by `app:{path}` are rewritten to `app:{bundle_id}`.
//! Idempotent: rows already in bundle-ID form are no-ops.

use crate::search::AppEntry;
use sqlx::SqlitePool;

/// Rewrite pin + usage-log IDs for any app whose `bundle_id` is now known.
/// Returns the total number of rows updated across both tables.
pub async fn migrate_app_ids_to_bundle_ids(
    pool: &SqlitePool,
    apps: &[AppEntry],
) -> Result<u64, sqlx::Error> {
    let mut total: u64 = 0;
    for app in apps {
        let Some(bid) = &app.bundle_id else { continue };
        let old_id = format!("app:{}", app.path.display());
        let new_id = format!("app:{bid}");
        if old_id == new_id {
            continue;
        }

        let pins_result = sqlx::query(
            "UPDATE launcher_pins SET item_id = ?1 \
             WHERE item_id = ?2 AND kind = 'application'",
        )
        .bind(&new_id)
        .bind(&old_id)
        .execute(pool)
        .await?;
        total += pins_result.rows_affected();

        let usage_result = sqlx::query(
            "UPDATE launcher_usage_log SET item_id = ?1 \
             WHERE item_id = ?2 AND kind = 'application'",
        )
        .bind(&new_id)
        .bind(&old_id)
        .execute(pool)
        .await?;
        total += usage_result.rows_affected();
    }
    Ok(total)
}
```

- [ ] **Step 9.4: Re-export from `lib.rs`**

In `crates/feature-launcher/src/lib.rs`, add `pub mod migration;` after line 8 (after `pub mod window_mgmt;`):

```rust
pub mod migration;
```

And add the re-export after the existing `pub use ...` block (after line 23):

```rust
pub use migration::migrate_app_ids_to_bundle_ids;
```

- [ ] **Step 9.5: Run tests, expect pass**

```bash
cargo nextest run -p feature-launcher --test pin_migration_test
```

Expected: 3 passes.

- [ ] **Step 9.6: Wire migration into the indexing task in `app-core`**

In `crates/app-core/src/init/launcher.rs`, replace the apps-source block (now around lines 46–56 after Task 8 edits):

```rust
    // Apps source — owns identity for installed apps; consumes signals.
    if launcher_config.sources.apps.enabled {
        let app_index = Arc::new(
            AppIndex::with_cache_dir(icon_cache_dir.clone())
                .with_running_signals(Arc::clone(&running_signals))
                .with_attention_signals(Arc::clone(&attention_signals)),
        );
        let idx = Arc::clone(&app_index);
        tokio::spawn(async move { idx.index_applications().await });
        sources.push(app_index);
    }
```

with:

```rust
    // Apps source — owns identity for installed apps; consumes signals.
    if launcher_config.sources.apps.enabled {
        let app_index = Arc::new(
            AppIndex::with_cache_dir(icon_cache_dir.clone())
                .with_running_signals(Arc::clone(&running_signals))
                .with_attention_signals(Arc::clone(&attention_signals)),
        );
        let idx = Arc::clone(&app_index);
        let migration_pool = pool.clone();
        tokio::spawn(async move {
            idx.index_applications().await;
            // After first index, rewrite any path-keyed pin/usage rows to bundle IDs.
            let apps_snapshot = idx.snapshot_apps();
            match feature_launcher::migrate_app_ids_to_bundle_ids(
                &migration_pool,
                &apps_snapshot,
            )
            .await
            {
                Ok(n) if n > 0 => tracing::info!("Migrated {n} launcher item ids to bundle-id form"),
                Ok(_) => {}
                Err(e) => tracing::warn!("launcher id migration failed: {e}"),
            }
        });
        sources.push(app_index);
    }
```

- [ ] **Step 9.7: Add the missing `snapshot_apps` accessor on `AppIndex`**

In `crates/feature-launcher/src/search/app_index.rs`, inside `impl AppIndex`, after `set_apps`, add:

```rust
    /// Snapshot the current app list. Used by the one-shot ID migration after
    /// initial indexing completes.
    pub fn snapshot_apps(&self) -> Vec<AppEntry> {
        self.apps.read().clone()
    }
```

- [ ] **Step 9.8: Build and test**

```bash
cargo check --workspace && cargo nextest run -p feature-launcher -p app-core
```

Expected: clean build, all green.

- [ ] **Step 9.9: Commit**

```bash
git add crates/feature-launcher/src/migration.rs \
        crates/feature-launcher/src/lib.rs \
        crates/feature-launcher/src/search/app_index.rs \
        crates/feature-launcher/tests/pin_migration_test.rs \
        crates/app-core/src/init/launcher.rs
git commit -m "$(cat <<'EOF'
feat(launcher): one-shot pin/frequency ID migration to bundle IDs

Runs after first index_applications() completes, rewrites any rows
of the form 'app:{path}' to 'app:{bundle_id}' for apps where the
bundle ID is now known. Idempotent — already-migrated rows are no-ops.
Apps without a bundle ID keep their path-based ID (the fallback).

Co-Authored-By: Claude Opus 4.7 (1M context) <noreply@anthropic.com>
EOF
)"
```

---

## Task 10: Integration test — Safari appears once with running + attention layers

**Files:**
- Create: `crates/feature-launcher/tests/app_dedup_test.rs`

- [ ] **Step 10.1: Write the integration test**

Create `crates/feature-launcher/tests/app_dedup_test.rs`:

```rust
//! End-to-end test: a single Safari row, decorated by all three sources,
//! with no duplicates. This is the load-bearing regression guard for the
//! decorator pattern — without it, anyone reverting the wiring sees no
//! test failures even though the bug returns.

use feature_launcher::{
    launcher_migrations, new_attention_signals, new_running_signals, AppEntry, AppIndex,
    AttentionSource, EntityAttentionRepo, EntityAttentionRow, LauncherItemKind, RunningSignal,
    SearchSource,
};
use smol_str::SmolStr;
use std::path::PathBuf;
use std::sync::Arc;
use storage::StoragePool;

async fn fresh_pool() -> sqlx::SqlitePool {
    let pool = StoragePool::connect_in_memory().await.unwrap().inner().clone();
    StoragePool::run_feature_migrations(&pool, &launcher_migrations())
        .await
        .unwrap();
    pool
}

#[tokio::test]
async fn safari_appears_once_with_running_and_attention_layers() {
    let pool = fresh_pool().await;

    let running_signals = new_running_signals();
    let attention_signals = new_attention_signals();

    // Build AppIndex with synthetic Safari entry (no plist walk).
    let app_index = AppIndex::new()
        .with_running_signals(Arc::clone(&running_signals))
        .with_attention_signals(Arc::clone(&attention_signals));

    app_index.set_apps(vec![AppEntry {
        name: "Safari".into(),
        path: PathBuf::from("/Applications/Safari.app"),
        bundle_id: Some(SmolStr::new("com.apple.Safari")),
        icon_path: None,
    }]);

    // Pre-populate the running signal directly (skip NSWorkspace).
    running_signals.insert(
        SmolStr::new("com.apple.Safari"),
        RunningSignal {
            pid: 99,
            path: PathBuf::from("/Applications/Safari.app"),
        },
    );

    // Seed entity_attention with a Safari record + a competing site.
    let attention_repo = Arc::new(EntityAttentionRepo::new(pool.clone()));
    attention_repo
        .upsert(&EntityAttentionRow {
            canonical_id: "com.apple.Safari".into(),
            kind: "app".into(),
            display_name: "Safari".into(),
            attention_secs: 3600,
            last_used_at: jiff::Timestamp::now().to_string(),
            icon_hint: None,
            category: Some("browsing".into()),
        })
        .await
        .unwrap();

    let attention =
        AttentionSource::new(Arc::clone(&attention_repo), Arc::clone(&attention_signals));

    // Drive AttentionSource.search to populate AttentionSignals (this is the
    // "search-time signal push" pattern).
    let _ = attention.search("safari", 10).await;

    // Now query AppIndex — this is the row the user sees.
    let results = app_index.search("safari", 10);
    let safari: Vec<_> = results.iter().filter(|i| i.title == "Safari").collect();

    assert_eq!(
        safari.len(),
        1,
        "expected exactly one Safari row, got {safari:#?}"
    );

    match &safari[0].kind {
        LauncherItemKind::Application { running, .. } => assert!(*running),
        other => panic!("expected Application kind, got {other:?}"),
    }

    let subtitle = safari[0].subtitle.as_deref().unwrap();
    assert!(subtitle.contains("Running"), "subtitle missing 'Running': {subtitle:?}");
    assert!(subtitle.contains("1h"), "subtitle missing '1h' (3600s): {subtitle:?}");

    assert_eq!(safari[0].id, "app:com.apple.Safari");
}

#[tokio::test]
async fn attention_only_app_with_no_install_is_suppressed() {
    let pool = fresh_pool().await;

    let attention_signals = new_attention_signals();

    let attention_repo = Arc::new(EntityAttentionRepo::new(pool.clone()));
    attention_repo
        .upsert(&EntityAttentionRow {
            canonical_id: "com.gone.App".into(),
            kind: "app".into(),
            display_name: "Gone App".into(),
            attention_secs: 999,
            last_used_at: jiff::Timestamp::now().to_string(),
            icon_hint: None,
            category: None,
        })
        .await
        .unwrap();

    let attention =
        AttentionSource::new(Arc::clone(&attention_repo), Arc::clone(&attention_signals));

    let items = attention.search("gone", 10).await;
    assert!(
        items.is_empty(),
        "uninstalled app must not appear (orphan suppression), got {items:?}"
    );
    // But the signal is still recorded for any installed app to consume:
    assert!(attention_signals.contains_key(&SmolStr::new("com.gone.App")));
}

#[tokio::test]
async fn site_attention_still_emits_url_navigation() {
    let pool = fresh_pool().await;

    let attention_signals = new_attention_signals();

    let attention_repo = Arc::new(EntityAttentionRepo::new(pool.clone()));
    attention_repo
        .upsert(&EntityAttentionRow {
            canonical_id: "github.com".into(),
            kind: "site".into(),
            display_name: "GitHub".into(),
            attention_secs: 3600,
            last_used_at: jiff::Timestamp::now().to_string(),
            icon_hint: None,
            category: Some("coding".into()),
        })
        .await
        .unwrap();

    let attention =
        AttentionSource::new(Arc::clone(&attention_repo), attention_signals);

    let items = attention.search("github", 10).await;
    assert_eq!(items.len(), 1);
    assert!(matches!(
        items[0].kind,
        LauncherItemKind::UrlNavigation { .. }
    ));
}
```

- [ ] **Step 10.2: Run, expect pass**

```bash
cargo nextest run -p feature-launcher --test app_dedup_test
```

Expected: 3 passes.

- [ ] **Step 10.3: Verify the regression guard fails if you revert decorator wiring**

This is a sanity check, not a permanent change:

```bash
# Temporarily revert AppIndex.search to ignore signals:
# In app_index.rs, replace `let running = bid.and_then(...)` with `let running: Option<_> = None;`
# Same for attention.
cargo nextest run -p feature-launcher --test app_dedup_test
```

Expected: `safari_appears_once_with_running_and_attention_layers` fails because subtitle no longer contains "Running".

**Restore the original code** before continuing:

```bash
git checkout crates/feature-launcher/src/search/app_index.rs
```

- [ ] **Step 10.4: Commit**

```bash
git add crates/feature-launcher/tests/app_dedup_test.rs
git commit -m "$(cat <<'EOF'
test(launcher): integration test for app dedup decorator pattern

Wires AppIndex + RunningSignals + AttentionSource together against
in-memory SQLite, asserts a single Safari row with both signal layers
applied, plus orphan suppression and unchanged site behavior.

Co-Authored-By: Claude Opus 4.7 (1M context) <noreply@anthropic.com>
EOF
)"
```

---

## Task 11: Add benchmark suite

**Files:**
- Create: `crates/feature-launcher/benches/app_index_dedup.rs`
- Modify: `crates/feature-launcher/Cargo.toml` (register the bench under `[[bench]]`)

- [ ] **Step 11.1: Verify Criterion is a dev-dep**

```bash
grep -n "criterion" crates/feature-launcher/Cargo.toml
```

Expected: `criterion` is listed under `[dev-dependencies]` (it's required by the existing `inverted_index.rs` bench). If absent, add `criterion.workspace = true`.

- [ ] **Step 11.2: Register the new bench in Cargo.toml**

In `crates/feature-launcher/Cargo.toml`, find the `[[bench]]` block for `inverted_index` and append below it:

```toml
[[bench]]
name = "app_index_dedup"
harness = false
```

- [ ] **Step 11.3: Create the bench file**

Create `crates/feature-launcher/benches/app_index_dedup.rs`:

```rust
use std::path::PathBuf;
use std::sync::Arc;

use criterion::{black_box, criterion_group, criterion_main, BenchmarkId, Criterion, Throughput};
use feature_launcher::{
    new_attention_signals, new_running_signals, AppEntry, AppIndex, AttentionStat, RunningSignal,
    RunningSignals,
};
use smol_str::SmolStr;

const QUERIES: &[&str] = &["s", "saf", "safari", "vsc", "fin"];

fn synth_apps(n: usize) -> Vec<AppEntry> {
    (0..n)
        .map(|i| AppEntry {
            name: format!("App{i}"),
            path: PathBuf::from(format!("/Applications/App{i}.app")),
            bundle_id: Some(SmolStr::new(format!("com.example.App{i}"))),
            icon_path: None,
        })
        .collect()
}

fn build_index_with_density(n: usize, density: f64) -> AppIndex {
    let apps = synth_apps(n);
    let running = new_running_signals();
    let attention = new_attention_signals();

    let pop_count = ((n as f64) * density) as usize;
    for app in apps.iter().take(pop_count) {
        let bid = app.bundle_id.clone().unwrap();
        running.insert(
            bid.clone(),
            RunningSignal {
                pid: 1,
                path: app.path.clone(),
            },
        );
        attention.insert(
            bid,
            AttentionStat {
                attention_secs: 1800,
                category: Some(SmolStr::new("misc")),
                last_used_at: jiff::Timestamp::now(),
            },
        );
    }

    let idx = AppIndex::new()
        .with_running_signals(running)
        .with_attention_signals(attention);
    idx.set_apps(apps);
    idx
}

fn search_bench(c: &mut Criterion) {
    let app_counts = [100usize, 500, 2_000];
    let signal_density = [0.0_f64, 0.25, 1.0];

    for &n in &app_counts {
        for &density in &signal_density {
            let idx = build_index_with_density(n, density);
            let mut group = c.benchmark_group(format!("app_index_search_n{n}_d{density:.2}"));
            group.throughput(Throughput::Elements(n as u64));

            for &q in QUERIES {
                group.bench_with_input(BenchmarkId::from_parameter(q), q, |b, q| {
                    b.iter(|| {
                        let r = idx.search(black_box(q), 20);
                        black_box(r.len());
                    });
                });
            }
            group.finish();
        }
    }
}

fn signals_refresh_bench(c: &mut Criterion) {
    use feature_launcher::apply_running_snapshot_for_bench as apply;
    use platform_macos::apps::RunningApp;

    let sizes = [10usize, 50, 200];
    let mut group = c.benchmark_group("running_signals_refresh");

    for &n in &sizes {
        let signals: RunningSignals = new_running_signals();
        for i in 0..n {
            signals.insert(
                SmolStr::new(format!("com.app.{i}")),
                RunningSignal {
                    pid: i as u32,
                    path: PathBuf::new(),
                },
            );
        }

        // Snapshot drops one app and adds one.
        let mut snapshot: Vec<RunningApp> = (0..n)
            .filter(|&i| i != 0)
            .map(|i| RunningApp {
                name: format!("App{i}"),
                bundle_id: Some(format!("com.app.{i}")),
                pid: i as i32,
                path: None,
            })
            .collect();
        snapshot.push(RunningApp {
            name: "NewApp".into(),
            bundle_id: Some(format!("com.app.{n}")),
            pid: n as i32,
            path: None,
        });

        group.throughput(Throughput::Elements(n as u64));
        group.bench_with_input(BenchmarkId::from_parameter(n), &n, |b, _| {
            b.iter(|| {
                apply(&signals, &snapshot);
                black_box(signals.len());
            });
        });
    }
    group.finish();
}

criterion_group!(benches, search_bench, signals_refresh_bench);
criterion_main!(benches);
```

- [ ] **Step 11.4: Expose `apply_snapshot` for the bench**

In `crates/feature-launcher/src/search/running_apps.rs`, change the visibility of `apply_snapshot` from `pub(crate)` to `pub`:

```rust
pub fn apply_snapshot(signals: &RunningSignals, snapshot: &[RunningApp]) {
```

In `crates/feature-launcher/src/lib.rs`, add a bench-friendly re-export. Append after the existing re-exports:

```rust
#[doc(hidden)]
pub use search::running_apps::apply_snapshot as apply_running_snapshot_for_bench;
```

(The `#[doc(hidden)]` keeps it out of the public API surface even though it's `pub` — communicates "for internal benches only".)

- [ ] **Step 11.5: Build the bench, expect 0 errors**

```bash
cargo bench -p feature-launcher --no-run --bench app_index_dedup
```

Expected: clean compilation.

- [ ] **Step 11.6: Run a smoke pass to confirm benches execute (don't gate on numbers)**

```bash
cargo bench -p feature-launcher --bench app_index_dedup -- --quick
```

Expected: all benches complete, no panics. Numbers will appear; they're informational at this step. The `--quick` flag runs a single iteration of each — fast enough to catch broken benches without full statistical convergence.

- [ ] **Step 11.7: Commit**

```bash
git add crates/feature-launcher/benches/app_index_dedup.rs \
        crates/feature-launcher/Cargo.toml \
        crates/feature-launcher/src/search/running_apps.rs \
        crates/feature-launcher/src/lib.rs
git commit -m "$(cat <<'EOF'
bench(launcher): app_index search + signals refresh

Sweeps app count × signal density × query shape. apply_snapshot
exposed via #[doc(hidden)] re-export so the bench can drive the
synchronous helper without spinning up NSWorkspace.

Co-Authored-By: Claude Opus 4.7 (1M context) <noreply@anthropic.com>
EOF
)"
```

---

## Task 12: Capture baseline ratios and document them

**Files:**
- Create: `docs/superpowers/notes/2026-04-28-launcher-app-dedup-bench-baseline.md`

This task is informational — it captures the post-change numbers so future regressions have a reference.

- [ ] **Step 12.1: Run benches with full statistical convergence**

```bash
cargo bench -p feature-launcher --bench app_index_dedup
```

Expected: ~5 minutes. Save the full output:

```bash
cargo bench -p feature-launcher --bench app_index_dedup 2>&1 | tee /tmp/launcher_bench.txt
```

- [ ] **Step 12.2: Capture the headline numbers**

Create `docs/superpowers/notes/2026-04-28-launcher-app-dedup-bench-baseline.md`:

```markdown
# Launcher App Dedup — Bench Baseline (2026-04-28)

Captured immediately after the decorator pattern landed. Numbers are
median time per `app_index.search()` call on the developer's machine.

## Acceptance thresholds (per spec Section 5.3)

| Bench | Threshold | Notes |
|---|---|---|
| `app_index_search_n2000_d0.00` (any query) | baseline | Density 0.0 = no signal lookups |
| `app_index_search_n2000_d1.00` (any query) | ≤ 1.30x of d0.00 | Density 1.0 = every hit lookups both maps |
| `running_signals_refresh` n=200 | < 50µs | "Feels instant" budget |

## Measured

(Fill in after running step 12.1 — paste the headline lines from
the criterion output, one per benchmark group.)

## Methodology

Run on macOS, dev tree compiled with `--release`. Numbers will vary
by machine; ratios within a single run are the load-bearing measure,
not absolute durations.
```

Fill in the **Measured** section with the actual output.

- [ ] **Step 12.3: Commit**

```bash
git add docs/superpowers/notes/2026-04-28-launcher-app-dedup-bench-baseline.md
git commit -m "$(cat <<'EOF'
docs(launcher): capture app dedup bench baseline

Headline numbers and threshold notes for the decorator pattern,
captured after Tasks 1-11 land. Future regressions can compare
against this snapshot.

Co-Authored-By: Claude Opus 4.7 (1M context) <noreply@anthropic.com>
EOF
)"
```

---

## Task 13: Workspace clippy + fmt + final test sweep

This is the close-out task. Catch any lints introduced in earlier tasks before they pile up.

- [ ] **Step 13.1: Run clippy for the affected crates**

```bash
cargo clippy -p feature-launcher -p platform-macos -p app-core --all-targets --all-features
```

Expected: 0 warnings. If there are warnings:
- Fix obvious ones inline (unused imports, redundant clones).
- For warnings that are intentional and pre-existing, do not change them. The "0 warnings" commitment in CLAUDE.md is for the final state — match the existing baseline.

- [ ] **Step 13.2: Run formatter**

```bash
cargo fmt --all
```

Then check:

```bash
cargo fmt --all --check
```

Expected: clean exit (no diff).

- [ ] **Step 13.3: Run all feature-launcher tests including the new integration tests**

```bash
cargo nextest run -p feature-launcher
```

Expected: green, including:
- `signals::tests` (3)
- `app_index::tests` (~10 — original + new)
- `running_apps::tests` (2)
- `attention::tests` (3)
- `tests/app_dedup_test.rs` (3)
- `tests/pin_migration_test.rs` (3)

- [ ] **Step 13.4: Workspace test sweep**

```bash
cargo nextest run --workspace
```

Expected: green. If any unrelated crate test fails, investigate — it may be a pre-existing flake or it may indicate the migration leaked into another crate.

- [ ] **Step 13.5: Commit any clippy/fmt fixes**

```bash
git status
# If files were touched:
git add -u
git commit -m "$(cat <<'EOF'
chore(launcher): clippy + fmt cleanup for app dedup

Co-Authored-By: Claude Opus 4.7 (1M context) <noreply@anthropic.com>
EOF
)"
```

If the working tree is clean, skip this step.

---

## Self-Review (run after writing the plan, before handoff)

### Spec coverage

| Spec section | Implemented in |
|---|---|
| §Architecture (decorator pattern) | Tasks 4, 5, 6, 7, 8 |
| §Components — `signals.rs` | Task 1 |
| §Components — `AppEntry.bundle_id` | Tasks 2, 3 |
| §Components — `AppIndex::search` join + boost | Task 5 |
| §Components — `RunningAppsSource` signal-only | Task 6 |
| §Components — `AttentionSource` filter + push | Task 7 |
| §Components — wiring | Task 8 |
| §Edge cases — apps without bundle_id | Tasks 3 (dedup), 5 (ID fallback) |
| §Edge cases — same bundle_id at two paths | Task 3 (`dedupe_by_bundle_id`) |
| §Edge cases — `RunningSignals` stale entries | Task 6 (`apply_snapshot` retain) |
| §Edge cases — `AttentionSignals` unbounded growth | **NOT IMPLEMENTED** — see note below |
| §Edge cases — bundle-ID extraction failure | Task 2 (PlistBuddy `Option<String>`) |
| §Edge cases — CoreServices walker scope | Task 3 (Step 3.4 dirs + max_depth) |
| §Edge cases — pin/freq ID migration | Task 9 |
| §Testing — unit tests | Tasks 1, 2, 3, 4, 5, 6, 7 |
| §Testing — integration tests | Tasks 9, 10 |
| §Benchmarks | Tasks 11, 12 |

**Gap:** `AttentionSignals` LRU cap + 90-day cleanup. Per the spec this is "defensive" — typical usage has < 200 unique apps, well under the 500-entry cap. **Decision:** defer to a follow-up. The map currently grows unbounded but in practice will not exceed reasonable size; if it does, the Mirror engine's reflection cycle is the natural place to trigger cleanup. Add to a follow-up plan rather than this one.

### Placeholder scan

Scanned: no "TBD", "TODO", "implement later", or "similar to Task N" appears in any task body. Every code step is complete copy-pasteable Rust.

### Type consistency

- `bundle_id: Option<SmolStr>` everywhere (struct field, signal map key via `SmolStr::new`, dedup `HashSet<SmolStr>`).
- Signal map types `RunningSignals` / `AttentionSignals` referred to consistently across Tasks 1, 4, 5, 6, 7, 8.
- `apply_snapshot` is `pub(crate)` in Task 6, promoted to `pub` + `#[doc(hidden)]` re-export in Task 11. No inconsistency — visibility transitions are explicit.
- `migrate_app_ids_to_bundle_ids` signature `(pool: &SqlitePool, apps: &[AppEntry]) -> Result<u64, sqlx::Error>` matches between definition (Task 9.3) and caller (Task 9.6) and tests (Task 9.1).

### Scope check

Single crate (`feature-launcher`) plus thin wiring touch (`app-core`) plus one helper add (`platform-macos`). One coherent feature, one plan. No decomposition needed.

---

## Plan complete

Plan saved to `docs/superpowers/plans/2026-04-28-launcher-app-dedup-plan.md`.

**Two execution options:**

1. **Subagent-Driven (recommended)** — I dispatch a fresh subagent per task, review between tasks, fast iteration. Each task is self-contained enough that a subagent can pick it up cold, and the test-first structure gives clear pass/fail signals.

2. **Inline Execution** — Execute tasks in this session using `executing-plans`, batch execution with checkpoints for review.

**Which approach?**
