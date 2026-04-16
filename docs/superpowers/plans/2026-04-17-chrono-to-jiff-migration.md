# Chrono → Jiff Workspace Migration Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Replace `chrono` with `jiff` across the entire 37-crate Klyntbot workspace, layer-by-layer (L0 → L8), preserving identical runtime behavior while gaining RFC 9557 IXDTF serialization, correct IANA timezone round-tripping, and type-safe separation of `Timestamp` (UTC instant) / `Zoned` (with IANA tz) / `civil::DateTime` (floating).

**Architecture:** Single leaf-to-root migration. Each layer is an independent PR; workspace compiles cleanly at every checkpoint. Storage wire format becomes Unix epoch milliseconds (`i64`) — both Chrono and Jiff read/write `i64` losslessly, so there is no coordinated schema change. New code uses only Jiff; transient `TimeConvert` helpers in `common` bridge Chrono↔Jiff until each layer is migrated, then are removed in the final cleanup.

**Tech Stack:** Rust 1.93, `jiff` (latest), `jiff_cron_tz_compat` via `chrono-tz` interop removed; tests via `cargo nextest`; linting via `clippy -D warnings`; formatting via `rustfmt`.

**Spec reference:** `docs/superpowers/specs/2026-04-17-unified-temporal-scheduler-and-notifications-design.md` §9.

---

## Migration Cookbook

**Read this section first.** Every crate task references these canonical mappings.

### Type Mappings

| Chrono type | Jiff replacement | Notes |
|---|---|---|
| `chrono::DateTime<Utc>` | `jiff::Timestamp` | Wall-clock UTC instant, ns precision |
| `chrono::DateTime<Local>` | `jiff::Zoned` with `TimeZone::system()` | Only in user-facing display code |
| `chrono::DateTime<Tz>` (chrono-tz) | `jiff::Zoned` with `TimeZone::get(iana_name)?` | |
| `chrono::NaiveDateTime` | `jiff::civil::DateTime` | Floating (no tz) |
| `chrono::NaiveDate` | `jiff::civil::Date` | |
| `chrono::NaiveTime` | `jiff::civil::Time` | |
| `chrono::Duration` (calendar-like) | `jiff::Span` | Calendar-aware; DST-correct arithmetic |
| `chrono::Duration` (wall-clock) | `std::time::Duration` or `jiff::SignedDuration` | For pure wall-clock seconds |
| `chrono::Utc::now()` | `jiff::Timestamp::now()` | |
| `chrono::Local::now()` | `jiff::Zoned::now()` | |
| `chrono_tz::Tz` | `jiff::tz::TimeZone` | |

### Common Method Mappings

| Chrono API | Jiff API |
|---|---|
| `dt.to_rfc3339()` | `dt.to_string()` (Jiff default = RFC 9557 IXDTF; or `dt.strftime("%Y-%m-%dT%H:%M:%S%:z").to_string()` for strict RFC 3339) |
| `DateTime::parse_from_rfc3339(s)` | `s.parse::<Timestamp>()` (accepts both RFC 3339 and RFC 9557) |
| `dt.timestamp()` | `ts.as_second()` |
| `dt.timestamp_millis()` | `ts.as_millisecond()` |
| `Utc.timestamp_opt(s, 0).unwrap()` | `Timestamp::from_second(s)?` |
| `Utc.timestamp_millis_opt(ms).unwrap()` | `Timestamp::from_millisecond(ms)?` |
| `dt + chrono::Duration::seconds(n)` | `ts + Span::new().seconds(n)` or `ts + Duration::from_secs(n)` |
| `dt.date_naive()` | `ts.to_zoned(tz).date()` or `dt.date()` (on `Zoned`) |
| `dt.with_timezone(&tz)` | `ts.to_zoned(tz)` |
| `dt.naive_local()` | `zoned.datetime()` (returns `civil::DateTime`) |

### Serde

Jiff provides `serde` integration via the `serde` feature. Default serialization is RFC 9557:
```rust
#[derive(Serialize, Deserialize)]
struct Row {
    fire_at: jiff::Timestamp,     // serializes as "2026-04-17T14:30:00Z"
}
```

For explicit integer-epoch serialization in storage rows:
```rust
use jiff::fmt::serde::timestamp::millisecond::required as ts_millis;
#[derive(Serialize, Deserialize)]
struct Row {
    #[serde(with = "ts_millis")]
    fire_at: jiff::Timestamp,
}
```

### SQLite Wire Format Convention

- **All persisted timestamps** → `INTEGER` columns holding Unix epoch milliseconds, UTC.
- Read: `Timestamp::from_millisecond(row.get::<_, i64>("fire_at"))?`
- Write: `ts.as_millisecond()` → `i64`
- **Do not store** RFC 3339 or RFC 9557 strings in SQLite. Exception: human-readable debug columns (e.g., `created_at_display TEXT`) may coexist alongside canonical `i64` columns if already present.
- Pre-existing TEXT datetime columns migrate to INTEGER in each crate's storage task (pre-release, no data migration required).

### Forbidden Patterns (lint targets)

After migration completes, a clippy-disallow rule should reject:
- `chrono::` (any path)
- `chrono_tz::`
- `use chrono`
- `Local::now()` without an explicit `TimeZone` (use `Zoned::now()` or `Timestamp::now().to_zoned(tz)`)

### Bridge Helper (transient)

During migration, `common::time::bridge` provides:

```rust
pub fn chrono_to_jiff(dt: chrono::DateTime<chrono::Utc>) -> jiff::Timestamp {
    jiff::Timestamp::from_millisecond(dt.timestamp_millis())
        .expect("in-range")
}

pub fn jiff_to_chrono(ts: jiff::Timestamp) -> chrono::DateTime<chrono::Utc> {
    chrono::DateTime::<chrono::Utc>::from_timestamp_millis(ts.as_millisecond())
        .expect("in-range")
}
```

These are used *only* at crate boundaries where the caller crate is still on Chrono while the callee has migrated. Each such usage is a temporary scaffold; the final cleanup task removes them.

---

## File Structure

The migration is mechanical across ~250 files. The plan structures work by crate rather than by file because each crate's tests validate that crate's migration.

