# Productivity Tracking V2 — Design Document

**Date:** 2026-03-06
**Status:** Approved
**Scope:** `feature-productivity` crate, `desktop` crate, `desktop-ui` frontend

## Context

The current productivity tracking system scores 62/100 in a comprehensive analysis. Strongest in OS-level tracking (macOS APIs, browser title parsing, distraction intervention) and weakest in automatic focus detection (manual only), analytics depth, and real-time dashboard experience. Since the product is pre-release, breaking changes are acceptable.

## Design Decisions

| Decision | Choice | Rationale |
|----------|--------|-----------|
| WebView-side tracking | **Skipped** | OS-level tracking covers all needs; in-app behavior tracking adds complexity without proportional value |
| Auto focus detection | **Confirmable** | Passive detection + toast confirmation; best data quality, trains model over time |
| Real-time dashboard | **Tauri event push** | Granular events (`activity:tick`, `focus:state_changed`, etc.), zero frontend polling |
| Aggregation | **Dual-tier retention** | 7-day raw events, 365-day 5-min buckets, indefinite daily summaries |
| Insight generation | **Heuristic-first** | Deterministic rules against personal baselines; LLM only for daily narrative summary |
| Distraction analysis | **Time-correlated patterns** | Trigger analysis + fatigue/time-of-day correlation; feeds proactive nudges |

## Architecture: Event Bus (Approach 2)

The `ActivityTracker` stays lean — it polls macOS APIs and publishes `ActivityTick` events to a `tokio::broadcast` channel. Independent subscribers consume the stream:

```
ActivityTracker (polls macOS every 5s)
    -> broadcast::Sender<ActivityTick>
        -> BatchWriter          (writes raw events to SQLite)
        -> AutoFocusDetector    (FSM, emits focus:detected events)
        -> BucketAggregator     (accumulates 5-min windows)
        -> DistractionAnalyzer  (trigger + fatigue correlation)
        -> DashboardEmitter     (forwards to Tauri events for real-time UI)
```

A new `ProductivityEngine` struct owns the broadcast sender and all subscribers, replacing the scattered initialization in `AppCore::init()`.

## 1. Core Event Bus & ActivityTick

### Shared event type

```rust
#[derive(Debug, Clone)]
pub struct ActivityTick {
    pub timestamp: DateTime<Utc>,
    pub app_name: String,
    pub bundle_id: Option<String>,
    pub window_title: Option<String>,     // raw (pre-privacy-filter)
    pub site_name: Option<String>,        // extracted for browsers
    pub category_id: Option<String>,
    pub category_type: Option<CategoryType>,
    pub is_idle: bool,
    pub idle_secs: f64,                   // continuous seconds since last input
    pub is_context_switch: bool,          // app or site changed since last tick
}
```

### Broadcast channel

- Capacity: 128 (10min buffer at 5s intervals)
- Lagged subscribers skip and catch up on next tick
- Each subscriber clones a receiver via `tick_tx.subscribe()`

### ProductivityEngine

```rust
pub struct ProductivityEngine {
    tracker: ActivityTracker,
    batch_writer: BatchWriter,
    auto_focus: AutoFocusDetector,
    bucket_aggregator: BucketAggregator,
    distraction_analyzer: DistractionAnalyzer,
    event_emitter: DashboardEmitter,
}
```

Replaces `AppCore`'s scattered `mpsc` wiring with a single coordinated struct.

## 2. AutoFocusDetector — State Machine

### States

```
Unfocused -> [3 consecutive 5-min windows, >80% productive, <2 switches] -> Building
Building  -> [15min elapsed] -> Focused
Building  -> [disruption] -> Unfocused
Focused   -> [idle >3min OR productive_ratio <0.5] -> Cooldown
Cooldown  -> [recovered within 2min] -> Focused
Cooldown  -> [not recovered] -> Ended -> Unfocused
```

### 5-minute evaluation windows

The detector accumulates ticks into 5-min windows, then evaluates:

```rust
struct WindowStats {
    productive_ticks: u32,
    total_ticks: u32,
    context_switches: u32,
    idle_ticks: u32,
    dominant_app: String,
    dominant_category: String,
}
```

### Confirmation toast

On session end, emits `focus:auto_detected` Tauri event with full session payload. Frontend shows non-intrusive toast with three actions:
- **Confirm** -> saves to `focus_sessions` with `source: 'auto_detected'`
- **Dismiss** -> discarded (stats still feed pattern analysis)
- **Adjust** -> mini-editor to tweak start/end times

### Schema change

```sql
ALTER TABLE focus_sessions ADD COLUMN source TEXT NOT NULL DEFAULT 'manual';
-- Values: 'manual', 'auto_detected', 'pomodoro'
```

## 3. BucketAggregator — Dual-Tier Storage

### Schema

```sql
CREATE TABLE IF NOT EXISTS activity_buckets (
    bucket_start      TEXT NOT NULL,     -- ISO 8601, 5-min aligned
    date              TEXT NOT NULL,     -- YYYY-MM-DD
    dominant_app      TEXT,
    dominant_site     TEXT,
    dominant_category TEXT,
    productive_secs   INTEGER NOT NULL DEFAULT 0,
    neutral_secs      INTEGER NOT NULL DEFAULT 0,
    distracting_secs  INTEGER NOT NULL DEFAULT 0,
    idle_secs         INTEGER NOT NULL DEFAULT 0,
    context_switches  INTEGER NOT NULL DEFAULT 0,
    focus_depth       REAL,             -- 0.0-1.0 sustained single-app ratio
    tick_count        INTEGER NOT NULL DEFAULT 0,
    PRIMARY KEY (bucket_start)
);
```

### Retention lifecycle

