# Insight Review Architecture V2 — System Design

**Date:** 2026-03-17
**Status:** Approved
**Scope:** Versioned insight history, configurable scope, smart merge deduplication, and learning progress tracking for the Insight Review system.

**Depends on:** Phase 4 (Perspectives + Personas) — assumes 5-tab insight pipeline, PersonaRepo, and persona management UI are complete.

**Related specs:**
- `2026-03-16-mirofish-integration-architecture.md` — Platform architecture
- `2026-03-16-insight-review-design.md` — Original 5-tab insight design

---

## 1. Problems Being Solved

### 1.1 Lost History
The current `insight_review_cache` table uses `(note_id, content_hash)` unique constraint — every regeneration overwrites the old insight. No version history, no learning-progress tracking, no ability to compare how understanding evolved.

### 1.2 Wrong Scope
Insight context is hardcoded to backlinks only. LanceDB embeddings exist but are unused for insight context. Notes in the same notebook or with similar semantic content are invisible to the system.

### 1.3 Duplicate Work
Each note generates an independent insight. Two related notes ("Deep Work" and "Focus Sessions") produce 80% overlapping analysis, wasting tokens and cluttering the review experience.

---

## 2. Architecture — `feature-insights` Crate

### Decision
Create a new `feature-insights` crate at **L4** (same layer as `feature-tasks`, `feature-notes`, `feature-finance`), following the `FeaturePackage` pattern.

### Rationale
- Insights now own versioned history, scope config, dedup logic, and learning progress tracking — this outgrows `feature-notes`
- Follows the "one feature = one crate" pattern that makes the workspace maintainable
- Clean deprecation of old `insight_review_cache` table in cognitive
- Fully testable with ephemeral SQLite (`StoragePool::connect_in_memory()`)

### Dependencies (upward only — no L5 imports)
- `common` (L0) — `Result<T>`, error types
- `tools-core` (L1) — `FeaturePackage`, `Tool`, derive macros
- `storage` (L2) — `SqlitePool`, migrations, `VectorStore` (LanceDB access lives here, not in cognitive)
- `providers` (L3) — `DynProvider` for LLM calls
- `context_engine` (L3) — `TokenCounter` for prompt truncation
- `feature-notes` (L4, read-only) — `NoteRepo` for fetching note content + backlinks + tags

**Dependency inversion for cognitive data (L5 → L4):**
`feature-insights` defines accessor traits; `app-core` (L7) wires implementations that call `cognitive` repos. This follows the established pattern used by `SpawnHandler`, `CronHandler`, `ExtractionHandler`, etc.

```rust
// In feature-insights/src/traits.rs

/// Provides cognitive memory data for insight context injection.
#[async_trait]
pub trait CognitiveAccessor: Send + Sync {
    /// Search semantic facts by query text, optionally filtered by domain.
    async fn search_facts(&self, query: &str, domain: Option<&str>, limit: usize) -> Vec<String>;
    /// Get recent episodic memories mentioning a note.
    async fn recent_memories(&self, note_id: &str, limit: usize) -> Vec<String>;
    /// Get active procedural rules for a domain.
    async fn domain_rules(&self, domain: &str) -> Vec<String>;
    /// Get user model summary for a domain (deep dive only).
    async fn user_model_summary(&self, domain: &str) -> Option<String>;
    /// Get entity graph neighborhood (deep dive only).
    async fn entity_neighborhood(&self, note_id: &str, depth: u8) -> Vec<String>;
    /// Get temporal fact history (deep dive only).
    async fn fact_history(&self, subject: &str) -> Vec<String>;
}

/// Provides flashcard review data for learning progress computation.
#[async_trait]
pub trait FlashcardAccessor: Send + Sync {
    /// Get average review success rate for an insight (0.0-1.0).
    async fn review_success_rate(&self, insight_review_id: &str, days: i64) -> f64;
}

/// Provides embedding operations for insight content.
#[async_trait]
pub trait InsightEmbedder: Send + Sync {
    /// Embed insight content and store in LanceDB.
    async fn embed_and_store(&self, insight_id: &str, content: &str) -> Result<(), String>;
    /// Get cosine similarity between two insight embeddings.
    async fn similarity(&self, id_a: &str, id_b: &str) -> Option<f64>;
}
```

