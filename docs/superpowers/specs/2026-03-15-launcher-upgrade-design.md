# Launcher Upgrade Design Spec

**Date:** 2026-03-15
**Status:** Approved (brainstorming)
**Goal:** Transform the launcher from a basic AI chat window into a full command center — fast utility actions, deep productivity integration, and AI only where it genuinely adds value.

## Design Principles

- **Speed first.** Dashboard renders in <16ms (cached data). Search results in <30ms for local providers, <80ms including SQLite.
- **AI where it earns its keep.** Activity categorization batch classification, not sprinkled everywhere.
- **Glanceable.** Dashboard shows what matters without typing. 50 opens/day should feel lightweight.
- **Keyboard-native.** Every action reachable without a mouse. Vim bindings alongside arrows.
- **Reuse existing infrastructure.** Activity tracking, categorization, and calendar already exist in `feature-productivity` and `activity-log`. The launcher surfaces this data, not duplicates it.

## Architecture Overview

### Single Process (Approach B)

All background services run as `tokio::spawn` tasks within the existing Tauri process. The app already runs persistently (tray + global shortcut). No separate daemon needed.

Background tasks:
- **App indexing** — walk `/Applications` on startup + FSEvents watcher
- **Clipboard monitoring** — poll `NSPasteboard.changeCount` every 500ms
- **Activity tracking** — delegates to existing `feature-productivity` tracker (already polls frontmost app)
- **Calendar refresh** — delegates to existing calendar sync in `app-core`
- **Script discovery** — watch `~/.klyntbot/scripts/` via `notify` crate
- **Batch AI classification** — hourly + on focus session end (extends existing categorization pipeline)

### Crate Architecture

The launcher spans two layers, following the existing "app-core + thin adapters" pattern:

**`feature-launcher` (L4)** — Data types, storage (frequencies, clipboard), provider interfaces, app indexing, clipboard management, window management, calculator, script runner, system commands. No dependency on other L4/L5 crates.

**`app-core` (L7)** — `LauncherService` orchestration and `LauncherSearchEngine` live here, where they can access all repos (`TaskRepo`, `NoteRepo`, `ProductivityRepo`, etc.) and delegate to the agent for batch classification. This matches how `AppCore` already orchestrates other features.

```
crates/feature-launcher/
├── src/
│   ├── lib.rs                ← FeaturePackage impl
│   ├── types.rs              ← LauncherItem, LauncherItemKind, SearchResult
│   ├── search/
│   │   ├── mod.rs            ← SearchProvider trait, frequency ranking logic
│   │   ├── app_index.rs      ← walk dirs, fuzzy match, FSEvents watch
│   │   ├── calculator.rs     ← try_eval() using meval
│   │   └── frequency.rs      ← FrequencyTracker: read/write launcher_frequencies table
│   ├── clipboard/
│   │   ├── mod.rs            ← ClipboardManager: poll, store, search
│   │   └── paste.rs          ← write-to-pasteboard + simulate ⌘V
│   ├── window_mgmt/
│   │   ├── mod.rs            ← WindowManager: get frontmost, set frame
│   │   ├── actions.rs        ← snap commands (half, third, maximize, cycle)
│   │   └── accessibility.rs  ← AXUIElement wrappers (raw FFI, same approach as feature-productivity/tracker/macos.rs)
│   ├── scripts/
│   │   └── mod.rs            ← ScriptRunner: watch dir, parse metadata, execute
│   ├── system_commands.rs    ← static actions + execution
│   └── migrations.rs         ← FeatureMigration for frequencies + clipboard tables

crates/app-core/src/handlers/launcher/
├── mod.rs                    ← LauncherService: starts/stops background tasks
├── search_engine.rs          ← LauncherSearchEngine: fan-out across all providers + merge + rank
└── dashboard.rs              ← DashboardData aggregation from existing repos
```

## Relationship to Existing Infrastructure

### Activity Tracking — reuse `feature-productivity` + `activity-log`

