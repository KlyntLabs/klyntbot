# Launcher Core Improvements — Design Spec

**Date:** 2026-04-19
**Status:** Draft (pending implementation plan)
**Scope:** `crates/feature-launcher`, `crates/app-core/src/init/launcher.rs`, `desktop-ui/src/features/launcher`, `crates/platform-macos/src/window.rs`
**Out of scope:** MCP tool exposure, snippet expansion, Hyper Key (covered by Spec 2: System-Wide Input Services).

## Motivation

Comparative analysis against SuperCmd (see brainstorming context, 2026-04-19) surfaced four high-leverage improvements where SuperCmd is materially ahead of Klynt's `feature-launcher`:

1. **Inverted-prefix file index + mdfind fallback** — Klynt currently linear-scans an `Arc<RwLock<Vec<_>>>` of file entries inside `FileSearchSource::search`, holding the read guard for the full fuzzy scan. SuperCmd's `Map<prefix, Vec<entry_id>>` with shortest-postings-list intersection is materially faster on multi-term queries and an `mdfind` fallback catches privacy-excluded paths.
2. **Inline argument UI** — Klynt has no concept of command arguments; SuperCmd renders inline chips for ≤3 args, avoiding modals.
3. **No-view command execution** — Klynt always shows the launcher panel during execution; SuperCmd's `NoViewRunner` allows hotkey-or-search execution that closes the panel and shows a small status badge.
4. **Window-management presets** — Klynt has ~6 actions (halves, maximize, center); SuperCmd has 50+ via a Swift binary backed by `AXUIElement`. Curated 25 essentials is the sweet spot.

All four are additive: no existing source, IPC, or tool contract breaks.

## Architecture Overview

Four features land sequentially as four sub-PRs. Cross-cutting principle: every type change is **additive** (`Option`, new variants, default-false bool), so existing sources compile unchanged.

| # | Feature | Surface | Net new LOC |
|---|---------|---------|-------------|
| 1 | `InvertedFileIndex` | `feature-launcher/src/search/inverted_index.rs` | ~600 |
| 2 | `LauncherExecuteResult` + `no_view` + status badge | types.rs, lib.rs, desktop-ui status window | ~300 |
| 3 | `ArgSpec` on items + template substitution + `ArgChipBar` | types.rs, engine.rs, desktop-ui | ~450 |
| 4 | `WindowPresetsSource` + 25-preset data table | window_mgmt/presets.rs, search/window_presets.rs | ~250 |

Sequence is intentional: 2 → 3 → 4 each consume the previous (window presets use `no_view`; system-command args use the arg infrastructure).

The `LauncherFeature::tools()` empty stub remains untouched. MCP exposure is the next spec.

## Component Designs

### 1. InvertedFileIndex

**Location:** `crates/feature-launcher/src/search/inverted_index.rs`

**Types:**
```rust
pub struct InvertedFileIndex {
    entries: Vec<IndexEntry>,
    postings: HashMap<SmolStr, Vec<u32>>,  // prefix → entry indices
    skip: SkipSet,
    roots: Vec<PathBuf>,
}

pub struct IndexEntry {
    path: PathBuf,
    name: String,
    kind: FileKind,
    depth: u8,
}

pub struct ScoredEntry {
    entry_idx: u32,
    score: i32,
}
```

**Indexing rules (from Q3, locked B + ii + 1M cap):**
- Roots from `config.launcher.sources.files.roots` (defaults to `[$HOME]`).
- Walk via `ignore::WalkBuilder` honoring gitignore + `SKIP_DIRS`.
- Tokenize **filename** by `[\s_\-.]`, lowercase, and **also** tokenize each path component. Index prefixes up to 12 chars per token.
- Hard cap `max_entries = 1_000_000` (configurable in `config.launcher.sources.files.max_entries`).
- Skip files larger than 1 GiB (avoid indexing huge binaries' name token but they're indexed by path).

**Concurrency model (from Q4, locked B + i):**
- `FileSearchSource` holds `Arc<ArcSwap<InvertedFileIndex>>`. Readers do `.load()` (zero-cost), never block.
- Initial build at startup BFS-walks roots, yielding to the runtime every 200 dirs via `tokio::task::yield_now`. Progressive availability: search returns from the partial index during build.
- `SourceFileWatcher` is rewired for the file source: instead of calling `refresh()`, it calls a new `apply_events(events)` that produces a *patched* clone via copy-on-write of changed postings, then `ArcSwap::store`. Rename = remove + add.
- Background task triggers a full rebuild every 30 minutes as a safety net for dropped FSEvents (after sleep/wake or under heavy I/O).

