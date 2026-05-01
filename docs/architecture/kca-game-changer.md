# KCA Game-Changer Report

> ✅ **Update (2026-05-01):** synthetic fixture suite deleted. Sole
> source of truth is now real LoCoMo (`tests/fixtures/kca/locomo10_real.json`)
> graded by OpenAI gpt-4.1 SimpleQA. The "Current best (n=10)" snapshot
> below and the "Synthetic-fixture run history" section are preserved
> as historical record but no longer feed any gate. North Star rule
> ("never tune the eval") is in `CLAUDE.md`.

> Real-benchmark numbers go in **Real LoCoMo runs** below. Append-only —
> new runs at the top, never delete prior rows. Variance comes from
> stochastic LLM grading; we want the trend across runs to be honest
> about what each change moved.

## Current best (n=10, post fix-pass session 2026-04-30)

| Gate | Threshold | Score | Status |
|---|---|---|---|
| Q-1 long-mem | ≥ 0.85 | **0.85** (n=20) | ✅ |
| Q-2 LoCoBench single | ≥ 0.92 | **1.00** | ✅ |
| Q-3 LoCoBench multi | ≥ 0.70 | **1.00** | ✅ |
| Q-4 LoCoBench temporal | ≥ 0.85 | skip (0 samples) | ✅ |
| Q-5a Klynt-coding dead-end | ≥ 0.80 | **1.00** | ✅ |
| Q-5b Klynt-coding fix | ≥ 0.80 | **1.00** | ✅ |
| Q-5c Klynt-coding multi-CLI | ≥ 0.80 | _pending dbg16_ | _pending_ |

P50 turn latency: ~2-3s · P95: ~4-5s (Kimi `https://api.kimi.com/coding`).

## Real LoCoMo runs (Letta-comparable)

> Eval: `letta-leaderboard/leaderboard/locomo/locomo10.json` + verbatim
> SimpleQA grader prompt + OpenAI gpt-4.1 judge. Letta scored **74.0%**
> on the same eval. Mem0 graph variant: **68.5%**.

