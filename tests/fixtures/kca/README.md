# KCA Test Fixtures

## Real, Letta-comparable benchmarks

- `locomo10_real.json` — full LoCoMo-10 dataset from
  [letta-ai/letta-leaderboard](https://github.com/letta-ai/letta-leaderboard/blob/main/leaderboard/locomo/locomo10.json).
  10 multi-session conversations, ~1986 QA pairs across 5 categories.
  Loaded by `crates/kca-bench/src/locomo_real.rs`. Do not modify — bit-for-bit
  parity with Letta's eval is the whole point.

## Internal regression / soak fixtures

- `regression_panel.jsonl` — 30 historical-bug reproducers consumed by
  `kca-e2e/tests/regression_panel.rs`. Each entry pins a fix; if the test goes
  red, the corresponding regression has returned.
- `soak_10k.jsonl` — 100 base fixtures replayed many times by `soak_test.rs`.
  Generated via `cargo run -p kca-bench --bin gen-soak`. Soak runs only on
  tagged release branches (`RUN_SOAK=1`).

## History

The synthetic LoCoBench / LongMemBench / klynt-coding subsets were removed
2026-05-01. They were Klynt-authored fixtures that the system had been tuned
against, so green numbers there did not predict real LoCoMo scores. We now
measure only against canonical eval datasets.