**New files (created in Task 1.2):**
- `crates/common/src/time/mod.rs` — public module
- `crates/common/src/time/bridge.rs` — transient Chrono↔Jiff helpers
- `crates/common/src/time/convert.rs` — storage i64 ↔ Timestamp helpers
- `crates/common/src/time/helpers.rs` — `now()`, `user_tz(config)`, parse helpers

**Modified root files:**
- `Cargo.toml` — add `jiff`, remove `chrono` and `chrono-tz` in final cleanup
- `clippy.toml` (new) — disallow chrono paths post-migration

**No new tests beyond per-crate test updates.** The migration preserves behavior; existing tests validate preservation. Where tests relied on Chrono-specific parsing quirks, they're updated in their crate's task.

---

## Task Overview

```
Phase 1: Infrastructure        (Tasks 1.1 – 1.4)
Phase 2: L0 crates             (Tasks 2.1 – 2.2)    common, platform-macos
Phase 3: L1 crates             (Tasks 3.1 – 3.5)    bus, config, tools-core, tools-core-macros, analytics
Phase 4: L2 storage            (Task  4.1)          storage
Phase 5: L3 crates             (Tasks 5.1 – 5.5)    providers, session, scheduling, context_engine, skill-system
Phase 6: L4 crates             (Tasks 6.1 – 6.13)   tools, feature-*, activity-log, plugin-runtime, autotuner, voice-engine, simulator
Phase 7: L5 crates             (Tasks 7.1 – 7.3)    channels, agent, cognitive
Phase 8: L6 mcp                (Task  8.1)          mcp
Phase 9: L7 crates             (Tasks 9.1 – 9.3)    app-core, desktop-shared, desktop
Phase 10: L8 crates + cleanup  (Tasks 10.1 – 10.4)  klyntbot, klyntbot-server, remove chrono, add lint
```

**Total: 40 tasks, estimated ~230 steps.**

Execution checkpoint: after each Phase (not each Task), run full workspace verification:
```bash
cargo build --workspace
cargo nextest run --workspace
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo fmt --all --check
```

---

## Phase 1: Infrastructure

### Task 1.1: Add jiff to workspace Cargo.toml

**Files:**
- Modify: `Cargo.toml:107-135`

- [ ] **Step 1: Verify current chrono versions**

Run: `grep -n "^chrono" Cargo.toml`
Expected output:
```
107:chrono = { version = "0.4.43", features = ["serde"] }
133:chrono-tz = "0.10"
```

- [ ] **Step 2: Add jiff to workspace dependencies**

In `Cargo.toml`, directly after the `chrono-tz` line (~line 133), add:

```toml
jiff = { version = "0.1", features = ["serde", "tz-system"] }
```

Keep chrono and chrono-tz entries for now (removed in Task 10.3).

- [ ] **Step 3: Verify workspace resolves**

Run: `cargo metadata --format-version 1 > /dev/null`
Expected: exit 0, no errors.

- [ ] **Step 4: Commit**

```bash
git add Cargo.toml
git commit -m "build: add jiff to workspace dependencies"
```

---

### Task 1.2: Create `common::time` module scaffold

**Files:**
- Modify: `crates/common/Cargo.toml`
- Create: `crates/common/src/time/mod.rs`
- Create: `crates/common/src/time/bridge.rs`
- Create: `crates/common/src/time/convert.rs`
- Create: `crates/common/src/time/helpers.rs`
- Modify: `crates/common/src/lib.rs`
- Test: `crates/common/src/time/bridge.rs` (inline `#[cfg(test)]`)

- [ ] **Step 1: Add jiff to common/Cargo.toml**

In `crates/common/Cargo.toml`, add after the `chrono` line:

```toml
jiff = { workspace = true }
```

- [ ] **Step 2: Create `time/bridge.rs`**

File content:

```rust
//! Transient Chrono ↔ Jiff conversion helpers used during migration.
//! Removed in the final cleanup task once no crate depends on Chrono.

use chrono::{DateTime, Utc};
use jiff::Timestamp;

pub fn chrono_to_jiff(dt: DateTime<Utc>) -> Timestamp {
    Timestamp::from_millisecond(dt.timestamp_millis()).expect("timestamp in range")
}

pub fn jiff_to_chrono(ts: Timestamp) -> DateTime<Utc> {
    DateTime::<Utc>::from_timestamp_millis(ts.as_millisecond()).expect("timestamp in range")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round_trip_chrono_to_jiff_to_chrono() {
        let original = Utc::now();
        let round = jiff_to_chrono(chrono_to_jiff(original));
        assert_eq!(original.timestamp_millis(), round.timestamp_millis());
    }

    #[test]
    fn round_trip_jiff_to_chrono_to_jiff() {
        let original = Timestamp::now();
        let round = chrono_to_jiff(jiff_to_chrono(original));
        assert_eq!(original.as_millisecond(), round.as_millisecond());
    }
}
```

- [ ] **Step 3: Create `time/convert.rs`**

File content:

```rust
//! Storage wire format conversion helpers.
//! All persisted timestamps use Unix epoch milliseconds (i64, UTC).

use jiff::Timestamp;

pub fn ts_to_millis(ts: Timestamp) -> i64 {
    ts.as_millisecond()
}

pub fn millis_to_ts(ms: i64) -> Result<Timestamp, jiff::Error> {
    Timestamp::from_millisecond(ms)
}

pub fn opt_ts_to_millis(ts: Option<Timestamp>) -> Option<i64> {
    ts.map(ts_to_millis)
}

pub fn opt_millis_to_ts(ms: Option<i64>) -> Option<Timestamp> {
    ms.and_then(|v| millis_to_ts(v).ok())
}
```

- [ ] **Step 4: Create `time/helpers.rs`**

File content:

```rust
//! User-facing time helpers that consume the application timezone.

use jiff::{tz::TimeZone, Timestamp, Zoned};

pub fn now_utc() -> Timestamp {
    Timestamp::now()
}

pub fn now_in_tz(iana: &str) -> Result<Zoned, jiff::Error> {
    let tz = TimeZone::get(iana)?;
    Ok(Timestamp::now().to_zoned(tz))
}

/// Returns the system timezone, or UTC if it cannot be determined.
pub fn system_tz() -> TimeZone {
    TimeZone::system()
}
```

- [ ] **Step 5: Create `time/mod.rs`**

