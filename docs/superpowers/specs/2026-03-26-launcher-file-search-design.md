# Launcher File Search Redesign

**Date:** 2026-03-26
**Status:** Approved
**Approach:** Replace `mdfind` with in-memory `ignore`-walk index + `nucleo` fuzzy search

## Problem

The current file search shells out to `mdfind -name <query>` on every keystroke (debounced 100ms). This has three issues:

1. **Searches the entire device** — no directory scoping, so results include system Ruby gems, library scripts, and files from every project
2. **Slow** — subprocess spawn + Spotlight query takes 200ms–2s per keystroke; results only cached for 5s
3. **No relevance ranking** — all results get a fixed `score: 0.8`, and `mdfind`'s own ordering is discarded by the cache

## Design

### Config Schema

Replace the bare `SourceToggle` for files with a new `FileSearchConfig`:

```rust
// crates/config/src/schema/launcher.rs
pub struct FileSearchConfig {
    pub enabled: bool,                  // default: true
    pub scan_dirs: Vec<String>,         // default: ["~/Projects", "~/Documents", "~/Desktop"]
    pub refresh_interval_secs: u64,     // default: 120
}
```

Additive change — existing `{ "enabled": true }` in config.json deserializes cleanly via `#[serde(default)]`.

### Indexing

On startup, `init_launcher()` spawns a background task that walks each `scan_dir` using the `ignore` crate's `WalkBuilder`:

- **Automatic `.gitignore` respect** — the `ignore` crate's core purpose; works recursively through nested `.gitignore` files
- **Hidden files/dirs skipped** by default (`.git/`, `.DS_Store`, etc.)
- **Hardcoded global ignores** as fallback for non-git dirs: `node_modules/`, `target/`, `__pycache__/`, `*.pyc`, `.DS_Store`
- **Permission errors on individual files** — skipped silently (same as `fd` behavior)
- **Non-existent `scan_dir`** — log warning, skip, don't fail the index

File entries stored in `Arc<RwLock<Vec<FileEntry>>>`:

```rust
struct FileEntry {
    name: String,       // "LauncherInput.tsx"
    path: String,       // "/Users/jayden/Projects/Klynt/bot/desktop-ui/src/..."
    extension: String,  // "tsx"
    dir_index: usize,   // which scan_dir this came from (for priority scoring)
}
```

### Search

Fuzzy match against `name` field using `nucleo_matcher` (same engine as 8+ other launcher sources). Score boosters:

- **Exact prefix match:** +0.2
- **Dir priority:** files from earlier scan dirs score higher (first dir = highest priority)

No subprocess, no cache needed — in-memory fuzzy match completes in microseconds.

### Refresh

Follows the existing `BackgroundRefresher` pattern:

1. Initial index build on app startup (background spawn, non-blocking to UI)
2. Re-scan every `refresh_interval_secs` (default 120s)
3. Re-scan builds a new `Vec<FileEntry>`, then swaps via `RwLock::write()` — zero lock contention during search

### Settings UI

Extend the existing `LauncherSettings.tsx` source definition for Files:

```ts
{
  key: "files",
  label: "Files",
  extraFields: [
    { key: "scanDirs", label: "Search directories", type: "dirs", placeholder: "~/Projects, ~/Documents" },
    { key: "refreshIntervalSecs", label: "Refresh interval (seconds)", type: "number" },
  ],
}
```

Uses the existing expandable row + comma-separated dir input pattern (same as Git Repos). Update `LauncherData` interface:

```ts
files?: { enabled?: boolean; scanDirs?: string[]; refreshIntervalSecs?: number };
```

No new components needed.

## Files Changed

### Rust

| File | Change |
|------|--------|
| `crates/config/src/schema/launcher.rs` | Replace `files: SourceToggle` with `files: FileSearchConfig` |
| `crates/feature-launcher/src/search/file_search.rs` | Replace `mdfind` subprocess with `ignore` walker + `nucleo` fuzzy search |
| `crates/feature-launcher/Cargo.toml` | Add `ignore` crate dependency |
| `crates/app-core/src/init/launcher.rs` | Wire `FileSearchConfig.scan_dirs` into `FileSearchSource`, register with `BackgroundRefresher` at configured interval |

### Frontend

| File | Change |
|------|--------|
| `desktop-ui/src/features/settings/pages/LauncherSettings.tsx` | Add `extraFields` to Files source def, update `LauncherData` type |

## What Gets Removed

- `mdfind` subprocess call in `file_search.rs`
- 5-second query cache for file search (unnecessary with in-memory search)

## Non-Goals

- Live file watching (`fsnotify`) — periodic re-scan is sufficient
- Content search (already handled by `ContentGrepSource` with `?` prefix)
- SQLite persistence of the file index — in-memory rebuild on startup is fast enough
