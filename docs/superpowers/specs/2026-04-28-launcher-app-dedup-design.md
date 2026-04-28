# Launcher App Deduplication Design

**Status:** Draft
**Date:** 2026-04-28
**Owner:** @vugomars

## Problem

Searching the launcher for an installed application that is also running and has productivity time-tracking attached returns the same logical entity up to three times.

The duplication originates from three independent search sources, each emitting a separate `LauncherItem` for the same logical app:

| Source | Item ID format | Item kind | Purpose |
|---|---|---|---|
| `AppIndex` | `app:{path}` | `Application { path, running: false }` | Walks `/Applications` for installed bundles |
| `RunningAppsSource` | `running:{pid}` | `RunningApp { pid, path }` | Reads `NSRunningApplication` |
| `AttentionSource` | `attention:app:{bundle_id}` | `Application { path: PathBuf::from(bundle_id), .. }` | Surfaces apps the user has spent time in (the "productivity record") |

Each uses a different opaque identifier (filesystem path / PID / bundle ID), so no merger can dedupe them post-hoc. `AttentionSource` additionally has no icon, because the productivity record stores time-attribution data only — leading to ugly icon-less rows for apps that already have a perfect icon in `AppIndex`.

## Goal

One row per app, regardless of how many sources observe it. The row carries the registry's identity (name, icon, bundle ID, install path) and is decorated with live signals (running PID, attention seconds, category) when those exist.

Out of scope: the parallel duplication issue for sites (bookmarks vs. history vs. attention). The same template defined here can be applied to sites in a follow-up.

## Architecture

Three sources, three roles — not three peers:

- **`AppIndex`** — identity owner. Sole emitter of `kind = Application` for installed apps. Joins signals at query time.
- **`RunningAppsSource`** — pure signal producer. `refresh()` populates a shared `RunningSignals` map. `search()` returns empty.
- **`AttentionSource`** — mixed. Emits `kind = Site` items as today, but for `kind = App` rows from FTS it pushes into a shared `AttentionSignals` map and emits no `LauncherItem`.

```
┌─────────────────────┐     RunningSignals
│ RunningAppsSource   │────▶ DashMap<bundle_id, RunningSignal>
│ (refresh-only,      │     { pid, path }
│  no search)         │
└─────────────────────┘                │
                                       ▼
┌─────────────────────┐     ┌─────────────────────┐
│ AppIndex            │────▶│ AppIndex.search(q)  │
│ (registry,          │     │ • fuzzy match apps  │
│  owns identity:     │     │ • for each hit:     │
│  name, path,        │     │   join RunningSignal│
│  bundle_id, icon)   │     │   join AttentionSig │
└─────────────────────┘     │ • emit unified item │
                            └─────────────────────┘
                                       ▲
┌─────────────────────┐                │
│ AttentionSource     │     AttentionSignals
│ (search emits       │────▶ DashMap<bundle_id,
│  kind=site only +   │             AttentionStat>
│  orphan apps;       │     { secs, category, last_used }
│  app rows go to     │
│  signals map)       │
└─────────────────────┘
```

### Why this shape

- Matches reality: `AppIndex` *is* the registry of installed apps; running and attention are observations *about* those apps.
- Eliminates duplication structurally rather than by post-hoc merge.
- Solves the icon UX issue for free — the unified row always carries the registry's icon.
- Reuses the existing `Arc<DashMap<…>>` shared-state pattern (`SourceRegistry::query_cache` is the same shape).
- Leaves room to apply the same template to sites later without rewriting the registry.

### Why not the alternatives

- **Post-fan-out merge in `SourceRegistry`** — every new `LauncherItemKind` variant has to teach the merge code a new identity rule. Wrong place for that knowledge.
- **Unified `entities` SQLite table** — most "correct" but a significant schema lift, and the entity store would need to be queryable for non-search purposes to justify itself. Premature for two duplicate domains.

## Components

### New types

In `crates/feature-launcher/src/search/signals.rs` (new file):

```rust
use dashmap::DashMap;
use smol_str::SmolStr;
use std::path::PathBuf;
use std::sync::Arc;

#[derive(Clone, Debug)]
pub struct RunningSignal {
    pub pid: u32,
    pub path: PathBuf,
}

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
```

