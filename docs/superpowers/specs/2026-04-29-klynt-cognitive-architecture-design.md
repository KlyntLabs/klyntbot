# Klynt Cognitive Architecture (KCA) — Design Spec

**Date:** 2026-04-29
**Status:** Draft for implementation
**Predecessor:** [`2026-04-28-memory-gaps-comprehensive.md`](../plans/2026-04-28-memory-gaps-comprehensive.md) (closes 20 verified gaps; merged in `feat/coding-memory-phase7-debt`)

---

## 1. Goal (one sentence)

Promote Klynt's memory subsystem from "rich substrate, mostly nightly synthesis" to a **3-tier cognitive loop** (hot per-turn, warm per-session, cold nightly) with a parallel critic ring, so that every chat turn produces graph-linked, contradiction-checked, retrieval-ready memory at a per-turn cost under one Sonnet call.

## 2. Why now

- The gaps plan closed all "broken-feature" issues (no dead-code, no missing wiring, no schema mismatch). The substrate is structurally sound.
- Cheap-token economics (Kimi K2 ~$0.15/1M input, DeepSeek V3.2 ~$0.28/1M, Haiku 4.5 $0.80/1M) re-prices the design space — multi-call architectures now cost the same as single-call ones did 18 months ago.
- Pre-release timing: schema is freely changeable; we will not need to migrate users.

## 3. Non-goals

- We will **not** add an OpenTelemetry stack, Prometheus, or external metrics dashboards. Existing `tracing` + Mirror tables + `pipeline_event_log` cover observability.
- We will **not** introduce a graph database (Neo4j, FalkorDB). SQLite + LanceDB remain authoritative.
- We will **not** rewrite Reforge — only extend it with the new "warm-path" sister cycle and additional handlers.
- We will **not** ship Letta-style agent-controlled memory tools. The agent does not get write access to the memory layer; only the cognitive pipeline writes.
- We will **not** redesign FSRS, decay, the autotuner, or the Mirror engine. They stay as-is.

## 4. Architecture overview

```
┌──────────────────────────────────────────────────────────────────────┐
│  TIER 3 — Cold path (nightly Reforge, 03:00 idle)                   │
│  9 phases, 5+ Sonnet/DeepSeek-R1 calls. EXISTS.                     │
│  KCA additions: Track 10 (cross-CLI transfer), Track 12 (skill     │
│  discovery synthesizer)                                              │
├──────────────────────────────────────────────────────────────────────┤
│  TIER 2 — Warm path (per-session-end, 1-30 min after last turn)     │
│  3-5 Haiku/Kimi calls, ~$0.005/session.                              │
│  KCA additions: Track 4 (micro-Reforge promotion),                   │
│  Track 11 (online community membership)                              │
├──────────────────────────────────────────────────────────────────────┤
│  TIER 1 — Hot path (per-turn, fire-and-forget after response)       │
│  2-3 Haiku/Kimi calls, ~$0.002/turn.                                 │
│  KCA additions: Track 1 (graph-grounded extraction prefetch),        │
│  Track 2 (per-turn graph linker), Track 3 (coding parity),           │
│  Track 9-typing (edge typing), Track 13 (temporal pruning at read)   │
├──────────────────────────────────────────────────────────────────────┤
│  PARALLEL — Critic ring + retrieval intelligence                     │
│  Track 5 (self-critique on extraction)                               │
│  Track 6 (PPR retrieval expansion, $0)                               │
│  Track 7 (predictive cache warming)                                  │
│  Track 8 (hierarchical episodic compression)                         │
└──────────────────────────────────────────────────────────────────────┘
```

## 5. Track summary (12 remaining)