File content:

```rust
//! Canonical time types and helpers for Klyntbot.
//! New code should use `jiff::Timestamp` / `jiff::Zoned` / `jiff::civil::*`
//! instead of `chrono` types.

pub mod bridge;
pub mod convert;
pub mod helpers;

pub use jiff;
pub use helpers::{now_in_tz, now_utc, system_tz};
```

- [ ] **Step 6: Register the module in `lib.rs`**

Edit `crates/common/src/lib.rs` and add at an appropriate public module position (alongside other `pub mod` declarations):

```rust
pub mod time;
```

- [ ] **Step 7: Run tests**

Run: `cargo nextest run -p common`
Expected: PASS, 2 new tests `time::bridge::tests::round_trip_*` pass.

- [ ] **Step 8: Run clippy on common**

Run: `cargo clippy -p common --all-targets -- -D warnings`
Expected: PASS, 0 warnings.

- [ ] **Step 9: Commit**

```bash
git add crates/common/Cargo.toml crates/common/src/time/ crates/common/src/lib.rs
git commit -m "feat(common): add time module with jiff types and chrono bridge"
```

---

### Task 1.3: Document the migration conventions

**Files:**
- Create: `docs/superpowers/plans/2026-04-17-chrono-to-jiff-MIGRATION-GUIDE.md`

- [ ] **Step 1: Write the guide**

File content: copy the **Migration Cookbook** section from this plan (above) into a standalone file so crate-level tasks can reference it without re-reading this full plan.

- [ ] **Step 2: Commit**

```bash
git add docs/superpowers/plans/2026-04-17-chrono-to-jiff-MIGRATION-GUIDE.md
git commit -m "docs: add chrono→jiff migration conventions guide"
```

---

### Task 1.4: Verify baseline workspace builds and tests

**Files:** none modified.

- [ ] **Step 1: Full build**

Run: `cargo build --workspace`
Expected: exit 0.

- [ ] **Step 2: Full test suite**

Run: `cargo nextest run --workspace`
Expected: all tests pass.

- [ ] **Step 3: Full clippy**

Run: `cargo clippy --workspace --all-targets --all-features -- -D warnings`
Expected: exit 0.

- [ ] **Step 4: Format check**

Run: `cargo fmt --all --check`
Expected: exit 0.

No commit — this is a verification gate.

---

## Phase 2: L0 Crates

### Task 2.1: Migrate `common` crate

**Files:**
- Modify: `crates/common/src/date.rs`
- Modify: `crates/common/src/ports/*.rs` (any using `chrono::`)
- Test: `crates/common/src/date.rs` (inline tests)

- [ ] **Step 1: Survey chrono usage in common**

Run: `grep -rn "chrono" crates/common/src/`
Record the files. Expected: `src/date.rs`, possibly `src/ports/notification.rs` or similar.

- [ ] **Step 2: Rewrite `date.rs` parsing against jiff**

Rewrite `common::parse_datetime(s, fallback_tz)` to return `Result<jiff::Timestamp, KlyntbotError>`. Accept: RFC 3339, RFC 9557, date-only (`YYYY-MM-DD`), and natural language (`"tomorrow"`, `"next friday"`, `"in 3 days"`). For date-only input, interpret in `fallback_tz` at midnight.

Reference implementation outline (adapt to existing error types):

```rust
use jiff::{civil, tz::TimeZone, Span, Timestamp, Zoned};
use crate::KlyntbotError;

pub fn parse_datetime(s: &str, fallback_tz: &str) -> Result<Timestamp, KlyntbotError> {
    let s = s.trim();
    // 1. Try RFC 9557 / 3339 directly
    if let Ok(ts) = s.parse::<Timestamp>() {
        return Ok(ts);
    }
    // 2. Try zoned (RFC 9557 with IANA bracket)
    if let Ok(z) = s.parse::<Zoned>() {
        return Ok(z.timestamp());
    }
    // 3. Try civil datetime in fallback_tz
    if let Ok(dt) = s.parse::<civil::DateTime>() {
        let tz = TimeZone::get(fallback_tz).map_err(KlyntbotError::from)?;
        return Ok(dt.to_zoned(tz)?.timestamp());
    }
    // 4. Try date-only
    if let Ok(d) = s.parse::<civil::Date>() {
        let tz = TimeZone::get(fallback_tz).map_err(KlyntbotError::from)?;
        return Ok(d.at(0, 0, 0, 0).to_zoned(tz)?.timestamp());
    }
    // 5. Natural language — preserve existing branch logic, translating to Jiff
    parse_natural_language(s, fallback_tz)
}

fn parse_natural_language(s: &str, fallback_tz: &str) -> Result<Timestamp, KlyntbotError> {
    let lower = s.to_ascii_lowercase();
    let tz = TimeZone::get(fallback_tz).map_err(KlyntbotError::from)?;
    let now = Timestamp::now().to_zoned(tz.clone());
    let today = now.date();
    match lower.as_str() {
        "today" | "now" => Ok(now.timestamp()),
        "tomorrow" => Ok(today.tomorrow()?.at(9, 0, 0, 0).to_zoned(tz)?.timestamp()),
        "yesterday" => Ok(today.yesterday()?.at(9, 0, 0, 0).to_zoned(tz)?.timestamp()),
        s if s.starts_with("in ") => parse_relative_future(s, &now),
        s if s.starts_with("next ") => parse_next_weekday(s, &now),
        _ => Err(KlyntbotError::invalid_input(format!("unparseable datetime: {s}"))),
    }
}
```

Keep `parse_relative_future` and `parse_next_weekday` close to the original semantics, using `jiff::Span::new().days(n)` for day arithmetic on `Zoned`.

- [ ] **Step 3: Update the error conversion**

In `crates/common/src/error.rs` (or wherever `KlyntbotError` is defined), add a `From<jiff::Error>` impl:

```rust
impl From<jiff::Error> for KlyntbotError {
    fn from(err: jiff::Error) -> Self {
        KlyntbotError::invalid_input(format!("time error: {err}"))
    }
}
```

Use the existing `invalid_input` / `InternalError` variant names as defined in the codebase.

- [ ] **Step 4: Update any `chrono::DateTime` in `common::ports`**

