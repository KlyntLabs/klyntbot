# Launcher Production Upgrade — Icons, Frecency, Speed, Polish

## Goal

Upgrade the Klynt launcher from functional prototype to production-grade quality that competes with Alfred/Raycast — real app icons without memory leaks, frecency-based smart ranking, sub-5ms perceived search latency, and visual polish.

## Architecture

The upgrade touches four independent subsystems: (1) icon pipeline — replace NSWorkspace with direct `sips` extraction + Tauri asset protocol serving, (2) ranking — replace `log2(count)` with exponential-decay frecency, (3) search speed — tiered fan-out with cancel-on-keystroke, (4) visual polish — grouped results, file-type SVG sprites, detail panel enrichment. Each subsystem can be implemented and tested independently.

## Tech Stack

Rust (sips CLI, SQLite, Tauri asset protocol), React 19 (Suspense, asset URLs), nucleo_matcher (fuzzy search)

---

## 1. Icon System — sips Disk Cache + Asset Protocol

### Problem

NSWorkspace `iconForFile:` triggers macOS IconServices to mmap `.isdata` files that are **never unmapped** during the process lifetime. Over a session with launcher searches, this grows from 100MB to 1.7GB+. The current workaround disables all icons.

### Solution

Extract icons at index time using `sips` CLI (reads `.icns` directly, never touches IconServices). Serve cached PNGs to the WebView via Tauri's `convertFileSrc()` asset protocol.

### Icon Pipeline

```
Index time (background, per app):
  1. Read Info.plist → CFBundleIconFile
  2. Find {app}/Contents/Resources/{icon}.icns
  3. If .icns exists:
     a. Check disk cache: {data_dir}/cache/app-icons/{stem}.png
     b. If cache miss or mtime changed:
        sips -s format png --resampleWidth 64 {icns} --out {cache}.png
     c. Store icon_path in AppEntry (not base64)
  4. If no .icns (asset catalog apps): icon_path = None → emoji fallback
```

### Icon Categories

| Source | Icon Strategy | Memory Cost |
|--------|--------------|-------------|
| Applications | sips-extracted 64x64 PNG on disk, served via asset:// | 0 (browser cache) |
| Running apps | Same as app icon (lookup by path) | 0 |
| Files | 5 SVG sprites: folder, code, doc, image, archive | ~2KB total |
| System commands | Emoji from ICON_MAP | 0 |
| Tasks/Notes/Contacts | Emoji from ICON_MAP | 0 |
| Calculator/AI Chat | Emoji from ICON_MAP | 0 |
| Bookmarks/History | Emoji from ICON_MAP | 0 |
| Brew/SSH/Git | Emoji from ICON_MAP | 0 |

### Frontend Rendering

```tsx
function ItemIcon({ kind, iconPath }: { kind: string; iconPath?: string | null }) {
  if (iconPath) {
    // Tauri asset protocol — browser handles caching/memory
    const src = convertFileSrc(iconPath);
    return <img src={src} alt="" className="size-6 rounded" loading="lazy" />;
  }
  // Emoji fallback
  return <span className="size-6 flex items-center justify-center">{ICON_MAP[kind]}</span>;
}
```

### Coverage

- ~70-80% of apps have `.icns` files (traditional apps, most third-party apps)
- ~20% use asset catalogs (newer Apple apps like Freeform, Journal) → emoji fallback
- All non-app items use styled emoji/SVG → zero macOS API calls

### Cleanup

- Remove `nsimage_to_png_data_uri()` and all `NSWorkspace.iconForFile:` calls from platform-macos
- Remove `icon_for_file_type()` — replaced by frontend SVG sprites
- Remove `resolved_icons` HashMap caches — no longer needed
- Keep `AppIconCache` struct but change it to only use `sips` extraction (no NSWorkspace fallback)

---

## 2. Frecency Ranking — Exponential Decay

### Problem

Current `log2(count + 1)` multiplier is flat — an app used 100 times 6 months ago ranks the same as one used 5 times today. No default view before typing.

### Algorithm

```
frecency(item) = Σ for each recorded usage:
  weight = e^(-λ * hours_since_use)
  
where λ = ln(2) / half_life_hours
  half_life = 72 hours (3 days)
```

This gives:
- Used 1 hour ago: weight ≈ 0.99
- Used 24 hours ago: weight ≈ 0.79
- Used 3 days ago: weight ≈ 0.50
- Used 7 days ago: weight ≈ 0.19
- Used 30 days ago: weight ≈ 0.001

### Schema Change

```sql
-- Replace single last_used with usage log for proper decay calculation.
-- Pre-release: drop and recreate.
DROP TABLE IF EXISTS launcher_frequencies;

CREATE TABLE launcher_usage_log (
  id INTEGER PRIMARY KEY AUTOINCREMENT,
  item_id TEXT NOT NULL,
  kind TEXT NOT NULL,
  used_at TEXT NOT NULL
);
CREATE INDEX idx_usage_log_item ON launcher_usage_log(item_id, kind);
CREATE INDEX idx_usage_log_time ON launcher_usage_log(used_at);

-- Pinned items for default view
CREATE TABLE launcher_pins (
  item_id TEXT NOT NULL,
  kind TEXT NOT NULL,
  position INTEGER NOT NULL DEFAULT 0,
  PRIMARY KEY (item_id, kind)
);
```

### Frecency Calculation

