# Subsystem 14 — Validation & Benchmarks

> **Status:** 🟡 In Progress — `full_pipeline` criterion bench is a stub; TTFT perf gate is a no-op skeleton; 3 referenced `kca-e2e` fixture files missing from repo; quality-gate enforcement is documentation-only
> **Status last verified:** 2026-05-16
> **Crates:** `kca-bench`, `kca-e2e`
> **Adjacent scripts:** `scripts/run_kca_validation.sh`, `scripts/run_chat_perf_gates.sh`, `scripts/run_chat_proptest_soak.sh`
> **Parent overview:** [`00-overview.md`](../00-overview.md)

---

## TL;DR

Two test crates plus three shell scripts that *should* gate merges to `main`. **They mostly do**, with significant caveats: the LoCoMo quality gate runs but doesn't `exit 1` on a low score, the TTFT perf gate is a compile-and-run skeleton with no numeric check, and `cargo test -p kca-e2e` fails on a clean checkout because 3 fixture files referenced in load-time assertions don't exist in the repo. The merge-gate reputation is partly aspirational; this doc is honest about which gates actually fail-fast and which are documentation-only.

`kca-bench` ships **3 criterion benchmarks** (one of them a stub) + **3 standalone binaries** for offline LoCoMo evaluation, trace analysis, and soak-fixture generation. `kca-e2e` is a test harness with a fixture loader and `ReplayContext` that boots a real `AppCore` per test and drives conversations through it. Synthetic fixtures were removed 2026-05-01 after producing false-green signals at ~30% real LoCoMo accuracy.

---

## Architecture diagram

```mermaid
flowchart TB
    classDef bench fill:#f5f5f5,stroke:#616161,color:#212121
    classDef harness fill:#fff8e1,stroke:#f9a825,color:#f57f17
    classDef gate fill:#ffcdd2,stroke:#c62828,color:#b71c1c
    classDef stub fill:#fce4ec,stroke:#c2185b,color:#880e4f
    classDef miss fill:#f5f5f5,stroke:#999,color:#616161,stroke-dasharray:5

    KB[kca-bench<br/><i>3 criterion benches<br/>+ 3 standalone binaries</i>]:::bench
    KE[kca-e2e<br/><i>ReplayContext · FixtureLoader<br/>real AppCore per test</i>]:::harness

    FP[full_pipeline<br/>STUB — black-boxes a value]:::stub
    PPR[ppr_only<br/><i>50-node + 2000-node chains</i>]:::bench
    EP[extraction_path<br/><i>SemanticFactRepo upsert × 100</i>]:::bench

    RLR[run-locomo-real<br/><i>10 conv × ~150 QA<br/>gpt-4.1 grader (SimpleQA A/B/C)<br/>Mimo fallback</i>]:::bench
    AT[analyze-trace<br/><i>single + diff modes<br/>grade transitions</i>]:::bench
    GS[gen-soak<br/><i>5 personas × 6 topics × 4 actions = 120 fixtures</i>]:::bench

    RKV[run_kca_validation.sh<br/><i>7-step merge gate<br/>quality gate is documentation-only</i>]:::gate
    RCPG[run_chat_perf_gates.sh<br/><i>4 numeric checks (1 is no-op)<br/>TTFT skeleton · throughput · cleanup · coalescer</i>]:::gate
    RCPS[run_chat_proptest_soak.sh<br/><i>10,000 cases<br/>active_streams_drains_soaked</i>]:::gate

    FIX1[(locomo10_real.json<br/>✅ present)]:::harness
    FIX2[(regression_panel.jsonl<br/>soak_10k.jsonl<br/>✅ present)]:::harness
    FIX3[(longmembench_subset.jsonl<br/>klynt_coding_bench.jsonl<br/>hallucination_planted.jsonl<br/>❌ MISSING — asserted in load test)]:::miss

    FP --> KB
    PPR --> KB
    EP --> KB
    RLR --> KB
    AT --> KB
    GS --> KB
    KE --> FIX1
    KE --> FIX2
    KE -.asserts non-empty.-> FIX3
    RLR --> FIX1
    RKV --> KE
    RKV --> RLR
    RKV --> RCPS
    RCPG --> FP
```

---

