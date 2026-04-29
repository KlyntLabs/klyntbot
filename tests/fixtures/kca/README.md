# KCA Test Fixtures

Each `*.jsonl` file is one ConversationFixture per line (see `crates/kca-e2e/src/fixtures/mod.rs`).

## Subsets

- `longmembench_subset.jsonl` — Klynt-curated long-context memory benchmark (modeled after Xiao et al's LongMem-style suite). 100 conversations.
- `locobench_subset.jsonl` — Klynt-curated conversational memory benchmark (modeled after the LoCoMo paper). 100 conversations.
- `klynt_coding_bench.jsonl` — custom: 50 dead-end retrieval, 50 fix-attempt recall, 30 multi-CLI transfer pairs.
- `multi_cli_replay.jsonl` — same conversation replayed across {ClaudeCode, Codex, KimiCli, OpenCode}.
- `hallucination_planted.jsonl` — synthetic conversations with extractor lures. Used to score Track 5 critic.
- `regression_panel.jsonl` — 30 historical-bug reproducers; if these pass, the corresponding regression has not returned.
- `soak_10k.jsonl` — 100 base fixtures replayed many times by `soak_test.rs`.

## Generating

`crates/kca-bench/src/dataset_loader.rs` holds helpers that fetch upstream sources and emit our subset format. For most CI runs the JSONL files are committed and used as-is.