```rust
fn frecency_score(usages: &[DateTime<Utc>], now: DateTime<Utc>) -> f64 {
    const HALF_LIFE_HOURS: f64 = 72.0;
    let lambda = (2.0_f64).ln() / HALF_LIFE_HOURS;
    
    usages.iter().map(|used_at| {
        let hours = (now - *used_at).num_seconds() as f64 / 3600.0;
        (-lambda * hours).exp()
    }).sum()
}
```

### Integration

- On `launcher_execute`: INSERT into `launcher_usage_log`
- On search: batch query recent usages (last 30 days) for all result items, compute frecency, multiply into base score
- Prune usage log entries older than 90 days (cron job, weekly)

### Default View (Before Typing)

When launcher opens with empty query:
1. Query top 8 items by frecency from `launcher_usage_log`
2. Prepend any pinned items from `launcher_pins`
3. Show immediately — no search needed, pure SQLite query

---

## 3. Search Speed — Tiered Fan-out

### Problem

Current search fans out to all 16 sources and waits for all to return. Slow sources (content grep, file search) delay fast sources (apps, commands).

### Tiered Architecture

```
Tier 1 — Instant (<1ms, in-memory):
  Apps, Running Apps, System Commands, Calculator
  → Show results immediately

Tier 2 — Fast (~5ms, SQLite indexed):
  Tasks, Notes, Clipboard, Bookmarks, Frecency defaults
  → Merge within one frame

Tier 3 — Medium (~50ms, filesystem):
  Files, Git Repos, System Prefs, Brew, SSH, Contacts
  → Stream in, only if query ≥ 2 chars

Tier 4 — Slow (~500ms+, external process):
  Content Grep, Browser History
  → Only on prefix (?) or query ≥ 4 chars
```

### Implementation

```rust
// In SourceRegistry, tag each source with a tier
enum SearchTier { Instant, Fast, Medium, Slow }

async fn search_tiered(&self, query: &str) -> Vec<LauncherItem> {
    let (instant, fast, medium, slow) = self.partition_by_tier();
    
    // Tier 1: await immediately (guaranteed <1ms)
    let mut results = search_sources(&instant, query).await;
    
    // Tier 2: await (guaranteed <10ms)
    results.extend(search_sources(&fast, query).await);
    
    // Tier 3+4: spawn concurrently, merge as they arrive
    if query.len() >= 2 {
        let medium_results = search_sources(&medium, query).await;
        results.extend(medium_results);
    }
    if query.len() >= 4 || query.starts_with('?') {
        let slow_results = search_sources(&slow, query).await;
        results.extend(slow_results);
    }
    
    results
}
```

### Frontend Changes

- **Debounce: 100ms → 30ms** — feels instant for common queries
- **Cancel-on-keystroke:** Use `AbortController` pattern — if user types while search is in-flight, cancel previous IPC and start new one
- **Optimistic Tier 1:** Show in-memory results immediately without waiting for IPC (requires caching app list in frontend store)

---

## 4. Visual Polish

### Result Grouping

When showing mixed results, add subtle section headers:

```
Default view (empty query):
  [Recently Used]
    Figma                        App
    Terminal                     App
    meeting-notes.md             File

Search results (typing):
    Finder              🖥      App      ← icon from sips cache
    Figma               🎨      App
    file_ops.rs          📄      File     ← SVG sprite
    Finance dashboard    📝      Note     ← emoji
```

Section headers only appear if 2+ categories present. Lightweight muted text, not full dividers.

### File Type SVG Sprites

Replace emoji for file results with 5 styled SVG icons:

| FileKind | SVG | Color |
|----------|-----|-------|
| Code | `</>` brackets | blue |
| Document | doc page | gray |
| Image | mountain/sun | green |
| Archive | zip box | orange |
| Folder | folder | yellow |
| File (default) | blank page | gray |

These are inline SVG — no external files, no network requests, ~200 bytes each.

### Animation Tuning

- Stagger: `min(index * 10, 100)ms` (was `min(index * 20, 200)ms`) — snappier
- Total animation time: 100ms max (was 200ms)

### Detail Panel Enrichment

When user presses Tab on a result:
- Apps: show 64x64 icon, "Last used 2h ago" from frecency data, app path
- Files: show file size, modified date, parent directory
- Notes: show creation date, tag count, word count

---

## 5. Cleanup — Remove Dead Code

- Delete `nsimage_to_png_data_uri()` from `platform-macos/src/apps.rs`
- Delete `icon_for_file_type()` from `platform-macos/src/apps.rs`
- Delete `icon_for_path()` from `platform-macos/src/apps.rs`
- Remove `resolved_icons` HashMap from `app_index.rs`, `running_apps.rs`
- Remove `ext_icons` HashMap from `file_search.rs`
- Remove `resolve_ext_icons()` from `file_search.rs`
- Remove unused `icon_cache` field from `RunningAppsSource`
- Clean up `AppIconCache` to only use sips path (no NSWorkspace fallback)

---

## Non-Goals

- **Plugin system for custom sources** — not needed before release
- **Theming/custom icon packs** — use system defaults
- **Multi-window launcher** — single window is fine
- **AI-powered semantic search** — already have the `AiChat` fallback item
- **Hotkey customization** — already configurable in settings

---

## Success Criteria

1. **Icons:** Real app icons visible for 70%+ of applications, zero NSWorkspace calls, zero IconServices mmap growth
2. **Memory:** Launcher usage does not increase process memory beyond ~5MB (icon PNGs in browser cache, not Rust heap)
3. **Speed:** First results appear in <5ms for app/command queries, <50ms for file queries
4. **Ranking:** Most-used apps appear at top within 3 days of normal use
5. **Default view:** Top 8 frecency items shown instantly on launcher open