The codebase already has:
- `activity_events` table in `feature-productivity` with columns: `app_name`, `bundle_id`, `window_title`, `category_id`, `focus_session_id`, `started_at`, `ended_at`, `duration_secs`
- `activity_categories` table with 18 seeded categories and JSON rules for app/URL matching
- `productivity_tracking_rules` and `productivity_categorization_cache` for rule-based + cached AI classification
- `activity-log` crate for activity ingestion and work context tracking
- `feature-productivity/tracker/macos.rs` with existing frontmost app polling via raw AXUIElement FFI

**No new activity tracking code in `feature-launcher`.** The launcher reads from existing `activity_events` and `activity_categories` via `ProductivityRepo`. The dashboard aggregates this data for display.

### Category Display Grouping

The existing 18 categories are grouped into 8 display groups for the launcher dashboard:

| Display Group | Color | Existing Categories |
|---------------|-------|-------------------|
| Coding | Blue | coding, developer_tools, cloud_devops |
| Communication | Purple | communication, email |
| Documentation | Teal | documentation, project_management |
| Research | Orange | browsing, news_forums |
| Learning | Green | learning |
| Design | Pink | design |
| Entertainment | Red | social_media, video_streaming, gaming, entertainment, music, shopping, finance |
| Other | Gray | ai_tools, uncategorized |

This mapping lives in `app-core` as a simple function — no new table needed.

### Calendar — reuse existing sync

The codebase already has `calendar_events` table, `CalendarEventRepo`, and sync handlers in `app-core/src/handlers/productivity/calendar.rs`. The launcher reads from `CalendarEventRepo` for dashboard display and search results. No new EventKit bridge needed.

### Batch AI Classification — extend existing pipeline

The existing `productivity_categorization_cache` and categorization pipeline handle AI classification. The launcher adds a trigger: when a focus session ends, queue a batch classification job for unclassified events from that session. This extends the existing pipeline rather than creating a parallel one.

## Search Pipeline

```
User input
  → frontend debounce (50ms)
  → launcher_search(query) Tauri command
  → LauncherSearchEngine (in app-core)
      ├─ AppIndex::search(query)        → Vec<SearchResult>  (in-memory)
      ├─ ClipboardHistory::search(query)→ Vec<SearchResult>  (FTS5)
      ├─ TaskRepo::search(query)        → Vec<SearchResult>  (SQLite)
      ├─ NoteRepo::search(query)        → Vec<SearchResult>  (SQLite)
      ├─ SystemCommands::search(query)  → Vec<SearchResult>  (in-memory)
      ├─ ScriptRunner::search(query)    → Vec<SearchResult>  (in-memory)
      ├─ Calculator::try_eval(query)    → Option<SearchResult>
      └─ merge + rank by (score × frequency_boost)
  → Vec<LauncherItem> returned to frontend
```

All providers searched concurrently via `tokio::join!`. Fast providers (apps, commands, calculator) return first; slower ones (SQLite queries) append.

### Result Types

```rust
enum LauncherItemKind {
    Application { path: PathBuf, running: bool },
    Task { id: String, status: TaskStatus },
    Note { id: String, preview: String },
    ClipboardEntry { id: u64, content_type: ClipboardContentType },
    SystemCommand { action: SystemAction },
    Script { path: PathBuf, name: String },
    Calculator { expression: String, result: f64 },
    Calendar { event_id: String, starts_at: DateTime<Utc> },
    AiChat { query: String },  // fallback: "Ask Klynt AI: {query}"
}
```

AI chat always appears last as a fallback.

### Ranking

Each provider returns results with a base relevance score (0.0–1.0). The engine multiplies by a frequency boost: `score × log2(count + 1)`. Frequency data stored in `launcher_frequencies` table.

### Input Prefixes

| Prefix | Mode | Example |
|--------|------|---------|
| (none) | Universal search | `vscode` |
| `=` | Calculator | `=sqrt(144) + 3` |
| `>` | System commands | `>lock` |
| `/` | Script runner | `/deploy staging` |
| `@` | AI chat (skip search) | `@summarize my day` |

**Prefix precedence:** `=` always routes to calculator mode exclusively. Without a prefix, if a query starts with a digit or `(`, `Calculator::try_eval()` attempts evaluation — if it returns `Ok`, the result is included alongside other search results (not exclusively). If `try_eval()` returns `Err`, it's silently skipped. This means `3+4` shows a calculator result AND any matching items, while `3d printer` only shows matching items (because `3d printer` fails to parse).