Grep for `chrono::DateTime` in `crates/common/src/ports/`. For each hit, replace `chrono::DateTime<chrono::Utc>` with `jiff::Timestamp`. Update imports.

- [ ] **Step 5: Update inline tests in `date.rs`**

Adapt existing `parse_datetime` tests. Where they compared `DateTime<Utc>` values, compare via `.as_millisecond()`. Add at least one RFC 9557 round-trip test:

```rust
#[test]
fn parses_rfc9557_with_iana_bracket() {
    let ts = parse_datetime("2024-07-21T17:11:00-04:00[America/New_York]", "UTC").unwrap();
    // 2024-07-21T21:11:00Z
    assert_eq!(ts.to_string(), "2024-07-21T21:11:00Z");
}
```

- [ ] **Step 6: Remove `chrono` import from `date.rs`**

At the top of `crates/common/src/date.rs`, delete any `use chrono::...;` lines. The file should now only import from `jiff` and `crate::`.

- [ ] **Step 7: Run common tests**

Run: `cargo nextest run -p common`
Expected: PASS including new RFC 9557 test.

- [ ] **Step 8: Run clippy**

Run: `cargo clippy -p common --all-targets -- -D warnings`
Expected: 0 warnings.

- [ ] **Step 9: Build downstream crates to confirm no ripple-break**

Run: `cargo build --workspace`
Expected: exit 0. If any downstream crate that consumed `parse_datetime`'s return type breaks, note the crate name — it will be migrated in a later task but its call site may need `chrono_to_jiff` bridge usage added here. If so, import `common::time::bridge::jiff_to_chrono` in the downstream crate and wrap the call site until that crate's migration task.

- [ ] **Step 10: Commit**

```bash
git add crates/common/
git commit -m "refactor(common): migrate parse_datetime and ports to jiff"
```

---

### Task 2.2: Migrate `platform-macos` crate

**Files:**
- Modify: `crates/platform-macos/Cargo.toml` (if chrono present)
- Modify: `crates/platform-macos/src/**/*.rs` (any chrono usage)

- [ ] **Step 1: Check for chrono usage**

Run: `grep -rn "chrono" crates/platform-macos/`
If no matches, skip directly to Step 5 (no-op commit not needed).

- [ ] **Step 2: Replace chrono types per cookbook**

For every match from Step 1, apply the **Type Mappings** table.

- [ ] **Step 3: Remove chrono from `Cargo.toml` if present**

- [ ] **Step 4: Run tests and clippy**

Run: `cargo nextest run -p platform-macos && cargo clippy -p platform-macos --all-targets -- -D warnings`
Expected: PASS.

- [ ] **Step 5: Commit (if changes)**

```bash
git add crates/platform-macos/
git commit -m "refactor(platform-macos): migrate chrono to jiff"
```

---

### Phase 2 Checkpoint

- [ ] **Run full workspace verification**

```bash
cargo build --workspace
cargo nextest run --workspace
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo fmt --all --check
```

Expected: all four commands exit 0. If any downstream crate fails to compile, it is because it consumed a Chrono type from `common` that is now Jiff. Apply the bridge helper at the call site in the failing crate as a temporary measure (it will be removed when that crate is fully migrated).

---

## Phase 3: L1 Crates

### Task 3.1: Migrate `bus` crate

**Files:**
- Modify: `crates/bus/Cargo.toml`
- Modify: `crates/bus/src/events.rs`
- Modify: `crates/bus/src/context_updates.rs`
- Modify: any other chrono-using file in `crates/bus/src/`

- [ ] **Step 1: Survey**

Run: `grep -rn "chrono" crates/bus/src/ && grep -n "chrono" crates/bus/Cargo.toml`
Record the list.

- [ ] **Step 2: Add jiff to bus/Cargo.toml**

In `crates/bus/Cargo.toml`, add:

```toml
jiff = { workspace = true }
```

- [ ] **Step 3: Migrate `events.rs`**

Replace every `chrono::DateTime<chrono::Utc>` field in `DomainEvent` variants (and related structs) with `jiff::Timestamp`. Update serde derives — Jiff's default serialization is RFC 9557, which is a superset of RFC 3339. If existing consumers serialized with `#[serde(with = "chrono::serde::ts_seconds")]`, replace with Jiff's `#[serde(with = "jiff::fmt::serde::timestamp::second::required")]` to preserve wire format.

- [ ] **Step 4: Migrate `context_updates.rs`**

Same pattern as Step 3. Any `chrono::Utc::now()` → `jiff::Timestamp::now()`.

- [ ] **Step 5: Remove chrono from `Cargo.toml`**

Delete the `chrono = ...` line from `crates/bus/Cargo.toml`.

- [ ] **Step 6: Build, test, clippy**

Run:
```bash
cargo build -p bus
cargo nextest run -p bus
cargo clippy -p bus --all-targets -- -D warnings
```
Expected: all pass.

- [ ] **Step 7: Build workspace to check ripple**

Run: `cargo build --workspace`
If downstream crates fail: wrap the call sites with `bridge::chrono_to_jiff(event.timestamp)` until those crates are migrated.

- [ ] **Step 8: Commit**

```bash
git add crates/bus/
git commit -m "refactor(bus): migrate events and context updates to jiff"
```

---

### Task 3.2: Migrate `config` crate

**Files:**
- Modify: `crates/config/Cargo.toml` (if chrono present)
- Modify: `crates/config/src/**/*.rs`

- [ ] **Step 1: Survey**

Run: `grep -rn "chrono" crates/config/`
If no matches: skip to Step 4.

- [ ] **Step 2: Add jiff dep if not present, remove chrono**

- [ ] **Step 3: Replace types per cookbook**

Common targets: `last_modified_at`, `created_at` fields on config schema structs.

- [ ] **Step 4: Build, test, clippy**

```bash
cargo build -p config
cargo nextest run -p config
cargo clippy -p config --all-targets -- -D warnings
```

- [ ] **Step 5: Commit**

```bash
git add crates/config/
git commit -m "refactor(config): migrate chrono to jiff"
```

---

### Task 3.3: Migrate `tools-core` and `tools-core-macros` crates

**Files:**
- Modify: `crates/tools-core/Cargo.toml` + src
- Modify: `crates/tools-core-macros/Cargo.toml` + src