## Mental model

**Two layers, three scripts:**

1. **`kca-bench`** — criterion micro-benchmarks + standalone CLI tools. Used both inside `run_kca_validation.sh` (release-LoCoMo step) and standalone for offline analysis.
2. **`kca-e2e`** — integration test harness. Boots `AppCore` in-memory, replays JSONL fixtures through it. Asserts functional correctness and exposes hooks for perf measurement.

The **three scripts** orchestrate:
- `run_kca_validation.sh` — the merge gate (7 steps)
- `run_chat_perf_gates.sh` — the chat path perf gates (4 numeric checks)
- `run_chat_proptest_soak.sh` — the property-test soak (10,000 cases gated)

### What "gate" actually means today

A gate is only as strong as the script that checks it. Three honesty notes:

| Claim | Reality |
|---|---|
| "LoCoMo quality gate" | `run_kca_validation.sh` runs `run-locomo-real` but **doesn't `exit 1` on a low score** — only on runtime error. Quality threshold is documentation-only. |
| "TTFT p95 ≤ 15ms" *(from earlier docs)* | Actual threshold default: `THRESHOLD_TTFT_P95_MS=25`. **And the check is a skeleton** — prints "numeric gate deferred to PR8" and never fails. |
| "Bundle budget: 30 KB gzipped" *(from earlier docs)* | Actual `.size-limit.json`: 350 KB gzipped for threads route, 2.5 MB total. **And it's not wired into any of the three scripts.** Off by an order of magnitude. |

So 3 of 4 chat perf gates and 0 of 1 LoCoMo quality gate actually fail builds today.

---

## Reference

### `kca-bench` — file map

| Path | Purpose |
|---|---|
| `src/lib.rs` | Crate-level doc explaining synthetic-fixture removal (2026-05-01) |
| `src/locomo_real.rs` | LoCoMo10 loader |
| `benches/full_pipeline.rs` | `bench_full_pipeline_stub` — **stub** (black-boxes a `ConversationFixture` value) |
| `benches/ppr_only.rs` | `ppr_50_nodes`, `ppr_2000_nodes` |
| `benches/extraction_path.rs` | `semantic_fact_upsert_100` |
| `src/bin/run_locomo_real.rs` | Real LoCoMo10 runner with LLM grader |
| `src/bin/analyze_trace.rs` | Offline JSONL trace analyzer (single + diff modes) |
| `src/bin/gen_soak.rs` | Generates `120` `ConversationFixture` records (5 personas × 6 topics × 4 actions) |

### Criterion benchmarks

All declared with `harness = false` in `Cargo.toml`.

| Bench | What it measures | Reality |
|---|---|---|
| **`full_pipeline`** | *Intended:* end-to-end agent turn | 🔴 **Stub.** Black-boxes a `ConversationFixture` value without invoking `AppCore`. Exists to hold the slot + prove compilation. |
| **`ppr_only`** | `personalized_pagerank` on chain graphs (50 nodes, 2000 nodes) | ✅ Working; pure graph-math throughput |
| **`extraction_path`** | `SemanticFactRepo::upsert` × 100 against in-memory SQLite | ✅ Working; hot write-path measurement |

### Standalone binaries

#### `run-locomo-real`

`src/bin/run_locomo_real.rs`. Loads `tests/fixtures/kca/locomo10_real.json`, replays every session through `AppCore::chat_complete` with a fresh `ReplayContext` per conversation, then grades QA pairs via LLM (default gpt-4.1, SimpleQA-style A/B/C).

**Env vars:**
| Var | Default | Effect |
|---|---|---|
| `OPENAI_API_KEY` | required | Grader API key |
| `KCA_LOCOMO_LIMIT` | all 10 | Cap on conversations |
| `KCA_LOCOMO_QA_LIMIT` | all ~199/conv | Cap on QA pairs per conversation |
| `KCA_LOCOMO_GRADER_MODEL` | `gpt-4.1` | Grader model |
| `KCA_LOCOMO_GRADER_URL` | OpenAI | Can route to Mimo when OpenAI rate-limited (Xiaomi `token-plan-cn.xiaomimimo.com/v1`) |
| `KCA_LOCOMO_GRADER_KEY` | — | Grader-specific API key |