**Note:** `/` prefix routes to script runner, not file paths. File content search is a non-goal (Spotlight handles it).

## Dashboard Glance View

Shown on ⌥Space before typing. Contextual widgets, top to bottom:

```
┌─────────────────────────────────────────┐
│  Search or type a command...            │  ← input, always on top
├─────────────────────────────────────────┤
│  Focus: "Implement launcher" · 23:45    │  ← if focus session active
├─────────────────────────────────────────┤
│  Team standup · in 12 min               │  ← next 1-2 events within 2h
│  Design review · 3:00 PM               │
├─────────────────────────────────────────┤
│  ☐ Fix search ranking bug              │  ← top 3 tasks by priority
│  ☐ Write launcher design spec          │
│  ☐ Review PR #42                        │
├─────────────────────────────────────────┤
│  Today: 4h 12m · Coding 68% · 82       │  ← productivity footer
└─────────────────────────────────────────┘
```

### Widget Rules

- **Focus widget** — only during active focus sessions. Shows task name + time. Click to complete/extend.
- **Calendar widget** — next 1-2 events within 2 hours. Hidden if nothing upcoming. Panel shrinks.
- **Tasks widget** — top 3 from current project/area. Click to complete, ⌘Enter to open in main window.
- **Productivity bar** — total time, top category %, score with color (green 70+, yellow 40-69, red <40).

### Data Push

Backend pushes via Tauri events, frontend caches in store:
- `launcher:focus_update` — every 1s during focus
- `launcher:calendar_update` — on show + every 15min
- `launcher:tasks_update` — on show + on task changes
- `launcher:productivity_update` — on show + every 5min

## Productivity Score

Uses existing `activity_events` data aggregated through the display group mapping:

```
productive_minutes = Coding + Documentation + Research + Learning + Design groups
score = (productive_minutes / total_tracked_minutes) × 100
```

Users customize which display groups count as productive in settings. Default: Coding, Documentation, Research, Learning, Design are productive.

### Launcher Productivity Integration

- **Dashboard footer:** `Today: 4h 12m · Coding 68% · 82`
- **"productivity" search command:** Today's breakdown by display group, streak, weekly comparison
- **Focus session end:** Summary of apps used, category breakdown, on-task assessment

## Feature Implementations

### App Launcher
- Index `/Applications`, `~/Applications`, `/System/Applications` (depth 3)
- `nucleo-matcher` crate for Sublime-style fuzzy scoring (actively maintained, used by Helix/Zed editors)
- `notify` crate for directory watching
- Launch via `open -a`

### Clipboard History
- Poll `NSPasteboard.general.changeCount` every 500ms via `objc2`
- Store last 1000 entries in `clipboard_history` table (FIFO eviction for unpinned entries when cap reached)
- FTS5 for search with sync triggers (see Database Schema)
- Images stored in `{data_dir}/clipboard-images/` with content-hash filenames. Max 5MB per image. Total image storage capped at 500MB with LRU eviction of unpinned entries.
- Paste: write to pasteboard + simulate ⌘V via CGEvent

### Calculator
- `meval` crate for expression evaluation (add to `feature-launcher/Cargo.toml`)
- `=` prefix: calculator-only mode, shows result exclusively
- Auto-detect (no prefix, starts with digit/`(`): `try_eval()` attempts parse, result shown alongside other search results if successful, silently skipped if not
- Enter copies result to clipboard

### Window Management
- Raw FFI to `AXUIElement` functions, same approach as existing `feature-productivity/tracker/macos.rs`
- Commands: Left/Right/Top/Bottom Half, Thirds, Maximize, Center, Restore
- **Cycling detection:** Track last action per window in an in-memory `HashMap<CGWindowID, (WindowAction, Instant)>`. If the same action is invoked on the same window within 2 seconds and the window's current frame matches the previous snap position (within 5px tolerance), cycle to next size variant (half → third → two-thirds → half).
- Configurable global keyboard shortcuts (registered alongside ⌥Space)