**Search algorithm:**
1. Tokenize query the same way as indexing.
2. For each query token, look up postings list. If any token has zero hits → return empty.
3. Sort lists by length ascending, intersect into a candidate set (textbook IR optimization).
4. Score each candidate via `score_entry_match`: exact-name +140, prefix +118, compact-prefix +106, subsequence +44, depth penalty −2 per level.
5. Sort by score desc, return top-`limit` (default 80).

**mdfind fallback (from Q5, locked B(N=20) + i + iii + skip_dirs filter):**
- After local search, if `local_results.len() < 20`, spawn one `mdfind` per root (`-onlyin <root> <query>`) with a 2-second `tokio::time::timeout`.
- Cancel via shared `CancellationToken` if a new keystroke arrives.
- Filter mdfind hits through the same `SkipSet` (drops `~/Library/Caches/...` etc.).
- Re-score with `score_entry_match` minus 5-point bias.
- Merge into local results, dedupe by canonical path, re-sort.

**Configuration:**
```jsonc
"launcher": {
  "sources": {
    "files": {
      "roots": ["$HOME"],         // existing
      "max_entries": 1000000,     // new
      "mdfind_fallback": true,    // new, default true
      "mdfind_threshold": 20,     // new, default 20
      "rebuild_interval_min": 30  // new, default 30
    }
  }
}
```

**Tests:**
- Unit: tokenize, score_entry_match, intersect-shortest-first.
- Unit: apply_events for create/remove/rename, cap eviction (oldest-by-add-order).
- Integration (`#[cfg(target_os = "macos")]`): create unique-name file in temp dir, run `mdimport`, assert mdfind merge picks it up.
- Bench (criterion, optional): query latency on a 100K-entry synthetic index.

### 2. LauncherExecuteResult + no-view + status badge

**New types in `crates/feature-launcher/src/types.rs`:**
```rust
pub struct LauncherExecuteResult {
    pub status: ExecStatus,
    pub message: Option<String>,
    pub badge: BadgeKind,
}

pub enum ExecStatus { Ok, Err(String) }
pub enum BadgeKind { Success, Warn, Error, Info }

impl From<()> for LauncherExecuteResult { /* Ok, no message, Info */ }
impl From<common::Result<()>> for LauncherExecuteResult { /* trivial */ }
```

**`LauncherItem` change:**
```rust
pub struct LauncherItem {
    // existing fields...
    pub no_view: bool,           // new, default false
    pub arguments: Vec<ArgSpec>, // new (used by feature 3), default empty
}
```

**IPC change (`crates/desktop/src/commands/launcher.rs`):**
- `launcher_execute(item_id, kind, args)` returns `LauncherExecuteResult` instead of `()`.
- All existing source `execute` paths get `Ok(())` → `LauncherExecuteResult::ok()` via blanket `From`. No source-side change required.