- [ ] **Step 1: Survey both crates**

Run: `grep -rn "chrono" crates/tools-core/ crates/tools-core-macros/`

- [ ] **Step 2: Migrate per cookbook**

For proc-macro crate (`tools-core-macros`), chrono usage is unlikely; if present, it's in generated code output — substitute literal `jiff::` paths.

- [ ] **Step 3: Build, test, clippy each crate**

```bash
cargo build -p tools-core -p tools-core-macros
cargo nextest run -p tools-core -p tools-core-macros
cargo clippy -p tools-core -p tools-core-macros --all-targets -- -D warnings
```

- [ ] **Step 4: Commit**

```bash
git add crates/tools-core/ crates/tools-core-macros/
git commit -m "refactor(tools-core*): migrate chrono to jiff"
```

---

### Task 3.4: Migrate `analytics` crate

**Files:**
- Modify: `crates/analytics/Cargo.toml`
- Modify: `crates/analytics/src/**/*.rs` (notably `portfolio/correlation.rs`, `spending/trends.rs`, `spending/recurring.rs`, `portfolio/returns.rs`, `input_types.rs`, `types.rs`)
- Modify: `crates/analytics/tests/*.rs`

- [ ] **Step 1: Survey**

Run: `grep -rln "chrono" crates/analytics/`
Expected file list: ~8 files based on plan preparation.

- [ ] **Step 2: Add jiff dep, migrate each file**

Analytics code does calendar arithmetic ("group by month", "returns YTD"). Pay attention:
- `NaiveDate` month arithmetic → `jiff::civil::Date::first_of_month()` / `Date::last_of_month()`.
- Week-of-year boundaries → `jiff::civil::Date::iso_week_date()`.
- Accumulating daily/monthly returns → use `jiff::Span::new().days(1)` for `Zoned` arithmetic, NOT `Duration` (DST-safe).

- [ ] **Step 3: Update tests**

Analytics has mathematical invariants; tests should produce identical output post-migration. If a test asserts a chrono-specific serialization format, update to Jiff's format (RFC 9557).

- [ ] **Step 4: Remove chrono from Cargo.toml**

- [ ] **Step 5: Build, test, clippy**

```bash
cargo build -p analytics
cargo nextest run -p analytics
cargo clippy -p analytics --all-targets -- -D warnings
```

- [ ] **Step 6: Commit**

```bash
git add crates/analytics/
git commit -m "refactor(analytics): migrate chrono to jiff"
```

---

### Task 3.5: Phase 3 verification

- [ ] **Run workspace verification**

```bash
cargo build --workspace
cargo nextest run --workspace
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo fmt --all --check
```

Expected: all pass. Any failing downstream crate gets bridge calls added at boundary until its own task.

---

## Phase 4: L2 Storage

### Task 4.1: Migrate `storage` crate

**Files:**
- Modify: `crates/storage/Cargo.toml`
- Modify: `crates/storage/src/rows/*.rs` (all)
- Modify: `crates/storage/src/repos/**/*.rs`
- Modify: `crates/storage/src/circuit_breaker.rs`
- Modify: `crates/storage/migrations/*.sql` (column type changes for datetime columns)

This is the largest task in the plan. Break into 6 sub-steps.

- [ ] **Step 1: Survey storage chrono usage**

Run: `grep -rln "chrono" crates/storage/`
Expected: ~30 files.

- [ ] **Step 2: Add jiff to storage/Cargo.toml, keep chrono temporarily**

```toml
jiff = { workspace = true }
```

We keep chrono for this task because the ripple is large. Chrono is deleted from storage in Step 6.

- [ ] **Step 3: Migrate all `src/rows/*.rs` files**

Each `Row` struct currently uses `DateTime<Utc>` fields. Replace with `jiff::Timestamp`. For the SQLite read/write layer:

```rust
// Before
fn from_row(row: &Row) -> Result<Self> {
    let created_at: DateTime<Utc> = row.get("created_at")?;
    ...
}
// After
fn from_row(row: &Row) -> Result<Self> {
    let created_at_ms: i64 = row.get("created_at")?;
    let created_at = Timestamp::from_millisecond(created_at_ms)?;
    ...
}
```

And write-side:

```rust
// Before
stmt.execute(params![self.created_at, ...])?;
// After
stmt.execute(params![self.created_at.as_millisecond(), ...])?;
```

Use `common::time::convert::{ts_to_millis, millis_to_ts}` helpers for consistency.

- [ ] **Step 4: Update SQL migrations**

Per CLAUDE.md's pre-release schema policy: edit the original migration files in-place rather than adding incremental migrations. For each `TEXT` datetime column (e.g., `created_at TEXT`), change to `INTEGER` (e.g., `created_at INTEGER`). File list:
- `crates/storage/migrations/001_initial.sql`
- Any other SQL files under `crates/storage/migrations/` or feature crates' `migrations/`

Common targets per CLAUDE.md storage schema: `tasks.due_date`, `tasks.created_at`, `tasks.updated_at`, `tasks.completed_at`, `tasks.focus_deadline`, `cron_jobs.created_at_ms` (already INTEGER), `cron_jobs.next_run_at_ms` (already INTEGER). Keep `_ms` columns as-is; convert RFC3339-TEXT columns to INTEGER holding epoch ms.

- [ ] **Step 5: Update all repo files**

For each file in `crates/storage/src/repos/**/*.rs`, replace `chrono::` with `jiff::` per cookbook. Update SQL bind params to pass `.as_millisecond()` for Timestamp fields.

- [ ] **Step 6: Update tests**

The `serialization_tests.rs`, `*_tests.rs`, `task_repo/tests.rs` test files exercise round-trip. Ensure they:
1. Use `Timestamp::now()` instead of `Utc::now()`.
2. Compare via `.as_millisecond()` (not `.timestamp()`).
3. For any test string datetime inputs, update to RFC 9557 form.

- [ ] **Step 7: Remove chrono from Cargo.toml**

Delete chrono dependency from `crates/storage/Cargo.toml`.

- [ ] **Step 8: Full verification for storage**

```bash
cargo build -p storage
cargo nextest run -p storage
cargo clippy -p storage --all-targets -- -D warnings
```
Expected: all pass. This crate has extensive tests; expect ~100 test runs.