### Calendar
- Reads from existing `CalendarEventRepo` (populated by existing calendar sync in `app-core`)
- Dashboard shows next 1-2 events within 2 hours, search shows today's full schedule
- No new native bridge needed

### System Commands
- Hardcoded list: Lock, Sleep, Restart, Shutdown, Empty Trash, Toggle Dark Mode, Toggle DND, Eject All
- Implemented via `osascript`, `pmset`, `CGSession`, `defaults write`

### Script Runner
- Watch `~/.klyntbot/scripts/` via `notify`
- Parse metadata comments: `# name:`, `# icon:`, `# description:`
- Execute in background process, toast on completion/failure

## macOS Permissions

The launcher requires several macOS permissions. The app should request them progressively (not all at once) and degrade gracefully when denied.

| Permission | Required For | When to Prompt | Fallback When Denied |
|------------|-------------|----------------|---------------------|
| Accessibility | Window title reading (activity tracking), window management | On first use of window management or when activity tracking is enabled | Activity tracking works but without window titles (app name only). Window management is disabled with a tooltip explaining why. |
| Automation (AppleScript) | System commands, script runner | On first system command or script execution | Show error toast: "Grant Automation permission in System Settings to use this command" |

**First-run flow:** No upfront permission dialogs. When the user first triggers a feature requiring permissions, show an in-launcher explanation card (not a system dialog) explaining what's needed and why, with a button that opens System Settings to the relevant pane. Track granted permissions in config to avoid re-prompting.

## Keyboard Navigation

| Key | Action |
|-----|--------|
| `↑/↓` | Navigate results |
| `⌃J/⌃K` | Navigate results (vim) |
| `Enter` | Execute selected item |
| `⌘Enter` | Execute + keep launcher open |
| `Tab` | Show item detail/actions panel |
| `Escape` | Clear input → dashboard → hide (3 levels) |
| `⌘/` | Expand to main window |
| `⌘C` | Copy result value |
| `⌘⇧C` | Copy & paste clipboard entry |
| `↑` (empty input) | Query history recall |

### Result Item Actions (Tab to reveal)

- **App:** Launch · Reveal in Finder · Force Quit
- **Task:** Complete · Open in main · Edit · Start focus
- **Note:** Open in main · Copy content · Quick edit
- **Clipboard:** Paste · Copy · Delete · Pin
- **Calendar:** Open Calendar · Copy link · Join call
- **Script:** Run · Edit · Copy path

## Frontend Architecture

### File Structure

```
desktop-ui/src/features/launcher/
├── pages/LauncherPage.tsx
├── components/
│   ├── LauncherInput.tsx
│   ├── Dashboard.tsx
│   ├── DashboardFocus.tsx
│   ├── DashboardCalendar.tsx
│   ├── DashboardTasks.tsx
│   ├── DashboardProductivity.tsx
│   ├── ResultsList.tsx
│   ├── ResultItem.tsx
│   ├── ResultActions.tsx
│   └── LauncherChat.tsx
├── hooks/
│   ├── useLauncherSearch.ts
│   ├── useDashboardData.ts
│   ├── useKeyboardNavigation.ts
│   └── useQueryHistory.ts
├── stores/launcherStore.ts
└── types.ts
```

### State Machine

```
Modes: dashboard | search | detail | chat

⌥Space → DASHBOARD
  typing → SEARCH
    Tab → DETAIL
    @ prefix / "Ask AI" → CHAT
  Escape chain: detail → search → dashboard → hide
```

### Migration from Current Code

- Move `LauncherChat.tsx` from `features/tray/components/` to `features/launcher/components/`
- Delete orphaned `features/chat/pages/LauncherChatPage.tsx`
- Full rewrite of `LauncherPage.tsx` — current hardcoded items replaced by state machine + sub-components

### New CSS Variables

- `--launcher-widget-gap` — dashboard widget spacing
- `--launcher-result-height` — fixed row height for virtualization

## Database Schema (New Tables)

Only two new tables — activity tracking reuses existing `activity_events`.