**Output:** accuracy by category, p50/p95 latency, estimated cost.

**Reference numbers** (printed at start by `run-locomo-real`):
- Letta + gpt-4o-mini = **74.0%**
- Mem0 graph = **68.5%**

**Comparability note:** Switching grader model away from `gpt-4.1` breaks Letta comparability — scores become self-consistent only.

#### `analyze-trace`

Offline JSONL trace analyzer. **Two modes:**

| Mode | Behavior |
|---|---|
| Single-file | A/B/C grade counts; 3 failure-mode buckets (zero-retrieval, retrieved-but-refused, retry-fired-still-failed); per-category accuracy; mechanism activity; p50/p95 latency |
| Two-file diff | Joins traces on `(conv_id, qa_index)`; prints C→A gains + A→C regressions with mechanism field diffs |

Used to **attribute accuracy lift** between bench phases (e.g., did Phase 3 help, did Phase 4 regress).

#### `gen-soak`

Generates `100` (actually `120` — see [Surprises](#surprises--non-obvious-facts)) base `ConversationFixture` records via cartesian product: 5 personas × 6 topics × 4 actions = **120**. Writes JSONL to stdout. Intended target: `tests/fixtures/kca/soak_10k.jsonl`.

### LoCoMo10 real dataset

| Property | Value |
|---|---|
| Path | `tests/fixtures/kca/locomo10_real.json` |
| Source | Bit-for-bit from `letta-ai/letta-leaderboard`'s `locomo10.json` |
| Conversations | 10 multi-session |
| QA pairs | ~1,986 across 5 categories |
| Category 1 | single-hop |
| Category 2 | multi-hop / temporal |
| Category 3 | open-domain |
| Category 4 | adversarial |
| Category 5 | abstention — **skipped** to maintain Letta-comparable scoring |
| Loaded by | `src/locomo_real.rs` via `serde_json` |
| **Mutation policy** | **Must not be modified** |

### `kca-e2e` — `ReplayContext`

```rust
ReplayContext::new()
   - Boots real AppCore (temp dir, IntelligenceMode::Deep)
   - Enables micro-reforge + predictive cache + hierarchical
   - Runs cognitive migrations
   - Subscribes background task to DomainEvent capture

.replay(fixture)
   - Per turn: feed TurnFixture.user through chat_complete
   - Calls await_cognitive_idle() after every turn

.chat_complete(content, session_key)
   - app.chat_send
   - Drains AgentEvent::ContentChunk + Done
   - **Manually publishes ChatTurnCompleted to domain bus** (bypasses relay_chat_stream)
   - Optional fallback to bench_direct_qa when KCA_BENCH_DIRECT_FALLBACK=1

.await_cognitive_idle()
   - Polls SemanticFactRepo::count_active every 750ms
   - Declares idle after 4 consecutive stable readings
   - **Mandatory 14-second floor** (covers Kimi K2's worst-case 12-second extraction tail)
   - 60-second hard timeout

.dump_facts()  → all active (subject, predicate, object) rows
.record_raw_episode(domain, content, occurred_at)
   - Gated by KCA_RAW_EPISODE_PERSIST=1
.consolidate_graph()
   - Gated by KCA_PHASE_3=1
   - Calls app.trigger_graph_consolidation()
```

### `kca-e2e` — `FixtureLoader`

```rust
load_jsonl::<T>(path) -> Vec<T>
   - Line-by-line; fails fast with line number on parse error

fixtures_root()
   - Resolves to tests/fixtures/kca/ relative to CARGO_MANIFEST_DIR
   - Two parent() hops from the crate manifest

Types:
- ConversationFixture { id, turns, queries, source, metadata }
- TurnFixture { user, assistant, tool_calls, ground_truth_facts, cli_source, recorded_at }
- QueryFixture { query, gold_answer, hop_type, required_fact_subjects }
```

### `kca-e2e` load-time fixture assertions — 🚨 broken on clean checkout

`src/lib.rs` unit test `loads_seed_fixtures_without_error` asserts these files exist and are non-empty:

| File | Present? |
|---|---|
| `tests/fixtures/kca/longmembench_subset.jsonl` | ❌ Missing |
| `tests/fixtures/kca/klynt_coding_bench.jsonl` | ❌ Missing |
| `tests/fixtures/kca/hallucination_planted.jsonl` | ❌ Missing |

**Consequence:** `cargo test -p kca-e2e` fails on a clean checkout. README notes these were removed but the load-time assertion wasn't updated. New P0 tech-debt item.

### `run_kca_validation.sh` — the 7-step merge gate

```bash
1. Lint            cargo fmt --check + cargo clippy -D warnings on kca-e2e + kca-bench
2. Unit + integ    cargo nextest run -p kca-e2e -p kca-bench --tests
3. E2E (real API)  cargo nextest run -p kca-e2e for:
                     - cancellation_safety
                     - migration_safety
                     - regression_panel
                     - multi_cli_parity
                     - full_pipeline
                   (requires ~/.klyntbot/config.json with API keys)
4. Bench build     cargo check -p kca-bench (compile-only)
5. Plan-mode E2E   cargo nextest run -p app-core --test plan_mode_e2e
6. Real LoCoMo     cargo run -p kca-bench --release --bin run-locomo-real
                   Default: KCA_LOCOMO_LIMIT=2, KCA_LOCOMO_QA_LIMIT=10 (~20 Qs, ~3 min)
                   Full:    KCA_LOCOMO_LIMIT=10 × ~150 QA ≈ 1,500 questions
                   Requires OPENAI_API_KEY
                   ⚠️ Script does NOT fail on low score — only on runtime error
7. (Optional) Soak if RUN_SOAK is set:
                   cargo nextest run -p kca-e2e --features soak --test soak_test
```

**Exit semantics:** `set -euo pipefail`. Any non-zero exit fails immediately. Prints `✅ KCA validation passed` only if all steps succeed.

**Quality-gate enforcement is documentation-only.** No `exit 1` on low LoCoMo score; the script only catches runtime errors.

### `run_chat_perf_gates.sh` — 4 numeric assertions (1 is a no-op)

| Gate | Source bench | Threshold | Enforced? |
|---|---|---|---|
| **TTFT p95** | `agent/benches/ttft_e2e.rs` | `THRESHOLD_TTFT_P95_MS=25` (default) | 🔴 **No.** "Numeric gate deferred to PR8" — only checks compile + run. |
| **Stream throughput** | `agent/benches/stream_throughput.rs` | `THROUGHPUT_THRESHOLD=5000 evt/s` | ✅ Yes. `awk` normalizes K/M suffixes; fails if result < 5000. |
| **Relay cleanup** | `desktop/benches/relay_cleanup_latency.rs` | `THRESHOLD_CLEANUP_P99_MS=1` ms | ✅ Yes. (Label says "p99" but criterion reports mean — slight naming inconsistency.) |
| **Coalescer** | `desktop-ui/__benches__/coalescer.bench.ts` | `THRESHOLD_COALESCER_10K_MS=16` ms | ✅ Yes. `grep -A1` extracts the "10,000 chunks" mean. |

**Bundle budget gate:** Not in `run_chat_perf_gates.sh`. `.size-limit.json` defines two budgets:
- "threads route" ≤ **350 kB gzipped** *(threads + messages + coding JS chunks)*
- "total app" ≤ 2.5 MB gzipped

Neither matches the "30 kB gzipped" claim in earlier docs. The size-limit file is not referenced by any of the three scripts. Manually invoked via `bun run size-limit`.

### `run_chat_proptest_soak.sh`

```bash
cargo nextest run --test property -E 'test(active_streams_drains_soaked)' --features soak
```

- With `--features soak`: 10,000 random op-sequences via `ProptestConfig::with_cases(10_000)`
- Without: 100 cases (default proptest)
- **Invariant tested (`CHAT-INV-1`):** After any random sequence of `Send(id)` / `Cancel(id)` / `SimulateError(id)` / `SimulateComplete(id)` ops (up to 5 session IDs, sequence length up to 20), `active_streams` is empty (all streams eventually drain).

---

## Workflows

### Running the full merge gate locally

```bash
# Set defaults (or override)
export OPENAI_API_KEY=sk-...
export KCA_LOCOMO_LIMIT=2
export KCA_LOCOMO_QA_LIMIT=10
export RUN_SOAK=1   # only for release branches

./scripts/run_kca_validation.sh
```

Wall-clock with defaults: ~10 minutes (lint + unit + E2E + bench check + plan-mode + small LoCoMo). Full LoCoMo (`KCA_LOCOMO_LIMIT=10`, full QA): add 1–2 hours.

### Running chat perf gates after a chat-path change

```bash
./scripts/run_chat_perf_gates.sh
```

Runs the 4 criterion benches + the vitest bench. Outputs measured values + threshold check. Fails on any non-skeleton gate that misses threshold.

### Attributing a LoCoMo accuracy change between two runs

```bash
# Run twice — once baseline, once with changes
cargo run -p kca-bench --release --bin run-locomo-real > baseline.jsonl
cargo run -p kca-bench --release --bin run-locomo-real > changes.jsonl

# Diff-mode trace analysis
cargo run -p kca-bench --release --bin analyze-trace -- diff baseline.jsonl changes.jsonl
```

Output: C→A gains list, A→C regressions list, mechanism field diffs per regression.

---

## Internals

### Why the 14-second mandatory floor in `await_cognitive_idle`

The default poll interval is 750ms with 4 stable readings → ~3 second idle window minimum. **But Kimi K2 has observed worst-case 12-second extraction tail.** Without the 14-second floor, the harness declares idle before LLM extraction finishes writing facts, and benchmarks query a half-populated store.

This is a non-obvious coupling between the bench harness and the cognitive pipeline. If the cognitive pipeline ever changes extraction latency characteristics (faster or slower), this floor needs adjustment.

### Why `chat_complete` manually publishes `ChatTurnCompleted`

```rust
// inside ReplayContext::chat_complete after draining events
app.bus.publish(DomainEvent::ChatTurnCompleted { ... });
```

Without this, `IngestionConsumer` never fires and `semantic_facts` stays empty — the bench would query a half-populated store. This is **a non-obvious coupling between the bench harness and the cognitive pipeline**: production `relay_chat_stream` publishes this event normally, but the harness bypasses the relay.

If the bus event shape changes, this manual publish must be updated in lockstep.

### Why synthetic fixtures were removed

`crates/kca-bench/src/lib.rs` doc comment: synthetic LoCoBench / LongMemBench / klynt-coding subsets were authored by Klynt and the pipeline had been *tuned against them*, producing false-green scores while real LoCoMo accuracy was stuck at ~30%. The eval was scoring itself against its own training. Real LoCoMo is the only honest measure now.

### How `awk` extracts criterion numbers

For throughput:
```bash
grep -oP 'thrpt:\s+\[\K[^\]]+'   # captures the bracketed three-number range
awk '{print $2}'                  # selects the median
# Then K/M normalization inline
```

For cleanup latency:
```bash
# Raw µs value extracted, divided by 1000 with awk
awk "BEGIN {printf \"%.3f\\n\", $RAW / 1000.0}"
```

For coalescer (vitest output):
```bash
grep -A1 "10,000 chunks"   # find the line below the label
# Strip "ms" suffix, compare to threshold
```

### How `analyze-trace` joins two runs

Diff mode reads both trace JSONLs into memory, indexes by `(conv_id, qa_index)`. For each common key, compares grade fields. C→A means the change fixed something previously wrong; A→C means the change broke something previously right. Mechanism diff highlights which retrieval/extraction stage's output differed.

### `gen-soak` outputs 120 fixtures (not 100)

`5 personas × 6 topics × 4 actions = 120`. README says "100 base fixtures" — off by 20. The downstream soak test asserts `fixtures.len() >= 100`, which passes either way.

### Grader model can route to Mimo

When OpenAI is rate-limiting, set `KCA_LOCOMO_GRADER_URL=https://token-plan-cn.xiaomimimo.com/v1/...` + `KCA_LOCOMO_GRADER_KEY=...`. The harness retries with exponential backoff (3x). **Switching grader models breaks Letta comparability** — scores become self-consistent only (still useful for relative regression detection, not for comparison to leaderboards).

---

## Dependencies & extension points

### Upstream deps

- `criterion` (criterion benches)
- `proptest` (soak)
- `serde` / `serde_json` (fixtures + traces)
- `tokio` (ReplayContext async)
- `reqwest` (grader API calls)
- `app_core`, `agent`, `cognitive`, `coding-memory` (the subjects under test)

### Adding a new criterion benchmark

1. Add `crates/kca-bench/benches/<name>.rs`.
2. Add to `Cargo.toml`:
   ```toml
   [[bench]]
   name = "<name>"
   harness = false
   ```
3. If the bench should be released-mode-only, gate appropriately.
4. Run via `cargo bench -p kca-bench --bench <name>`.

### Adding a new perf gate to `run_chat_perf_gates.sh`

1. Run the bench inside the script.
2. Extract the numeric output via `grep`/`awk`.
3. Compare to a `THRESHOLD_*` env var (with a sensible default).
4. `FAIL=1` on miss; print a clear failure message.
5. Document the threshold in this doc.

### Adding a new fixture file consumed by `kca-e2e`

1. Place under `tests/fixtures/kca/`.
2. Add a load-time assertion in `crates/kca-e2e/src/lib.rs::loads_seed_fixtures_without_error` (or a similar test).
3. **Always commit the fixture.** Otherwise `cargo test -p kca-e2e` fails on clean checkouts — see [Open questions](#open-questions--debt).

### Adding a new merge-gate step

1. Edit `scripts/run_kca_validation.sh`.
2. Add the step with clear logging.
3. `set -euo pipefail` ensures any failure aborts.
4. Document the gate in this doc.

### Adding a soak proptest

1. Write a `#[cfg(feature = "soak")]` block in the relevant test module.
2. Use `ProptestConfig::with_cases(10_000)` (or higher).
3. Mark the non-soak variant with `with_cases(100)` so default runs are fast.
4. Update `run_chat_proptest_soak.sh` to include the new test name.

---

## Open questions & debt

- **`cargo test -p kca-e2e` fails on a clean checkout.** Three fixture files (`longmembench_subset.jsonl`, `klynt_coding_bench.jsonl`, `hallucination_planted.jsonl`) are asserted non-empty in `lib.rs` but absent from the repo. **P0 — blocks new contributors.** Either commit the fixtures or remove the assertions.
- **`full_pipeline` criterion bench is a stub.** Black-boxes a value; doesn't drive `AppCore`. Implement or remove the slot.
- **TTFT perf gate is a no-op skeleton.** Threshold `25ms` (not `15ms` as earlier docs claim); check `numeric gate deferred to PR8`. Implement the numeric assertion.
- **LoCoMo quality gate is documentation-only.** `run-locomo-real` runs but the script never `exit 1`s on a low score. Add a threshold check.
- **Bundle budget is 350 kB, not 30 kB** as earlier docs claim. Wire `.size-limit.json` into `run_chat_perf_gates.sh` or similar.
- **`gen-soak` outputs 120 fixtures**, not 100 as the README says. Either align README or change the cartesian product.
- **14-second mandatory floor** in `await_cognitive_idle` is hardcoded for Kimi K2's worst case. Should be configurable.
- **Bench harness's manual `ChatTurnCompleted` publish** is a non-obvious coupling. Coupling test or runtime assertion that production publishes the same event.
- **Mimo grader routing breaks Letta comparability** but isn't surfaced as a warning in `run-locomo-real`'s output.
- **No `cancellation_safety`/`migration_safety`/`regression_panel`/etc. test enumeration** outside the merge script. Add to the doc when the test names stabilize.

See [`TECH_DEBT.md`](../TECH_DEBT.md) categories #2 (stubs), #5 (doc drift), #9 (untracked surfaces) for specifics.

---

## Cross-references

- [`00-overview.md`](../00-overview.md) — perf gate thresholds (corrected by this doc)
- [`02-storage.md`](./02-storage.md) — `SemanticFactRepo` used by `extraction_path` bench
- [`05-cognitive-memory.md`](./05-cognitive-memory.md) — KCA validation gates this subsystem enforces; `KCA_DISABLE_COMPRESSION` / `KCA_COMMUNITY_SUMMARIES` / `KCA_REFORGE_COMPRESS` env flags
- [`13-desktop-frontend.md`](./13-desktop-frontend.md) — frontend bundle budget; coalescer perf gate