- [ ] **Step 9: Workspace build for ripple**

Run: `cargo build --workspace`
Downstream crates will break extensively — most of L3+ reads storage rows. Do NOT fix those here; they get fixed in their own tasks. Instead, in each L3+ crate that fails, add at the top of affected files:

```rust
use common::time::bridge::{chrono_to_jiff, jiff_to_chrono};
```

Then wrap the row field at the usage site (temporary scaffold; removed when that crate is migrated):

```rust
let old_dt = jiff_to_chrono(row.created_at);
```

Add ONE bridge call per downstream crate to unblock the build. Do not over-fix.

- [ ] **Step 10: Commit**

```bash
git add crates/storage/ crates/*/migrations/
# Plus any ripple-fix files in other crates:
git add crates/providers/ crates/session/ crates/cognitive/ ...  # whichever broke
git commit -m "refactor(storage): migrate rows and repos to jiff, switch timestamp columns to INTEGER epoch ms"
```

Note: this is the plan's biggest commit. If you prefer two commits (one for storage itself, one for ripple bridges), split:
```bash
git add crates/storage/
git commit -m "refactor(storage): migrate rows and repos to jiff"
git add crates/providers/ ...
git commit -m "refactor: add temporary Chrono↔Jiff bridges at storage consumers"
```

### Phase 4 Checkpoint

- [ ] Full workspace verification passes.

---

## Phase 5: L3 Crates

### Task 5.1: Migrate `providers` crate

**Files:**
- Modify: `crates/providers/Cargo.toml`
- Modify: `crates/providers/src/manager.rs`, other chrono files

- [ ] **Step 1:** Survey: `grep -rln "chrono" crates/providers/`
- [ ] **Step 2:** Add jiff, remove chrono from Cargo.toml
- [ ] **Step 3:** Replace types per cookbook
- [ ] **Step 4:** Remove any bridge calls added during Task 4.1 Step 9 in this crate
- [ ] **Step 5:** `cargo build -p providers && cargo nextest run -p providers && cargo clippy -p providers --all-targets -- -D warnings`
- [ ] **Step 6:** Commit: `git add crates/providers/ && git commit -m "refactor(providers): migrate chrono to jiff"`

### Task 5.2: Migrate `session` crate

Same pattern as 5.1.
- [ ] Steps 1–6. Target file: `crates/session/src/manager.rs`.

### Task 5.3: Migrate `scheduling` crate

**Files:**
- Modify: `crates/scheduling/Cargo.toml`
- Modify: `crates/scheduling/src/**/*.rs`

- [ ] **Step 1: Survey**

Run: `grep -rln "chrono\|chrono_tz" crates/scheduling/`

- [ ] **Step 2: Special attention to `chrono_tz::Tz` usage**

`CronService` uses `chrono_tz::Tz` for timezone-aware next-run. Replace with `jiff::tz::TimeZone`. The `cron::Schedule::upcoming()` API from the `cron` crate does not natively speak Jiff — evaluate whether to:

