# KCA Real Benchmarks + End-to-End Tracing

**Date:** 2026-04-30
**Owner:** memory-system
**Status:** drafted, awaiting approval

## Why this plan exists

The previous KCA benchmark suite is a synthetic toy
(`tests/fixtures/kca/longmembench_subset.jsonl` etc — see the README disclosure
"Klynt-curated, modeled after"). Tuning that suite to 85% does NOT prove we
beat Mem0 / Letta / Graphiti — they publish numbers on the **real** LoCoMo and
LongMemEval datasets:

- Letta + gpt-4o-mini: **74.0%** on real LoCoMo
- Mem0 graph variant: **68.5%** on real LoCoMo
- Source: <https://www.letta.com/blog/benchmarking-ai-agent-memory>
  with eval at `github.com/letta-ai/letta-leaderboard`,
  file `leaderboard/locomo/locomo_benchmark.py`

To honestly claim "game-changer," we score on the **same eval Letta runs**
and beat 74%. Anything else is marketing.

## Non-goals

- Tuning scoring thresholds, fixture rewrites, or any other
  "make-the-bench-pass" tactic. We **revert** the paraphrastic xCLI scorer
  (already done) and refuse to add similar accommodations.
- Claiming a Mem0/Letta-comparable result on `klynt_smoke_*` (our internal
  suite). Those become regression-only, not gates.

## What "game-changer" means in this plan

Pass thresholds across 5 categories simultaneously, each scored on a
**publicly-reproducible** eval where possible, and a documented honest
methodology where no public eval exists.

| # | Category | Target | Eval source |
|---|---|---|---|
| 1 | Factual accuracy | LoCoMo ≥ 75%, LongMemEval ≥ 60% | letta-leaderboard, HF `xiaowu0162/LongMemEval` |
| 2 | Coding-specific | dead-end ≥ 80%, fix ≥ 80%, xCLI ≥ 60% | Klynt-built (real GitHub-issue replays, not synthetic) |
| 3 | Continual learning | bi-temporal update ≥ 90%, contradiction ≥ 85%, FSRS-5 forgetting curve within 10% of theoretical | Klynt-built; methodology documented |
| 4 | Cost / scalability | LoCoMo p95 ≤ 5s, store growth sub-linear in turn count, dedupe ratio ≥ 0.6 | Instrumented in-process |
| 5 | Skill / meta-cog | rule-application precision ≥ 70%, mirror miss-detection recall ≥ 60% | Klynt-built research metrics |

## Scope (what changes in the codebase)

### New crates / modules

```
crates/kca-bench/src/
  locomo_real.rs        # NEW — pulls letta-leaderboard locomo, scores per their spec
  longmemeval_real.rs   # NEW — pulls HF dataset, scores per upstream
  continual_learning.rs # NEW — bi-temporal + contradiction + FSRS-5 evals
  scalability.rs        # NEW — token / latency / store-growth dashboards
  skills_metacog.rs     # NEW — rule + mirror metrics

crates/kca-trace/        # NEW CRATE
  src/lib.rs             # span taxonomy, trace_id propagation, JSON sink
```

### Renames / removals

- `longmembench_subset.jsonl` → `klynt_smoke_factual.jsonl` (regression-only)
- `locobench_subset.jsonl` → `klynt_smoke_multihop.jsonl` (regression-only)
- `klynt_coding_bench.jsonl` (xCLI synthesizes) → keep `kcb_dead_*` and
  `kcb_fix_*` as smoke; **rebuild xCLI from real multi-CLI session captures**
- Gate enforcement in `run_bench.rs`: replace `enforce_gates` thresholds to
  read from real benchmarks only; smoke suites print warnings, don't fail.

### Reverted / removed

- ✅ `klynt_coding.rs::paraphrastic_match` — already reverted in this session
- The Q-1 prompt hardening in `context_source.rs` (`CRITICAL — never claim
  ignorance`) — keep, it's defensive not eval-tuning.
- The triple-level dedupe in `semantic_fact.rs::upsert` — keep, this is a
  real architectural improvement.
- Third-person regex backstop + prompt — keep, fixes a real extraction gap.

## End-to-end tracing taxonomy

Every memory-touching code path emits a `tracing::span!` with a stable name +
fields. A custom layer in `crates/kca-trace/` collects these into a
**per-bench-query JSON tree** so we can see exactly where each lost
percentage point comes from.

### Span hierarchy (all under a root `bench.query` span)

```
bench.query                         {fixture_id, query_id, gold, predicted, correct}
├── agent.process                   {session_key, intent_summary, mode}
│   ├── context.build_prompt        {sources, tokens_total}
│   │   ├── context.cognitive       {facts_loaded, facts_after_dedup, tokens}
│   │   │   ├── recall.fts          {hits, scores}
│   │   │   ├── recall.vector       {hits, scores, latency_ms}
│   │   │   └── recall.ppr          {seeds, expanded_nodes, latency_ms}
│   │   ├── context.episodic        {episodes, tokens}
│   │   ├── context.procedural      {rules_loaded, rules_active}
│   │   └── context.recall_registry {ranked_domains}
│   ├── llm.call                    {model, prompt_tokens, response_tokens, latency_ms}
│   └── llm.stream                  {chunks, total_chars}
└── ingest.chat_turn                {extracted_facts, persisted, deduped, mirrored}
    ├── extract.llm                 {observation_count, facts, latency_ms}
    ├── extract.regex_1p            {hits}
    ├── extract.regex_3p            {hits}
    ├── upsert.semantic_fact        {triple, dedup_hit (bool), convergence_delta}
    ├── ingest.identity_bind        {name, mirrored_count}
    └── ingest.episodic             {salience, persisted}
```

