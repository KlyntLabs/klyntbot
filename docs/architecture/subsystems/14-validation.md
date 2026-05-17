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

**Runtime feature flags prefixed `KCA_*` are still in the source.** These are independent of the benchmark — they gate live agent and cognitive behavior:

| Flag | Crate | Effect |
|---|---|---|
| `KCA_DISABLE_COMPRESSION=1` | `context_engine` | Skips tiered history compression — verbatim history mode |
| `KCA_PHASE_4=1` | `agent` | Enables Letta-style memory-refusal recovery nudge |
| `KCA_PHASE_4_TOOL_DRIVEN=1` | `agent` | Tool-call nudge instead of text nudge |
| `KCA_PHASE_4_LEGACY_NUDGE=1` | `agent` | Falls back to legacy A/B nudge text |
| `KCA_COMMUNITY_SUMMARIES=1` | `cognitive` | Enables community summary generation in reforge |
| `KCA_REFORGE_COMPRESS=1` | `cognitive` | Enables LLM merge compression in reforge |
| `KCA_EPISODIC_THRESHOLD=<f32>` | `cognitive` | Overrides episodic memory importance threshold (default 0.3) |
| `KCA_TRACE_FSRS=1` | `cognitive` | Emits per-card FSRS trace logs to stderr |
| `KCA_VECTOR=<provider>` | `app-core` | Forces a specific embedding provider |
| `KCA_OPENAI_EMBED_MODEL=<model>` | `tools/embedding` | Overrides OpenAI embedding model |
| `KCA_FACT_SEARCH_HANDLER=1` | `agent` | Routes fact search through the handler path |

These will be renamed (drop the `KCA_` prefix) or hard-coded into defaults in a separate cleanup pass once the replacement benchmark is wired and the team picks a per-flag fate.

**Chat-runtime perf scripts were also kept** (`run_chat_perf_gates.sh`, `run_chat_proptest_soak.sh`) — they test live chat behavior (TTFT, stream throughput, coalescer p95, relay cleanup, property-soaked event sequences), not memory quality, so they're orthogonal to the KCA removal.

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