```sql
-- Frequency learning for search ranking
CREATE TABLE launcher_frequencies (
    item_id TEXT NOT NULL,
    kind TEXT NOT NULL,       -- 'app', 'task', 'command', 'script', etc.
    count INTEGER DEFAULT 0,
    last_used TIMESTAMP,
    PRIMARY KEY (item_id, kind)
);

-- Clipboard history
CREATE TABLE clipboard_history (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    content TEXT NOT NULL,
    content_type TEXT NOT NULL,  -- 'text', 'image', 'file'
    source_app TEXT,
    preview TEXT,                -- first 200 chars for display
    file_path TEXT,              -- for images stored in {data_dir}/clipboard-images/
    pinned BOOLEAN DEFAULT FALSE,
    created_at TIMESTAMP NOT NULL
);

-- FTS5 index for clipboard search with sync triggers
CREATE VIRTUAL TABLE clipboard_fts USING fts5(
    content, preview, content='clipboard_history', content_rowid='id'
);

-- Triggers to keep FTS5 in sync with base table
CREATE TRIGGER clipboard_fts_insert AFTER INSERT ON clipboard_history BEGIN
    INSERT INTO clipboard_fts(rowid, content, preview)
    VALUES (new.id, new.content, new.preview);
END;

CREATE TRIGGER clipboard_fts_delete AFTER DELETE ON clipboard_history BEGIN
    INSERT INTO clipboard_fts(clipboard_fts, rowid, content, preview)
    VALUES ('delete', old.id, old.content, old.preview);
END;

CREATE TRIGGER clipboard_fts_update AFTER UPDATE ON clipboard_history BEGIN
    INSERT INTO clipboard_fts(clipboard_fts, rowid, content, preview)
    VALUES ('delete', old.id, old.content, old.preview);
    INSERT INTO clipboard_fts(rowid, content, preview)
    VALUES (new.id, new.content, new.preview);
END;
```

## Data Retention

- **Clipboard history:** Capped at 1000 entries. When cap reached, oldest unpinned entries evicted (FIFO). Pinned entries are never auto-evicted. Image files cleaned up when their clipboard entry is evicted.
- **Activity events:** Managed by existing `feature-productivity` retention policy. Raw events kept for 90 days, daily summaries kept indefinitely.
- **Launcher frequencies:** No eviction. Grows proportionally to unique items used (bounded).

## Tauri Commands

```rust
// Search & execute
launcher_search(query: String) -> Vec<LauncherItem>
launcher_execute(item_id: String, kind: String) -> ()
launcher_dashboard() -> DashboardData

// Clipboard
launcher_clipboard_paste(id: u64) -> ()
launcher_clipboard_delete(id: u64) -> ()
launcher_clipboard_pin(id: u64) -> ()

// Window management
launcher_window_action(action: WindowAction) -> ()

// Scripts & system
launcher_run_script(path: String) -> ()
launcher_system_command(action: SystemAction) -> ()
```

All thin adapters in `crates/desktop/src/commands/launcher.rs` delegating to `AppCore` → `LauncherService`. The module must export `pub const DEV_COMMANDS: &[&str]` listing all command names (enforced by `dev_server_covers_all_tauri_commands` test).

## Performance Budget

| Operation | Target | Strategy |
|-----------|--------|----------|
| Dashboard render | <16ms | Cached data, single frame |
| Local search (apps, commands, calc) | <30ms | In-memory indexes |
| Full search (incl. SQLite) | <80ms | FTS5, concurrent fan-out via `tokio::join!` |
| Activity poll | ~0 CPU | Handled by existing feature-productivity tracker |
| Clipboard poll | ~0 CPU | 500ms interval, changeCount check only |
| App re-index | <1s | Only on FSEvents notification |
| LLM batch classify | background | Hourly, non-blocking, extends existing pipeline |

## Non-Goals

- No plugin/extension API (scripts cover custom automation)
- No file content search (filename only, Spotlight handles content)
- No real-time AI classification (batch only)
- No window management GUI settings (keyboard shortcuts configured in config.json)
- No new EventKit native bridge (reuse existing calendar sync)
- No new activity tracking storage (reuse existing `activity_events` table)