| # | Track | Tier | Phase | LLM cost / turn | Code touch points |
|---|---|---|---|---|---|
| 1 | Graph-grounded extraction prefetch | Hot | A | $0 (existing call) | `cognitive::services::background`, `cognitive::repos::semantic_fact`, `cognitive::repos::entity` |
| 2 | Per-turn graph linker | Hot | A | ~$0.0004 (gated) | `cognitive::services::graph_linker` (new), `agent::adapters::cognitive_handlers` |
| 3 | Coding facts → graph parity | Hot | A | $0 (reuses Track 2) | `coding-memory::distiller::writer`, `coding-memory::distiller::reconcile` |
| 9-typing | Causal vs correlational edge typing | Hot | A | $0 (added to Track 2) | `cognitive::repos::entity_relationship`, migration |
| 4 | Online procedural rule promotion (micro-Reforge) | Warm | B | ~$0.005/10 turns | `cognitive::services::micro_reforge` (new), `app-core::init::cron` |
| 5 | Self-critique loop on extraction | Hot | B | ~$0.0005 | `cognitive::services::extraction_critic` (new), `cognitive::repos::extraction_critic_log` |
| 11 | Online community membership | Warm | B | ~$0.001/session | `cognitive::services::community_membership_online` (new) |
| 6 | PPR-based retrieval expansion | Read | C | $0 | `cognitive::services::ppr_retrieval` (new), uses `petgraph` |
| 7 | Predictive cache warming | Read | C | ~$0.0003 | `cognitive::services::predictive_cache` (new), `agent::agent_runtime` |
| 8 | Hierarchical episodic compression | Cold/Warm | C | ~$0.001/hour | `cognitive::services::hierarchical_compressor` (new), cron |
| 13 | Temporal pruning at retrieval | Read | C | ~$0.0003 | `cognitive::services::temporal_pruner` (new), `agent::agent_runtime` |
| 10 | Cross-CLI cognitive transfer | Cold | D | within Reforge budget | `coding-memory::reforge::cross_cli_synthesis` (new) |
| 12 | Memory-grounded skill discovery | Cold | D | within Reforge budget | `cognitive::services::reforge::skill_discovery` (new) |

## 6. Phasing (matches plan files)

| Phase | Plan file | Tracks | Sequence rationale |
|---|---|---|---|
| **A — Online Graph Integrity** | `2026-04-29-kca-phase-a-online-graph-integrity.md` | 1, 2, 3, 9-typing | Foundational: every later phase reads typed edges and the warm graph. |
| **B — Continuous Learning** | `2026-04-29-kca-phase-b-continuous-learning.md` | 4, 5, 11 | Depends on A's typed edges + write paths. |
| **C — Retrieval Intelligence** | `2026-04-29-kca-phase-c-retrieval-intelligence.md` | 6, 7, 8, 13 | Read-side improvements; mostly orthogonal but benefit from B's stable graph. |
| **D — The Moat** | `2026-04-29-kca-phase-d-the-moat.md` | 10, 12 | Reforge-only changes; lowest risk, deepest competitive advantage. |
| **E — Testing & Benchmarks** | `2026-04-29-kca-testing-and-benchmarks.md` | All | Cross-phase A→Z verification + LongMemEval/LoCoMo + Klynt-specific bench. |

Each phase plan contains its own per-phase integration test section. Phase E adds the **whole-system** suite that proves no past-deployment misalignment recurs.

## 7. Success criteria (measurable)

These are the gates Phase E must verify. Failing any one blocks release.

### 7.1 Functional gates

| ID | Gate | Verification |
|---|---|---|
| F-1 | Every chat turn that extracts ≥1 fact also writes ≥1 entity-relationship row (when graph context exists) | E2E test: 100 fixture turns → assert `entity_relationships` row count ≥ extracted-fact count × 0.6 |
| F-2 | Coding turns write entity-relationship rows at parity with chat turns | E2E test: 100 coding fixture turns → same gate |
| F-3 | Per-turn graph linker LLM call gating skips ≥30% of turns (cold-start guard) | Telemetry counter over 1000-turn replay; assert skip rate ≥ 0.30 |
| F-4 | Self-critique catches injected hallucinations | Synthetic fixture with planted hallucinations; assert critic auto-supersedes ≥ 95% |
| F-5 | PPR retrieval finds multi-hop facts that flat embedding misses | Curated multi-hop benchmark; PPR recall @10 ≥ flat recall @10 + 0.10 |
| F-6 | Online community membership keeps ≥80% of new facts within 24h of clustering | Compare with nightly-only baseline over 7-day replay |
| F-7 | Cross-CLI transfer detects ≥1 transferable pattern per CLI pair on 30-day fixture | Reforge dry-run on multi-CLI fixture |
| F-8 | Skill discovery proposes ≥1 candidate skill per 7-day session window with ≥3 procedural rules | Reforge dry-run |

### 7.2 Performance gates