These traits are implemented in `agent` (L5) or `app-core` (L7) where `cognitive` repos are available, then injected into `InsightService` as `Arc<dyn Trait>` during `AppCore::init()`.

### Crate Structure

```
crates/feature-insights/
├── migrations/
│   └── (consolidated into cognitive 001_cognitive_tables.sql — pre-release)
├── src/
│   ├── lib.rs                        # FeaturePackage impl, re-exports
│   ├── traits.rs                     # CognitiveAccessor, FlashcardAccessor, InsightEmbedder traits
│   ├── repo.rs                       # InsightReviewRepo + InsightProgressRepo
│   ├── service.rs                    # InsightService (orchestrator)
│   ├── scope.rs                      # ScopeResolver (backlinks/semantic/project/manual)
│   ├── merge.rs                      # SmartMergeEngine (pre-gen dedup + parent injection)
│   ├── progress.rs                   # ProgressComputer (4 signals → snapshot)
│   ├── prompt_builder.rs             # Insight context builder (note + scope + cognitive)
│   ├── prompts.rs                    # Tab prompt templates (moved from app-core)
│   └── types.rs                      # InsightReview, ScopeConfig, ProgressSnapshot, etc.
```

### Migration from Old System
Since we haven't released production yet (per CLAUDE.md: "Pre-release — no user data to migrate"):
- Update `001_cognitive_tables.sql` in-place, bump cognitive feature migration version to 4
- `DROP TABLE IF EXISTS insight_review_cache` — removes old cache table
- Create new `insight_reviews` + `insight_progress_snapshots` tables in the same SQL file
- `app-core/handlers/notes/insight.rs` becomes a thin adapter delegating to `InsightService`
- Old `InsightCacheRepo` and `InsightCacheRow` deprecated from cognitive crate

**Migration ownership note:** Ideally `feature-insights` would own its migration via `FeatureMigration` (like `feature-tasks` does). However, since the new tables replace `insight_review_cache` which is already in cognitive's migration file, and we're pre-release, consolidating into cognitive's SQL is simpler. Post-release, if insights need schema changes, they should get their own `FeatureMigration` in the `feature-insights` crate.

### Tools
`feature-insights` exposes no agent tools in v1 — insights are UI-driven only. `FeaturePackage::tools()` returns an empty vec. An `insight` tool for the agent (e.g., "generate insight for this note") can be added later as a natural extension.

---

## 3. Data Model

### 3.1 Schema

```sql
-- Replaces old insight_review_cache
DROP TABLE IF EXISTS insight_review_cache;

CREATE TABLE IF NOT EXISTS insight_reviews (
    id                  TEXT PRIMARY KEY,
    note_id             TEXT NOT NULL,
    version             INTEGER NOT NULL DEFAULT 1,
    generated_at        TEXT NOT NULL,
    content             TEXT NOT NULL,                    -- JSON blob: InsightContent
    input_hash          TEXT NOT NULL,                    -- SHA-256(note content + scope config + related IDs)
    scope_config        TEXT NOT NULL,                    -- JSON: ScopeConfig
    persona_ids         TEXT DEFAULT '[]',                -- JSON array of persona IDs used
    parent_insight_id   TEXT REFERENCES insight_reviews(id),  -- for Smart Merge lineage
    token_cost_usd      REAL,
    superseded_at       TEXT,                             -- soft-archive timestamp
    UNIQUE(note_id, version)
);

CREATE INDEX IF NOT EXISTS idx_insight_reviews_note ON insight_reviews(note_id, version);
CREATE INDEX IF NOT EXISTS idx_insight_reviews_hash ON insight_reviews(input_hash);
CREATE INDEX IF NOT EXISTS idx_insight_reviews_parent ON insight_reviews(parent_insight_id);

CREATE TABLE IF NOT EXISTS insight_progress_snapshots (
    id                  TEXT PRIMARY KEY,
    insight_review_id   TEXT NOT NULL REFERENCES insight_reviews(id) ON DELETE CASCADE,
    version             INTEGER NOT NULL,
    flashcard_success   REAL NOT NULL DEFAULT 0.0,        -- FSRS average (0-1)
    semantic_drift      REAL NOT NULL DEFAULT 0.0,        -- cosine distance from previous (0-1)
    gap_closure         REAL NOT NULL DEFAULT 0.0,        -- % of prior gaps addressed (0-1)
    quiz_score          REAL NOT NULL DEFAULT 0.0,        -- self-assessment average (0-1)
    overall_progress    REAL NOT NULL DEFAULT 0.0,        -- weighted composite (0-1)
    computed_at         TEXT NOT NULL,
    UNIQUE(insight_review_id, version)
);

CREATE INDEX IF NOT EXISTS idx_progress_insight ON insight_progress_snapshots(insight_review_id, version);
```

