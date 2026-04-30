# KCA Game-Changer Report

Generated: 2026-04-30T04:45:27.787134Z

*Run config: `KCA_BENCH_LIMIT=5`, model from `~/.klyntbot/config.json`, scoring = substring + 80% token-overlap recall.*

## Quality

| Benchmark | Accuracy | Sample size |
|---|---|---|
| Long-memory | **20.0%** | 2/10 queries correct |
| LoCoBench single-hop | **0.0%** | — |
| LoCoBench multi-hop | **0.0%** | — |
| LoCoBench temporal | **0.0%** | — |
| Klynt-coding dead-end | **50.0%** | 1/2 |
| Klynt-coding fix-attempt | **50.0%** | 1/2 |
| Klynt-coding multi-CLI | **0.0%** | 0/2 |

### Long-memory by hop type

| Hop | Correct/Total | Accuracy |
|---|---|---|
| single | 2/10 | 20.0% |

## Performance

- Hot-path P50 turn latency: **2773ms**
- Hot-path P95 turn latency: **8564ms**
- Long-mem query P95: **8564ms** (real cloud LLM round-trip)

## Methodology


- **Fixtures**: `tests/fixtures/kca/{longmembench,locobench,klynt_coding_bench}.jsonl`. Each fixture contains a multi-turn conversation followed by gold-answer queries. Klynt-coding fixtures are stratified across three buckets (`kcb_dead_*` dead-ends, `kcb_fix_*` fix attempts, `kcb_xcli_*` cross-CLI patterns).
- **Pipeline**: each fixture instantiates a fresh `AppCore` over an ephemeral SQLite + LanceDB store. Conversation turns are replayed via `chat_complete` (drains the streaming reply). The replayer then waits for the cognitive extraction background task (3s batch window + LLM extraction call) to settle before queries fire — preventing race conditions between memory write and recall.
- **Scoring**: two-pass match. First, normalized substring (handles short factual answers like `"Anthropic"`). Second, token-overlap recall ≥80% over content tokens (handles narrative replies where the LLM phrases the answer in a sentence).
- **Memory layers exercised end-to-end**: FTS5 BM25 over semantic_facts + episodic_memories + procedural_rules; vector recall via Lance; PPR multi-hop expansion; conversation recall service. Reforge nightly cycle is *not* triggered during the bench (it requires real time progression).
- **Latencies** include the full agent loop: system prompt assembly, recall, LLM call, response streaming. Cloud LLM round-trip dominates wall time.


## Comparison Matrix


| Capability | Klynt KCA | Graphiti | Mem0 v3 | HippoRAG-2 | GraphRAG | LightRAG | LangMem | Letta |
|---|---|---|---|---|---|---|---|---|
| Per-turn entity resolution (LLM) | ✅ | ✅ | ⚠ Embed only | ❌ | ❌ | ❌ | ❌ | ❌ |
| Bi-temporal validity | ✅ | ✅ | ⚠ Soft | ❌ | ❌ | ❌ | ❌ | ❌ |
| Edge invalidation on contradiction | ✅ Linker + temporal prune | ✅ | ⚠ | ❌ | ❌ | ❌ | ❌ | ❌ |
| Procedural rules / skill learning | ✅ | ❌ | ❌ | ❌ | ❌ | ❌ | ❌ | ❌ |
| Episodic memory with stability decay | ✅ FSRS-5 | ✅ | ⚠ | ❌ | ❌ | ❌ | ⚠ | ✅ |
| Causal vs correlational edge typing | ✅ Track 9-typing | ❌ | ❌ | ❌ | ❌ | ❌ | ❌ | ❌ |
| Hebbian co-activation | ✅ | ❌ | ❌ | ❌ | ❌ | ❌ | ❌ | ❌ |
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
