# Sprint 5: Semantic Search — Business Requirements

**Sprint**: 5 (Week 9-10)
**Feature**: Semantic Search for Todo Tasks
**Analyst**: Business Analyst Agent
**Date**: 2026-02-15
**Status**: Draft — Pending Architecture & UX Review

---

## 1. Problem Statement

The current `todo search` command only supports exact substring matching (case-insensitive). Users who search for "auth safety" will NOT find tasks titled "login security" or "authentication hardening", despite those being semantically equivalent. This limits findability as task lists grow.

**Current behavior** (from `crates/cli/src/todo/search.rs`): String `.contains()` matching on title, description, and optionally attachments. No synonym handling, no concept matching, no relevance ranking.

---

## 2. User Stories

### US-1: Semantic Search (Primary)
**As a** Klyntbot user with a growing task list,
**I want** to search tasks by meaning (not just exact keywords),
**So that** I can find related tasks even when I use different terminology than the original task title.

#### Acceptance Criteria (Given/When/Then)

```gherkin
Scenario: Synonym matching
  Given a task exists with title "Fix login security vulnerability"
  When I run `klyntbot todo search --semantic "auth safety"`
  Then the task appears in results with a similarity score

Scenario: Conceptual matching
  Given tasks exist: "Add password reset flow", "Implement OAuth2 login", "Fix XSS in auth form"
  When I run `klyntbot todo search --semantic "authentication"`
  Then all three tasks appear ranked by relevance

Scenario: Score display
  Given semantic search returns results
  When results are displayed
  Then each result shows a similarity score (0.0-1.0) alongside the task
```

### US-2: Automatic Embedding Generation
**As a** user,
**I want** embeddings to be generated automatically when I create or update tasks,
**So that** semantic search works without any extra steps on my part.

#### Acceptance Criteria

```gherkin
Scenario: Embedding on add
  Given the embedding engine is initialized
  When I run `klyntbot todo add "Fix authentication bug in login form"`
  Then an embedding vector (384 dimensions) is generated and stored
  And the embedding is stored in todos_embeddings.jsonl (NOT in todos.jsonl)

Scenario: Embedding on update
  Given a task exists with an embedding
  When I update its title via `klyntbot todo update <id> --title "New title"`
  Then the embedding is regenerated from the updated title + description + tags

Scenario: No user action required
  Given embeddings are configured
  When I use normal todo add/update commands
  Then I never need to manually trigger embedding generation
```

### US-3: Hybrid Search
**As a** user,
**I want** a combined search mode that uses both keyword and semantic matching,
**So that** I get the best of both worlds — exact matches ranked high, plus conceptually related results.

#### Acceptance Criteria

```gherkin
Scenario: Hybrid merges both result sets
  Given a task "Fix login bug" exists
  And a task "Authentication security audit" exists
  When I search hybrid for "login"
  Then "Fix login bug" ranks highest (exact keyword match)
  And "Authentication security audit" also appears (semantic match)

Scenario: RRF ranking
  Given results from full-text search and semantic search
  When hybrid search merges them
  Then Reciprocal Rank Fusion (RRF) is used to combine rankings
  And no single search mode dominates unfairly
```

### US-4: Backfill Migration
**As a** user with existing tasks,
**I want** embeddings to be generated for my pre-existing tasks,
**So that** all my tasks are searchable semantically, not just new ones.

#### Acceptance Criteria

```gherkin
Scenario: Backfill on first run
  Given 50 existing tasks have no embeddings
  When the embedding backfill job runs
  Then all 50 tasks get embedding vectors generated
  And embeddings are stored in todos_embeddings.jsonl

Scenario: Idempotent backfill
  Given some tasks already have embeddings
  When the backfill job runs again
  Then only tasks missing embeddings are processed
  And existing embeddings are not regenerated
```

### US-5: Edge Cases
**As a** user,
**I want** graceful handling of edge cases,
**So that** the system never crashes or returns confusing output.

#### Acceptance Criteria

```gherkin
Scenario: Empty query
  Given the user runs `klyntbot todo search --semantic ""`
  Then the system returns an error message: "Search query cannot be empty"
  And does NOT crash or return all tasks

Scenario: No tasks have embeddings
  Given todos_embeddings.jsonl is empty or missing
  When the user runs semantic search
  Then the system returns "No tasks have embeddings yet. Run backfill or add new tasks."
  And suggests running the backfill

Scenario: Zero results
  Given tasks exist but none match the query semantically
  When similarity scores are all below threshold
  Then the system returns "No semantically similar tasks found for '<query>'"

Scenario: Very long query
  Given a query string exceeds 512 tokens
  When the embedding model receives it
  Then the query is truncated to model's max input length
  And search still works without error
```

