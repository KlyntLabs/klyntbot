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
  - MCP `get_last_enhancement_trace` action on the `memory` tool — not yet wired in `crates/tools/src/domain/memory_tool.rs`.
  - Desktop UI Query Enhancement settings group under the existing Cognitive tab — not yet built.
  - Spec/code drift: autotuner has `rerank_term_overlap_weight` instead of the spec's `rerank_co_activation_weight`. Decide whether to add the co-activation knob or update the spec.

### 2. Add Abstractive History Summarization
- **Status**: UNWIRED
- **Effort**: Medium
- **Impact**: Better compressed conversation context vs. extractive snippets
- **Details**: `SummaryProvider` trait defined but never implemented. History compression only uses extractive (first 150 chars).
- **What exists**: `crates/context_engine/src/summary_provider.rs` (trait), `HistoryCompressor`
- **What's needed**: LLM-backed `SummaryProvider` impl, wire into compressor

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
