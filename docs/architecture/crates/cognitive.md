# Crate: `cognitive`

> **Status:** 🟡 In Progress (active phase work; multiple env-gated paths; two name-collision traits)
> **Subsystem:** [05 — Cognitive Memory & Self-Improvement](../subsystems/05-cognitive-memory.md)
> **Status last verified:** 2026-05-16
> **One-liner:** The largest single-crate doc — every memory service, the 16-phase Reforge cycle, Mirror engine, Louvain + PPR first-party impls

---

## TL;DR

Klyntbot's memory system. This crate implements:
- **Memory services** — `UnifiedMemoryService` (embedding + BM25 + PPR), `ConversationRecallService`, `SessionMemoryService`, `CognitiveContextSource`
- **Extraction** — `ExtractionHandler` trait, `ExtractedFact`/`Entity`/`Relationship`, `ExtractionCritic`
- **Reforge** — `run_reforge` (26-parameter signature, **16 phase markers**, 3 handler-level LLM calls, **6 extension hook traits**)
- **Mirror** — `MirrorEngine::start` returns `StartedMirror` with **8 unconditional + 2 conditional signal sources** (`SkillEffectivenessSource` is a stub)
- **Schedulers** — `fsrs5` (power-law `retrievability` for flashcards) + `decay` (exponential `retrievability` for retrieval scoring) — **two functions with the same name and different formulas**
- **Graph** — `louvain.rs` (394 LOC, `UnGraph<String, f64>`) + `ppr_retrieval.rs` (404 LOC, `DiGraph<String, f32, u32>`) — both first-party
- **25+ repos** — `SemanticFactRepo`, `EpisodicMemoryRepo`, `KnowledgeAtomRepo`, `FlashcardRepo`, `EntityRepo`, `CommunityRepo`, …