### Required fields on every span

- `fixture_id` (string) — propagated via `tracing` baggage / parent context
- `query_id` (string)
- `latency_us` (u64) — auto-recorded via span lifetime
- `result_size` (usize, where applicable)

### Sink

`crates/kca-trace/src/lib.rs` registers a `Layer` that:
1. Buffers spans per `query_id`.
2. On root span close, emits one JSON object to `target/kca-traces/<run>/<query_id>.json`.
3. After bench completes, runs a roll-up: per-stage hit/miss counts, latency
   histograms, dedup-hit ratio, fact-count distributions.

### Diagnostic value

When real LoCoMo gives us, say, 38%, the trace tree tells us:
- Of 62% misses, how many had the right fact in `recall.fts`? (retrieval
  upstream is fine, ranking is wrong)
- How many had it in `extract.llm` but missed `upsert`? (ingestion bug)
- How many never saw the source observation in `extract.*`? (extraction
  prompt insufficient)
- p95 of `llm.call` vs `recall.*` — where is latency budget burnt?

That tells us **which mini-percent improvement to chase next**.

## Sequencing (4 weeks calendar, ~3 working days each)

### Week 1 — Real LoCoMo + tracing skeleton

- [ ] Day 1: create `crates/kca-trace`, define span schema, JSON sink,
      wire into `run-bench` binary.
- [ ] Day 2: instrument `cognitive::consumers::ingestion`,
      `cognitive::services::extraction`, `cognitive::repos::semantic_fact`
      with the spans above.
- [ ] Day 3: instrument `agent::execution::*`, `context_engine`,
      `cognitive::services::context_source`. Verify a single fixture
      produces a complete tree.
- [ ] Day 4: write `kca-bench/src/locomo_real.rs`:
      - Fetch `letta-ai/letta-leaderboard` locomo dataset (script in their repo)
      - Adapt their question/answer schema to our `ConversationFixture`
      - Apply their exact scoring (LLM-judge or string-match per their `locomo_benchmark.py`)
- [ ] Day 5: run, record baseline. Likely 30-50%. Document in
      `docs/architecture/kca-game-changer.md` under **"Real benchmarks (n=N)"** section.

### Week 2 — LongMemEval + smoke rename

- [ ] Day 1: `kca-bench/src/longmemeval_real.rs` — HF dataset loader,
      5-ability split.
- [ ] Day 2: run, record baseline.
- [ ] Day 3: rename `longmembench_subset.jsonl` →
      `klynt_smoke_factual.jsonl`; update bench wiring.
- [ ] Day 4: update `run_bench.rs::enforce_gates` to read real-bench scores
      only; smoke suites become non-blocking reports.
- [ ] Day 5: review traces from real-LoCoMo failures — identify top-3 root
      causes of missed answers (e.g., "extraction skipped 40% of multi-fact
      sentences"). Pick one to fix in week 3.

### Week 3 — Continual learning + scalability

- [ ] Day 1-2: build `continual_learning.rs` evals
- [ ] Day 3-4: build `scalability.rs` — replay 10K turns,
      record store growth, dedupe ratio, latency histogram.
- [ ] Day 5: investigate top-3 root cause from W2 day 5; ship one
      mini-improvement (target +2-5pp on real LoCoMo).

### Week 4 — Skill / meta-cog + iteration

- [ ] Day 1-2: build `skills_metacog.rs` evals
- [ ] Day 3-5: data-driven iteration based on traces — chase
      mini-percentage-points based on per-stage hit-rate gaps. **This is
      where the trace investment pays off.**

## Acceptance criteria

We ship a `docs/architecture/kca-game-changer.md` with a table of:
- Our score on real LoCoMo, LongMemEval, our continual-learning evals,
  scalability, skill metrics.
- Letta's published score for direct comparison.
- A trace-summary appendix showing per-stage hit rates so any reader can
  validate the methodology.

If we beat 74% on real LoCoMo, the "game-changer" claim is honest. If not,
we publish the actual number, the trace breakdown, and the
mini-improvements we're chasing.

## Risks / unknowns

- **`letta-leaderboard` repo may have non-trivial agent harness**
  (it expects an API; adapting to in-process AppCore needs a shim).
  Mitigation: write a thin HTTP wrapper around `AppCore::chat_complete`
  matching their expected interface.
- **HF LongMemEval download may be large** (~100K-token contexts × 500 qs).
  Mitigation: stream + cache locally.
- **Trace volume may swamp logs.** Mitigation: separate `kca-trace`
  layer that only fires when `KCA_TRACE_PATH` env is set.
- **First real-LoCoMo run may take hours** (LLM-grading at the back is
  slow). Mitigation: parallelize per-conversation, cache LLM judgments.