**Insight embeddings** are stored in LanceDB (not SQLite). This requires adding an `insight_embeddings` table to `VectorStore`:

1. Add schema definition in `crates/storage/src/vector_store/schemas.rs` — 384-dim vector (matching existing tables like `note_embeddings`)
2. Add table creation in `VectorStore::connect()` — same pattern as the 7 existing tables
3. Add `ensure_indexes()` entry for IVF-PQ index

The `InsightEmbedder` trait (defined in `feature-insights/src/traits.rs`) abstracts this — the concrete implementation in `app-core` calls `VectorStore` directly since it's at L2 (accessible from L7).

### 3.2 Content Storage

All 5 tabs stored as a single JSON blob in the `content` column:

```rust
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct InsightContent {
    pub synthesis: Option<String>,
    pub gap_analysis: Option<String>,
    pub self_assessment: Option<String>,    // JSON-encoded quiz questions
    pub concept_map: Option<String>,
    pub perspectives: Option<String>,
}
```

**Why JSON blob:** Schema stays simple and extensible (adding a 6th tab doesn't require `ALTER TABLE`). We never query by individual tab content — only by `note_id` and `version`.

### 3.3 Scope Configuration

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum ScopeType {
    Backlinks,
    Semantic,
    Project,
    Manual,
}

impl Default for ScopeType {
    fn default() -> Self { Self::Backlinks }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ScopeConfig {
    #[serde(default)]
    pub scope_type: ScopeType,
    /// Similarity threshold for semantic scope (0.0-1.0)
    #[serde(default = "default_radius")]
    pub radius: f64,
    /// Explicit note IDs for manual scope
    #[serde(default)]
    pub node_ids: Vec<String>,
    /// Whether to inject cognitive facts + episodic memories
    #[serde(default = "default_true")]
    pub include_cognitive: bool,
    /// Deep dive mode — includes UserModel + entity graph + temporal history
    #[serde(default)]
    pub deep_dive: bool,
    /// Merge threshold for parent insight detection
    #[serde(default = "default_merge_threshold")]
    pub merge_threshold: f64,
}

fn default_radius() -> f64 { 0.72 }
fn default_true() -> bool { true }
fn default_merge_threshold() -> f64 { 0.60 }
```

---

## 4. Scope Resolution

### 4.1 Scope Types

| Scope Type | How it works | Infrastructure used | When to recommend |
|-----------|-------------|---------------------|-------------------|
| `backlinks` | Current behaviour — wikilink references | `NoteRepo::get_backlinks_with_context()` | Default for new users |
| `semantic` | LanceDB embedding similarity search | `VectorStore::search()` with `min_similarity` from `radius` | Best for most cases |
| `project` | All notes in the same notebook/project | `NoteRepo::list_notes()` filtered by `notebook_id` | When user is in a big project |
| `manual` | User selects extra note IDs in UI | `node_ids` from `ScopeConfig` | Power-user mode |

### 4.2 Scope Resolver

```rust
#[async_trait]
pub trait ScopeResolver: Send + Sync {
    async fn resolve(&self, note_id: &str, config: &ScopeConfig) -> Vec<String>;
}
```

Injectable via `Arc<dyn ScopeResolver>` for testability. The default implementation combines the selected scope type with always-included backlinks.

### 4.3 "Best Node" UI Guidance

When user opens any note, run a cheap LanceDB query for similar notes (top-5, score > 0.75). Show tooltip:
> "Triggering insight here will see 12 related notes (including 'Deep Work' and 'Focus Sessions')."

---

## 5. Cognitive Context Injection

Two tiers of cognitive data injection, controlled by `ScopeConfig.include_cognitive` and `ScopeConfig.deep_dive`.

### 5.1 Medium (default, `include_cognitive: true`)

All cognitive data is accessed via the `CognitiveAccessor` trait (dependency inversion — no direct L5 imports).

| Source | Trait method | Token budget |
|--------|-------------|-------------|
| Semantic facts (domain-relevant) | `accessor.search_facts(note_title, Some(domain), 10)` | ~500 tokens |
| Episodic memories (recent sessions) | `accessor.recent_memories(note_id, 5)` | ~500 tokens |
| Procedural rules (domain-active) | `accessor.domain_rules(domain)` | ~200 tokens |

### 5.2 Deep Dive (opt-in, `deep_dive: true`)

Everything above, plus:

| Source | Trait method | Token budget |
|--------|-------------|-------------|
| User model summary | `accessor.user_model_summary(domain)` | ~300 tokens |
| Entity graph neighborhood | `accessor.entity_neighborhood(note_id, 2)` | ~500 tokens |
| Temporal fact history | `accessor.fact_history(note_title)` | ~400 tokens |

**Implementation note:** The concrete `CognitiveAccessor` implementation (in `app-core` or `agent`) calls the actual repos: `SemanticFactRepo::search_fts(query, domain, limit)` (3 args), `EpisodicMemoryRepo`, `ProceduralRuleRepo`, `load_user_model()`, `EntityRepo::get_neighborhood(entity_id, depth)`, and `TemporalService::get_fact_history(subject, predicate)`. The trait abstracts these into simpler string-based interfaces so `feature-insights` never imports `cognitive` directly.

---

## 6. Smart Merge + Deduplication Engine

### 6.1 Pre-Generation Flow

```
1. Resolve scope (ScopeResolver)
   → Collect note IDs that will be fed to the LLM

2. Compute input_hash
   → SHA-256(note_body + sorted scope_note_ids + scope_config JSON)

3. Check for exact match (same input_hash in insight_reviews)
   → If found: return existing insight (no LLM call)

4. Check force_new_version flag
   → If set: skip merge, treat as completely new version

5. Check for semantic overlap (Smart Merge)
   → Search insight_reviews WHERE note_id IN (scope_note_ids)
   → Find best parent: sort by (overlap_score DESC, generated_at DESC)
   → If Jaccard overlap > merge_threshold (default 0.60): set as parent

6. Build prompt with parent context (if parent found)
   → Inject parent insight summary into system prompt
   → LLM focuses on what's NEW or DIFFERENT

7. Generate → Store as new version with parent_insight_id set
```

### 6.2 Scope Overlap Calculation

```rust
fn scope_overlap(scope_a: &[String], scope_b: &[String]) -> f64 {
    let set_a: HashSet<&str> = scope_a.iter().map(|s| s.as_str()).collect();
    let set_b: HashSet<&str> = scope_b.iter().map(|s| s.as_str()).collect();
    let intersection = set_a.intersection(&set_b).count();
    let union = set_a.union(&set_b).count();
    if union == 0 { return 0.0; }
    intersection as f64 / union as f64  // Jaccard similarity
}
```

### 6.3 Smart Merge Prompt Injection

When a parent insight is found, the prompt gets an extra section:

```
--- PRIOR ANALYSIS (from related note "{parent_note_title}", generated {date}) ---
{parent_insight_synthesis, truncated via token estimator to ~1000 tokens}

Key gaps identified: {parent_gap_bullets}
---

Your task: Analyze the CURRENT note's content. Build on the prior analysis above.
- Don't repeat conclusions already established
- Highlight what's NEW, DIFFERENT, or CONTRADICTORY
- If the current note closes any of the prior gaps, note that explicitly
```

Parent summary truncation uses the existing `tiktoken_rs` token estimator from `ContextEngine`.

### 6.4 Post-Generation Embedding

After storing the insight, embed the full content via the `InsightEmbedder` trait (concrete implementation uses `EmbeddingEngine` from `tools` crate + `VectorStore` from `storage` crate) and store in LanceDB `insight_embeddings` table. This enables:
- Semantic drift calculation between versions (cosine distance)
- Cross-note dedup search for the timeline view
- Future "similar insights" discovery

---

## 7. Learning Progress + Insight Evolution Timeline

### 7.1 Progress Computation

`ProgressComputer::compute()` runs in two contexts:
1. **Synchronously** — immediately after a new insight version is saved (~5ms)
2. **Background cron** — daily re-computation for insights with new flashcard reviews since last snapshot

### 7.2 Four Signals

**Signal 1 — Flashcard Success (weight: 0.40)**

The `flashcards` table has no per-review `quality` column — FSRS review quality is consumed immediately to update `stability`, `difficulty`, `state`, and `lapses`. We derive success from these stored FSRS metrics:

```sql
SELECT AVG(
    CASE
        WHEN state = 'review' AND lapses = 0 THEN
            MIN(1.0, stability / 10.0)  -- high stability + no lapses = mastery
        WHEN state = 'review' AND lapses > 0 THEN
            MAX(0.2, MIN(0.7, stability / 10.0))  -- recovered but weaker
        WHEN state = 'relearning' THEN 0.1  -- currently struggling
        ELSE 0.0  -- 'new' or no data
    END
) as success_rate
FROM flashcards
WHERE insight_review_id = ?1
  AND review_count > 0
```

This is wrapped by the `FlashcardAccessor` trait — the concrete implementation lives in `app-core` where `FlashcardRepo` is accessible. Falls back to 0.0 if no flashcards exist for this insight.

**Signal 2 — Semantic Drift (weight: 0.25)**

Cosine distance between current and previous version embeddings via `InsightEmbedder::similarity(current_id, prev_id)`. Low drift (< 0.2) = stable understanding. High drift (> 0.5) = significant evolution. v1 has no drift (0.0).

**Signal 3 — Gap Closure (weight: 0.20)**

Parse gap analysis bullets from previous version's `InsightContent.gap_analysis`. Check how many gap topics appear in the current note body (case-insensitive substring match). `closed / total_gaps`.

**Signal 4 — Quiz Score (weight: 0.15)**

Average self-assessment correctness from the Self-Assessment tab. Quiz responses persisted when user answers questions.

### 7.3 Composite Score

```rust
overall = (
    0.40 * flashcard_success +
    0.25 * (1.0 - semantic_drift) +   // less drift = better mastery
    0.20 * gap_closure +
    0.15 * quiz_score
)
```

Weights configurable via `insights.progressWeights` in config.

### 7.4 Change Note Generation

```rust
fn generate_change_note(current: &ProgressSnapshot, previous: Option<&ProgressSnapshot>) -> String {
    let Some(prev) = previous else {
        return "Initial insight generated".to_string();
    };
    let delta = current.overall_progress - prev.overall_progress;
    let direction = if delta > 0.0 { "Improved" } else { "Declined" };
    let pct = (delta.abs() * 100.0).round() as i32;

    let signals = [
        ("flashcard reviews", current.flashcard_success - prev.flashcard_success),
        ("content stability", (1.0 - current.semantic_drift) - (1.0 - prev.semantic_drift)),
        ("gap closure", current.gap_closure - prev.gap_closure),
        ("quiz performance", current.quiz_score - prev.quiz_score),
    ];
    let best = signals.iter()
        .max_by(|a, b| a.1.abs().partial_cmp(&b.1.abs()).unwrap())
        .unwrap();

    format!("{direction} {pct}% — driven by {}", best.0)
}
```

### 7.5 Timeline UI Contract

```rust
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct InsightEvolutionResponse {
    pub note_id: String,
    pub note_title: String,
    pub versions: Vec<InsightEvolutionPoint>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct InsightEvolutionPoint {
    pub version: i32,
    pub date: String,
    pub flashcard_success: f64,
    pub semantic_drift: f64,
    pub gap_closure: f64,
    pub quiz_score: f64,
    pub overall_progress: f64,
    pub change_note: String,
}
```

---

## 8. Configuration

```json
{
  "insights": {
    "defaultScope": "semantic",
    "defaultRadius": 0.72,
    "includeCognitive": true,
    "deepDive": false,
    "mergeThreshold": 0.60,
    "dedupEmbeddingThreshold": 0.88,
    "autoRegenOnEdit": false,
    "progressWeights": {
      "flashcard": 0.40,
      "drift": 0.25,
      "gap": 0.20,
      "quiz": 0.15
    }
  }
}
```

---

## 9. InsightService API

The central orchestrator:

```rust
pub struct InsightService {
    repo: InsightReviewRepo,
    progress_repo: InsightProgressRepo,
    scope_resolver: Arc<dyn ScopeResolver>,
    merge_engine: SmartMergeEngine,
    progress_computer: ProgressComputer,
    prompt_builder: PromptBuilder,
    cognitive: Arc<dyn CognitiveAccessor>,
    flashcards: Arc<dyn FlashcardAccessor>,
    embedder: Arc<dyn InsightEmbedder>,
}

impl InsightService {
    /// Generate a new insight version for a note
    pub async fn generate(&self, note_id: &str, scope: ScopeConfig, provider: &DynProvider) -> Result<InsightReview>;

    /// Get the latest insight for a note (no LLM call)
    pub async fn get_latest(&self, note_id: &str) -> Result<Option<InsightReview>>;

    /// List all versions for a note
    pub async fn list_versions(&self, note_id: &str) -> Result<Vec<InsightReview>>;

    /// Get the evolution timeline with progress snapshots
    pub async fn get_evolution(&self, note_id: &str) -> Result<InsightEvolutionResponse>;

    /// Regenerate a specific tab of the latest version
    pub async fn regenerate_tab(&self, note_id: &str, tab: &str, provider: &DynProvider) -> Result<InsightContent>;

    /// Force a completely new version (bypasses Smart Merge)
    pub async fn force_new_version(&self, note_id: &str, scope: ScopeConfig, provider: &DynProvider) -> Result<InsightReview>;

    /// Compute and store progress snapshot for an insight
    pub async fn compute_progress(&self, insight_id: &str) -> Result<ProgressSnapshot>;

    /// Background: recompute stale progress snapshots
    pub async fn refresh_stale_progress(&self, max_age_days: i64) -> Result<usize>;
}
```

---

## 10. Migration Path

### What changes in existing crates

| Crate | Change |
|-------|--------|
| `cognitive` | Update `001_cognitive_tables.sql`: drop `insight_review_cache`, add `insight_reviews` + `insight_progress_snapshots`. Bump migration version. Deprecate `InsightCacheRepo`, `InsightCacheRow`. |
| `app-core` | `insight.rs` becomes thin adapter delegating to `InsightService`. Remove `insight_cache_repo` and `flashcard_repo` fields from `AppCore` in favor of `insight_service: Arc<InsightService>`. Keep persona handlers as-is. |
| `desktop-shared` | Update `InsightReviewResponse` to match new `InsightReview` structure. Add `InsightEvolutionResponse` DTO. |
| `desktop` | Add Tauri commands for `list_versions`, `get_evolution`, `force_regenerate`. |
| Frontend | Update `useInsightReview` hook for versioned responses. Add Insight Evolution timeline component. Add scope config UI in panel header. |

### What stays the same
- PersonaRepo and persona management (Phase 4) — unchanged
- Flashcard system (FSRS) — unchanged, just better queried by `InsightProgressRepo`
- Prompt templates — moved to `feature-insights/prompts.rs` but content unchanged
- Tauri event streaming — same `insight:synthesis-chunk` etc. events

---

## 11. Implementation Phases

### Phase 1: Core Infrastructure
- Create `feature-insights` crate with `FeaturePackage` impl
- Schema migration (drop old cache, create new tables)
- `InsightReviewRepo` + `InsightProgressRepo`
- Move prompt templates from app-core
- `InsightService::generate()` and `get_latest()`
- Wire into `app-core` as thin adapter

### Phase 2: Smart Merge
- `ScopeResolver` trait + default implementation (backlinks + semantic)
- `SmartMergeEngine` (pre-gen hash check, Jaccard overlap, parent injection)
- `PromptBuilder` with cognitive context injection (Medium tier)
- Post-generation LanceDB embedding storage

### Phase 3: Learning Progress
- `ProgressComputer` with all 4 signals
- `InsightProgressRepo` for snapshot storage
- Background cron for stale progress refresh
- `InsightEvolutionResponse` API

### Phase 4: Frontend + Deep Dive
- Insight Evolution timeline component (recharts line chart)
- Scope configuration UI in panel header
- Version history sidebar (list versions, compare)
- Deep Dive toggle (full cognitive injection)
- "Best node" tooltip guidance