---

## 3. Business Rules

### BR-1: Embedding Generation Timing
- Embeddings are generated **synchronously** during `todo add` and `todo update` operations
- The embedding input text is: `"{title} {description} {tags.join(' ')}"`
- If the embedding engine fails, the task is still created/updated (embedding is Optional)
- Failed embeddings should be logged as warnings, not errors

### BR-2: Embedding Model
- **Model**: `all-MiniLM-L6-v2` via `fastembed` crate (384 dimensions)
- **Runs locally**: No API calls, no network dependency for embeddings
- **Library**: `fastembed = "3.0"` (Rust port of sentence-transformers)

### BR-3: Similarity Threshold
- **OPEN QUESTION** (see Ambiguities §5): Recommended default threshold: **0.5** for inclusion in results
- Results below the threshold are excluded from output
- Results are always sorted by descending similarity score

### BR-4: Hybrid Search — Reciprocal Rank Fusion (RRF)
- **OPEN QUESTION** (see Ambiguities §5): RRF constant `k` parameter (standard default: 60)
- Formula: `RRF_score(d) = Σ 1/(k + rank_i(d))` where `rank_i` is the rank in each search method
- Both full-text and semantic searches contribute equally (no weighting)

### BR-5: Storage Separation
- Embeddings stored in `~/.klyntbot/todos_embeddings.jsonl` (separate from `todos.jsonl`)
- Format: `{"id": "<todo-id>", "embedding": [f32; 384]}`
- This preserves human-readability of the main todo store
- Embedding file is a derived artifact — can be regenerated from todos via backfill

### BR-6: Performance
- Semantic search on 1000 todos must complete in **< 500ms**
- This includes: query embedding generation + cosine similarity computation + sorting
- Embedding generation per task should be < 50ms (model is lightweight)

### BR-7: CLI Interface
- New flag: `--semantic` on `klyntbot todo search`
- `klyntbot todo search "query"` → existing keyword search (unchanged)
- `klyntbot todo search --semantic "query"` → semantic search only
- Future: `--hybrid` flag for combined mode (or make hybrid the default — see Ambiguities)

---

## 4. Success Metrics / KPIs

| Metric | Target | Measurement |
|--------|--------|-------------|
| Synonym recall | "login" → finds "authentication" tasks | Integration test 5.T.3 |
| Search latency (1000 todos) | < 500ms | Performance test 5.T.5 |
| Embedding dimensionality | Exactly 384 | Unit test 5.T.1 |
| Zero regressions | Existing keyword search unchanged | Existing test suite passes |
| Clippy clean | 0 warnings | CI check |
| No external API calls | Model runs 100% locally | Code review / no network in embeddings.rs |

---

## 5. Ambiguities & Open Questions

### Q1: Similarity Threshold (for Architect)
**Question**: What cosine similarity threshold should we use to filter "relevant" results?
- **Option A**: 0.5 (inclusive — shows more results, some may be loosely related)
- **Option B**: 0.7 (strict — only highly relevant matches)
- **Option C**: Configurable via `config.todo.search.semanticThreshold` (default 0.5)
- **Recommendation**: Option C — configurable, with 0.5 default. Power users can tune.

### Q2: Hybrid Search — Default or Opt-in? (for Architect)
**Question**: Should `klyntbot todo search "query"` default to hybrid, or require `--hybrid`?
- **Option A**: Keyword-only remains default, `--semantic` and `--hybrid` are explicit flags
- **Option B**: Hybrid becomes the default search (breaking change to existing behavior)
- **Recommendation**: Option A for v1 — non-breaking. Reconsider for v2.

### Q3: RRF `k` Parameter (for Architect)
**Question**: What value for the RRF constant `k`?
- Standard academic default is `k=60`. Lower values (e.g., 20) favor top-ranked results more aggressively.
- **Recommendation**: `k=60` (standard) with config override option.

### Q4: Multi-lingual Embeddings (for Architect + UX)
**Question**: Should we support multi-lingual semantic search?
- `all-MiniLM-L6-v2` is English-focused. `paraphrase-multilingual-MiniLM-L12-v2` supports 50+ languages but is larger.
- **Recommendation**: Start with English-only (`all-MiniLM-L6-v2`). Add multi-lingual as a future config option.

