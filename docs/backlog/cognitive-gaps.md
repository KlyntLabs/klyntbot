# Cognitive Architecture — Implementation Gaps Backlog

> Identified: 2026-04-10
> Source: Deep codebase scan + competitive analysis
> Last updated: 2026-04-10

## Medium Priority

### 1. Implement Heuristic Query Rewriter
- **Status**: STUBBED
- **Effort**: Medium
- **Impact**: Reduce LLM calls, lower latency on retrieval
- **Details**: `QueryRewriter` adapter heuristic path always returns `None`. Rules for active task boosting, skill-aware enrichment not implemented.
- **What exists**: `crates/agent/src/adapters/query_rewriter.rs`, `RewriteResult` types, `RetrievalContext` input
- **What's needed**: Implement heuristic rules using RetrievalContext fields

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
