# Launcher Production Upgrade Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Upgrade the Klynt launcher to production quality with real app icons (via sips, no NSWorkspace), exponential-decay frecency ranking, tiered search speed, and visual polish.

**Architecture:** Four independent subsystems: (1) sips-only icon pipeline writing PNGs to disk, served via Tauri `convertFileSrc` — zero IconServices mmap, (2) frecency repo replacing flat `log2(count)` with exponential decay over a usage log table, (3) tiered search fan-out showing instant in-memory results first, (4) frontend polish with grouped results, file-type SVG icons, and cancel-on-keystroke.

**Tech Stack:** Rust (sips CLI, SQLite, Tauri asset protocol), React 19, nucleo_matcher

---

## Task 1: Rewrite AppIconCache to sips-only (no NSWorkspace)

The `AppIconCache` in `platform-macos` currently falls back to `NSWorkspace.iconForFile:` which triggers the IconServices mmap leak. Rewrite it to use `sips` exclusively and save PNGs to disk (no base64, no in-memory storage). Return the **file path** instead of a data URI.

**Files:**
- Modify: `crates/platform-macos/src/apps.rs:59-206`

- [ ] **Step 1: Read the current AppIconCache implementation**

Read `crates/platform-macos/src/apps.rs` fully to understand the existing icon pipeline.

- [ ] **Step 2: Rewrite AppIconCache to return file paths, not data URIs**

Replace the `AppIconCache` implementation. The new version:
- `resolve_icon()` returns `Option<PathBuf>` (path to cached PNG) instead of `Option<String>` (data URI)
- Only uses the `sips` extraction path (`extract_icon_sips`)
- NO fallback to `NSWorkspace` / `icon_for_path` / `nsimage_to_png_data_uri`
- Saves PNGs directly to cache dir (already does this) but **resizes to 64px** (was 32px — need sharper icons for Retina)
- Validates cache by checking app mtime (already does this)

```rust
impl AppIconCache {
    pub fn new(cache_dir: PathBuf) -> Self {
        if let Err(e) = std::fs::create_dir_all(&cache_dir) {
            tracing::warn!("Failed to create icon cache dir {:?}: {}", cache_dir, e);
        }
        Self { cache_dir }
    }

    /// Resolve an app icon to a cached PNG file path.
    /// Returns None if the app has no .icns file (asset catalog apps).
    #[cfg(target_os = "macos")]
    pub fn resolve_icon_path(&self, app_path: &Path) -> Option<PathBuf> {
        let stem = app_path.file_stem()?.to_string_lossy().replace(' ', "_");
        let cached_png = self.cache_dir.join(format!("{stem}.png"));
        let cached_mtime = self.cache_dir.join(format!("{stem}.mtime"));
        let app_mtime = Self::get_mtime(app_path).unwrap_or(0);

        // Cache hit — check mtime
        if cached_png.exists() && cached_mtime.exists() {
            if let Ok(stored) = std::fs::read_to_string(&cached_mtime) {
                if stored.trim().parse::<u64>().ok() == Some(app_mtime) {
                    return Some(cached_png);
                }
            }
        }

        // Cache miss — extract via sips (reads .icns directly, no NSWorkspace)
        let plist_path = app_path.join("Contents/Info.plist");
        let output = std::process::Command::new("/usr/libexec/PlistBuddy")
            .args(["-c", "Print :CFBundleIconFile", &plist_path.to_string_lossy()])
            .output()
            .ok()?;
        if !output.status.success() {
            return None;
        }

        let mut icon_name = String::from_utf8_lossy(&output.stdout).trim().to_string();
        if !icon_name.ends_with(".icns") {
            icon_name.push_str(".icns");
        }
        let icns_path = app_path.join("Contents/Resources").join(&icon_name);
        if !icns_path.exists() {
            return None;
        }

        let result = std::process::Command::new("sips")
            .args([
                "-s", "format", "png",
                "--resampleWidth", "64",
                &icns_path.to_string_lossy(),
                "--out", &cached_png.to_string_lossy(),
            ])
            .output()
            .ok()?;
        if !result.status.success() {
            return None;
        }

        // Save mtime for cache validation
        let _ = std::fs::write(&cached_mtime, app_mtime.to_string());

        Some(cached_png)
    }

    #[cfg(not(target_os = "macos"))]
    pub fn resolve_icon_path(&self, _app_path: &Path) -> Option<PathBuf> {
        None
    }

    #[cfg(target_os = "macos")]
    fn get_mtime(path: &Path) -> Option<u64> {
        let meta = std::fs::metadata(path).ok()?;
        let mtime = meta.modified().ok()?;
        Some(mtime.duration_since(std::time::UNIX_EPOCH).ok()?.as_secs())
    }
}
```