**Status badge UI:**
- New Tauri command `show_status_badge(text: String, kind: BadgeKind, duration_ms: u32)` in a new `commands/status_badge.rs`. Don't forget `DEV_COMMANDS` export (per CLAUDE.md gotcha).
- Spawns a frameless 280×40 always-on-top window anchored top-right of the focused screen, fades out after `duration_ms` (default 2000).
- Reuses `glass-panel` Tailwind class for visual consistency.
- If a badge is already showing, replace its content (don't queue).

**Frontend behavior:**
- `useLauncherExecute` mutation: on success, if `item.no_view === true`, call `closeLauncher()` then `invoke('show_status_badge', ...)` with `result.message ?? item.title`.
- If `result.message` is absent and item has no name worth showing (e.g. window preset), suppress badge entirely.

**Tests:**
- Backend: unit test that `From<()>` produces an `Ok` result.
- Backend: integration test that `launcher_execute` returns structured result.
- Frontend: Vitest test that `no_view: true` triggers `closeLauncher` + `show_status_badge`.
- Manual: visual check of badge window position on multi-display.

### 3. Inline arguments

**New types:**
```rust
pub struct ArgSpec {
    pub name: String,
    pub placeholder: String,
    pub kind: ArgKind,
    pub required: bool,
}

pub enum ArgKind {
    Text,
    Number,
    Choice(Vec<String>),
}
```

**Template substitution (from Q6, locked A + ii + v):**
- `LauncherSearchEngine::execute(item_id, kind, args: HashMap<String, String>)` performs `{{name}}` replacement on string fields of the `kind` payload before dispatching to the kind-specific handler. Implemented once, transparent to every source.
- Missing required args → return `LauncherExecuteResult::err("Missing argument: {name}")`.
- Frontend should not call execute with missing required args; this is a defense in depth.

**Initial consumers:**
- `ScriptRunner` — parse `# arg: name=placeholder` front-matter lines, populate `ArgSpec`s; script body can reference `{{name}}` in its filename or interpreter args.
- `SystemCommandsSource` — DND command gets `duration` arg (`30m`, `2h`, `until tomorrow`), forwarded to `shortcuts run "Set DND"` with the duration baked into the AppleScript template.

**Frontend `ArgChipBar`:**
- New component in `desktop-ui/src/features/launcher/components/ArgChipBar.tsx`.
- Renders one chip per `ArgSpec` inline in the search bar row.
- Tab / Shift+Tab navigates between chips. Enter on last chip submits.
- Backspace from an empty chip returns to the results list (cancels arg mode).
- Placeholder text shown when chip is empty; disabled state for `Choice` chips uses a small inline dropdown.
- `Number` kind validates with HTML `inputMode="numeric"` plus a `parseFloat` check before submit.

**Tests:**
- Backend unit test for template substitution with: simple replace, missing arg, escaped `{{ }}` (literal), nested args.
- Frontend Vitest test for chip Tab navigation, Enter submit, Backspace cancel.

### 4. Window-management presets

**Data table** in `crates/feature-launcher/src/window_mgmt/presets.rs`:
```rust
pub struct Preset {
    pub name: &'static str,        // canonical id, e.g. "left-third"
    pub display_name: &'static str, // user-facing, e.g. "Left Third"
    pub keywords: &'static [&'static str],
    pub frame: PresetFrame,
}
pub struct PresetFrame { pub x: f32, pub y: f32, pub w: f32, pub h: f32 }

pub const PRESETS: &[Preset] = &[
    // halves (4)
    Preset { name: "left-half",   display_name: "Left Half",   keywords: &["left","half"],   frame: PresetFrame{x:0.0, y:0.0, w:0.5, h:1.0} },
    Preset { name: "right-half",  display_name: "Right Half",  keywords: &["right","half"],  frame: PresetFrame{x:0.5, y:0.0, w:0.5, h:1.0} },
    Preset { name: "top-half",    display_name: "Top Half",    keywords: &["top","half","up"],   frame: PresetFrame{x:0.0, y:0.0, w:1.0, h:0.5} },
    Preset { name: "bottom-half", display_name: "Bottom Half", keywords: &["bottom","half","down"], frame: PresetFrame{x:0.0, y:0.5, w:1.0, h:0.5} },
    // thirds (6): 1/3 wide left/center/right, 2/3 wide left/right, full middle column
    // quarters (4 corners)
    // maximize, almost-maximize (5% margin), center, restore
    // total: 25
];
```

(Full table populated during PR-4; the design above shows the shape and ~6 of the 25 entries.)

**Action variant** in `window_mgmt/actions.rs`:
```rust
pub enum WindowAction {
    // existing variants kept for back-compat
    Preset(&'static str),  // new
}
```

**Engine (`WindowManager::apply_preset`):**
1. Look up preset by name; bail with `LauncherExecuteResult::err` if not found.
2. Get focused window via existing `platform_macos::window::get_frontmost_window()`.
3. Get the screen frame for the screen containing the window's center.
4. Compute absolute rect: `Rect { x: screen.x + frame.x * screen.w, ... }`.
5. Cache pre-preset frame in a `DashMap<WindowId, Rect>` (LRU 50 entries) for `restore`.
6. Call existing `set_window_frame(window, rect)`.

**Discoverability (`WindowPresetsSource`):**
- New file `crates/feature-launcher/src/search/window_presets.rs`.
- Implements `SearchSource` with `prefix: Some("w/")`.
- On `search`, fuzzy-matches query against each preset's `display_name` and `keywords` via `nucleo_matcher`.
- Emits `LauncherItem { kind: WindowAction(WindowAction::Preset(name)), no_view: true, ... }`.
- Wired in `app-core/src/init/launcher.rs` behind `config.launcher.sources.window_presets` (default true).

**Tests:**
- Unit: every preset's rect calculation against a 1920×1080 mock screen.
- Unit: keyword fuzzy match returns expected preset for queries ("left" → Left Half top, "1/3 right" → Right Third).
- Integration: `apply_preset("center")` round-trip on a real window (gated to manual run; flaky in CI).

## Cross-Cutting Concerns

**Migrations:** No schema changes. The dead `launcher_pins` table is left alone (out of scope; would also be a small follow-up).

**Backward compatibility:** Every type change is additive. Every old IPC return becomes wrapped in `LauncherExecuteResult` via `From<()>`. The `WindowAction::Preset` variant is new but old variants stay; existing hotkey configs continue to work.

**Performance budgets:**
- File index initial build: ≤ 5 s for 500K entries on M-series Mac (yielded BFS, no blocking).
- Search latency: ≤ 20 ms for any query against a 1M-entry index (P95).
- mdfind fallback: ≤ 2 s timeout, cancellable.
- Status badge window open → visible: ≤ 50 ms.

**Observability:** Use existing `tracing` macros. New events: `launcher.index.built {entries, duration_ms}`, `launcher.index.applied_event {kind, ms}`, `launcher.mdfind {query, hits, ms}`, `launcher.preset.applied {name, ms}`. No metrics infra (per CLAUDE.md non-goals).

**Telemetry & privacy:** No query strings logged at info level. Path values logged only at debug.

## Sequencing

| Sub-PR | Depends on | Lands |
|--------|-----------|-------|
| PR-1 InvertedFileIndex | — | Core index, mdfind, FSEvent integration. No UI. |
| PR-2 ExecuteResult + no-view + badge | — (parallel with PR-1) | Type refactor, badge window, frontend hookup. |
| PR-3 Inline arguments | PR-2 | `ArgSpec`, template substitution, `ArgChipBar`, ScriptRunner+SystemCommands consumers. |
| PR-4 WindowPresetsSource | PR-2, PR-3 | 25 presets, `WindowAction::Preset`, source registration. |

PR-1 and PR-2 can land in parallel (different files, no overlap). PR-3 needs PR-2's `LauncherExecuteResult` shape. PR-4 depends on both for `no_view` and the searchable-source pattern.

Each PR ships independently green: `cargo nextest run --workspace`, `cargo clippy --workspace --all-targets --all-features` zero warnings, `cd desktop-ui && bun run lint && bun run test`.

## Risks & Mitigations

| Risk | Mitigation |
|------|-----------|
| FSEvents drops events during sleep/wake → index drift | 30-min full safety rebuild |
| mdfind subprocess pile-up under fast typing | shared `CancellationToken`, 2 s timeout, single in-flight per root |
| `ArcSwap` pinning surprise readers across multi-second searches | Search drops the `Arc<...>` reference at end of fn; old indexes free naturally |
| Argument template injection (script args containing `{{...}}`) | Substitute exactly once, single pass; no recursive expansion |
| Status badge window steals focus | Use `set_focusable(false)` on the Tauri WebviewWindow |
| Window preset on multi-display picks wrong screen | Use window's center point, not top-left, to choose screen |
| 25-preset table grows organically into 50+ | Acceptable — data-driven design absorbs growth |

## Out of Scope (explicit)

- **MCP tool exposure** of launcher operations. Strong follow-up but separate spec.
- **Snippet expansion** and **Hyper Key** — Spec 2 (System-Wide Input Services).
- **Display-targeted moves** (`pin to display 2`) — possible PR-5.
- **Nudge/resize family** (10 px directional) — possible PR-5.
- **`File` kind** for `ArgKind` (file picker) — defer until a real consumer asks.
- **Frecency rework / cross-source score normalization** — separate concern, separate spec.
- **Cleaning up dead `launcher_pins` / `launcher_frequencies` schema** — orthogonal.

## Acceptance Criteria

- File search latency on 500K-entry corpus: P95 ≤ 20 ms.
- mdfind fallback returns at least one privacy-excluded path that local index misses (manual: file in `~/Library/Application Support/`).
- DND command with `2h` arg via inline chip closes launcher and shows "DND on for 2h" badge.
- Typing `w/left half` and pressing Enter snaps focused window to left half of current screen, no panel left open.
- All 4 PRs shippable independently; main branch never broken.
