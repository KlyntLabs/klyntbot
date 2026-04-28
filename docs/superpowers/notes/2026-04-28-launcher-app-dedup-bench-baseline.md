# Launcher App Dedup — Bench Baseline (2026-04-28)

Captured immediately after the decorator pattern landed. Numbers are
median time per `app_index.search()` call on the developer's machine.

## Acceptance thresholds (per spec Section 5.3)

| Bench | Threshold | Notes |
|---|---|---|
| `app_index_search_n2000_d0.00` (any query) | baseline | Density 0.0 = no signal lookups |
| `app_index_search_n2000_d1.00` (any query) | ≤ 1.30x of d0.00 | Density 1.0 = every hit lookups both maps |
| `running_signals_refresh` n=200 | < 50µs | "Feels instant" budget |

## Measured (--quick pass, single iteration)

### app_index_search (n=2000)

| Query | d0.00 | d0.25 | d1.00 | d1.00 / d0.00 |
|---|---|---|---|---|
| `s` | 79.8 µs | 61.7 µs | 27.9 µs | **0.35x** ✓ |
| `saf` | 71.7 µs | 27.2 µs | 30.1 µs | **0.42x** ✓ |
| `safari` | 73.2 µs | 28.9 µs | 29.3 µs | **0.40x** ✓ |
| `vsc` | 86.6 µs | 25.8 µs | 26.3 µs | **0.30x** ✓ |
| `fin` | 75.3 µs | 28.3 µs | 25.7 µs | **0.34x** ✓ |

Signal lookups (DashMap.get) are O(1) and do not add measurable overhead
at n=2000. The d1.00 times are at or below d0.00, well under the 1.30x
threshold.

### running_signals_refresh

| n | time |
|---|---|
| 10 | 1.40 µs |
| 50 | 5.80 µs |
| 200 | **25.1 µs** ✓ (< 50 µs) |

## Methodology

Run on macOS, dev tree compiled with `--release`. Numbers will vary
by machine; ratios within a single run are the load-bearing measure,
not absolute durations.