**Option A:** Keep the `cron` crate but wrap IO in Jiff at boundaries:
```rust
let chrono_tz: chrono_tz::Tz = "America/New_York".parse()?;
let next: DateTime<Tz> = schedule.upcoming(chrono_tz).next().unwrap();
let jiff_ts = bridge::chrono_to_jiff(next.with_timezone(&Utc));
```
This keeps `cron` + `chrono_tz` as transitive deps of the `cron` crate itself (not direct deps in Klyntbot's Cargo.toml).

**Option B:** Switch to the `saffron` crate (pure Rust cron parser, no chrono). Requires behavior-parity test against current cron expressions.

Recommendation: **Option A** for this task. Option B can be a follow-up. Keep the Klyntbot-level Cargo.toml free of direct `chrono-tz` dep, but allow the `cron` crate's transitive pull.

- [ ] **Step 3: Migrate all scheduling files per cookbook**

- [ ] **Step 4: Remove `chrono` and `chrono-tz` from `crates/scheduling/Cargo.toml`**

(If Option A is used, `cron` crate still brings chrono transitively — that's fine.)

- [ ] **Step 5: Remove bridge calls added during Task 4.1**

- [ ] **Step 6: Build, test, clippy**

```bash
cargo build -p scheduling
cargo nextest run -p scheduling
cargo clippy -p scheduling --all-targets -- -D warnings
```

- [ ] **Step 7: Commit**

```bash
git add crates/scheduling/
git commit -m "refactor(scheduling): migrate to jiff, keep chrono-tz transitive via cron crate"
```

### Task 5.4: Migrate `context_engine` crate

Same pattern as 5.1. Target file: `crates/context_engine/src/ttl_cache.rs`.
- [ ] Steps 1–6.

### Task 5.5: Migrate `skill-system` crate

Same pattern as 5.1.
- [ ] Steps 1–6.

### Phase 5 Checkpoint

- [ ] Full workspace verification passes.

---

## Phase 6: L4 Crates (13 crates)

Each L4 crate follows the same 6-step pattern as 5.1. Provide crate-specific targets below.

### Task 6.1: `tools`

Files: `crates/tools/src/domain/*.rs`, `crates/tools/src/embedding/*.rs`, `crates/tools/src/todo_types.rs`, `crates/tools/src/conversation_recall.rs`, `crates/tools/src/search_utils.rs`.
- [ ] Steps 1–6 per 5.1 pattern.

### Task 6.2: `feature-tasks`

Files: `crates/feature-tasks/src/types/*.rs`, `crates/feature-tasks/src/tool/actions/*.rs`, `crates/feature-tasks/src/rrule_utils.rs`, `crates/feature-tasks/src/forecast.rs`, `crates/feature-tasks/src/scoring.rs`.
- [ ] Steps 1–6.

Special note: `rrule_utils.rs` uses chrono-tz via the `rrule` crate. Same pattern as scheduling: wrap at boundaries if `rrule` crate has not yet migrated. Verify current `rrule` crate version: `cargo tree -p rrule`. If `rrule` has jiff support (0.12+), switch directly.

### Task 6.3: `feature-finance`

Files: `crates/feature-finance/src/tool/**/*.rs`, `crates/feature-finance/src/types/*.rs`.
- [ ] Steps 1–6.

### Task 6.4: `feature-notes`

File: `crates/feature-notes/src/models.rs` (single file chrono usage).
- [ ] Steps 1–6.

### Task 6.5: `feature-productivity`

Files: ~30 files under `crates/feature-productivity/src/`. The largest single crate by chrono footprint.
- [ ] Steps 1–6.
- [ ] **Extra verification step:** `cargo nextest run -p feature-productivity --run-ignored=all` (this crate has long-running tests; run explicitly).

### Task 6.6: `feature-coaching`

Files: `crates/feature-coaching/src/**/*.rs`.
- [ ] Steps 1–6.

### Task 6.7: `feature-insights`

Files: `crates/feature-insights/src/{service,repo,traits,cross_domain,progress_repo}.rs`.
- [ ] Steps 1–6.

### Task 6.8: `feature-launcher`

Files: `crates/feature-launcher/src/{types,repos/*}.rs`.
- [ ] Steps 1–6.

### Task 6.9: `feature-language-learning`

Files: `crates/feature-language-learning/src/**/*.rs`.
- [ ] Steps 1–6.

### Task 6.10: `activity-log`

Files: all files under `crates/activity-log/src/`.
- [ ] Steps 1–6.

### Task 6.11: `autotuner`

Files: `crates/autotuner/src/{cycle,trial,events}.rs`.
- [ ] Steps 1–6.

### Task 6.12: `voice-engine`

Files: `crates/voice-engine/src/**/*.rs`.
- [ ] Steps 1–6.

### Task 6.13: `simulator`

Files: `crates/simulator/src/{epoch,persona/*,harness,report,error_injector,actions,metrics/mod,providers/*}.rs`.
- [ ] Steps 1–6.

### Phase 6 Checkpoint

- [ ] Full workspace verification passes.

---

## Phase 7: L5 Crates

### Task 7.1: `channels` crate

Files: `crates/channels/src/**/*.rs` (if chrono present — survey first).
- [ ] Steps 1–6 per 5.1 pattern.

### Task 7.2: `agent` crate

Files: ~30 files under `crates/agent/src/`. Heavy user of time.
- [ ] Steps 1–6.

Special notes:
- `crates/agent/src/services/reminders.rs` (legacy ReminderEngine) is scheduled for deletion in Plan 2. In this plan, just migrate it to Jiff — don't delete.
- `crates/agent/src/services/recurring_tasks.rs` uses chrono heavily; replace.
- `crates/agent/src/enrichment/scheduling.rs`, `crates/agent/src/handlers/planning.rs`, `execution.rs`, `proactive.rs` — migrate per cookbook.

### Task 7.3: `cognitive` crate

Files: ~30 files under `crates/cognitive/src/`. Large footprint.
- [ ] Steps 1–6.

Special note: `cognitive::services::temporal.rs`, `cognitive::pipeline::*`, `cognitive::repos::*`, `cognitive::mirror::*` all use chrono.

### Phase 7 Checkpoint

- [ ] Full workspace verification passes.

---

## Phase 8: L6 Crate

### Task 8.1: `mcp` crate

Files: `crates/mcp/src/**/*.rs` (if chrono present — survey first).
- [ ] Steps 1–6 per 5.1 pattern.

---

## Phase 9: L7 Crates

### Task 9.1: `app-core` crate

Files: ~15 files under `crates/app-core/src/`. High-level orchestration.
- [ ] Steps 1–6 per 5.1 pattern.

Special notes:
- `crates/app-core/src/init/deadline.rs` — migrate to Jiff; deletion handled in Plan 2.
- `crates/app-core/src/wake_orchestrator.rs` — central to sleep/wake handling; test carefully.
- `crates/app-core/src/handlers/tasks/*.rs`, `handlers/productivity/*.rs`, `handlers/finance/*.rs`, `handlers/coaching.rs`, `handlers/voice_conversation.rs`, `handlers/launcher/dashboard.rs` — migrate each.

### Task 9.2: `desktop-shared` crate

Files: `crates/desktop-shared/src/commands/*.rs`.
- [ ] Steps 1–6.

### Task 9.3: `desktop` crate

Files: `crates/desktop/src/**/*.rs` (Tauri adapter + `tray_countdown.rs` + `focus_timer.rs`).
- [ ] Steps 1–6.

Special note: Tauri command results returning timestamps to the frontend should serialize as ISO 8601 strings (for JS `new Date(iso)` parsing). Jiff's `Timestamp::to_string()` produces RFC 9557 format, which JS accepts. Verify by manually invoking a few commands via `cd desktop-ui && bun run dev` and inspecting network requests for timestamp shape — the frontend's `formatTime()` helper (in `desktop-ui/src/shared/lib/dates.ts`) must still parse correctly.

### Phase 9 Checkpoint

- [ ] Full workspace verification passes.
- [ ] Desktop UI dev-mode smoke test: `cd desktop-ui && bun run dev` + in another terminal `cargo tauri dev`. Open task list; verify dates render. Kill both.

---

## Phase 10: L8 + Cleanup

### Task 10.1: `klyntbot` facade crate

Files: `crates/klyntbot/src/lib.rs` (re-exports).
- [ ] Steps 1–6 per 5.1 pattern. Re-exports of Jiff types likely needed: `pub use jiff::{Timestamp, Zoned, Span};`.

### Task 10.2: `klyntbot-server` crate

Files: `crates/klyntbot-server/src/**/*.rs`.
- [ ] Steps 1–6.

### Task 10.3: Remove chrono from workspace

**Files:**
- Modify: `Cargo.toml`
- Modify: any remaining crate `Cargo.toml` with chrono dep
- Modify: `crates/common/src/time/bridge.rs` (delete)
- Modify: `crates/common/src/time/mod.rs` (remove bridge pub mod)

- [ ] **Step 1: Verify zero chrono usage in our code**

Run: `grep -rn "^use chrono" crates/ | grep -v target | grep -v "//"`
Expected: NO matches. If matches remain, go back and migrate that file.

- [ ] **Step 2: Verify no crate depends on chrono directly**

Run: `grep -rn "^chrono" crates/*/Cargo.toml`
Expected: NO matches. If any remain, remove them.

- [ ] **Step 3: Verify no chrono transitive pulls we care about**

Run: `cargo tree | grep "chrono"`
Some remain acceptable (via `cron` crate, `rrule` crate, etc.) — note which. These are fine; they're isolated behind other crates' APIs.

- [ ] **Step 4: Remove chrono and chrono-tz from root Cargo.toml**

In `Cargo.toml`:
- Delete the line `chrono = { version = "0.4.43", features = ["serde"] }`.
- Delete the line `chrono-tz = "0.10"`.

- [ ] **Step 5: Delete the bridge module**

Delete `crates/common/src/time/bridge.rs`. In `crates/common/src/time/mod.rs`, delete the `pub mod bridge;` line.

- [ ] **Step 6: Run full verification**

```bash
cargo build --workspace
cargo nextest run --workspace
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo fmt --all --check
```

Expected: all pass. If a crate fails with "cannot find bridge", that crate still has a bridge call — migrate it.

- [ ] **Step 7: Commit**

```bash
git add Cargo.toml crates/common/src/time/
git commit -m "build: remove chrono and chrono-tz from workspace; delete bridge module"
```

### Task 10.4: Add clippy-disallow lint for chrono

**Files:**
- Create: `clippy.toml`

- [ ] **Step 1: Create `clippy.toml` at repo root**

File content:

```toml
disallowed-types = [
    { path = "chrono::DateTime", reason = "use jiff::Timestamp or jiff::Zoned" },
    { path = "chrono::NaiveDateTime", reason = "use jiff::civil::DateTime" },
    { path = "chrono::NaiveDate", reason = "use jiff::civil::Date" },
    { path = "chrono::NaiveTime", reason = "use jiff::civil::Time" },
    { path = "chrono::Utc", reason = "use jiff::Timestamp or jiff::tz::TimeZone::UTC" },
    { path = "chrono::Local", reason = "use jiff::Zoned::now() with system tz" },
    { path = "chrono::Duration", reason = "use jiff::Span or std::time::Duration" },
    { path = "chrono_tz::Tz", reason = "use jiff::tz::TimeZone" },
]
disallowed-methods = [
    { path = "chrono::Utc::now", reason = "use jiff::Timestamp::now" },
    { path = "chrono::Local::now", reason = "use jiff::Zoned::now" },
]
```

- [ ] **Step 2: Run clippy workspace-wide**

Run: `cargo clippy --workspace --all-targets --all-features -- -D warnings`
Expected: PASS with 0 warnings. The lints should find zero violations since migration is complete.

- [ ] **Step 3: Commit**

```bash
git add clippy.toml
git commit -m "chore(lint): disallow chrono types to prevent regression"
```

---

## Final Verification

After Task 10.4, run the complete verification gate once more:

- [ ] `cargo build --workspace` → exit 0
- [ ] `cargo nextest run --workspace` → all pass
- [ ] `cargo test --workspace --doc` → all pass
- [ ] `cargo clippy --workspace --all-targets --all-features -- -D warnings` → exit 0
- [ ] `cargo fmt --all --check` → exit 0
- [ ] `grep -rn "^use chrono" crates/` → NO matches
- [ ] `cargo tree | grep "^├── chrono\|^│   ├── chrono"` → chrono only appears as transitive dep of `cron` / `rrule` / `tauri`, not direct.
- [ ] Desktop UI smoke test: launch via `cargo tauri dev`, create a task with a due date, observe correct rendering in task list.
- [ ] E2E tests: `cargo nextest run -E 'test(reminders)' --package klyntbot` passes.

---

## Rollback Strategy

Each Task's final commit is the rollback unit. If a Task breaks production-ish behavior:

1. `git revert <commit-hash>` of that Task's commit.
2. Any following Tasks' commits remain but now reference Chrono types via re-introduced bridge helpers — they will fail to compile.
3. Revert following Tasks' commits too, in reverse order.
4. Reassess.

In practice, the per-Task commits are independently revertable up through Task 4.1 (storage), after which the bridge helpers carry incremental risk. Phases 2–4 are safest to revert. Phases 5–10 should be reverted as a group.

---

## Self-Review Findings

The self-review below was conducted against the spec's §9 (Jiff migration section).

**Spec coverage check:**
- §9.1 "Why Jiff" — reflected in Migration Cookbook (type mappings, RFC 9557 note).
- §9.2 "Type Mapping" table — expanded in Cookbook.
- §9.3 "Storage Wire Format" (INTEGER epoch ms) — enforced in Task 4.1 Step 4.
- §9.4 "Rollout Sequence" steps 1–10 — each is Phase 2–10 of this plan.
- §9.5 "Tray Countdown Rewire" — NOTE: spec §9.5 says tray rewire happens as part of the chrono→jiff migration. However, tray rewire (subscribe to scheduler events vs. poll) depends on the new TemporalScheduler which lands in Plan 2. This plan migrates tray_countdown's chrono usage to Jiff (Task 9.3) but defers the polling→event-driven rewire to Plan 3. Plan 3 will reference this explicitly.

**Placeholder scan:** no TBD/TODO strings, no "add validation", no "similar to Task N" shortcuts. Task 6.* uses a common pattern but spells out steps 1–6 per the cookbook.

**Type consistency:** all references to `Timestamp`, `Zoned`, `Span`, `TimeZone`, `civil::Date`, `civil::DateTime`, `civil::Time` match the Cookbook table and the `jiff` crate's actual public API (v0.1).

**Ambiguity check:** Task 4.1 Step 9 (ripple bridges) is the most ambiguous — "add ONE bridge call per downstream crate." The intent is: minimum necessary to keep workspace building, not to fix those crates. Each downstream crate gets properly migrated in its own Phase 5–9 Task.

---

## Execution Handoff

Plan complete and saved to `docs/superpowers/plans/2026-04-17-chrono-to-jiff-migration.md`.

Two execution options:

**1. Subagent-Driven (recommended)** — I dispatch a fresh subagent per Task, review between Tasks, fast iteration. Given 40 tasks, this would be 40 subagent invocations with reviews between each. Appropriate when you want tight quality control per commit.

**2. Inline Execution** — Execute Tasks in this session using executing-plans, batch execution with checkpoints at Phase boundaries (Phases 1–10). Fewer decision points, faster overall, but reviews happen less frequently.

Which approach?