### Q5: Embedding Engine Initialization (for Architect)
**Question**: The fastembed model needs to be downloaded on first use (~23MB). How should we handle this?
- **Option A**: Download on first `todo add` (may surprise user with delay)
- **Option B**: Download during `klyntbot init` wizard (Step 8?)
- **Option C**: Lazy download with progress bar on first use
- **Recommendation**: Option C — lazy with clear user feedback.

### Q6: Search Result Display (for UX)
**Question**: How should similarity scores be displayed to the user?
- **Option A**: Raw float (e.g., `0.87`)
- **Option B**: Percentage (e.g., `87% match`)
- **Option C**: Qualitative labels (e.g., `strong match`, `partial match`)
- **Recommendation**: Ask UX teammate.

---

## 6. Dependency Map

```
5.1.1 (Add fastembed dep) ──→ 5.1.2 (EmbeddingEngine) ──→ 5.1.4 (Embed on upsert)
                                                        ──→ 5.1.6 (Semantic search)
                                                        ──→ 5.1.10 (Backfill job)
5.1.3 (Embedding field on Todo) ──→ 5.1.4
5.1.5 (Separate storage) ──→ 5.1.4, 5.1.6
5.1.6 ──→ 5.1.7 (TodoTool action) ──→ 5.1.9 (CLI command)
5.1.6 ──→ 5.1.8 (Hybrid search)
```

**Critical path**: 5.1.1 → 5.1.2 → 5.1.5 → 5.1.4 → 5.1.6 → 5.1.7 → 5.1.9

---

## 7. Risk Assessment

| Risk | Likelihood | Impact | Mitigation |
|------|-----------|--------|------------|
| fastembed 3.0 has breaking API changes | Medium | High | Pin version, check docs before starting |
| Model download fails (network issues) | Low | Medium | Graceful fallback to keyword-only search |
| Embedding storage grows large (1000 tasks × 384 floats × 4 bytes = ~1.5MB) | Low | Low | Acceptable size; compress if needed later |
| Cosine similarity is too slow at scale | Low | Medium | 1000 tasks × 384 dims is trivial; profile if > 5000 |
| Existing tests break from new dependency | Low | Medium | Embedding engine is Optional, existing code paths unchanged |

---

## 8. Edge Case Catalog

| # | Scenario | Expected Behavior |
|---|----------|-------------------|
| EC-1 | Empty query string (`""`) | Return error: "Search query cannot be empty" |
| EC-2 | Whitespace-only query (`"   "`) | Trim, then treat as EC-1 |
| EC-3 | No tasks have embeddings (fresh install / empty store) | Return message: "No tasks have embeddings yet. Add tasks or run backfill." |
| EC-4 | Some tasks have embeddings, some don't | Search only tasks with embeddings; note skipped count in output |
| EC-5 | All results below similarity threshold | Return: "No semantically similar tasks found for '<query>'" |
| EC-6 | Query exceeds model max input length (256 tokens for MiniLM) | Truncate silently; model handles this internally |
| EC-7 | `todos_embeddings.jsonl` is missing/deleted | Treat as EC-3; embeddings will regenerate on next add/update |
| EC-8 | `todos_embeddings.jsonl` is corrupted (invalid JSON line) | Skip corrupted entries, log warning, search remaining |
| EC-9 | Todo exists in `todos.jsonl` but not in embeddings file | Skip that todo in semantic search; include in backfill |
| EC-10 | Embedding exists but todo was deleted | Orphaned embedding is harmless; cleaned up on next compaction |
| EC-11 | Concurrent writes to embedding file | Use same atomic-write pattern as `todos.jsonl` (write tmp + rename) |
| EC-12 | `fastembed` model fails to initialize (missing model files) | Graceful fallback: disable semantic search, log error, keyword search still works |
| EC-13 | `--semantic` flag with `--include-attachments` | Embed attachment text too? **Decision needed** — recommend NO for v1 (title+desc+tags only) |
| EC-14 | Very short query (1-2 chars) | Allow it — embedding model handles short inputs; results may be low quality |
| EC-15 | Hybrid search with zero full-text results | RRF uses only semantic results; still returns ranked output |
| EC-16 | Hybrid search with zero semantic results | RRF uses only full-text results; still returns ranked output |

---

## 9. Validation Rules

### Input Validation