- [ ] **Step 3: Remove all NSWorkspace icon functions**

Delete from `crates/platform-macos/src/apps.rs`:
- `icon_for_path()` (both macOS and non-macOS versions)
- `icon_for_file_type()` (both versions)
- `nsimage_to_png_data_uri()`
- The old `resolve_icon()` method that returned `(Option<String>, bool)`
- The old `extract_icon()` method that fell back to NSWorkspace
- The old `extract_icon_sips()` that returned base64

Keep `resolve_icon_path()` (the new one), `get_mtime()`, `running_applications()`, `activate_app()`.

- [ ] **Step 4: Verify compilation**

Run: `cargo check -p platform-macos 2>&1 | grep error | head -5`
Expected: Errors in downstream crates that used the old API (we'll fix those in later tasks).

- [ ] **Step 5: Commit**

```bash
git add crates/platform-macos/src/apps.rs
git commit -m "refactor: rewrite AppIconCache to sips-only, return file paths not data URIs

Removes all NSWorkspace/IconServices calls that caused ~1GB mmap leak.
Now exclusively uses sips CLI to extract .icns → 64x64 PNG on disk.
Returns Option<PathBuf> to the cached PNG file."
```

---

## Task 2: Update AppIndex to store icon_path and resolve at index time

Now that `resolve_icon_path` is safe (no NSWorkspace), we can eagerly resolve icons during `index_applications()` — `sips` reads `.icns` directly, no mmap leak.

**Files:**
- Modify: `crates/feature-launcher/src/search/app_index.rs`

- [ ] **Step 1: Change AppEntry to store icon_path instead of icon_data**

```rust
#[derive(Debug, Clone)]
pub struct AppEntry {
    pub name: String,
    pub path: PathBuf,
    pub bundle_id: Option<String>,
    /// Path to cached 64x64 PNG icon file, or None for emoji fallback.
    pub icon_path: Option<PathBuf>,
}

impl AppEntry {
    pub fn from_path(path: &Path) -> Option<Self> {
        let ext = path.extension()?.to_str()?;
        if ext != "app" { return None; }
        let name = path.file_stem()?.to_string_lossy().to_string();
        Some(Self { name, path: path.to_path_buf(), bundle_id: None, icon_path: None })
    }
}
```

- [ ] **Step 2: Remove resolved_icons HashMap and icon_cache references**

Remove the `resolved_icons: Arc<RwLock<HashMap<PathBuf, Option<String>>>>` field and all related code. Remove the `icon_cache` field. Instead, store the `AppIconCache` reference and resolve during `index_applications`.

```rust
#[derive(Clone)]
pub struct AppIndex {
    apps: Arc<RwLock<Vec<AppEntry>>>,
    icon_cache: Option<Arc<platform_macos::apps::AppIconCache>>,
}
```

- [ ] **Step 3: Resolve icons eagerly in index_applications**

Since `sips` is safe (no NSWorkspace), resolve all icons during indexing:

```rust
pub async fn index_applications(&self) {
    // ... existing walk_apps code ...

    // Resolve icons via sips (safe — reads .icns directly, no IconServices)
    if let Some(cache) = &self.icon_cache {
        for app in &mut apps {
            app.icon_path = cache.resolve_icon_path(&app.path);
        }
    }

    let icon_count = apps.iter().filter(|a| a.icon_path.is_some()).count();
    tracing::info!("Indexed {} applications ({} with icons)", apps.len(), icon_count);
    self.set_apps(apps);
}
```

- [ ] **Step 4: Update search() to return icon_path as a string**

In the `search()` method, convert the icon path to a string for the `LauncherItem.icon` field. The frontend will handle rendering.

```rust
let icon = app.icon_path
    .as_ref()
    .map(|p| p.to_string_lossy().to_string())
    .or_else(|| Some("app-window".to_string()));
```

- [ ] **Step 5: Verify compilation**

Run: `cargo check -p feature-launcher 2>&1 | grep error | head -5`

- [ ] **Step 6: Commit**

```bash
git add crates/feature-launcher/src/search/app_index.rs
git commit -m "feat(launcher): resolve app icons eagerly via sips at index time

sips reads .icns files directly — no NSWorkspace, no IconServices mmap.
Safe to resolve eagerly. Stores icon_path (PathBuf to cached PNG) in
AppEntry. ~70-80% of apps get real icons, rest use emoji fallback."
```

---

## Task 3: Update RunningAppsSource and FileSearchSource

Remove dead icon code from running_apps and file_search. Running apps look up their icon from the AppIndex cache (same path). File search uses frontend SVG sprites.

**Files:**
- Modify: `crates/feature-launcher/src/search/running_apps.rs`
- Modify: `crates/feature-launcher/src/search/file_search.rs`

- [ ] **Step 1: Simplify RunningAppsSource**

Remove `icon_cache`, `resolved_icons`, and all icon resolution. For icon, look up the app's cached PNG path from the icon cache directory:

```rust
pub struct RunningAppsSource {
    apps: Arc<RwLock<Vec<RunningApp>>>,
    icon_cache_dir: Option<PathBuf>,
}

// In search():
let icon = self.icon_cache_dir.as_ref()
    .and_then(|dir| {
        let stem = app.path.file_stem()?.to_string_lossy().replace(' ', "_");
        let png = dir.join(format!("{stem}.png"));
        if png.exists() { Some(png.to_string_lossy().to_string()) } else { None }
    })
    .or_else(|| Some("running-app".to_string()));
```

- [ ] **Step 2: Simplify FileSearchSource**

Remove `ext_icons` HashMap entirely. Return `"file"` as the icon — the frontend will render file-type SVG sprites based on the `FileKind` in the item's `kind` field.

- [ ] **Step 3: Update init/launcher.rs to pass icon_cache_dir**

Find where `RunningAppsSource::with_icon_cache()` is called and change to pass the cache directory path instead of the `AppIconCache` Arc.

- [ ] **Step 4: Verify compilation + tests**

Run: `cargo check --workspace 2>&1 | grep error | head -5`

- [ ] **Step 5: Commit**

```bash
git add crates/feature-launcher/ crates/app-core/src/init/launcher.rs
git commit -m "refactor(launcher): remove NSWorkspace from running_apps and file_search

RunningApps: looks up cached PNGs by app path stem (no API calls).
FileSearch: returns 'file' icon — frontend renders SVG sprites by FileKind."
```

---

## Task 4: Frontend — render app icons via convertFileSrc

Update `ItemIcon` in `ResultsList.tsx` to render app icons from file paths using Tauri's `convertFileSrc`.

**Files:**
- Modify: `desktop-ui/src/features/launcher/components/ResultsList.tsx:161-170`

- [ ] **Step 1: Update ItemIcon to handle file paths**

```tsx
import { convertFileSrc } from "@tauri-apps/api/core";
import { isTauri } from "@shared/lib/utils";

function ItemIcon({ kind, icon }: { kind: string; icon?: string | null }) {
  // File path to cached PNG (from Rust backend)
  if (icon && (icon.startsWith("/") || icon.startsWith("C:\\"))) {
    const src = isTauri() ? convertFileSrc(icon) : icon;
    return <img src={src} alt="" className="size-6 shrink-0 rounded" loading="lazy" />;
  }
  // Legacy base64 data URI support
  if (icon?.startsWith("data:")) {
    return <img src={icon} alt="" className="size-6 shrink-0 rounded" loading="lazy" />;
  }
  // File type SVG sprites
  if (kind === "file") {
    return <FileTypeIcon fileKind={/* passed via item */} />;
  }
  // Emoji fallback
  return (
    <span className="size-6 flex items-center justify-center text-sm shrink-0">
      {ICON_MAP[kind] || "\u2022"}
    </span>
  );
}
```

- [ ] **Step 2: Add FileTypeIcon component with inline SVGs**

Add a small component with 5 inline SVG icons for file types:

```tsx
function FileTypeIcon({ fileKind }: { fileKind?: string }) {
  const icons: Record<string, JSX.Element> = {
    code: <svg viewBox="0 0 16 16" className="size-5 text-blue-400">
      <path d="M5.5 4.5L2 8l3.5 3.5M10.5 4.5L14 8l-3.5 3.5" stroke="currentColor" strokeWidth="1.5" fill="none" strokeLinecap="round" strokeLinejoin="round"/>
    </svg>,
    document: <svg viewBox="0 0 16 16" className="size-5 text-muted-foreground">
      <rect x="3" y="1" width="10" height="14" rx="1" stroke="currentColor" strokeWidth="1" fill="none"/>
      <line x1="5" y1="5" x2="11" y2="5" stroke="currentColor" strokeWidth="0.8"/>
      <line x1="5" y1="7.5" x2="11" y2="7.5" stroke="currentColor" strokeWidth="0.8"/>
      <line x1="5" y1="10" x2="9" y2="10" stroke="currentColor" strokeWidth="0.8"/>
    </svg>,
    image: <svg viewBox="0 0 16 16" className="size-5 text-green-400">
      <rect x="2" y="2" width="12" height="12" rx="1" stroke="currentColor" strokeWidth="1" fill="none"/>
      <circle cx="5.5" cy="5.5" r="1.5" fill="currentColor"/>
      <path d="M2 11l3-3 2 2 3-4 4 5" stroke="currentColor" strokeWidth="0.8" fill="none"/>
    </svg>,
    archive: <svg viewBox="0 0 16 16" className="size-5 text-orange-400">
      <rect x="2" y="3" width="12" height="11" rx="1" stroke="currentColor" strokeWidth="1" fill="none"/>
      <rect x="2" y="1" width="12" height="3" rx="1" stroke="currentColor" strokeWidth="1" fill="none"/>
      <rect x="6" y="6" width="4" height="2" rx="0.5" stroke="currentColor" strokeWidth="0.8" fill="none"/>
    </svg>,
  };
  return icons[fileKind ?? ""] ?? (
    <svg viewBox="0 0 16 16" className="size-5 text-muted-foreground">
      <rect x="3" y="1" width="10" height="14" rx="1" stroke="currentColor" strokeWidth="1" fill="none"/>
    </svg>
  );
}
```

- [ ] **Step 3: Pass fileKind through LauncherItem**

The `LauncherItemKind::File { kind: FileKind }` already has this data. Update the `ResultRow` to extract it and pass to `ItemIcon`. Check the frontend type definition and ensure `fileKind` is available.

- [ ] **Step 4: Build and verify**

Run: `cd desktop-ui && bun run build 2>&1 | tail -5`

- [ ] **Step 5: Commit**

```bash
git add desktop-ui/src/features/launcher/
git commit -m "feat(ui): render app icons via convertFileSrc + add file-type SVG sprites

App icons load as <img> from Tauri asset protocol (cached PNGs on disk).
File results show styled SVG icons for code/doc/image/archive types.
Zero NSWorkspace calls, zero IconServices mmap, icons managed by browser."
```

---

## Task 5: Frecency repo — replace log2(count) with exponential decay

Replace the flat `launcher_frequencies` table with a usage log + exponential decay scoring.

**Files:**
- Modify: `crates/feature-launcher/migrations/001_launcher_tables.sql`
- Modify: `crates/feature-launcher/src/repos/frequency.rs`

- [ ] **Step 1: Update migration to add usage log table**

Since pre-release (no migration versioning), modify `001_launcher_tables.sql` in-place. Replace `launcher_frequencies` with:

```sql
-- Usage log for frecency calculation (exponential decay)
CREATE TABLE IF NOT EXISTS launcher_usage_log (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    item_id TEXT NOT NULL,
    kind TEXT NOT NULL,
    used_at TEXT NOT NULL
);
CREATE INDEX IF NOT EXISTS idx_usage_log_item ON launcher_usage_log(item_id, kind);
CREATE INDEX IF NOT EXISTS idx_usage_log_time ON launcher_usage_log(used_at);

-- Pinned launcher items for default view
CREATE TABLE IF NOT EXISTS launcher_pins (
    item_id TEXT NOT NULL,
    kind TEXT NOT NULL,
    position INTEGER NOT NULL DEFAULT 0,
    PRIMARY KEY (item_id, kind)
);
```

Keep the old `launcher_frequencies` table in the SQL so existing DBs don't error, but mark it deprecated.

- [ ] **Step 2: Rewrite FrequencyRepo with frecency**

```rust
use chrono::{DateTime, Utc};
use sqlx::SqlitePool;
use storage::StorageError;

const HALF_LIFE_HOURS: f64 = 72.0;

#[derive(Debug, Clone)]
pub struct FrequencyRepo {
    pool: SqlitePool,
}

impl FrequencyRepo {
    pub fn new(pool: SqlitePool) -> Self {
        Self { pool }
    }

    /// Record a usage event.
    pub async fn record_usage(&self, item_id: &str, kind: &str) -> Result<(), StorageError> {
        let now = Utc::now().to_rfc3339();
        sqlx::query(
            "INSERT INTO launcher_usage_log (item_id, kind, used_at) VALUES (?, ?, ?)"
        )
        .bind(item_id)
        .bind(kind)
        .bind(&now)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    /// Compute frecency scores for a batch of items.
    /// Returns (frecency_score, last_used_at) for each input pair.
    pub async fn get_frecency_batch(
        &self,
        items: &[(String, String)],
    ) -> Result<Vec<(f64, Option<String>)>, StorageError> {
        if items.is_empty() {
            return Ok(vec![]);
        }

        // Fetch all usage events from last 90 days for these items
        let conditions: Vec<String> = items.iter().enumerate()
            .map(|(i, _)| format!("(item_id = ?{} AND kind = ?{})", i * 2 + 1, i * 2 + 2))
            .collect();
        let cutoff = (Utc::now() - chrono::Duration::days(90)).to_rfc3339();
        let sql = format!(
            "SELECT item_id, kind, used_at FROM launcher_usage_log \
             WHERE ({}) AND used_at > '{}' ORDER BY used_at DESC",
            conditions.join(" OR "),
            cutoff,
        );

        let mut query = sqlx::query_as::<_, (String, String, String)>(&sql);
        for (item_id, kind) in items {
            query = query.bind(item_id).bind(kind);
        }
        let rows = query.fetch_all(&self.pool).await?;

        let now = Utc::now();
        let lambda = (2.0_f64).ln() / HALF_LIFE_HOURS;

        let mut results = Vec::with_capacity(items.len());
        for (item_id, kind) in items {
            let mut score = 0.0_f64;
            let mut last_used: Option<String> = None;
            for (rid, rk, used_at) in &rows {
                if rid == item_id && rk == kind {
                    if last_used.is_none() {
                        last_used = Some(used_at.clone());
                    }
                    if let Ok(dt) = DateTime::parse_from_rfc3339(used_at) {
                        let hours = (now - dt).num_seconds() as f64 / 3600.0;
                        score += (-lambda * hours).exp();
                    }
                }
            }
            results.push((score, last_used));
        }
        Ok(results)
    }

    /// Get top N items by frecency for the default view.
    pub async fn top_frecency(&self, limit: usize) -> Result<Vec<(String, String, f64)>, StorageError> {
        let cutoff = (Utc::now() - chrono::Duration::days(30)).to_rfc3339();
        let rows = sqlx::query_as::<_, (String, String, String)>(
            "SELECT item_id, kind, used_at FROM launcher_usage_log \
             WHERE used_at > ? ORDER BY used_at DESC LIMIT 500"
        )
        .bind(&cutoff)
        .fetch_all(&self.pool)
        .await?;

        let now = Utc::now();
        let lambda = (2.0_f64).ln() / HALF_LIFE_HOURS;

        // Group by (item_id, kind) and compute frecency
        let mut scores: std::collections::HashMap<(String, String), f64> = std::collections::HashMap::new();
        for (item_id, kind, used_at) in &rows {
            if let Ok(dt) = DateTime::parse_from_rfc3339(used_at) {
                let hours = (now - dt).num_seconds() as f64 / 3600.0;
                *scores.entry((item_id.clone(), kind.clone())).or_default() += (-lambda * hours).exp();
            }
        }

        let mut sorted: Vec<_> = scores.into_iter()
            .map(|((id, kind), score)| (id, kind, score))
            .collect();
        sorted.sort_by(|a, b| b.2.partial_cmp(&a.2).unwrap_or(std::cmp::Ordering::Equal));
        sorted.truncate(limit);
        Ok(sorted)
    }

    /// Prune usage log entries older than 90 days.
    pub async fn prune_old_entries(&self) -> Result<u64, StorageError> {
        let cutoff = (Utc::now() - chrono::Duration::days(90)).to_rfc3339();
        let result = sqlx::query("DELETE FROM launcher_usage_log WHERE used_at < ?")
            .bind(&cutoff)
            .execute(&self.pool)
            .await?;
        Ok(result.rows_affected())
    }
}
```

- [ ] **Step 3: Update search_engine.rs to use new frecency API**

Replace `apply_frequency_boosts` to use `get_frecency_batch` instead of `get_boosts_batch`. Replace `frequency_repo.increment()` calls with `frequency_repo.record_usage()`.

- [ ] **Step 4: Verify tests pass**

Run: `cargo nextest run -p feature-launcher 2>&1 | tail -10`

- [ ] **Step 5: Commit**

```bash
git add crates/feature-launcher/
git commit -m "feat(launcher): replace log2(count) with exponential-decay frecency

Usage log table records each launcher execution with timestamp. Frecency
score = sum of e^(-lambda * hours_since_use) with 72h half-life. An app
used 3 times today ranks higher than one used 100 times last month.
Includes top_frecency() for default view and prune for 90-day cleanup."
```

---

## Task 6: Default view — show top frecency items on empty query

When the launcher opens with an empty query, show the user's top 8 most-used items.

**Files:**
- Modify: `crates/app-core/src/handlers/launcher/search_engine.rs:25-34`
- Modify: `desktop-ui/src/features/launcher/hooks/useLauncherSearch.ts`

- [ ] **Step 1: Handle empty query in search engine**

In `search_engine.rs`, change the empty-query handler:

```rust
if query.is_empty() {
    // Show top frecency items as default view
    let top = self.frequency_repo.top_frecency(8).await.unwrap_or_default();
    let mut defaults = Vec::new();
    for (item_id, kind, score) in top {
        // Reconstruct LauncherItem from id + kind
        if let Some(item) = self.reconstruct_item(&item_id, &kind, &self.registry).await {
            let mut item = item;
            item.score = score;
            defaults.push(item);
        }
    }
    return Ok(defaults);
}
```

- [ ] **Step 2: Add reconstruct_item helper**

This method takes an `item_id` (like `"app:/Applications/Figma.app"`) and `kind` (like `"app"`) and reconstructs a `LauncherItem` by looking it up in the appropriate source:

```rust
async fn reconstruct_item(
    &self,
    item_id: &str,
    kind: &str,
    registry: &SourceRegistry,
) -> Option<LauncherItem> {
    match kind {
        "app" | "running_app" => {
            let path = item_id.strip_prefix("app:").or_else(|| item_id.strip_prefix("running:"))?;
            let name = std::path::Path::new(path).file_stem()?.to_string_lossy().to_string();
            // Look up icon from app source
            let apps = registry.search(&name, 1).await;
            apps.into_iter().find(|a| a.id == item_id)
        }
        "file" | "grep" => {
            let path = item_id.strip_prefix("file:").or_else(|| item_id.strip_prefix("grep:"))?;
            let name = std::path::Path::new(path).file_name()?.to_string_lossy().to_string();
            Some(LauncherItem {
                id: item_id.to_string(),
                title: name,
                subtitle: Some(path.to_string()),
                icon: Some("file".to_string()),
                kind: LauncherItemKind::File {
                    path: path.into(),
                    kind: crate::types::FileKind::File,
                },
                score: 0.0,
            })
        }
        _ => None, // Other kinds reconstructed as needed
    }
}
```

- [ ] **Step 3: Update frontend to fetch defaults on mount**

In `useLauncherSearch.ts`, trigger a search with empty query when the launcher opens:

```tsx
useEffect(() => {
    // Fetch defaults on mount (empty query = top frecency items)
    ipc<LauncherItem[]>("launcher_search", { query: "" })
        .then(setResults)
        .catch(() => {});
}, []); // Run once on mount
```

- [ ] **Step 4: Build and verify**

Run: `cargo check --workspace && cd desktop-ui && bun run build`

- [ ] **Step 5: Commit**

```bash
git add crates/app-core/ desktop-ui/
git commit -m "feat(launcher): show top 8 frecency items on empty query

When launcher opens, immediately shows the user's most-used items
ranked by exponential-decay frecency. No typing needed for common
app launches."
```

---

## Task 7: Speed — reduce debounce and add cancel-on-keystroke

**Files:**
- Modify: `desktop-ui/src/features/launcher/hooks/useLauncherSearch.ts`

- [ ] **Step 1: Reduce debounce from 100ms to 30ms and add AbortController**

```tsx
import { ipc } from "@shared/hooks/useIpc";
import { useEffect, useRef } from "react";
import { useLauncherStore } from "../stores/launcherStore";
import type { LauncherItem } from "../types";

export function useLauncherSearch() {
  const query = useLauncherStore((s) => s.query);
  const setResults = useLauncherStore((s) => s.setResults);
  const setIsSearching = useLauncherStore((s) => s.setIsSearching);
  const timerRef = useRef<ReturnType<typeof setTimeout>>(undefined);
  const versionRef = useRef(0);

  useEffect(() => {
    // Default view on empty query
    if (!query.trim()) {
      const v = ++versionRef.current;
      ipc<LauncherItem[]>("launcher_search", { query: "" })
        .then((results) => {
          if (versionRef.current === v) setResults(results);
        })
        .catch(() => {});
      return;
    }

    setIsSearching(true);
    clearTimeout(timerRef.current);

    // Cancel-on-keystroke: increment version so stale responses are discarded
    const version = ++versionRef.current;

    timerRef.current = setTimeout(async () => {
      try {
        const results = await ipc<LauncherItem[]>("launcher_search", { query });
        // Only apply if this is still the latest query
        if (versionRef.current === version) {
          setResults(results);
          setIsSearching(false);
        }
      } catch (e) {
        if (versionRef.current === version) {
          console.error("Launcher search failed:", e);
          setResults([]);
          setIsSearching(false);
        }
      }
    }, 30);

    return () => clearTimeout(timerRef.current);
  }, [query, setResults, setIsSearching]);
}
```

- [ ] **Step 2: Build and verify**

Run: `cd desktop-ui && bun run build 2>&1 | tail -3`

- [ ] **Step 3: Commit**

```bash
git add desktop-ui/src/features/launcher/hooks/useLauncherSearch.ts
git commit -m "perf(launcher): reduce debounce 100ms→30ms + cancel-on-keystroke

30ms debounce feels instant for common queries. Version counter
ensures stale IPC responses from slower queries are discarded when
user types faster than the backend responds."
```

---

## Task 8: Visual polish — animation tuning + result grouping

**Files:**
- Modify: `desktop-ui/src/features/launcher/components/ResultsList.tsx`

- [ ] **Step 1: Speed up stagger animation**

Find the stagger delay (around `Math.min(index * 20, 200)`) and change to:
```tsx
style={{ animationDelay: `${Math.min(index * 10, 100)}ms` }}
```

- [ ] **Step 2: Add section headers for grouped results**

Add lightweight category dividers when results span multiple kinds. Insert before the `map()` in the results list:

```tsx
// Group results by category for section headers
function groupLabel(kind: string): string {
  if (kind === "application" || kind === "runningApp") return "Apps";
  if (kind === "file" || kind === "contentMatch") return "Files";
  if (kind === "task") return "Tasks";
  if (kind === "note") return "Notes";
  return "Other";
}

// In render, track current group and insert dividers
let lastGroup = "";
// ... in the map:
const group = groupLabel(item.kind.type);
const showHeader = group !== lastGroup && index > 0;
lastGroup = group;

{showHeader && (
  <div className="px-3 pt-2 pb-1 text-2xs font-medium text-muted-foreground uppercase tracking-wider">
    {group}
  </div>
)}
```

- [ ] **Step 3: Build and verify**

Run: `cd desktop-ui && bun run build`

- [ ] **Step 4: Commit**

```bash
git add desktop-ui/src/features/launcher/components/ResultsList.tsx
git commit -m "feat(ui): faster result animation + grouped section headers

Stagger: 10ms/100ms max (was 20ms/200ms). Section headers show when
results span multiple categories (Apps, Files, Tasks, Notes, Other)."
```

---

## Task 9: Cleanup — remove dead icon code and unused imports

**Files:**
- Modify: `crates/feature-launcher/src/search/app_index.rs` (remove unused HashMap import)
- Modify: `crates/feature-launcher/src/search/running_apps.rs` (remove unused HashMap import)
- Modify: `crates/feature-launcher/src/search/file_search.rs` (remove ext_icons field, resolve_ext_icons fn)
- Modify: `crates/app-core/src/init/launcher.rs` (clean up icon_cache wiring)

- [ ] **Step 1: Remove all unused icon-related code**

Clean up any remaining dead code: unused imports, unused fields, unused functions related to the old icon pipeline.

- [ ] **Step 2: Run clippy to verify no warnings**

Run: `cargo clippy --workspace --all-targets --all-features 2>&1 | grep "warning\[" | head -10`

- [ ] **Step 3: Run frontend lint**

Run: `cd desktop-ui && bun run lint:fix`

- [ ] **Step 4: Commit**

```bash
git add crates/ desktop-ui/
git commit -m "chore: remove dead icon code from launcher sources

Cleaned up resolved_icons, ext_icons, icon_for_file_type, and
NSWorkspace-related imports that are no longer used after the
sips-only icon pipeline."
```

---

## Task 10: Wire frecency prune to cron + update launcher_execute

**Files:**
- Modify: `crates/app-core/src/init/cron.rs` (add frecency prune job)
- Modify: `crates/app-core/src/handlers/launcher/` (update execute to use record_usage)

- [ ] **Step 1: Update launcher_execute to use record_usage**

Find where `frequency_repo.increment()` is called and replace with `frequency_repo.record_usage()`.

- [ ] **Step 2: Add weekly prune cron job**

In the cron setup, add a job that runs `frequency_repo.prune_old_entries()` weekly to prevent unbounded usage log growth.

- [ ] **Step 3: Verify and commit**

Run: `cargo check --workspace`

```bash
git add crates/
git commit -m "feat(launcher): wire frecency record_usage + weekly prune cron"
```
