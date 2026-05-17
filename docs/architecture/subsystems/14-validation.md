# Subsystem 14 — Validation & Benchmarks

> **Status:** 🟠 Scaffolded (replacement pending) — the KCA benchmark suite was removed 2026-05-17; LoCoMo + Letta external evaluations are planned but not yet wired.
> **Status last verified:** 2026-05-17
> **Crates:** *(none — the previous `kca-bench` and `kca-e2e` crates were deleted)*
> **Parent overview:** [`00-overview.md`](../00-overview.md)

---

## TL;DR

The previous validation system used two in-tree crates (`kca-bench`, `kca-e2e`) plus a merge-gating script (`scripts/run_kca_validation.sh`) that ran custom LoCoMo evaluations and asserted quality / perf / stability gates. **All of that was removed on 2026-05-17** in favor of using widely-recognized, externally-published benchmarks for better transparency and easier third-party comparison.

The decision: rather than maintain a custom evaluation harness whose results nobody outside the project can cross-check, wire two established benchmarks:

1. **LoCoMo via mem0** — [github.com/mem0ai/mem0](https://github.com/mem0ai/mem0). Standard long-conversation memory eval. Mem0 publishes their own LoCoMo numbers, so KlyntBot's numbers will be directly comparable.
2. **Letta's MemGPT eval suite** — [github.com/letta-ai/letta](https://github.com/letta-ai/letta). Their public leaderboard makes Letta-vs-others comparison easy.

Both will be wired as out-of-tree harnesses that read the KlyntBot API rather than as in-tree crates.

Until the replacement is wired, **the only enforced gates are the chat-runtime perf scripts**:

```bash
./scripts/run_chat_perf_gates.sh        # TTFT p95, stream throughput, relay cleanup, coalescer p95
./scripts/run_chat_proptest_soak.sh     # 10,000-case event-sequence invariants (gated on release branches)
```

These are runtime-behavior gates, not memory-quality gates — they catch perf regressions and concurrency bugs in the chat path but say nothing about the cognitive memory's accuracy.

---

## What was removed (2026-05-17)

| Artifact | Lines | Purpose |
|---|---|---|
| `crates/kca-bench/` | 1,294 LOC, 11 files | LoCoMo + cost + latency bench, custom grader, trace analyzer, soak generator |
| `crates/kca-e2e/` | 1,095 LOC, 10 files | End-to-end replayer, fixture loader, cognitive-idle wait helper |
| `scripts/run_kca_validation.sh` | ~250 LOC | 7-step merge gate (lint → unit → E2E → bench build → plan-mode E2E → LoCoMo → optional soak) |
| `crates/cognitive/src/bench_hooks.rs` | ~70 LOC | Cross-thread hit counters consumed only by the bench |
| 4 bench-instrumentation call sites in `cognitive/services/{memory_retriever,retrieval}.rs` | 4 lines | `record_hits` / `record_entities` no-ops in production |
| Comment references to `KCA_AUDD` env gate (3 files) | — | The env gate was already removed; comments were stale |
| ~12 bench-only `KCA_*` env vars | — | `KCA_RUN_ID`, `KCA_E2E_LIMIT`, `KCA_BENCH_*`, `KCA_LOCOMO_*`, `KCA_PHASE_1/2/3`, `KCA_AUDD`, `KCA_TRACE`, `KCA_PER_TURN_INGEST`, `KCA_RAW_EPISODE_PERSIST` |

---

## What was kept

**Chat-runtime perf scripts** (`run_chat_perf_gates.sh`, `run_chat_proptest_soak.sh`) — they test live chat behavior (TTFT, stream throughput, coalescer p95, relay cleanup, property-soaked event sequences), not memory quality, so they're orthogonal to the KCA removal.

## Follow-up: all `KCA_*` env vars removed

The 11 runtime feature flags prefixed `KCA_*` (originally kept after the bench removal as "scope B") were also removed later on 2026-05-17. The per-flag fates:

| Flag | Fate |
|---|---|
| `KCA_DISABLE_COMPRESSION` | Deleted (compression always on) |
| `KCA_PHASE_4` + `_TOOL_DRIVEN` + `_LEGACY_NUDGE` (3 flags) | Deleted (memory-refusal nudge feature dropped) |
| `KCA_COMMUNITY_SUMMARIES` | Deleted (reforge Phase 6.7 dropped) |
| `KCA_REFORGE_COMPRESS` | Deleted (reforge Phase 7.7 dropped) |
| `KCA_EPISODIC_THRESHOLD` | Migrated to `config.cognitive.episodicImportanceThreshold` |
| `KCA_TRACE_FSRS` | Deleted; `tracing::debug!` made unconditional (filter via `RUST_LOG=cognitive::services::retrieval=debug`) |
| `KCA_VECTOR` | Hard-coded ON when `vector_store` exists |
| `KCA_OPENAI_EMBED_MODEL` | Migrated to `config.cognitive.openaiEmbeddingModel` |
| `KCA_FACT_SEARCH_HANDLER` | Deleted (experimental routing never enabled) |

Reasoning: features that had been OFF in production for months were dead toggles — deleting them removes carrying-cost without changing observable behavior. Real tunables became proper config fields. Debug traces use standard `RUST_LOG` filtering.

---

## Replacement plan

### Phase 1 — Wire LoCoMo via mem0 (TBD)

Out-of-tree harness that:
1. Spins up an isolated KlyntBot instance with `KLYNTBOT_HOME=<tmpdir>`.
2. Replays the LoCoMo conversation fixture via the `chat_send` Tauri command / MCP `agent` tool.
3. Queries final-state QAs via the same surface.
4. Scores using mem0's published grading prompt.

Output: a directly-comparable accuracy number against the published mem0 / Letta / Memgpt baseline.

### Phase 2 — Wire Letta's eval suite (TBD)

Similar pattern, against Letta's public test set.

### Phase 3 — CI integration (TBD)

Once both harnesses produce stable numbers, gate merges on regression vs the previous main commit's score.

---

## Open questions & debt

- **No memory-quality gate currently fails the build.** Chat-perf gates only. Until LoCoMo/Letta are wired, regressions in retrieval quality can land without anything blocking.
- **The `KCA_*` runtime flags should be renamed.** They still carry the brand prefix even though the bench they were designed alongside is gone. Either rename to `KLYNTBOT_*` or hard-code current defaults and remove the env-flag layer entirely (per-flag decision).
- **Chat-perf gates have known no-ops.** TTFT numeric assertion was "deferred to PR8" and never landed; the script runs the bench but never `exit 1`s on threshold breach. See `TECH_DEBT.md`.

---

## Dependencies & extension points

Nothing else in the workspace depends on this subsystem today. When the replacement harnesses land, document the integration shape here (where they live, how they're invoked, what the merge-gate contract is).