| Field | Rule | Error Message |
|-------|------|---------------|
| `query` (search) | Non-empty after trim | "Search query cannot be empty" |
| `query` (search) | Max 1000 chars | "Query too long (max 1000 characters)" |
| `limit` (search) | 1–100, default 10 | "Limit must be between 1 and 100" |
| `action` (tool) | Must be `"search_semantic"` or `"search_hybrid"` | "Unknown action: {action}" |

### Embedding Validation

| Rule | When | Action |
|------|------|--------|
| Vector dimension = 384 | On generation | Assert; if wrong, error (model mismatch) |
| No NaN/Inf in vector | On generation | Filter out; log warning |
| Cosine similarity in [-1.0, 1.0] | On search | Clamp; NaN → 0.0 |
| Embedding ID matches existing todo | On load | Skip orphans silently |

---

## 10. Error Scenarios & Recovery

### ES-1: Embedding Engine Initialization Failure
- **Trigger**: `fastembed::TextEmbedding::try_new()` fails (model download error, disk full)
- **Impact**: Semantic search unavailable; existing keyword search unaffected
- **Recovery**: Log error at startup. `--semantic` flag returns: "Semantic search unavailable: {error}. Use keyword search instead."
- **User action**: Check network, retry, or run `klyntbot init` to re-download model

### ES-2: Embedding Generation Failure on Add/Update
- **Trigger**: `embedding_engine.embed()` returns error during `todo add`
- **Impact**: Task is still created/updated (embedding is Optional)
- **Recovery**: Log warning. Task appears in keyword search but not semantic search. Backfill can regenerate later.

### ES-3: Cosine Similarity Edge Cases
- **Trigger**: Zero vector (all zeros) produces NaN in cosine similarity
- **Impact**: Task would be ranked unpredictably
- **Recovery**: If either vector is zero-norm, return similarity = 0.0

### ES-4: Embedding File I/O Error
- **Trigger**: `todos_embeddings.jsonl` unreadable (permissions, corruption)
- **Impact**: Semantic search fails
- **Recovery**: Return error with actionable message. Suggest deleting file and running backfill.

### ES-5: Model Version Mismatch
- **Trigger**: User upgrades fastembed, model produces different dimensions
- **Impact**: Stored embeddings incompatible with new model
- **Recovery**: Detect dimension mismatch on load. If mismatch, log warning and trigger full re-embedding via backfill.

---

## 11. Integration Points (Codebase-Specific)

Based on codebase analysis:

### Data Model Extension
- **File**: `crates/tools/src/todo_types.rs` (line 12, `pub struct Todo`)
- **Pattern**: Follow existing `#[serde(default)]` + `Option<T>` pattern (see `parent_id`, `project_id`, etc.)
- **Note**: Embedding NOT stored on Todo struct directly (separate file), but Todo needs no schema change

### Store Extension
- **File**: `crates/tools/src/todo_store.rs` (line 34, `pub struct TodoStore`)
- **Pattern**: TodoStore uses in-memory `HashMap<String, Todo>` index + JSONL journal
- **New**: Companion `EmbeddingStore` with same JSONL pattern, keyed by todo ID
- **Consideration**: TodoStore.add() and update() should call embedding engine if available

### Tool Extension
- **File**: `crates/tools/src/todo.rs`
- **Pattern**: Match on `action` string in `execute()`, dispatch to store methods
- **New actions**: `"search_semantic"`, `"search_hybrid"`

### CLI Extension
- **File**: `crates/cli/src/todo/search.rs` (existing, 144 lines)
- **Pattern**: Current search does in-memory substring matching
- **New**: Add `--semantic` flag, route to `TodoTool::execute()` with `action: "search_semantic"`

### Dependency Injection
- **Pattern**: Same as `SpawnHandler`/`CronHandler`/`CalendarHandler` — define trait in `tools` (Layer 3), implement in `agent` (Layer 5)
- **New trait**: Not needed if `EmbeddingEngine` lives in `tools` crate (no circular dep)
- **Alternative**: If embedding needs agent context, use `EmbeddingHandler` trait with DI

---

## 12. Out of Scope (Sprint 5)

- Real-time embedding updates via file watcher
- GPU-accelerated inference
- Multi-lingual model support (future sprint)
- Embedding-based task clustering / auto-tagging
- Vector database integration (HNSW, Faiss) — brute-force is fine for < 10k tasks
- Embedding of attachment content (title+description+tags only for v1)
- Embedding-based duplicate detection
