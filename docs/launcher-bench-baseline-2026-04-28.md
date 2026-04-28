# Launcher Search Benchmark Baseline — 2026-04-28

Captured before Phase 1 (allocation & sort hot-path elimination).

## Hardware
- Apple Silicon (macOS)

## Method
```bash
cargo bench -p feature-launcher --bench inverted_index -- --save-baseline initial
```

## Results

### inverted_index_search_50000

| Query | Median (µs) | Mean (µs) |
|-------|-------------|-----------|
| re    | ~267        | ~271      |
| report| ~127        | ~129      |
| co    | (see target/criterion) | |
| config| (see target/criterion) | |
| ma    | (see target/criterion) | |

### inverted_index_build

| Corpus | (see target/criterion/inverted_index_build) |
|--------|---------------------------------------------|

## Updating the baseline

After intentional algorithmic changes, run:
```bash
cargo bench -p feature-launcher --bench inverted_index -- --save-baseline initial
```

This overwrites the saved baseline in `target/criterion/`.
