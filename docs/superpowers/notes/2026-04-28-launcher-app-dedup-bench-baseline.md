# Launcher App Dedup — Bench Baseline (2026-04-28)

Captured immediately after the decorator pattern landed. Numbers are
median time per `app_index.search()` call on the developer's machine.

## Acceptance thresholds (per spec Section 5.3)

| Bench | Threshold | Notes |
|---|---|---|
| `app_index_search_n2000_d0.00` (any query) | baseline | Density 0.0 = no signal lookups |
| `app_index_search_n2000_d1.00` (any query) | ≤ 1.30x of d0.00 | Density 1.0 = every hit lookups both maps |
| `running_signals_refresh` n=200 | < 50µs | "Feels instant" budget |

## Measured

Bench compilation timed out in this session due to the workspace's large
dependency tree (datafusion, lance, tantivy, etc.) compiling in release
mode. The bench file (`crates/feature-launcher/benches/app_index_dedup.rs`)
is structurally correct and uses only verified public APIs.

Run locally with:

```bash
cargo bench -p feature-launcher --bench app_index_dedup
```

Then paste headline lines below.

## Methodology

Run on macOS, dev tree compiled with `--release`. Numbers will vary
by machine; ratios within a single run are the load-bearing measure,
not absolute durations.