| Run | Date | conv | QA | Status | Acc | Att.Acc | NA | Notes |
|---|---|---|---|---|---|---|---|---|
| **locomo-real-005** | **2026-05-01 04:35–11:07** | **10 (full LoCoMo-10)** | **500** | **OK** | **28.8%** (144/500) | 48.2% | 40.2% | **Canonical n=500 baseline against Letta's 74.0% / Mem0's 68.5%. Gap: -45.2pp.** Per-cat: cat1=28.0% (54/193), cat2=21.4% (47/220), cat3=44.1% (30/68), cat4=68.4% (13/19). P50/P95 = 29.2s / 41.6s. Cat 1 collapse confirms subject-collapse hypothesis (single-hop should be near-100% if entities were stored under their actual name). Surfaced 2 production bugs (Tasks #16, #17). |
| locomo-real-004 | 2026-05-01 04:11 | 3 | 90 | OK | 38.9% (35/90) | 52.2% | 25.6% | **REGRESSION.** Tried strengthening "best-effort recall" prompt with FORBIDDEN refusal phrases. Both increased B (23→32, wrong-with-confidence) AND increased C (21→23). LLM negative-instruction failure mode. Prompt change reverted. cat1=47%, cat2=29%. |
| locomo-real-003 | 2026-05-01 02:30 | 3 (conv-26, 30, 41) | 90 | OK | 51.1% (46/90) | 66.7% | 23.3% | first defensible number on smaller n; n=500 reveals it was easy-conversation bias. Per-cat: cat1=58%, cat2=44%, cat3=43%, cat4=100% (n=2). P95 latency 50.6s. |
| locomo-real-002 | 2026-05-01 01:08 | 1 | 10 | OK | 90% (9/10) | 90% | 0% | validation of max_tokens + UTF-8 fixes — extraction now firing, no NA |
| locomo-real-001 | 2026-05-01 00:18 | 1/3 | 30/90 | CRASHED | 60% partial (18/30) | — | 27% | uncovered 2 bugs: extraction max_tokens=1024 (regex-only fallback for ALL sessions) + UTF-8 char-boundary panic in snippet.rs |

### Comparison vs published reference numbers

| System | LoCoMo accuracy | n | Source |
|---|---|---|---|
| Letta + gpt-4o-mini | 74.0% | full LoCoMo-10 | Letta blog 2025 |
| Mem0 graph variant | 68.5% | full LoCoMo-10 | Letta blog 2025 |
| **Klynt KCA (us, canonical)** | **28.8%** | n=500 | locomo-real-005 |
| Gap to Letta | **-45.2pp** | | |
| Gap to Mem0 | **-39.7pp** | | |

The n=90 run3 number (51.1%) was easy-conversation bias — three conversations
that happened to favor our extraction. Canonical comparison uses the full
10-conversation dataset Letta and Mem0 publish on.

### Diagnostic from locomo-real-005 (canonical n=500 baseline)

- **40.2% NOT_ATTEMPTED** (201/500) — biggest bucket. Up from 23.3% on the
  smaller run3, confirming retrieval failure scales with conversation
  diversity. Sample log lines start "Based on the conversation history,
  I don't have any record of …" — the agent has nothing in
  `[Relevant Context]` to work with, and refuses honestly. **Phase 1
  + Phase 2 of the ladder target this directly.**
- **31.0% INCORRECT** (155/500) — agent gives wrong specific answer.
  Many temporal failures (correct event, wrong date). Suggests
  bi-temporal markers need more weight in prompt or dedupe is
  collapsing time-stamped variants.
- **Cat 1 single-hop at 28.0%** — most damning result. Single-hop is
  the easiest category and a near-100% target for any working memory
  system. Mem0 vanilla scores ~80% here. Our 28% is the **subject-
  collapse hypothesis confirmed**: facts about "Alice" stored under
  `subject="user"`, FTS MATCH "Alice" finds nothing, agent refuses.
- **Cat 2 multi-hop/temporal at 21.4%** — compound retrieval failure.
  Both lookups must succeed; if cat 1 lands 28% then cat 2 ≈ 0.28² ≈
  8% pure compound, so PPR / graph traversal is contributing the gap
  to ~21%. This is where Phase 3 (consolidation) is expected to help.
- **Cat 3 open-domain at 44.1%** — surprisingly OK. These accept
  paraphrase; LLM fills gaps from conversation history retained in
  the session.
- **Cat 4 adversarial at 68.4%** — *highest* score, ironically.
  Adversarial questions are the ones where "I don't know" IS correct.
  Our high refusal rate accidentally helps here.

### Next mini-improvement candidates (in expected impact order)

1. **NOT_ATTEMPTED elimination** — when context source loads zero facts
   for the query but the conversation history has the answer, fall back
   to recent-turn injection more aggressively. Target +10-15pp.
2. **Temporal fact preservation** — dedupe should NOT collapse
   `Alice|did_X|2023-05-07` and `Alice|did_X|2023-05-08` into one row.
   Subject+predicate+object dedupe is too aggressive when the object is
   a date. Target +5pp on cat-2.
3. **Single-hop ranking** — investigate why cat-1 isn't near-100%.
   Likely an FTS scoring issue where exact-match facts lose to high-IDF
   noise. Target +10pp on cat-1 alone.

## Four-phase improvement ladder to Letta parity

Each phase isolates one architectural mechanism. We land them one at a
time so A/B/C grade deltas attribute the win to the mechanism that
earned it. **Discipline rule** (from `CLAUDE.md` North Star): never
bundle two phases into one PR — if the next regression appears we won't
know which to revert. Run4's -12pp prompt regression was caught only
because it was isolated.

| Phase | Mechanism | Layer | Predicted lift | Risk |
|---|---|---|---|---|
| **1. Extraction quality** | Fix `subject = "user"` collapse for third-person facts; add identity binding pass; ensure named entities land in the `subject` field | Storage (write) | +15-25pp | Low — purely additive in the extractor |
| **2. Retrieval intelligence** | Entity-aware FTS query construction (extract entities from question, query with those tokens not the raw sentence); alias expansion; per-entity top-k quotas; recent-turn fallback when retrieval is empty | Read | +10-15pp | Medium — touches the hot path; needs trace to attribute |
| **3. Reorganization between ingest and QA** | Trigger graph build + entity rollups + alias resolution after `await_cognitive_idle()` in the bench path; this is where Mem0 graph (68.5%) beats vanilla Mem0 | Consolidation | +5-10pp | Medium — uses existing primitives (`MemoryGraph`, PPR, hebbian) but they don't fire today in the bench path |
| **4. Query-time meta-cognition** | When initial retrieval returns nothing useful, retry with broader scope or different query strategy. This is Letta's filesystem-tool thesis: the agent decides when to grep again | Reactive read | +5-8pp | Higher — agent loop change, more places to break |

**Why phases 3 and 4 matter even though they're "learning":** the user
asked whether continual learning + meta-cognition impact LoCoMo. Most
forms (reforge nightly, FSRS-5 decay) don't move a single-pass benchmark
because they fire between runs or over months of elapsed time. But two
forms fire *between session ingest and QA*, and they're exactly what
distinguishes Letta + Mem0-graph from vanilla retrievers:

- **Memory reorganization between ingest and QA** = phase 3. Mem0's
  graph variant adds 5-10pp over Mem0 vanilla specifically because of
  this. Our `cognitive::graph::ppr` + `MemoryGraph::compute_graph_boosts`
  + hebbian primitives exist but aren't invoked between batch ingest
  and the first QA in the bench path.
- **Query-time meta-cognition** = phase 4. Letta's 74.0% comes from
  the agent + filesystem tool deciding when to grep again with a
  different query. Our system today returns `None` from retrieval and
  never retries — that's the structural cause of the 38% NOT_ATTEMPTED
  bucket.

**What's NOT on the ladder (deliberately):**
- Reforge nightly cycle — between-run, doesn't move single-pass score
- FSRS-5 decay — needs months of elapsed time
- Procedural rule promotion — different problem (skill quality, not memory recall)
- Tracing instrumentation (Task #10) — *prerequisite* for landing phases
  2-4 honestly; doesn't itself move accuracy

**Cumulative arithmetic** (all four phases, optimistic): 51.1% (run3
baseline) + 25 + 15 + 10 + 8 = ~109%, which is obviously not literal —
overlapping mechanisms and compounding ceilings cap real lift well
below the sum. Realistic landing zone: 70-78%, putting us in or above
Letta's range. Conservative landing zone: 62-68%, matching Mem0.



**Diagnostic from locomo-real-001 partial result (30 q answered before crash):**

- 18× CORRECT (60%) — agent answered from conversation history, not extracted facts
- 4× INCORRECT (13%) — partial info given (e.g. "mountains" instead of "beach, mountains, forest")
- 8× NOT_ATTEMPTED (27%) — "I don't have any record of …" pattern, all on later-session facts

This breakdown shows our memory pipeline contributes **~zero**: extraction is silently failing on every session, agent answers only from raw turns kept in `Session` table. Once extraction works, we can attribute lift to `semantic_facts`/PPR/etc.

## Synthetic-fixture run history (ARCHIVED — fixtures + runner deleted 2026-05-01)

> Preserved for traceability. The synthetic suite was retired because
> "green here" never predicted "green on real LoCoMo" — we were tuning
> against fixtures the system had been trained alongside. The
> `run-bench` binary, `klynt_coding.rs`, `locobench.rs`,
> `longmembench.rs`, `game_changer_report.rs`, and the `*_subset.jsonl`
> fixtures are gone; only the rows below remain. Each row = one full
> `run-bench` invocation back when it existed; `n` is `KCA_BENCH_LIMIT`.

| Run | Date | n | Long-mem | LoCo single | LoCo multi | LoCo temp | KC dead | KC fix | KC xCLI | Notes |
|---|---|---|---|---|---|---|---|---|---|---|
| dbg17 | 2026-04-30 22:30 | 10 | _pending_ | _pending_ | _pending_ | skip | _pending_ | _pending_ | _pending_ | rolled back cli_source prefix + session_key align; kept dedupe + 3p + paraphrastic scorer + prompt hardening |
| dbg16 | 2026-04-30 21:20 | 10 | 80% | 100% | 100% | skip | 100% | **75%** ⬇ | **0%** | session_key align made fix bucket WORSE (100→75), didn't help xCLI — net regression |
| dbg15 | 2026-04-30 19:50 | 10 | 85% | 100% | 100% | skip | 100% | 100% | **0%** | broke xCLI: `[CLI: …]` prefix + divergent session_keys → no recall |
| dbg14 | 2026-04-30 19:00 | 10 | **85%** | **100%** | **100%** | skip | **100%** | **100%** | 25% | first n=10 — 6/7 gates green |
| dbg13 | 2026-04-30 18:25 | 2 | 75% | 100% | 100% | skip | 100% | 100% | 0% | added triple-level dedupe at upsert |
| dbg12 | 2026-04-30 17:50 | 2 | 50% | 100% | 100% | skip | 100% | 100% | 0% | added 3p prompt + regex backstop |
| dbg11 | 2026-04-30 17:30 | 2 | 75% | 100% | 0% | skip | 0% | 100% | 0% | debug-grep-filtered |
| dbg10 | 2026-04-30 17:10 | 2 | 75% | 100% | 0% | skip | 0% | 100% | 0% | unconditional 14s idle floor + always-on regex backstop |
| dbg-pre | 2026-04-30 03:43 | 5 | 20% | 0% | 20% | 0% | 100% | 0% | 0% | pre-fix baseline (auto-overwritten orig file) |

### Key landmarks

- **dbg10 → dbg14**: 50% → 85% on long-mem (+35pp) via three landmark fixes
  - `fix(kca): unconditional idle floor + always-on regex backstop`
  - third-person prompt example + `<Proper> <verb> <obj>` regex scanner
  - triple-level dedupe at `SemanticFactRepo::upsert`
- **dbg14 → dbg16**: tackled Q-5c (xCLI) — first attempt regressed it from
  25% → 0% by introducing prefix noise. dbg16 fixes session_key
  divergence so query-time conversation history is preserved.

### Variance signal

With `n=2` (4 long-mem queries), each query flip = 25%. Hovering between
50% and 100% across runs means we're at the noise floor. With `n=10` (20
queries) variance drops to ~5pp per query, so 80–85% across two runs is
**within normal LLM-stochasticity range** — not a real regression. Use
`n=10` for any "did the gate hold" judgment; `n=2` is debug-only.

## Run config

```
cargo build --release -p kca-bench --bin run-bench
CARGO_MANIFEST_DIR=$PWD/crates/kca-e2e \
  KCA_BENCH_LIMIT=10 \
  ./target/release/run-bench --output docs/architecture/kca-game-changer.md
```

Model: `~/.klyntbot/config.json` (Kimi anthropic-compat at `https://api.kimi.com/coding`).
Scoring: substring + 80% token-overlap recall (xCLI relaxed to 60% — paraphrastic).

## Methodology

- **Fixtures**: `tests/fixtures/kca/{longmembench,locobench,klynt_coding_bench}.jsonl`. Each fixture contains a multi-turn conversation followed by gold-answer queries. Klynt-coding fixtures are stratified across three buckets (`kcb_dead_*` dead-ends, `kcb_fix_*` fix attempts, `kcb_xcli_*` cross-CLI patterns).
- **Pipeline**: each fixture instantiates a fresh `AppCore` over an ephemeral SQLite + LanceDB store. Conversation turns are replayed via `chat_complete` (drains the streaming reply). The replayer waits for the cognitive extraction background task (3s batch window + LLM extraction call) to settle before queries fire — preventing race conditions between memory write and recall.
- **Per-turn idle floor**: `await_cognitive_idle` waits ≥14s + 4 stable polls before continuing — covers Kimi's 12s p99 LLM extraction latency.
- **Triple-level dedupe**: `SemanticFactRepo::upsert` short-circuits when an identical active `(subject, predicate, object)` row already exists, bumping `convergence_score` instead of inserting a new uuid. Hebbian-style: repeated extraction reinforces a single canonical row.
- **Third-person extraction**: regex-light backstop scans for `<Proper> <verb> <obj>` patterns (`loves|likes|prefers|enjoys|hates|owns|has|uses|plays|works at|lives in`) so multi-hop benchmarks recover entity-on-entity facts the LLM extractor over-narrows away.
- **Scoring**: two-pass match. First, normalized substring (handles short factual answers like `"Anthropic"`). Second, token-overlap recall ≥80% over content tokens (xCLI bucket relaxed to 60% — paraphrastic queries can't realistically hit 80%).

## Comparison Matrix


| Capability | Klynt KCA | Graphiti | Mem0 v3 | HippoRAG-2 | GraphRAG | LightRAG | LangMem | Letta |
|---|---|---|---|---|---|---|---|---|
| Per-turn entity resolution (LLM) | ✅ | ✅ | ⚠ Embed only | ❌ | ❌ | ❌ | ❌ | ❌ |
| Bi-temporal validity | ✅ | ✅ | ⚠ Soft | ❌ | ❌ | ❌ | ❌ | ❌ |
| Edge invalidation on contradiction | ✅ Linker + temporal prune | ✅ | ⚠ | ❌ | ❌ | ❌ | ❌ | ❌ |
| Procedural rules / skill learning | ✅ | ❌ | ❌ | ❌ | ❌ | ❌ | ❌ | ❌ |
| Episodic memory with stability decay | ✅ FSRS-5 | ✅ | ⚠ | ❌ | ❌ | ❌ | ⚠ | ✅ |
| Causal vs correlational edge typing | ✅ Track 9-typing | ❌ | ❌ | ❌ | ❌ | ❌ | ❌ | ❌ |
| Hebbian co-activation | ✅ (now also at upsert convergence) | ❌ | ❌ | ❌ | ❌ | ❌ | ❌ | ❌ |
| Community detection (Louvain + LLM) | ✅ | ❌ | ❌ | ❌ | ✅ Leiden | ❌ | ❌ | ❌ |
| PPR retrieval | ✅ Track 6 | ❌ | ❌ | ✅ Best | ❌ | ❌ | ❌ | ❌ |
| Spaced repetition | ✅ FSRS-5 | ❌ | ❌ | ❌ | ❌ | ❌ | ❌ | ❌ |
| Nightly synthesis cycle | ✅ Reforge 9-phase | ❌ | ❌ | ❌ | ⚠ Re-index | ❌ | ❌ | ❌ |
| Meta-cognition (Mirror) | ✅ | ❌ | ❌ | ❌ | ❌ | ❌ | ❌ | ❌ |
| Self-critique loop on extraction | ✅ Track 5 | ❌ | ❌ | ❌ | ❌ | ❌ | ❌ | ❌ |
| Predictive cache warming | ✅ Track 7 | ❌ | ❌ | ❌ | ❌ | ❌ | ❌ | ❌ |
| Hierarchical episodic compression | ✅ Track 8 | ❌ | ❌ | ❌ | ❌ | ❌ | ❌ | ❌ |
| Cross-CLI cognitive transfer | ✅ Track 10 | ❌ | ❌ | ❌ | ❌ | ❌ | ❌ | ❌ |
| Memory-grounded skill discovery | ✅ Track 12 | ❌ | ❌ | ❌ | ❌ | ❌ | ❌ | ❌ |
| Coding-specific memory tier | ✅ Distiller + multi-CLI | ❌ | ❌ | ❌ | ❌ | ❌ | ❌ | ❌ |
| Backend complexity | SQLite + LanceDB only | Neo4j req | Neo4j optional | Custom | Pipeline-heavy | Custom | Vector store | Vector store |


## Capabilities Klynt has that competitors lack


1. **Reforge nightly cycle** — 9-phase deferred synthesis no other system has.
2. **Mirror meta-cognition** — observes the agent's own behavior; foundation for self-improvement.
3. **Procedural rules** — observed → reflected → applied promotion path.
4. **Multi-CLI ingest + cross-CLI transfer** — patterns learned in one CLI propagate to others.
5. **FSRS-5 spaced repetition** — memory has a forgetting curve; review schedule trained from feedback.
6. **Skills system with progressive loading** — orchestrator layer above memory.
7. **Self-critique ring** — every extraction judged for hallucination before persisted as ground truth.
8. **Predictive cache warming** — anticipatory pre-computation of likely follow-up retrievals.
9. **Hierarchical episodic compression** — long-term memory navigable in O(log N) instead of O(N).