This crate carries **the most name collisions and naming gotchas** of any in the workspace. Read [Internals — common confusion points](#common-confusion-points) before making changes.

---

## Module map

```
crates/cognitive/src/
├── lib.rs                  ← Re-exports
├── types.rs                ← KnowledgeAtom, SemanticFact, …
├── embedder.rs             ← TextEmbedder + SemanticFactEmbedder traits
├── bench_hooks.rs          ← criterion bench helpers
├── specta_helpers.rs       ← specta type aliases for Tauri serialisation
│
├── pipeline/               ← Signal collection pipeline
│   ├── mod.rs
│   ├── signal.rs           ← PipelineSignal type
│   ├── collector.rs        ← base Collector trait
│   ├── atom_collector.rs           ← collects raw KnowledgeAtom signals
│   ├── chat_turn_collector.rs      ← collects ChatTurn signals
│   ├── coaching_collector.rs       ← collects Coaching signals
│   ├── recall_collector.rs         ← collects Recall signals
│   ├── session_collector.rs        ← collects Session signals
│   ├── consolidator.rs     ← buffers + batches signals
│   └── writer.rs           ← bulk-writes collected signals to repos
│
├── consumers/
│   ├── mod.rs
│   ├── ingestion.rs        ← ingestion consumer (signal → DB write)
│   ├── metric.rs           ← MetricHarvestConsumer (metric_samples → DB)
│   └── retrieval.rs        ← retrieval consumer
│
├── repos/                  ← 25+ repo modules
│   ├── mod.rs              ← cognitive_migrations()
│   ├── semantic_fact.rs    ← SemanticFactRepo (bi-temporal facts)
│   ├── episodic_memory.rs
│   ├── knowledge_atom.rs
│   ├── flashcard.rs        ← FlashcardRepo, ReviewLogEntry, ReviewQuality
│   ├── fsrs_params.rs      ← per-domain FSRS weights
│   ├── entity.rs
│   ├── community.rs        ← Louvain communities
│   ├── co_activation.rs    ← edge weights for graph
│   ├── procedural_rule.rs  ← procedural memory storage (live!)
│   ├── accumulated_observation.rs
│   ├── fact_changelog.rs   ← bi-temporal audit
│   ├── retention_history.rs ← daily FSRS curve
│   ├── review_stats.rs
│   ├── review_session.rs
│   ├── ai_signal_index.rs  ← IndexedSignal
│   ├── ai_metric_samples.rs ← MetricRepo
│   ├── annotation.rs
│   ├── atom_extraction_cache.rs
│   ├── book_tree.rs        ← KnowledgeSnapshotRepo (book-tree snapshots)
│   ├── deck_preference.rs
│   ├── enhancement_trace.rs
│   ├── enrichment.rs       ← GraphEnrichmentRepo
│   ├── event_log.rs        ← EventLogRepo, DomainEventRow, PipelineEventRecord
│   ├── extraction_critic_log.rs
│   ├── failed_observation.rs
│   ├── gt_link.rs          ← GroundTruthLinkRepo
│   ├── markdown_parser.rs  ← markdown → KnowledgeAtom parser
│   └── pending_memory.rs
│
├── search/
│   └── bm25.rs             ← BM25 full-text scoring (pure, no DB)
│
├── services/
│   ├── mod.rs              ← re-exports
│   ├── fsrs5.rs            ← ⚠️ FSRS-5 retrievability (power-law)
│   ├── decay.rs            ← ⚠️ retrievability (exponential) — DIFFERENT FUNCTION
│   ├── fsrs_optimizer.rs   ← gradient-descent weight tuner
│   ├── atom_decay.rs       ← scheduled decay sweep
│   ├── atom_extraction.rs  ← LLM-backed atom extraction
│   ├── extraction.rs       ← ExtractedFact/Entity/Relationship + ExtractionHandler trait
│   ├── extraction_critic.rs ← LLM-backed fact critic
│   ├── extraction_critic_types.rs
│   ├── consolidation.rs    ← ConsolidationHandler trait + execute_memory_ops
│   ├── compaction.rs       ← run_compaction
│   ├── community_intelligence/ ← Phase 6.5b merge/split planner
│   ├── community_membership_online.rs ← incremental updater
│   ├── context_source.rs   ← CognitiveContextSource + CognitiveRetrievalConfig
│   ├── conversation_recall.rs ← ConversationRecallService
│   ├── graph_enrichment.rs ← GraphEnrichmentInput/Output
│   ├── graph_linker.rs
│   ├── graph_linker_types.rs
│   ├── graph_retrieval.rs
│   ├── hierarchical_compressor.rs ← deterministic fact dedup
│   ├── louvain.rs          ← 394 LOC — Louvain community detection
│   ├── memory_retriever.rs ← UnifiedMemoryService (embed + BM25 + PPR)
│   ├── micro_reforge.rs    ← single-session mini-cycle
│   ├── micro_reforge_types.rs
│   ├── ppr_retrieval.rs    ← 404 LOC — Personalized PageRank
│   ├── predictive_cache.rs ← KCA Track 7 prefetch cache
│   ├── retrieval.rs        ← generic retrieval helpers
│   ├── scoring.rs          ← relevance scorer; calls Louvain inline
│   ├── session_memory.rs   ← SessionMemoryService
│   ├── situation.rs        ← compute_situation()
│   ├── temporal.rs         ← bi-temporal fact history
│   ├── temporal_pruner.rs
│   ├── tiptap_parser.rs    ← Tiptap JSON → plain-text
│   ├── value_density.rs
│   ├── background.rs       ← BackgroundConsolidationService + PipelineEvent
│   └── reforge/
│       ├── mod.rs          ← 6 hook traits
│       ├── service.rs      ← run_reforge (~1700 LOC, 16 phase markers)
│       ├── collector.rs    ← FeedbackSources + skill-file diff collector
│       ├── feedback.rs     ← strategy_records raw-SQL loader
│       ├── skill_discovery.rs
│       ├── skill_files.rs  ← SkillFileManager
│       └── types.rs        ← SynthesizeInput/Output, ReviewInput/Output, NarrateInput, ReforgeResult
│
└── mirror/
    ├── mod.rs              ← re-exports
    ├── engine.rs           ← MirrorEngine::start + StartedMirror
    ├── facade.rs           ← MirrorFacade (public query/feedback API)
    ├── narratives.rs       ← NarrativeHandler trait
    ├── repo.rs             ← MirrorRepo (SQLite-backed)
    ├── retention.rs        ← retention curve helpers
    ├── types.rs            ← MirrorState, BrainVersion, MetaRule, TrialPreview,
    │                          EarlyTrialEvaluator, ⚠️ AutotunerBridge (mirror's)
    └── sources/
        ├── approval_history.rs       ← ApprovalHistorySource (conditional)
        ├── coding_bash.rs            ← BackgroundJobSignalSource (conditional)
        ├── coding_todo.rs            ← TodoSignalSource
        ├── config_archiver.rs        ← ConfigArchiverSource
        ├── cost_ceiling.rs           ← CostCeilingSource
        ├── finance_drift.rs          ← FinanceSpendingDriftSource
        ├── meta_rule.rs              ← MetaRuleSignalSource
        ├── routing.rs                ← RoutingSignalSource
        ├── skill_effectiveness.rs    ← 🔴 STUB — accumulate + flush are no-ops
        ├── task_focus.rs             ← TaskFocusPatternSource
        └── trial.rs                  ← TrialPreviewSource
```

---

## Public API surface

### Memory services

#### `UnifiedMemoryService`

```rust
pub struct UnifiedMemoryService { /* embed + BM25 + optional PPR boost */ }

impl UnifiedMemoryService {
    pub async fn search(
        &self,
        query: &str,
        session_key: Option<&str>,
        top_k: usize,
    ) -> Result<Vec<MemoryEntry>>;

    pub async fn search_with_ppr_boost(
        &self,
        query: &str,
        seed_entities: Vec<String>,
        top_k: usize,
    ) -> Result<Vec<MemoryEntry>>;
}
```

#### `ConversationRecallService`

```rust
pub struct ConversationRecallService { /* opaque */ }

pub struct RecallConfig {
    pub top_k: usize,
    pub include_episodic: bool,
    pub include_semantic: bool,
    pub time_range: Option<(Timestamp, Timestamp)>,
    // ...
}

pub struct RecallResult {
    pub items: Vec<RecallItem>,
    pub metadata: RecallMetadata,
}

pub struct RecallMetadata {
    pub query_expansion: Vec<String>,
    pub embedding_ms: u64,
    pub search_ms: u64,
    pub rerank_ms: u64,
}
```

#### `SessionMemoryService`

```rust
pub struct SessionMemoryService { /* opaque */ }

pub struct SessionMemoryConfig {
    pub max_session_facts: usize,
    pub decay_half_life: Duration,
    // ...
}
```

#### `CognitiveContextSource`

Implements `context_engine::ContextSource`. Provides cognitive recall results as a context-window-budgeted text block.

```rust
pub struct CognitiveContextSource { /* opaque */ }
pub struct CognitiveRetrievalConfig {
    pub max_tokens: usize,
    pub min_score: f64,
    // ...
}
```

#### `compute_situation`

```rust
pub fn compute_situation(inputs: SituationInputs) -> UserSituation;

pub struct SituationInputs {
    pub active_tasks: Vec<TaskRow>,
    pub recent_messages: Vec<Message>,
    pub focus_state: Option<FocusState>,
    pub time_of_day: Timestamp,
    // ...
}

pub struct UserSituation {
    pub current_focus: Option<String>,
    pub active_topics: Vec<String>,
    pub estimated_load: f32,            // 0.0..1.0
    // ...
}
```

### Extraction

```rust
#[async_trait]
pub trait ExtractionHandler: Send + Sync {
    async fn extract(&self, input: &BatchExtraction) -> Result<BatchExtractionResult>;
}

pub struct ExtractedFact {
    pub subject: String,
    pub predicate: String,
    pub object: String,
    pub confidence: f64,
    pub source_message_id: Option<String>,
}

pub struct ExtractedEntity {
    pub name: String,
    pub kind: String,                    // "person" | "project" | "place" | …
    pub aliases: Vec<String>,
}

pub struct ExtractedRelationship {
    pub source_entity: String,
    pub target_entity: String,
    pub kind: String,
}

pub struct BatchExtraction { pub items: Vec<...> }
pub struct BatchExtractionResult { pub facts: Vec<ExtractedFact>, pub entities: Vec<ExtractedEntity>, pub relationships: Vec<ExtractedRelationship> }
```

`ExtractionCritic` (separate type) scores each fact for validity post-extraction; rejects low-quality candidates.

### Reforge — the entry point

```rust
pub async fn run_reforge(
    reforge_state_repo: &ReforgeStateRepo,
    skill_version_repo: &SkillVersionRepo,
    session_memory_repo: &SessionMemoryRepo,
    fact_repo: &SemanticFactRepo,
    episodic_repo: &EpisodicMemoryRepo,
    rule_repo: &ProceduralRuleRepo,
    handler: &dyn ReforgeHandler,
    skill_mgr: &SkillFileManager,
    pre_read_skill_files: Option<HashMap<String, Vec<SkillFile>>>,
    mirror_repo: Option<&MirrorRepo>,
    feedback_repo: Option<&RetrievalFeedbackRepo>,
    autotuner_bridge: Option<&dyn AutotunerBridge /* reforge variant */>,
    autotuner_ctx: Option<AutotunerContext>,
    feedback_sources: Option<&FeedbackSources<'_>>,
    graph_enrichment_handler: Option<&dyn GraphEnrichmentHandler>,
    density_repo: Option<&ConversationDensityRepo>,
    entity_repo: Option<&EntityRepo>,
    snapshot_repo: Option<&KnowledgeSnapshotRepo>,
    community_intelligence_handler: Option<&dyn CommunityIntelligenceHandler>,
    community_repo: Option<&CommunityRepo>,
    co_activation_repo_for_split: Option<&CoActivationRepo>,
    domain_event_bus: Option<Arc<DomainEventBus>>,
    coding_phase_runner: Option<&dyn CodingPhaseRunner>,
    cross_cli_runner: Option<&dyn CrossCliPhaseRunner>,
    skill_discovery_runner: Option<&dyn SkillDiscoveryRunner>,
) -> Option<ReforgeResult>;
```

**26 parameters.** Most are `Option<&dyn Trait>` extension hooks — the cycle degrades gracefully when hooks aren't installed. A `ReforgeContext` builder refactor would help, but hasn't been done.

### 6 Reforge hook traits

```rust
#[async_trait]
pub trait ReforgeHandler: Send + Sync {
    async fn synthesize(&self, input: &SynthesizeInput) -> Result<SynthesizeOutput>;
    async fn review(&self, input: &ReviewInput) -> Result<ReviewOutput>;
    async fn narrate(&self, input: &NarrateInput) -> Result<String>;
}

#[async_trait]
pub trait AutotunerBridge: Send + Sync {
    async fn run_evaluation(&self, ctx: AutotunerContext) -> Result<CycleResult>;
    async fn create_trials(&self, ctx: AutotunerContext) -> Result<Vec<Trial>>;
}
// ⚠️ This is the REFORGE AutotunerBridge.
// There's also a MIRROR AutotunerBridge with the same name in mirror/types.rs.

#[async_trait]
pub trait GraphEnrichmentHandler: Send + Sync {
    async fn enrich_graph(&self, input: GraphEnrichmentInput) -> Result<GraphEnrichmentOutput>;
}

#[async_trait]
pub trait CommunityIntelligenceHandler: Send + Sync {
    async fn analyze_communities(&self, input: CommunityIntelligenceInput) -> Result<CommunityIntelligenceOutput>;
}

#[async_trait]
pub trait CodingPhaseRunner: Send + Sync {
    async fn run_synthesis(&self, …) -> Result<()>;
    async fn run_rule_artifacts(&self, …) -> Result<()>;
    async fn run_selective_delete(&self, …) -> Result<()>;
    async fn run_cross_session_dedup(&self, …) -> Result<()>;
}

#[async_trait]
pub trait CrossCliPhaseRunner: Send + Sync {
    async fn run_cross_cli_transfer(&self, …) -> Result<()>;
}

#[async_trait]
pub trait SkillDiscoveryRunner: Send + Sync {
    async fn run_skill_discovery(&self, …) -> Result<()>;
}
```

### The 16-phase cycle

| # | Phase | LLM call? | Hook | Notes |
|---:|---|---|---|---|
| 1 | Collect | No | — | Read recent strategy files + behavioral feedback + mirror signals |
| 2 | **Synthesize** | **Yes (#1)** | `ReforgeHandler::synthesize` | LLM JSON. Provider from `config.cognitive.provider`. T=0.2, max_tokens=4096 |
| 2.5 | Coding Synthesis | (via hook) | `CodingPhaseRunner::run_synthesis` | |
| 2.6 | Cross-CLI transfer | (via hook) | `CrossCliPhaseRunner::run_cross_cli_transfer` | |
| 3 | **Review** | **Yes (#2)** | `ReforgeHandler::review` | LLM JSON. Critique candidate rewrites |
| 3.5 | Rule Artifact Generation | (via hook) | `CodingPhaseRunner::run_rule_artifacts` | |
| 3.6 | Skill discovery | (via hook) | `SkillDiscoveryRunner::run_skill_discovery` | |
| 4 | **Narrate** | **Yes (#3)** | `ReforgeHandler::narrate` | LLM free-text. Human-readable summary |
| 5 | Apply | No | — | Persist `reforge_suggestions` + rewrite strategy files |
| 6 | Optimize | No (autotuner math) | `AutotunerBridge::{run_evaluation, create_trials}` + `CodingPhaseRunner::run_selective_delete` | |
| 6.5 | Graph Consolidation | (via hook) | `GraphEnrichmentHandler::enrich_graph` | |
| 6.5b | Community Intelligence | (via hook) | `CommunityIntelligenceHandler::analyze_communities` | Calls Louvain inline |
| 6.5-ext | Cross-session fact dedup | (via hook) | `CodingPhaseRunner::run_cross_session_dedup` | |
| 6.7 | Community Summaries | No (deterministic) | — | Env-gated `KCA_COMMUNITY_SUMMARIES=1`. "Phase C will add LLM call." |
| 7 | Compact | No | — | Trim retired data; rebuild indexes |
| 7.7 | Compression | No (deterministic dedup) | — | Env-gated `KCA_REFORGE_COMPRESS=1`. "Phase C will add LLM merge." |

**3 LLM calls at the `ReforgeHandler` level** (Synthesize / Review / Narrate). **Hook traits may add their own LLM calls in the agent layer** — `CodingPhaseRunner::run_synthesis` typically calls an LLM, `GraphEnrichmentHandler::enrich_graph` may, `CommunityIntelligenceHandler::analyze_communities` may, `CrossCliPhaseRunner::run_cross_cli_transfer` may, `SkillDiscoveryRunner::run_skill_discovery` may. **True total LLM-call count per nightly cycle is indeterminate from the spec alone** — it depends entirely on which hook impls are wired and what each one does internally. To know your specific deployment's count, instrument the providers and observe a real cycle.

### Mirror

```rust
pub struct MirrorEngine;

impl MirrorEngine {
    /// NOTE: No longer takes Arc<DomainEventBus> — was removed.
    pub fn start(
        repo: MirrorRepo,
        narrative_handler: Arc<dyn NarrativeHandler>,
        autotuner_bridge: Arc<dyn AutotunerBridge /* mirror's variant */>,
        episodic_repo: Option<EpisodicMemoryRepo>,
        rule_repo: Option<ProceduralRuleRepo>,
        trial_evaluator: Arc<dyn EarlyTrialEvaluator>,
        approval_history_repo: Option<CodingApprovalHistoryRepo>,
        approval_pattern_repo: Option<ApprovalPatternHistoryRepo>,
        bash_repo: Option<BashJobRepo>,
    ) -> StartedMirror;
}

pub struct StartedMirror {
    pub facade: MirrorFacade,
    pub consumers: Vec<Arc<dyn SignalConsumer>>,
    pub flush_handles: Vec<JoinHandle<()>>,
    pub shutdown: CancellationToken,
}

pub struct MirrorFacade { /* opaque */ }
impl MirrorFacade {
    pub async fn brain_state(&self) -> Result<MirrorState>;
    pub async fn recent_narratives(&self, limit: u32) -> Result<Vec<Narrative>>;
    pub async fn record_feedback(&self, feedback: Feedback) -> Result<()>;
    pub async fn list_brain_versions(&self) -> Result<Vec<BrainVersion>>;
    // ... ~15 more
}

#[async_trait]
pub trait NarrativeHandler: Send + Sync { /* ... */ }

#[async_trait]
pub trait EarlyTrialEvaluator: Send + Sync {
    async fn evaluate_trial_early(
        &self, trial_id: &str, since: Timestamp,
    ) -> Result<TrialEarlySignals>;
}

// Mirror's AutotunerBridge (DIFFERENT from reforge's)
#[async_trait]
pub trait AutotunerBridge: Send + Sync {
    async fn promote_champion(&self, trial_id: &str) -> Result<()>;
    async fn revert_to_previous(&self) -> Result<()>;
}
```

**Always-on signal sources (8):**
- `RoutingSignalSource`
- `MetaRuleSignalSource`
- `ConfigArchiverSource`
- `TrialPreviewSource`
- `TaskFocusPatternSource`
- `FinanceSpendingDriftSource`
- `TodoSignalSource`
- `CostCeilingSource`

**Conditionally registered (only when optional repos `Some`):**
- `ApprovalHistorySource` — needs `approval_history_repo` AND `approval_pattern_repo`
- `BackgroundJobSignalSource` — needs `episodic_repo` AND `bash_repo`

**Stub (NOT registered):**
- `SkillEffectivenessSource` — `accumulate` + `flush` are no-ops with `TODO(T7)` comments

### FSRS-5 (pure math)

```rust
// crates/cognitive/src/services/fsrs5.rs — pure functions, no I/O
pub const DEFAULT_WEIGHTS: [f64; 19];

/// Power-law formula: 1 / (1 + t/(9S))
pub fn retrievability(elapsed_days: f64, stability: f64) -> f64;

pub fn initial_stability(rating: u8, w: &[f64; 19]) -> f64;
pub fn initial_difficulty(rating: u8, w: &[f64; 19]) -> f64;
pub fn next_difficulty(current_d: f64, rating: u8, w: &[f64; 19]) -> f64;
pub fn next_stability_success(s: f64, d: f64, r: f64, rating: u8, w: &[f64; 19]) -> f64;
pub fn next_stability_failure(s: f64, d: f64, r: f64, w: &[f64; 19]) -> f64;
```

### Decay (retrieval-side, DIFFERENT formula)

```rust
// crates/cognitive/src/services/decay.rs
/// Exponential formula: exp(ln(0.9) * elapsed_days / stability)
/// ⚠️ SAME NAME as fsrs5::retrievability but different math
pub fn retrievability(elapsed_days: f64, stability: f64) -> f64;

pub struct RelevanceWeights {
    pub semantic: f64,
    pub retrievability: f64,           // weight on the retrievability score
    pub importance: f64,
    pub frequency: f64,
    pub situation: f64,
    pub temporal: f64,
    pub hierarchy: f64,
    pub path_coherence: f64,
    pub community: f64,
    pub cross_note: f64,
    pub recall_support: f64,
    pub graph_path_boost: f64,
}

pub fn relevance_score(
    semantic: f64, retr: f64, importance: f64, frequency: f64,
    situation: f64, temporal: f64, hierarchy: f64, path_coherence: f64,
    community: f64, cross_note: f64, recall_support: f64, graph_path_boost: f64,
) -> f64;
```

### Louvain

```rust
// crates/cognitive/src/services/louvain.rs — 394 LOC, first-party
pub fn detect_communities(pairs: &[(String, String, f64)]) -> Vec<HashSet<String>>;
```

Builds an undirected `UnGraph<String, f64>` using `petgraph`. Returns communities as sets of node IDs.

### PPR

```rust
// crates/cognitive/src/services/ppr_retrieval.rs — 404 LOC, first-party
pub fn personalized_pagerank(
    graph: &DiGraph<String, f32, u32>,
    seeds: &[NodeIndex<u32>],
    teleport_prob: f32,           // typically 0.15
    iterations: u32,              // typically 50
) -> Vec<(NodeIndex<u32>, f32)>;

pub struct CachedPprGraph {
    graph: DiGraph<String, f32, u32>,
    last_rebuilt: Timestamp,
}

impl CachedPprGraph {
    pub fn rebuild(&mut self, edges: Vec<(String, String, f32)>);
    pub fn run(&self, seeds: Vec<String>, top_k: usize) -> Vec<(String, f32)>;
}
```

**`u32` index intentionally caps node count at ~4 billion** for memory efficiency.

### Selected repos

#### `SemanticFactRepo`

```rust
pub struct SemanticFactRepo { pool: StoragePool }

impl SemanticFactRepo {
    pub fn new(pool: StoragePool) -> Self;

    pub async fn upsert(&self, fact: SemanticFact) -> Result<i64, StorageError>;
    pub async fn find(&self, id: i64) -> Result<Option<SemanticFact>, StorageError>;
    pub async fn find_active(&self, subject: &str, predicate: &str) -> Result<Vec<SemanticFact>, StorageError>;
    pub async fn supersede(&self, predecessor_id: i64, successor_id: i64) -> Result<(), StorageError>;
    pub async fn invalidate(&self, id: i64, valid_until: Timestamp) -> Result<(), StorageError>;
    pub async fn count_active(&self) -> Result<u64, StorageError>;

    pub async fn search_by_subject(&self, subject: &str) -> Result<Vec<SemanticFact>, StorageError>;
    pub async fn search_full_text(&self, query: &str, limit: u32) -> Result<Vec<SemanticFact>, StorageError>;
}
```

#### `FlashcardRepo` + `FsrsParamsRepo`

```rust
pub struct FlashcardRepo { pool: StoragePool }
impl FlashcardRepo {
    pub async fn create(&self, card: NewFlashcard) -> Result<Flashcard, StorageError>;
    pub async fn record_review(
        &self, card_id: i64, rating: ReviewQuality, reviewed_at: Timestamp,
    ) -> Result<(), StorageError>;
    pub async fn due_cards(&self, before: Timestamp, limit: u32) -> Result<Vec<Flashcard>, StorageError>;
}

pub enum ReviewQuality { Again = 1, Hard = 2, Good = 3, Easy = 4 }
pub struct ReviewLogEntry { pub card_id: i64, pub rating: ReviewQuality, pub reviewed_at: Timestamp }

pub struct FsrsParamsRepo { pool: StoragePool }
impl FsrsParamsRepo {
    pub async fn get_for_domain(&self, domain: &str) -> Result<[f64; 19], StorageError>;
    pub async fn update(&self, domain: &str, weights: [f64; 19]) -> Result<(), StorageError>;
}
```

#### `ProceduralRuleRepo`

Live storage for distilled procedural rules ("when X happens, do Y"). Full CRUD + FTS search.

```rust
pub struct ProceduralRuleRepo { pool: StoragePool }
impl ProceduralRuleRepo {
    pub async fn insert(&self, rule: NewProceduralRule) -> Result<i64, StorageError>;
    pub async fn search_text(&self, query: &str, limit: u32) -> Result<Vec<ProceduralRule>, StorageError>;
    pub async fn find_active_for_context(&self, ctx_key: &str) -> Result<Vec<ProceduralRule>, StorageError>;
    pub async fn deactivate(&self, id: i64) -> Result<(), StorageError>;
}
```

**Per the cognitive-memory scan: this is live, not a stub** — despite CLAUDE.md noting Computer Use & Procedural Memory "not yet implemented." The repo is live; the higher-level *Procedural Memory feature* (UI, agent behavior) is the unimplemented part.

---

## Common confusion points

These are the things that trip people up in this crate. Read before making changes.

### Two `retrievability` functions — DIFFERENT FORMULAS

| Function | Formula | Use case |
|---|---|---|
| `services::fsrs5::retrievability(t, s)` | Power-law: `1 / (1 + t/(9S))` | Flashcard scheduling |
| `services::decay::retrievability(t, s)` | Exponential: `exp(ln(0.9) * t/s)` | Retrieval-time relevance scoring |

**Importing the wrong one is silent and subtly wrong.** Flashcard scheduling using the exponential version produces incorrect intervals; relevance scoring using the power-law version produces incorrect retrieval boosts. Rename one or co-locate with clear docstrings — see [TECH_DEBT.md].

### Two `AutotunerBridge` traits — SAME NAME

| Trait | Location | Used for |
|---|---|---|
| `cognitive::services::reforge::AutotunerBridge` | `services/reforge/mod.rs` | Phase 6 trial orchestration (`run_evaluation`, `create_trials`) |
| `cognitive::mirror::AutotunerBridge` | `mirror/types.rs` | MirrorFacade's champion-promotion flow (`promote_champion`, `revert_to_previous`) |

Different methods, different semantics. Concrete impls live in `app-core::adapters::autotuner_bridge` for both.

### Path: `~/.klyntbot/skills/<skill>/`, not `strategy/`

Strategy files are skill files. They live in `~/.klyntbot/skills/<skill-name>/` directories with `SKILL.md` + `references/` + optional `scripts/`. The `SkillFileManager` root is passed at construction; not hard-coded.

### `MirrorEngine::start` no longer takes the bus

Older `MirrorEngine::start` signatures required `Arc<DomainEventBus>` because some sources subscribed directly. That coupling was removed; sources now flow through `SignalConsumer`. **CLAUDE.md still says it takes the bus — wrong.** The bus is still passed to `run_reforge` (Phase 6.5b community events), but mirror itself doesn't need it.

### `strategy_records` table is raw-SQL accessed

`reforge/feedback.rs:173` reads `strategy_records` via a raw SQL query, not through a typed `*Repo`. Easy to miss when surveying repo coverage.

### KCA env-var flags

| Flag | Effect |
|---|---|
| `KCA_DISABLE_COMPRESSION=1` | Bypasses `TieredHistoryCompressor` (Letta benchmark mode) |
| `KCA_PHASE_4=1` + `KCA_PHASE_4_LEGACY_NUDGE=1` | Legacy text-nudge memory-refusal retry |
| `KCA_PHASE_4_TOOL_DRIVEN=1` | Tool-call memory-refusal retry |
| `KCA_COMMUNITY_SUMMARIES=1` | Enables Reforge Phase 6.7 (deterministic) |
| `KCA_REFORGE_COMPRESS=1` | Enables Reforge Phase 7.7 (deterministic) |

### Reforge `service.rs:1` doc comment says "8 phases"

**Stale.** Actual is 16 phase markers. Update the doc comment.

---

## Internals

### Signal pipeline (from `pipeline/` modules)

```
1. Feature publishes DomainEvent on DomainEventBus
2. SignalRouter (in ai-core, but driven from here) translates → AiSignal
3. Salience verdict:
   - Extract → push to extraction pipeline (LLM)
   - Accumulate → merge into AccumulatedObservationRepo
   - Discard → drop
4. ExtractionHandler produces ExtractedFact / Entity / Relationship
5. ExtractionCritic scores facts; rejects low-quality
6. ConsolidationHandler::execute_memory_ops:
   - SemanticFactRepo.upsert (bi-temporal)
   - EpisodicMemoryRepo.insert
   - EntityRepo.upsert + CoActivationRepo edges
7. (Async) Embedding into LanceDB via SemanticFactEmbedder
8. (Periodic) Community detection (Louvain) via Phase 6.5b
9. (Nightly) Reforge synthesizes rule rewrites
```

### Where Louvain + PPR are actually called

- **Louvain** — `services/scoring.rs:126` (inline during retrieval scoring) AND `services/community_intelligence/mod.rs:244` (Phase 6.5b split analysis). **Not called from `run_reforge` directly** — driven from scoring and community-planner layers.
- **PPR** — `services/memory_retriever.rs` via `CachedPprGraph` and `retrieve_with_ppr_boost`. Runs at retrieval time as an additive boost after embedding + BM25.

### `run_reforge` execution model

`service.rs::run_reforge` is ~1700 LOC. Each phase is a sequential block. Failures in optional-hook phases are caught and logged; the cycle continues. Failures in core phases (Collect, Synthesize, Apply) abort the cycle.

### Cost-ceiling guard

LLM-calling phases check `cost_ceiling_usd` before invoking the provider. If the per-cycle cost ceiling is exceeded, the phase logs a warning and returns early. Subsequent phases skip their LLM calls. Acceptable degradation for nightly autonomous runs.

### Mirror sources accumulate + flush

Each signal source has an `accumulate` method (called per signal) and a `flush` method (called periodically by the `flush_handles` background tasks). Accumulators are in-memory; flushes persist to `MirrorRepo` tables.

### `SkillEffectivenessSource` is a stub

```rust
// crates/cognitive/src/mirror/sources/skill_effectiveness.rs
// TODO(T7): Extract tool_name and success from coding-memory queries
async fn accumulate(&self, _signal: &AiSignal) { /* no-op */ }

// TODO(T7): Query coding_memory for recent tool executions
async fn flush(&self) -> Result<()> { Ok(()) /* no-op */ }
```

**Not registered by `MirrorEngine::start`** — the wiring is conditional on having implementation. Listed here as a known TODO.

---

## Workflows

### Memory retrieval (assistant turn)

```rust
let mem_service: Arc<UnifiedMemoryService> = …;
let results = mem_service.search(
    /* query */ "What did we discuss about Q4 budget?",
    /* session_key */ Some("desktop:thread-abc"),
    /* top_k */ 10,
).await?;
// Embedding ANN → BM25 → RRF merge → PPR boost (if seeds available)
```

### Nightly reforge

```rust
// Triggered by JOB_REFORGE_NIGHTLY cron (03:00 local), registered in app-core
let result: Option<ReforgeResult> = run_reforge(
    &repos.reforge_state,
    &repos.skill_version,
    &repos.session_memory,
    &repos.semantic_fact,
    &repos.episodic_memory,
    &repos.procedural_rule,
    &reforge_handler,              // ReforgeHandler impl
    &skill_file_manager,
    None,
    Some(&mirror_repo),
    Some(&repos.retrieval_feedback),
    Some(&autotuner_bridge),       // app-core impl
    Some(autotuner_ctx),
    Some(&feedback_sources),
    Some(&graph_enrichment_handler),
    Some(&conversation_density_repo),
    Some(&repos.entity),
    Some(&knowledge_snapshot_repo),
    Some(&community_intelligence_handler),
    Some(&community_repo),
    Some(&co_activation_repo),
    Some(domain_bus.clone()),
    Some(&coding_phase_runner),
    Some(&cross_cli_runner),
    Some(&skill_discovery_runner),
).await;
```

### Mirror startup

```rust
let started = MirrorEngine::start(
    mirror_repo,
    narrative_handler,
    autotuner_bridge,                  // mirror's variant
    Some(repos.episodic_memory),
    Some(repos.procedural_rule),
    trial_evaluator,
    Some(repos.coding_approval_history),
    Some(repos.approval_pattern_history),
    Some(repos.bash_job),
);

// MUST be stored — dropping flush_handles cancels all flushers!
core.mirror = Some(Arc::new(started.facade));
core.mirror_consumers = Some(started.consumers);
core.mirror_flush_handles = Some(started.flush_handles);
core.mirror_shutdown = Some(started.shutdown);
```

### Flashcard review

```rust
let due = repos.flashcard.due_cards(now, 20).await?;
for card in due {
    let rating = present_to_user(&card);  // get user's rating
    repos.flashcard.record_review(card.id, rating, now).await?;
    // Internally:
    //   - Get FSRS weights for card.domain via FsrsParamsRepo
    //   - Compute next stability via fsrs5::next_stability_success / failure
    //   - Update card.due_at, card.stability, card.difficulty
}
```

---

## Testing approach

### In-memory pool + cognitive migrations

```rust
let pool = StoragePool::connect_in_memory().await.unwrap();
cognitive::repos::cognitive_migrations()
    .into_iter()
    .map(|m| sqlx::query(&m.sql).execute(pool.inner()))
    .collect::<futures::stream::FuturesUnordered<_>>()
    .try_collect::<Vec<_>>()
    .await
    .unwrap();
```

### Test Louvain on a known graph

```rust
let pairs = vec![
    ("a".to_string(), "b".to_string(), 1.0),
    ("a".to_string(), "c".to_string(), 1.0),
    ("b".to_string(), "c".to_string(), 1.0),
    ("d".to_string(), "e".to_string(), 1.0),
];
let communities = louvain::detect_communities(&pairs);
assert_eq!(communities.len(), 2);
```

### Test FSRS-5 with default weights

```rust
let r = fsrs5::retrievability(7.0, 30.0);
assert!((r - 0.8889).abs() < 0.001);  // 1 / (1 + 7/(9*30))
```

### Mock `ReforgeHandler` for reforge tests

```rust
struct ConstHandler { result: ReforgeResult }

#[async_trait]
impl ReforgeHandler for ConstHandler {
    async fn synthesize(&self, _: &SynthesizeInput) -> Result<SynthesizeOutput> { Ok(self.result.synth.clone()) }
    async fn review(&self, _: &ReviewInput) -> Result<ReviewOutput> { Ok(self.result.review.clone()) }
    async fn narrate(&self, _: &NarrateInput) -> Result<String> { Ok(self.result.narrative.clone()) }
}
```

### Skip extra phases by passing `None`

For unit testing the core flow, pass `None` for all hook traits except `ReforgeHandler`. Verify the core path (Collect → Synth → Review → Narrate → Apply → Compact) without coding/community/graph branches.

---

## Extension points

### Add a memory service

1. Implement under `services/`.
2. If it should be consumed by `context_engine`, also implement `MemoryRetriever`.
3. Register in `app-core::init::cognitive` as a struct field.

### Add a Reforge hook

1. Define the trait in `services/reforge/mod.rs`.
2. Add `Option<&dyn YourTrait>` parameter to `run_reforge`.
3. Implement in `app-core::adapters` (to avoid circular deps).
4. Pass to `run_reforge` from `app-core::init::cognitive`.

### Add a mirror signal source

1. Create `mirror/sources/my_source.rs` implementing `MirrorSignalSource`.
2. Register in `MirrorEngine::start` — unconditional or behind `Option<&Repo>` parameters.
3. Implement `accumulate` + `flush`. The flush gets its own tokio task; handle returned in `StartedMirror.flush_handles`.

### Add a `RecallDomain` variant (ai-core)

```rust
// In crates/ai-core/src/recall_domain.rs
pub enum RecallDomain {
    General, Tasks, Finance, Productivity, Learning,
    Mirror, Coaching, Notes, LanguageLearning,
    MyDomain,                        // ← add here
}
```

Update `RecallProviderRegistry` consumers. New variants are forward-compatible (no schema migration needed).

### Add a cognitive migration

Append to `cognitive_migrations()` in `repos/mod.rs`. Pre-1.0: edit existing migrations in place. Post-1.0: add a new migration file with `INSERT OR IGNORE` for idempotency.

---

## Key constants

| Constant | Value | Location |
|---|---|---|
| `DEFAULT_WEIGHTS` (FSRS-5) | `[19 f64 values]` | `services/fsrs5.rs` |
| `RelevanceWeights` (12 factors) | per-deployment tuning | `services/decay.rs` |
| Reforge T (temperature) | `0.2` | `services/reforge/service.rs` |
| Reforge max_tokens | `4096` | `services/reforge/service.rs` |
| `cost_ceiling_usd` (per-cycle) | per-config | `services/reforge/service.rs` |
| PPR teleport probability | `0.15` (typical) | `services/ppr_retrieval.rs` |
| PPR iterations | `50` (typical) | `services/ppr_retrieval.rs` |

---

## Open questions

- **`run_reforge` 26-parameter signature.** Refactor to `ReforgeContext` builder.
- **`service.rs:1` doc comment says "8 phases"** — actual is 16. Update.
- **Two `AutotunerBridge` traits with same name.** Rename one (e.g., `ReforgeAutotunerBridge` + `MirrorAutotunerBridge`).
- **Two `retrievability` functions with same name + different formulas.** Rename or co-locate.
- **`MirrorEngine::start` no longer takes the bus** — CLAUDE.md says it does. Update CLAUDE.md.
- **`SkillEffectivenessSource` is a stub** (`TODO(T7)`). Implement.
- **`strategy_records` is raw-SQL accessed.** Add a typed `StrategyRecordsRepo`.
- **`KCA_COMMUNITY_SUMMARIES` and `KCA_REFORGE_COMPRESS` are env-only.** Migrate to config once LLM versions land ("Phase C").
- **Procedural memory storage is live but the higher-level feature isn't** — surface clearly in subsystem doc and feature roadmap.

See [`TECH_DEBT.md`](../TECH_DEBT.md) categories #2 + #5 + #7 + #8.

---

## Cross-references

- [Subsystem 05 — Cognitive Memory & Self-Improvement](../subsystems/05-cognitive-memory.md) (parent)
- [`crates/storage.md`](./storage.md) — all cognitive repos
- [`crates/providers.md`](./providers.md) — `create_cognitive_provider(ProviderRole)` for reforge
- [`crates/agent.md`](./agent.md) — `MemoryScorer` + KCA env flags consumed
- [`crates/context_engine.md`](./context_engine.md) — `MemoryRetriever` + `MemoryScorer` impls live here
- [`crates/coding-memory.md`](./coding-memory.md) — coding-side Reforge phases plug into the hooks
- [`crates/app-core.md`](./app-core.md) — Phase 5 of init constructs cognitive services + Phase 10 wires coding-memory
