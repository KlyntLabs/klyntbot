# Productivity Tracking & Focus Management — Design

**Status:** Approved
**Date:** 2026-03-04
**Inspired by:** [Rize.io](https://rize.io/) — reimagined as an AI-agent-integrated system

## Overview

Add Rize-like productivity tracking to Klyntbot: passive activity monitoring, focus session management, proactive AI nudges, and a dedicated productivity dashboard. Unlike Rize (a passive observer), Klyntbot acts on the data — contextualizing agent responses, proactively nudging, and integrating with the existing task/project/calendar system.

## Architecture: `feature-productivity` Crate

Follows the `FeaturePackage` pattern (like `feature-todo`, `feature-finance`). Self-contained crate with own migrations, tools, config, and health checks.

### Key Design Decisions

- **Passive + Active tracking**: Always-on background window monitoring PLUS explicit focus sessions
- **Tracker lives inside Tauri app**: Uses macOS native APIs from the Tauri Rust backend. Runs only when desktop app is running.
- **Proactive nudges**: AI decides when to nudge (break reminders, focus suggestions, burnout alerts, daily summaries)
- **Soft distraction blocking**: Gentle notification when opening distracting apps during focus — dismissible, not enforced
- **New dedicated Productivity view**: Full-width dashboard in the desktop app sidebar
- **No focus music**: Deferred to future iteration

## 1. Data Model

### `activity_events` — Raw Activity (high-frequency)

```sql
CREATE TABLE activity_events (
    id            INTEGER PRIMARY KEY AUTOINCREMENT,
    app_name      TEXT NOT NULL,
    window_title  TEXT,
    bundle_id     TEXT,
    url           TEXT,
    category_id   TEXT REFERENCES activity_categories(id),
    started_at    TEXT NOT NULL,
    ended_at      TEXT,
    duration_secs INTEGER,
    is_idle       BOOLEAN NOT NULL DEFAULT FALSE,
    metadata      TEXT
);

CREATE INDEX idx_activity_events_started ON activity_events(started_at DESC);
CREATE INDEX idx_activity_events_category ON activity_events(category_id);
```

- Polled every 5 seconds, batch-written every 30 seconds
- Raw events retained for 90 days (configurable), then purged
- Window titles stored locally only, never sent to LLM

### `activity_categories` — Categorization Rules

```sql
CREATE TABLE activity_categories (
    id            TEXT PRIMARY KEY,
    name          TEXT NOT NULL,
    category_type TEXT NOT NULL DEFAULT 'productive',  -- productive / neutral / distracting
    color         TEXT,
    icon          TEXT,
    rules         TEXT,  -- JSON: {app_names: [], bundle_ids: [], url_patterns: []}
    is_system     BOOLEAN NOT NULL DEFAULT FALSE,
    created_at    TEXT NOT NULL DEFAULT (datetime('now'))
);
```

Categorization pipeline: rule-based match → AI inference (async, batch) → user correction (updates rules).

### `focus_sessions` — Explicit Deep Work

```sql
CREATE TABLE focus_sessions (
    id            TEXT PRIMARY KEY,
    action_id     TEXT REFERENCES actions(id),
    project_id    TEXT REFERENCES projects(id),
    session_type  TEXT NOT NULL DEFAULT 'focus',  -- focus / pomodoro / break
    target_mins   INTEGER,
    started_at    TEXT NOT NULL,
    ended_at      TEXT,
    actual_mins   INTEGER,
    interruptions INTEGER NOT NULL DEFAULT 0,
    distraction_events TEXT,  -- JSON array
    quality_score REAL,       -- 0-1
    completed     BOOLEAN NOT NULL DEFAULT FALSE,
    notes         TEXT
);
```

Links to existing `actions` and `projects` tables.

**Quality score formula:**
```
quality = (on_task_ratio * 0.6) + (focus_continuity * 0.3) + (completion_bonus * 0.1)
```

### `daily_summaries` — Aggregated Daily Data

```sql
CREATE TABLE daily_summaries (
    date                 TEXT PRIMARY KEY,
    total_active_secs    INTEGER NOT NULL DEFAULT 0,
    total_focus_secs     INTEGER NOT NULL DEFAULT 0,
    total_break_secs     INTEGER NOT NULL DEFAULT 0,
    total_idle_secs      INTEGER NOT NULL DEFAULT 0,
    productive_secs      INTEGER NOT NULL DEFAULT 0,
    neutral_secs         INTEGER NOT NULL DEFAULT 0,
    distracting_secs     INTEGER NOT NULL DEFAULT 0,
    focus_sessions_count INTEGER NOT NULL DEFAULT 0,
    avg_session_quality  REAL,
    interruptions_count  INTEGER NOT NULL DEFAULT 0,
    context_switches     INTEGER NOT NULL DEFAULT 0,
    top_apps             TEXT,       -- JSON
    top_categories       TEXT,       -- JSON
    ai_summary           TEXT,
    computed_at          TEXT NOT NULL DEFAULT (datetime('now'))
);
```

Computed nightly from `activity_events`. Also recomputable on demand.

### `nudge_history` — Prevents Over-nudging

```sql
CREATE TABLE nudge_history (
    id            INTEGER PRIMARY KEY AUTOINCREMENT,
    nudge_type    TEXT NOT NULL,
    message       TEXT NOT NULL,
    channel       TEXT,
    acknowledged  BOOLEAN NOT NULL DEFAULT FALSE,
    created_at    TEXT NOT NULL DEFAULT (datetime('now'))
);

CREATE INDEX idx_nudge_history_type_created ON nudge_history(nudge_type, created_at DESC);
```

## 2. Activity Tracker (macOS Native)

Runs as a background `tokio` task inside the Tauri Rust backend.

**Window capture (macOS):**
- `NSWorkspace.sharedWorkspace().frontmostApplication()` → app name + bundle ID
- `CGWindowListCopyWindowInfo` → active window title
- `AXUIElement` accessibility API as fallback
- Crates: `objc2`, `objc2-app-kit`

**Polling:** Every 5 seconds (~0.1% CPU). Consecutive events for the same app are merged (extend `ended_at`).

**Idle detection:** `CGEventSourceSecondsSinceLastEventType` — if no mouse/keyboard input for 120s, mark as idle.

**Context switch detection:** When `app_name` changes between consecutive non-idle events, increment the context switch counter.

**Data flow:**
```
Window Poll (5s) → ActivityEvent → SQLite buffer (batch write every 30s)
                                 → Context Switch Detector
                                 → Idle Detector
                                 → Focus Session Monitor (if active)
```

**Tauri permissions required:**
- macOS Accessibility permission (user consent prompt on first launch)

**Privacy:**
- Window titles stored locally only, never sent to LLM
- LLM sees only aggregated data: "2h in VS Code (coding), 45min in Slack (communication)"
- Configurable: exclude specific apps, disable window title capture, exclude URL patterns

## 3. Focus Sessions & Distraction Management

### Entry Points

1. **Chat command**: "Start a focus session on MailGate for 45 minutes" → `focus_start` tool
2. **Desktop UI**: Click "Focus" button on a task → Tauri command
3. **Quick launcher**: Alt+Space → "Focus on [task]"

### Soft Blocking

During active focus sessions, when user switches to a "distracting" app:
- Tauri notification: "You're in focus mode on MailGate (32min remaining). Want to continue?"
- Distraction logged in `focus_sessions.distraction_events`
- After 3+ distractions: "Lots of context switches — want me to snooze Slack until you're done?"
- The AI learns which apps are distracting per-user based on patterns

### Break Reminders

- Default: 90min continuous work → suggest 10min break
- Context-aware: "You've been coding for 90 min. Next meeting in 45 min. Good time for a break."
- Cooldown: 15 minutes between same nudge type (`nudge_history`)

### Burnout Detection

- Total active > 8 hours with < 30 min breaks → burnout alert
- 7-day trend of increasing hours → "you've been working more than usual"
- Feeds into daily/weekly AI summaries

## 4. Agent Integration

### Tools (via `FeaturePackage`)

| Action | Description |
|--------|-------------|
| `focus_start` | Start a focus session, optionally linked to a task/project |
| `focus_end` | End current focus session |
| `focus_status` | Check active focus session details |
| `activity_summary` | Get aggregated productivity data for a date range |
| `activity_today` | Quick today's summary |
| `activity_week` | Weekly trend data |
| `set_category` | Create/update app category and rules |
| `list_categories` | List all activity categories |
| `configure_nudges` | Adjust nudge preferences |

### `ProductivityContextSource` (priority 55)

Injected into system prompt on every agent message (60s cache):
- Current focus session status (if active)
- Today's productive/neutral/distracting time
- Recent productivity patterns from learning system

Example: "User is in a focus session on 'MailGate Setup' (28min elapsed). Today: 3.5h productive, 1.2h neutral, 0.3h distracting."

### Learning System Integration

- `PatternAnalyzer` gains `productivity` pattern type: peak focus hours, average session length, preferred break intervals
- `LearningContextSource` enriched with productivity patterns
- `daily_summaries` feed into task agent's `daily-planner` and `weekly-review` skills

### `NudgeService` (Background Loop)

60-second check interval:
- Continuous active time > break threshold → break reminder
- Better time for focus available → focus suggestion
- End of day → daily summary
- Burnout indicators → burnout alert

Delivered via the channel the user last interacted on. All nudges go through `nudge_history` to prevent spam. Quiet hours configurable.

## 5. Desktop UI — Productivity View

New full-width view accessible from the sidebar.

### Components

1. **Focus Status Card** — Live timer, progress bar, distraction count, quality indicator, end button
2. **Today's Summary** — Active/focus/break totals, productive/neutral/distracting breakdown bars
3. **Timeline** — Horizontal time bar showing app usage blocks color-coded by category
4. **Focus Sessions List** — Today's sessions with quality scores
5. **Top Apps** — Bar chart of most-used apps
6. **Weekly Trend** — Focus hours, context switches, average quality per day

### Date Navigation

Toggle between Today, This Week, and specific date ranges.

### Styling

Uses existing dark-mode tokens: `surface-base`, `text-primary`, `brand` (focus), `success` (productive), `destructive` (distracting), `text-muted` (neutral).

## 6. Configuration

```rust
pub struct ProductivityConfig {
    pub enabled: bool,                      // default: true
    pub tracking: TrackingConfig,
    pub focus: FocusConfig,
    pub nudges: NudgeConfig,
    pub privacy: PrivacyConfig,
}

pub struct TrackingConfig {
    pub poll_interval_secs: u64,            // default: 5
    pub idle_threshold_secs: u64,           // default: 120
    pub batch_write_interval_secs: u64,     // default: 30
    pub retention_days: u64,                // default: 90
}

pub struct FocusConfig {
    pub default_duration_mins: u64,         // default: 45
    pub break_interval_mins: u64,           // default: 90
    pub break_duration_mins: u64,           // default: 10
    pub max_daily_focus_hours: u64,         // default: 8
    pub soft_block_enabled: bool,           // default: true
}

pub struct NudgeConfig {
    pub break_reminders: bool,              // default: true
    pub focus_suggestions: bool,            // default: true
    pub daily_summary: bool,                // default: true
    pub burnout_alerts: bool,               // default: true
    pub cooldown_mins: u64,                 // default: 15
    pub quiet_hours_start: Option<String>,
    pub quiet_hours_end: Option<String>,
}

pub struct PrivacyConfig {
    pub excluded_apps: Vec<String>,
    pub exclude_window_titles: bool,        // default: false
    pub excluded_url_patterns: Vec<String>,
}
```

Feature pack: `productivity` (classified as "Recommended").

### Default Categories

| Category | Type | App Patterns |
|----------|------|-------------|
| Coding | productive | VS Code, Xcode, IntelliJ, Terminal, iTerm |
| Communication | neutral | Slack, Discord, Telegram, WhatsApp, Messages |
| Browsing | neutral | Safari, Firefox, Chrome |
| Design | productive | Figma, Sketch, Adobe * |
| Documentation | productive | Notion, Obsidian, Google Docs |
| Entertainment | distracting | YouTube, Netflix, Twitter/X, Reddit, TikTok |
| Email | neutral | Mail, Gmail |

## 7. System Architecture

```
Desktop App (Tauri)
├── Activity Tracker (5s poll, macOS native APIs)
├── Focus Session Manager (start/stop, distraction monitor)
├── Productivity View (React — timeline, summary, sessions, trends)
└── SQLite (5 feature tables)
        │
        ▼
Agent Backend (Rust)
├── feature-productivity (FeaturePackage: tools, migrations, config)
├── ProductivityContextSource (priority 55, 60s cache)
├── NudgeService (60s loop — break/focus/burnout/summary)
└── AgentRuntime
    ├── ContextEngine (assembles productivity context)
    └── LearningSystem
        ├── PatternAnalyzer (now includes productivity patterns)
        └── LearningContextSource (enriched with focus/activity data)
```

## Out of Scope

- Focus music (future iteration)
- Team features / shared dashboards
- Mobile tracking
- Hard distraction blocking
- Browser extension
- Billing / client invoicing
