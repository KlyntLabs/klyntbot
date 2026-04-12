# Cognitive Architecture — Implementation Gaps Backlog

> Identified: 2026-04-10
> Source: Deep codebase scan + competitive analysis
> Last updated: 2026-04-11

## Medium Priority

### ~~1. Query Enhancement Pipeline~~ DONE (with minor follow-ups)
- Two-pipeline design shipped: `QueryPipeline` + `RankingPipeline` in `crates/context_engine/src/enhancement/`, with all 5 stages (`SignalEnrichmentStage`, `PrfStage`, `MultiQueryStage`, `HeuristicRerankStage`, `LlmRerankStage`).
- `EnhancementBudget` cost model wired through `DepthMode`; `QueryEnhancementConfig` in `crates/config/src/schema/cognitive.rs`; assembler calls `retrieve_with_bundle()`; builder wires both pipelines.
- Observability: `EnhancementTraceRepo` persists traces, `AgentEvent::RetrievalEnhanced` emitted on SSE, Reforge `collect_enhancement_signals()` consumes 7-day window, autotuner `TrialParams` extended with PRF/rerank/budget knobs.
- **Follow-ups (small):**
  - ~~MCP `get_last_enhancement_trace` action on the `memory` tool~~ DONE (2026-04-11).
  - ~~Desktop UI Query Enhancement settings group~~ DONE (2026-04-11). Lives in the existing Work Contexts page; now exposes PRF fetch limit, multi-query + LLM-rerank model dropdowns, and per-depth-mode budget overrides (backed by a new `cognitive.queryEnhancement.budgetOverrides` config section).
  - ~~Autotuner Phase 8 wiring~~ DONE (2026-04-11). All 5 Phase 8 `TrialParams` (`prf_score_threshold`, `prf_max_expansion_terms`, `rerank_term_overlap_weight`, `multi_query_max_variants`, `enhancement_budget_latency_ms`) now flow end-to-end: stages read champion overrides, the trial generator advertises the bounds, runtime applies budget overrides per depth mode. Spec §11.1 renamed to reflect shipped code.
- **Future spec (carved out):** Heuristic rerank §3.4 originally described three signals (co-activation, term-overlap, recency). Only term-overlap is implemented; adding co-activation + recency needs `MemoryEntry` timestamps and a `CoActivationRepo` injection — will need its own spec.

### ~~2. Tiered History Compression~~ DONE
- `TieredHistoryCompressor` replaces the old `HistoryCompressor`. Groups messages into conversation turns, optionally scores via cognitive relevance (`MemoryScorer` trait), assigns tiers (Verbatim/Detailed/Condensed), and compresses with tier-specific LLM prompts. `LlmSummaryProvider` now accepts a `CompressionTier` parameter for differentiated prompts. Hybrid extractive-first: skips LLM when extractive fits; falls back on LLM failure. Tool-result microcompaction pre-pass reduces stale read_file/bash/grep results before tier compression. Delta compaction (`compress_with_delta`) reuses existing compressed prefix on session resume, only processing new messages. Tier 1 demotion re-compresses old detailed summaries to condensed when they age past the threshold. `HistoryCompressionConfig` in `config.json` → `cognitive.historyCompression`; `ContextTieredCompressed` event emitted for observability. Compressed prefix persisted in sessions table (`compressed_prefix`, `compressed_through_idx`, `compressed_at` columns). `CognitiveMemoryScorer` adapter wraps `MemoryRetriever` for cognitive-aware tier promotion.
- **Spec**: `docs/superpowers/specs/2026-04-11-tiered-history-compression-design.md`

### 3. Agent-Initiated Memory Operations (Letta-style)
- **Status**: NOT STARTED
- **Effort**: Large
- **Impact**: Hybrid system-managed + agent-controlled memory; gives LLM tools to explicitly store/search
- **Details**: Currently all memory management is system-driven. Adding tools like `memory_store`, `memory_search`, `memory_forget` would let the agent participate.
- **What's needed**: New tool implementations, integration with cognitive pipeline

## Future Considerations

### 4. Multi-Modal Memory
- **Status**: NOT STARTED
- **Effort**: Large
- **Impact**: First-mover advantage — no platform handles image/audio memory yet
- **Details**: All memory pipelines assume text. Would need embedding models for images/audio, schema extensions, retrieval adaptations.

### 5. Hierarchical Community Detection
- **Status**: NOT STARTED
- **Effort**: Medium
- **Impact**: Better navigation of large knowledge graphs
- **Details**: Current Louvain is flat single-level. Nested/hierarchical communities would allow drill-down.

### 6. Memory Provenance Tracking
- **Status**: PARTIAL
- **Effort**: Medium
- **Impact**: Auditability — trace source confidence chains
- **Details**: `fact_changelog` exists for mutations. Missing: full provenance chain (conversation → extraction → consolidation → promotion → usage in decision).

### ~~7. Co-Activation Expiration~~ DONE
- `expire_stale(90 days)` added to `CoActivationRepo`, wired into compaction step 8a (runs every cycle, before weekly decay).