`SmolStr` keys: bundle IDs are short reverse-DNS strings. Most fit inline (no allocation). Matches the `token_index.rs` choice.

### Modified `AppEntry`

```rust
pub struct AppEntry {
    pub name: String,
    pub path: PathBuf,
    pub bundle_id: Option<SmolStr>,   // populated from Info.plist
    pub icon_path: Option<PathBuf>,
}
```

Bundle ID extracted via `CFBundle::main_bundle_with_path` during `index_applications()`. On extraction failure: `bundle_id = None`, app still indexes, but cannot receive signals.

### Modified `AppIndex::search`

```rust
pub fn search(&self, query: &str, limit: usize) -> Vec<LauncherItem> {
    let apps = self.apps.read();
    let scored = super::fuzzy_match(query, &apps, |a| &a.name, limit);

    scored.into_iter().map(|(score, app)| {
        let bid = app.bundle_id.as_ref();

        let running = bid.and_then(|b| self.running_signals.get(b).map(|r| r.clone()));
        let attention = bid.and_then(|b| self.attention_signals.get(b).map(|s| s.clone()));

        let subtitle = compose_subtitle(&app.path, &running, &attention);
        let boost = score_boost(&running, &attention);  // 1.0 baseline → ~2.0 max

        LauncherItem {
            id: bid.map(|b| format!("app:{b}"))
                .unwrap_or_else(|| format!("app:{}", app.path.display())),
            title: app.name.clone(),
            subtitle: Some(subtitle),
            icon: load_icon(app),
            kind: LauncherItemKind::Application {
                path: app.path.clone(),
                running: running.is_some(),
            },
            score: (score as f64 / 1000.0) * boost,
            no_view: false,
            arguments: vec![],
            pinned: false,
        }
    }).collect()
}
```

Stable IDs switch from `app:{path}` to `app:{bundle_id}` when present. Pin/frequency entries gain stability across reinstalls and OS upgrades.

The boost is **multiplicative**, not additive. Mirrors the `entity_attention.fts_search` blend (`bm25 * 0.4 + attention_norm * 0.6`) — keeps user typing intent first, uses signals as tiebreakers.

### Modified `RunningAppsSource`

```rust
pub struct RunningAppsSource {
    signals: RunningSignals,  // shared with AppIndex
}

#[async_trait]
impl SearchSource for RunningAppsSource {
    fn name(&self) -> &'static str { "running_apps" }

    async fn refresh(&self) {
        let snapshot = tokio::task::spawn_blocking(|| {
            platform_macos::apps::running_applications()
        }).await.unwrap_or_default();

        let live: HashSet<SmolStr> = snapshot.iter()
            .filter_map(|a| a.bundle_id.as_deref().map(SmolStr::new))
            .collect();
        self.signals.retain(|k, _| live.contains(k));
        for app in snapshot {
            if let Some(bid) = app.bundle_id {
                self.signals.insert(SmolStr::new(&bid), RunningSignal {
                    pid: app.pid as u32,
                    path: app.path.unwrap_or_default(),
                });
            }
        }
    }

    async fn search(&self, _q: &str, _limit: usize) -> Vec<LauncherItem> {
        Vec::new()  // signal-only source
    }
}
```

The empty `search()` is the YAGNI choice: stays a `SearchSource` for registration symmetry; cost is one empty `Vec` per query.

### Modified `AttentionSource`

```rust
async fn search(&self, query: &str, limit: usize) -> Vec<LauncherItem> {
    let rows = /* same FTS query as today */;

    rows.into_iter().filter_map(|row| match row.kind.as_str() {
        "site" => Some(into_site_item(row)),  // unchanged
        "app" => {
            self.attention_signals.insert(
                SmolStr::new(&row.canonical_id),
                AttentionStat {
                    attention_secs: row.attention_secs,
                    category: row.category.clone().map(SmolStr::new),
                    last_used_at: jiff::Timestamp::from_str(&row.last_used_at).ok()?,
                }
            );
            None  // never emit app rows directly — AppIndex owns identity
        },
        _ => None,
    }).collect()
}
```