| Tier | Table | Retention |
|------|-------|-----------|
| Raw events | `activity_events` | 7 days |
| 5-min buckets | `activity_buckets` | 365 days |
| Daily summaries | `daily_summaries` | Indefinite |

### DailyAggregator fallback

For dates within 7-day raw retention: query `activity_events` (existing logic).
For older dates: query `activity_buckets` with SUM aggregation. Transparent to consumers.

## 4. DistractionAnalyzer — Time-Correlated Patterns

### Schema

```sql
CREATE TABLE IF NOT EXISTS distraction_patterns (
    id                  INTEGER PRIMARY KEY AUTOINCREMENT,
    date                TEXT NOT NULL,
    hour_of_day         INTEGER NOT NULL,
    hours_active_today  REAL NOT NULL,
    mins_since_break    REAL NOT NULL,
    preceding_app       TEXT,
    preceding_category  TEXT,
    preceding_duration_mins REAL,
    distraction_app     TEXT NOT NULL,
    distraction_category TEXT,
    recovery_secs       INTEGER,
    created_at          TEXT NOT NULL DEFAULT (datetime('now'))
);
```

### In-memory state

Tracks: last productive tick, productive streak start, last break time, day start time, pending recovery measurement.

### Trigger detection

On each tick: if current tick is distracting AND previous was not distracting -> distraction transition. Record preceding app, duration, fatigue signals. Set pending recovery. When next productive tick arrives, compute recovery_secs and update DB.

### NudgeService integration

`DistractionAnalyzer::is_high_risk_window()` checks current fatigue signals against historical patterns. NudgeService calls this to suggest breaks proactively.

## 5. Heuristic Insight Engine

### Insight types

| Type | Trigger example |
|------|-----------------|
| `DeepWorkTrend` | today's deep work blocks > baseline avg + 1 |
| `DistractionSpike` | distraction rate > baseline * 1.5 |
| `PeakHourShift` | peak focus hours changed from historical pattern |
| `StreakAchieved` | N consecutive days meeting a goal |
| `FatigueWarning` | distraction patterns show >2x rate after 3h |
| `RecoveryImprovement` | avg recovery time < baseline * 0.7 |
| `CategoryShift` | significant time shift between categories |
| `NewPersonalBest` | today's score > max of last 30 days |
| `ConsistencyNote` | N consecutive days above score threshold |

### Personal baselines

Rolling 14-day averages computed from daily summaries: productive hours, deep work blocks, context switches, distraction rate, recovery time, productivity score (with std dev for anomaly detection).

### Deduplication

```sql
CREATE TABLE IF NOT EXISTS insight_cards (
    id              TEXT PRIMARY KEY,
    insight_type    TEXT NOT NULL,
    title           TEXT NOT NULL,
    body            TEXT NOT NULL,
    sentiment       TEXT NOT NULL,
    metric_value    REAL,
    baseline_value  REAL,
    date            TEXT NOT NULL,
    dismissed       BOOLEAN NOT NULL DEFAULT FALSE,
    generated_at    TEXT NOT NULL
);
```

Same `insight_type` + `date` = skip generation.

### Generation triggers

1. End of day (via DailyAggregator) — comprehensive analysis
2. On dashboard open — quick real-time checks (streaks, personal bests)

## 6. Real-Time Dashboard — Tauri Event Push

### Events

| Event | Frequency | Purpose |
|-------|-----------|---------|
| `activity:tick` | Every 5s | Live activity indicator |
| `activity:switch` | On context change | Activity feed append |
| `focus:state_changed` | On FSM transition | Focus indicator |
| `focus:auto_detected` | On session end | Confirmation toast |
| `score:updated` | Every 5 min | Score ring update |
| `bucket:completed` | Every 5 min | Timeline segment append |
| `insight:generated` | On new insight | Insight card animation |

### Frontend changes

- **Remove:** `setInterval` polling from `ActivityFeed`
- **New components:** `FocusStateIndicator`, `AutoFocusToast`, `InsightCardList`, `LiveScoreRing`
- **Update:** `ActivityFeed` (subscribe to `activity:switch`), `Timeline` (append on `bucket:completed`), `PomodoroTimer` (show auto-focus state), `DayView` (include insights section)

## 7. Breaking Changes

### Config

- `TrackingConfig.retention_days` -> replaced by `raw_retention_days` (7) + `bucket_retention_days` (365)
- `FocusConfig` gains: `auto_detect_enabled`, `auto_detect_min_mins`, `auto_detect_productive_threshold`, `auto_detect_max_switches`, `cooldown_grace_secs`

### Types

- `FocusSession` gains `source: SessionSource` field
- `DailySummary` gains `deep_work_blocks`, `deep_work_secs`, `avg_recovery_secs`
- New types: `ActivityTick`, `InsightCard`, `InsightType`, `Sentiment`, `SessionSource`

### New modules in `feature-productivity`

- `tick.rs` — ActivityTick definition
- `engine.rs` — ProductivityEngine (owns broadcast + all subscribers)
- `batch_writer.rs` — extracted from old tracker loop
- `auto_focus.rs` — FSM state machine
- `buckets.rs` — 5-min bucket aggregator
- `distraction_analyzer.rs` — trigger + fatigue pattern tracking
- `insights.rs` — heuristic insight engine
- `repos/bucket.rs`, `repos/distraction_pattern.rs`, `repos/insight.rs` — new repos

### New migration

`002_productivity_v2.sql` — new tables + ALTER for `focus_sessions.source`

### Desktop crate

- `AppCore::init()` simplified — delegates to `ProductivityEngine::new().start()`
- New Tauri commands: `productivity_insights`, `productivity_auto_focus_confirm`, `productivity_auto_focus_dismiss`
- New events emitted from `DashboardEmitter`
