# Chrono → Jiff Migration Guide (Cookbook)

This is a standalone reference extracted from the main plan
(`2026-04-17-chrono-to-jiff-migration.md`). Every crate-level task
references this guide rather than repeating the mappings inline.

## Type Mappings

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

## Common Method Mappings

| Chrono API | Jiff API |
|---|---|
| `dt.to_rfc3339()` | `dt.to_string()` (Jiff default = RFC 9557 IXDTF) |
| `DateTime::parse_from_rfc3339(s)` | `s.parse::<Timestamp>()` (accepts RFC 3339 + RFC 9557) |
| `dt.timestamp()` | `ts.as_second()` |
| `dt.timestamp_millis()` | `ts.as_millisecond()` |
| `Utc.timestamp_opt(s, 0).unwrap()` | `Timestamp::from_second(s)?` |
| `Utc.timestamp_millis_opt(ms).unwrap()` | `Timestamp::from_millisecond(ms)?` |
| `dt + chrono::Duration::seconds(n)` | `ts + Span::new().seconds(n)` or `ts + Duration::from_secs(n)` |
| `dt.date_naive()` | `ts.to_zoned(tz).date()` or `dt.date()` (on `Zoned`) |
| `dt.with_timezone(&tz)` | `ts.to_zoned(tz)` |
| `dt.naive_local()` | `zoned.datetime()` (returns `civil::DateTime`) |

## Serde

Default Jiff serialization = RFC 9557:

```rust
#[derive(Serialize, Deserialize)]
struct Row {
    fire_at: jiff::Timestamp,     // serializes as "2026-04-17T14:30:00Z"
}
```

For explicit integer-epoch serialization:

```rust
use jiff::fmt::serde::timestamp::millisecond::required as ts_millis;
#[derive(Serialize, Deserialize)]
struct Row {
    #[serde(with = "ts_millis")]
    fire_at: jiff::Timestamp,
}
```

## SQLite Wire Format Convention

- All persisted timestamps → `INTEGER` columns, Unix epoch milliseconds, UTC.
- Read: `Timestamp::from_millisecond(row.get::<_, i64>("fire_at"))?`
- Write: `ts.as_millisecond()` → `i64`
- Do not store RFC 3339/9557 strings in SQLite (exception: human-readable debug columns).
- Pre-existing TEXT datetime columns migrate to INTEGER in each crate's storage task
  (pre-release policy: edit migrations in place per CLAUDE.md).

## Bridge Helper (Transient)

In `common::time::bridge`, used only at crate boundaries during migration:

```rust
pub fn chrono_to_jiff(dt: chrono::DateTime<chrono::Utc>) -> jiff::Timestamp;
pub fn jiff_to_chrono(ts: jiff::Timestamp) -> chrono::DateTime<chrono::Utc>;
```

Removed in Task 10.3 after all crates are migrated.

## Forbidden Patterns (Post-Migration Lints)

Task 10.4 adds these to `clippy.toml`:

- `chrono::` (any path) — use `jiff::` equivalent
- `chrono_tz::` — use `jiff::tz::TimeZone`
- `use chrono` — forbidden
- `Local::now()` without explicit tz — use `Zoned::now()` or `Timestamp::now().to_zoned(tz)`

## Per-Crate Migration Checklist

1. Survey: `grep -rln "chrono" crates/<name>/`
2. Add jiff to `Cargo.toml`: `jiff = { workspace = true }`
3. Apply type mappings per this guide to every `.rs` file
4. Update any tests that compare serialized time values (RFC 3339 → RFC 9557)
5. Remove `chrono` line from `Cargo.toml` (keep if the crate genuinely still needs the bridge helper)
6. Verify: `cargo build -p <name> && cargo nextest run -p <name> && cargo clippy -p <name> --all-targets -- -D warnings`
7. Commit per layer PR conventions

## Common Pitfalls

- **Jiff's `Timestamp` has nanosecond precision, Chrono's has nanosecond too** — no data loss on round-trip via milliseconds.
- **Jiff doesn't have `Local` as a type** — use `TimeZone::system()` which returns a `TimeZone`, then `ts.to_zoned(tz)`.
- **`parse::<Timestamp>()` is strict** — requires `Z` suffix or explicit offset. Civil datetimes (no tz) must use `civil::DateTime`.
- **`Span` arithmetic on `Timestamp` requires a timezone for calendar units** — `Timestamp + Span::new().days(1)` errors if Span has day-level units. Use `SignedDuration` or compute on `Zoned`.
- **Serde field rename** — if migrating a struct that used `#[serde(with = "chrono::serde::ts_seconds")]`, replace with `#[serde(with = "jiff::fmt::serde::timestamp::second::required")]`.