Search-time signal updates are intentional: the `entity_attention` table is updated continuously by the productivity engine, but the launcher only needs fresh stats *while the user is searching*. Pushing into `AttentionSignals` from inside `search()` means data is at most one query stale.

Orphan suppression is implicit: an attention record for an uninstalled bundle ID has no matching `AppEntry`, so the unified row never appears.

### Wiring (caller)

```rust
let running_signals = Arc::new(DashMap::new());
let attention_signals = Arc::new(DashMap::new());

let app_index = AppIndex::new()
    .with_running_signals(running_signals.clone())
    .with_attention_signals(attention_signals.clone());

let running_apps = RunningAppsSource::new(running_signals);
let attention = AttentionSource::new(repo, attention_signals);

registry.register(Arc::new(app_index));
registry.register(Arc::new(running_apps));
registry.register(Arc::new(attention));
```

Signal maps owned at the wiring layer, shared by `Arc::clone`. No singleton, no global state.

## Edge cases

### Apps without a bundle ID

`AppEntry.bundle_id = None`. Falls back to `format!("app:{}", path)` for the row ID. Cannot receive signals. Still ranks via fuzzy score with `boost = 1.0`.

### Same bundle ID at two paths

Possible after a botched OS upgrade (e.g. Safari at both `/Applications` and `/System/Applications`). Resolved at `set_apps` time:

```rust
fn dedupe_by_bundle_id(mut apps: Vec<AppEntry>) -> Vec<AppEntry> {
    apps.sort_by_key(|a| match a.path.starts_with("/Applications") {
        true => 0,
        false => 1,
    });
    let mut seen = HashSet::new();
    apps.retain(|a| match &a.bundle_id {
        Some(b) => seen.insert(b.clone()),
        None => true,
    });
    apps
}
```

`/Applications` wins over `/System/Applications` and nested helper bundles — encodes user intent.

### `RunningSignals` stale entries

Replaced wholesale every `refresh()` via `retain` + `insert`. No TTL needed.

### `AttentionSignals` unbounded growth

Cap at 500 entries (LRU-evict by `last_used_at` when over cap). Once per `refresh_all` cycle, drop entries older than 90 days (matches `EntityAttentionRepo::purge_older_than`).

### Bundle-ID extraction failure

Wrap CFBundle calls in `catch_unwind` (Objective-C bridge — defensive). On failure: log at `debug!`, set `bundle_id = None`, continue. Never abort the walk.

### CoreServices walker scope

Add to `index_applications` dirs:

```rust
"/System/Library/CoreServices",
"/System/Library/CoreServices/Applications",
```

Bump `walk_apps`'s `max_depth` from 3 to 4 for nested helper bundles. Closes the orphan gap for `com.apple.finder`, `com.apple.dock`, etc.

### ID migration for pins / frequency

Today: `app:{path}`. After: `app:{bundle_id}`. One-shot at startup, after first `index_applications()` completes:

```sql
UPDATE pins      SET item_id = 'app:' || ?bundle_id WHERE item_id = 'app:' || ?path;
UPDATE frequency SET item_id = 'app:' || ?bundle_id WHERE item_id = 'app:' || ?path;
```

Idempotent. Pre-release era so we *could* drop the rows — but the migration is small and same pattern is needed post-release, so it ships now.

## Testing

### Unit tests

**`signals.rs`**
- `RunningSignals` insert + `retain` semantics.
- `AttentionStat` round-trip with RFC-3339 `last_used_at`.

**`app_index.rs`** (extends existing `#[cfg(test)] mod tests`)
- `bundle_id_extracted_from_info_plist` — tempdir + minimal `Info.plist`.
- `bundle_id_missing_does_not_panic`.
- `dedupe_by_bundle_id_keeps_user_path`.
- `search_joins_running_signal` — pre-populate signals, assert `running: true` and boost applied.
- `search_joins_attention_signal` — assert boost compounds.
- `search_falls_back_to_path_id_when_bundle_missing`.

**`running_apps.rs`** (replaces existing tests)
- `refresh_replaces_signal_snapshot`.
- `search_returns_empty`.

**`attention.rs`** (extends existing tests)
- `search_filters_app_rows_into_signals`.
- `site_search_unchanged` — regression.

### Integration tests

**`crates/feature-launcher/tests/app_dedup_test.rs` (new)**