| ID | Gate | Threshold |
|---|---|---|
| P-1 | Per-turn hot-path memory write latency P95 (excluding extraction LLM) | ≤ 400ms |
| P-2 | Per-turn graph linker LLM call P95 (when fired) | ≤ 800ms (Haiku/Kimi) |
| P-3 | Total hot-path cost per turn (Tier 1 only) | ≤ $0.003 (Haiku) or ≤ $0.0008 (Kimi K2) |
| P-4 | Per-session warm path cost | ≤ $0.01 |
| P-5 | Reforge nightly cost (10K turns/month user) | ≤ $1.00 |
| P-6 | Read-time retrieval P95 (with PPR + temporal prune + cache warm) | ≤ 250ms |
| P-7 | Cache warm hit rate after 10 turns | ≥ 30% |

### 7.3 Quality gates

| ID | Gate | Threshold |
|---|---|---|
| Q-1 | LongMemEval accuracy (Klynt vs flat-vector baseline) | +15% absolute |
| Q-2 | LoCoMo single-hop accuracy | ≥ 92% |
| Q-3 | LoCoMo multi-hop accuracy | ≥ 70% |
| Q-4 | LoCoMo temporal accuracy | ≥ 85% |
| Q-5 | Custom Klynt-coding benchmark (dead-end retrieval, fix-attempt recall, multi-CLI transfer) | ≥ 80% on each axis |
| Q-6 | Hallucinated-fact rate (% of facts whose subject/object isn't grounded in turn content) | ≤ 1% (down from baseline) |
| Q-7 | Stale-fact recall rate (% of retrieved facts where `valid_until` < now) | ≤ 0.5% |

### 7.4 Stability gates

| ID | Gate | Threshold |
|---|---|---|
| S-1 | Zero new clippy warnings | `cargo clippy --workspace --all-targets --all-features -- -D warnings` clean |
| S-2 | All existing tests pass | `cargo nextest run --workspace` clean |
| S-3 | Property tests pass for 64 cases each (Inv 7 cross-CLI, no-DELETE invariant, dual-write parity) | `cargo nextest run -E 'test(/prop_/)'` clean |
| S-4 | No new `Arc<RwLock>` patterns introduced (StoragePool wraps SqlitePool directly) | code review checklist |
| S-5 | All public `AppCore` handler methods are `#[tracing::instrument]` | grep audit |

## 8. Tech choices

| Concern | Choice | Rationale |
|---|---|---|
| Hot-path LLM | Haiku 4.5 (default), Kimi K2 (config override) | Sub-second latency, cheap, good JSON adherence |
| Warm-path LLM | Haiku 4.5 batched | Same |
| Reforge LLM | Sonnet 4.6 / DeepSeek R1 (config) | Reasoning quality matters for synthesis |
| Embedding | Existing `EmbeddingProvider` trait | Already wired to `MemoryRetriever` and Lance |
| PPR algorithm | `petgraph`'s personalized PageRank or hand-rolled (we already use petgraph for Louvain) | Already a workspace dep |
| Cache | In-memory LRU keyed on session_key + query hash; 5min TTL | Bounded memory, deterministic eviction |
| Schema migration | Pre-release rule (CLAUDE.md): in-place edits to `FeatureMigration` SQL allowed | Saves migration churn; gates flip after first release |

## 9. Critical guardrails (carried forward from previous deployments)

These prevent the "broken-feature, lack-of-alignment" problems the user called out. Every phase plan must respect them.

1. **No `--no-verify`, no clippy-bypass, no `#[allow(dead_code)]` without a tracking issue.**
2. **All new tools are `#[derive(Tool)]` from `tools-core-macros`** — never raw `#[tauri::command]`.
3. **All new public `AppCore` handler methods carry `#[tracing::instrument(skip(self), err)]`.**
4. **All new MCP-exposed tools added to `default_exposed_tools()` AND have at least one integration test that calls them via `ToolRegistryBridge`.**
5. **All new domain events added to `bus::DomainEvent` AND listed in `docs/architecture/domain-event-subscribers.md`** (created by gaps-plan F9).
6. **All new SQL tables follow the bi-temporal pattern** (`valid_from` / `valid_until` / `superseded_by`) where time-travel matters.
7. **All new repos use `&StoragePool` (clone, no `Arc<RwLock>`).**
8. **Dependency direction stays upward** (L0→L8, no cycles, no L4 importing L5+).
9. **Every new LLM call goes through `DynProvider`** — never a hardcoded HTTP client.
10. **All new prompts live as `const &str` in a `prompts.rs` file in the calling crate** — never inline; this keeps prompt audit easy.

## 10. Risk register

| Risk | Severity | Mitigation |
|---|---|---|
| Track 2 LLM call adds perceived latency | Medium | Fire-and-forget via tokio::spawn AFTER response stream completes; user never blocks on it. |
| Track 4 fires too often, double-promoting rules | High | Dedup gate: skip if any rule with same `(domain, rule_text_hash)` exists. Dry-run mode for first 7 days. |
| Track 5 critic is itself wrong → false positives mark good facts as hallucinated | High | Critic verdicts demote stability (×0.5) instead of hard supersede. Reforge re-evaluates. |
| Track 6 PPR cost on dense graphs | Medium | Bound walk to top-100 entities by degree; cap iterations at 30. |
| Track 7 cache warming wastes spend on miss-prone queries | Low | Hit-rate < 20% over rolling 100 queries → auto-disable for 24h. |
| Track 8 hierarchical compression loses fidelity | Medium | Always retain raw episodic for 30 days; hierarchy is additive. |
| Track 10 cross-CLI transfer leaks personal patterns to wrong context | High | Scope guard: only transfer rules with `scope_repo_id IS NULL` AND `confidence > 0.85`. |
| Track 12 skill discovery proposes nonsense skills | Medium | All proposals require user approval (Mirror UI flow exists). Auto-applied = false. |
| Cross-track integration regressions | High | **Phase E** runs full A→Z suite + benchmarks before any phase merges to main. |

## 11. Testing & benchmark approach (summary; full plan in Phase E)

### 11.1 Per-task TDD (in every phase plan)

Every implementation step has a failing test written first, then a minimal-pass implementation, then a commit. No implementation without a test. No exceptions.

### 11.2 Per-phase integration tests

Each phase plan ends with a section that wires its tracks together against fixture conversations and asserts the F-* and P-* gates relevant to that phase.

### 11.3 Phase E whole-system tests

- **E2E pipeline test:** 1000-turn fixture replay that exercises every track end-to-end, asserts graph density growth, asserts no fact written without entity edges, asserts critic + temporal-prune outputs.
- **Multi-CLI parity test:** Same fixture replayed across all 4 ingest sources; asserts cross-CLI transfer detects shared patterns.
- **Soak test:** 10K-turn fire-hose replay; asserts stable memory growth, no leaks, no clippy regressions.
- **Migration safety test:** Run all migrations in fresh + populated test pools; assert idempotence.

### 11.4 Phase E benchmarks

- **LongMemEval** (vs. flat-vector baseline)
- **LoCoMo** (vs. published Mem0/Zep numbers)
- **Klynt-coding bench** (custom: 50 dead-end scenarios, 50 fix-attempt recall, 30 multi-CLI transfer pairs)
- **Latency dashboard** (P50/P95/P99 per tier, per track)
- **Cost dashboard** (per turn / per session / per night)
- **Game-changer report** (auto-generated markdown comparing Klynt vs Graphiti / Mem0 / HippoRAG / GraphRAG / LightRAG / LangMem / Letta on each capability axis)

## 12. Glossary

- **Hot path / Tier 1** — work that runs in `AgentRuntime::process_message` Phase 3 (after-response), fire-and-forget per turn.
- **Warm path / Tier 2** — work that runs at session-end (`SessionEnded` event) or every N turns (micro-Reforge timer).
- **Cold path / Tier 3** — Reforge nightly cycle at 03:00 idle.
- **Critic ring** — parallel async validators that judge memory writes/reads; verdicts go to Mirror tables.
- **PPR** — Personalized PageRank, used as graph traversal for retrieval expansion (HippoRAG-style).
- **Micro-Reforge** — Track 4's mid-session synthesis trigger; not the full nightly cycle.
- **KCA** — Klynt Cognitive Architecture; the umbrella term for this design.

---

## 13. Cross-references

- Predecessor (closed gaps): [`plans/2026-04-28-memory-gaps-comprehensive.md`](../plans/2026-04-28-memory-gaps-comprehensive.md)
- Computer-use design (Track 14 multipart was prerequisite): [`specs/2026-04-28-computer-use-and-procedural-memory-design.md`](2026-04-28-computer-use-and-procedural-memory-design.md)
- Original coding-memory design: [`specs/2026-04-22-coding-memory-design.md`](2026-04-22-coding-memory-design.md)
- Domain event subscriber registry: `docs/architecture/domain-event-subscribers.md` (created by gaps-plan F9)

The phase plans implement this spec in order. Read them sequentially.