```rust
#[tokio::test]
async fn safari_appears_once_with_running_and_attention_layers() {
    // Arrange: in-memory storage, all three sources sharing signal maps.
    // Seed AppIndex with synthetic /Applications/Safari.app + plist.
    // Seed RunningSignals with com.apple.Safari → pid 99.
    // Seed entity_attention with com.apple.Safari → 3600s, category "browsing".
    // Drive AttentionSource.search("safari") to load AttentionSignals.

    let results = registry.search("safari", 10).await;

    let safari: Vec<_> = results.iter().filter(|i| i.title == "Safari").collect();
    assert_eq!(safari.len(), 1);
    assert!(matches!(safari[0].kind,
        LauncherItemKind::Application { running: true, .. }));
    assert!(safari[0].subtitle.as_deref().unwrap().contains("Running"));
    assert!(safari[0].subtitle.as_deref().unwrap().contains("1h"));
}
```

**`pin_migration_test.rs` (new)**
- Seed pins table with `app:/Applications/Safari.app`. Trigger first `refresh()`. Assert rewritten to `app:com.apple.Safari`.

### Regression coverage

- `feature-launcher/tests/repos_test.rs` — pin/frequency repo behavior must round-trip the new ID format.
- `crates/feature-productivity/tests/integration_test.rs` — productivity writers into `entity_attention` are upstream; should pass unchanged.

### Out of scope

- Real `NSRunningApplication` enumeration — exercised at runtime, not worth mocking.
- Icon resolution end-to-end — covered by existing `AppIconCache` tests.
- Cross-source cache TTL interaction — `AttentionSource` declares 30s TTL; signals can be up to 30s stale via cache hit. Documented and accepted.

## Benchmarks

New file: `crates/feature-launcher/benches/app_index_dedup.rs`. Matches the existing `inverted_index.rs` Criterion idiom.

### Group A — `app_index_search`

Hot path. Sweeps app count × signal density × query shape.

```rust
let app_counts = [100, 500, 2_000];
let signal_density = [0.0, 0.25, 1.0];
let queries = ["s", "saf", "safari", "vsc", "fin"];
```

`signal_density` is the regression knob — 0.0 reproduces today's no-join path, 1.0 is worst case.

### Group B — `app_index_build`

Indexing now does plist parsing per `.app`. Tempdir of synthetic `Foo.app/Contents/Info.plist` files. Counts: 100 / 500 / 2000.

### Group C — `running_signals_refresh`

`retain` + `insert` snapshot replacement on the launcher's "feels instant" path. Counts: 10 / 50 / 200 running apps.

### Acceptance thresholds

| Bench group | Ratio (post / pre) | If exceeded |
|---|---|---|
| `app_index_search` (density 0.0) | ≤ 1.05x | Investigate per-iter overhead from new `bundle_id` field |
| `app_index_search` (density 1.0) | ≤ 1.30x | Cache the join inside the fuzzy_match scoring closure |
| `app_index_build` | ≤ 2.0x | Parallelize plist reads with `rayon` |
| `running_signals_refresh` | ≤ 1.15x | Replace `retain` + `insert` with swap-style |

Build can absorb a 2x hit (one-shot at startup); search cannot (every keystroke). CI does not gate on these — running locally before merging large changes to this crate is sufficient.

### Out of scope

- `AttentionSource::search` itself (sqlx FTS — not our code).
- End-to-end `SourceRegistry::search` — too many moving parts; integration test covers composition correctness.
- CFBundle plist extraction in isolation — covered indirectly by Group B.

## Open questions

None remaining. All Q1–Q3 resolved in brainstorming.

## Implementation order (preview for the plan)

1. Add `signals.rs` types.
2. Populate `AppEntry.bundle_id` from `Info.plist`. Add CoreServices walker scope.
3. Wire `RunningSignals` and `AttentionSignals` through `AppIndex` constructor.
4. Convert `RunningAppsSource` to signal-only.
5. Convert `AttentionSource` app-row handling to signal push.
6. Modify `AppIndex::search` to join signals and emit unified items.
7. Add pin/frequency ID migration on first refresh.
8. Add unit + integration tests.
9. Add benches and run baseline + post comparison.
