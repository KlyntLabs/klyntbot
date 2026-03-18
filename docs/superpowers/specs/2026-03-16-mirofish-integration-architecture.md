# MiroFish-Inspired Platform Architecture — System Design (v2)

**Date:** 2026-03-16
**Status:** Approved
**Scope:** System-wide AI intelligence upgrades inspired by MiroFish's multi-agent simulation platform. Covers context engine, agent runtime, cognitive memory, and all feature packages.

**Key principle:** These are **platform-level capabilities**, not feature-specific. Upgrading the context engine improves tasks, finance, notes, productivity, and every future feature — simultaneously.

**Related specs:**
- `2026-03-16-insight-review-design.md` — Notes Insight Review (consumer of this architecture)

---

## 1. Vision

MiroFish builds digital worlds from documents, simulates thousands of agents, and lets users interview the results. We don't need social media simulation — but we need the **thinking patterns** that make it powerful:

| MiroFish Pattern | Klynt Adaptation | Layer | Phase |
|---|---|---|---|
| InsightForge 3-tier search | Multi-dimensional context retrieval | Context Engine (L3) | 1 |
| Auto-ontology generation | Unified entity knowledge graph | Cognitive Memory (L5) | 0 |
| Temporal fact tracking | Memory evolution & contradiction detection | Cognitive Memory (L5) | 3 |
| Multi-agent perspectives | Pluggable analysis personas (shared registry) | Agent Runtime (L5) | 0 (internal), 4 (explicit) |
| ReACT report generation | Reports as skill in existing ReactiveEngine | Skill System (L3) | 5 |
| Scenario simulation | Generalized what-if using graph neighborhoods | Cross-Feature | 5 |

**What this unlocks:** An AI agent that doesn't just respond to queries, but **deeply understands connections**, **tracks how understanding evolves**, **reasons from multiple perspectives**, and **closes the loop** between analysis and action.

---

## 2. Unified Knowledge Graph (Option A — Single Source of Truth)

### Decision

Since we haven't released yet, we build a clean, unified entity system from day one. The `entities` + `entity_relationships` tables become the **single source of truth** for all entity awareness. No three separate entity layers.

### Schema

```sql
-- Entities: people, projects, technologies, concepts, etc.
CREATE TABLE IF NOT EXISTS entities (
    id TEXT PRIMARY KEY,
    name TEXT NOT NULL,
    entity_type TEXT NOT NULL,  -- person, project, technology, concept, place, organization
    description TEXT,
    source TEXT NOT NULL,       -- extracted, user_created, imported, backfill
    source_id TEXT,             -- note_id, task_id, session_key — where first seen
    first_seen_at TEXT NOT NULL,
    last_seen_at TEXT NOT NULL,
    mention_count INTEGER NOT NULL DEFAULT 1,
    metadata JSON,
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL
);

CREATE INDEX idx_entities_type ON entities(entity_type);
CREATE INDEX idx_entities_name ON entities(name);
-- FTS5 for fuzzy entity name search (SQLite-native, replaces pg_trgm)
CREATE VIRTUAL TABLE IF NOT EXISTS entities_fts USING fts5(name, description, tokenize='porter unicode61');

-- Relationships between entities (temporal from day one)
CREATE TABLE IF NOT EXISTS entity_relationships (
    id TEXT PRIMARY KEY,
    source_entity_id TEXT NOT NULL REFERENCES entities(id),
    target_entity_id TEXT NOT NULL REFERENCES entities(id),
    relationship_type TEXT NOT NULL,  -- works_on, uses, depends_on, knows, references, etc.
    strength REAL NOT NULL DEFAULT 0.5,  -- 0.0-1.0, incremented on co-occurrence
    evidence TEXT,               -- brief text explaining why this relationship exists
    valid_from TEXT,             -- temporal: when this relationship started
    valid_until TEXT,            -- NULL = still active
    source TEXT NOT NULL,        -- extracted, inferred, user_stated
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL
);

CREATE INDEX idx_relationships_source ON entity_relationships(source_entity_id);
CREATE INDEX idx_relationships_target ON entity_relationships(target_entity_id);
CREATE INDEX idx_relationships_type ON entity_relationships(relationship_type);
-- FTS5 triggers auto-sync for entities (same pattern as semantic_facts_fts)
```

### Entity Lifecycle (Three Unified Sources)

```
Source 1: Conversations
  DomainEvent → BackgroundConsolidationService → extract facts + entities (ONE LLM call)
  → entities table + entity_relationships table + semantic_facts table

Source 2: Notes
  Note save → extract_links_and_mentions() → note_entity_mentions (write-through, keeps note queries working)
  → ALSO upsert into entities + create "references" edges

Source 3: Task/Finance/Productivity events
  TaskCompleted, TransactionRecorded, FocusSessionEnded
  → heuristic entity extraction from structured event data (zero LLM cost)
  → entities + relationships
```

### Extraction Strategy

Piggyback on the existing `BackgroundConsolidationService` pipeline. The extraction prompt is extended to return entities + relationships **alongside** SPO facts — one LLM call, not two:

```
Extract from this observation:
1. Facts (subject-predicate-object triples) — as before
2. Entities (name, type: person|project|technology|concept|place|organization)
3. Relationships (source_entity → relationship_type → target_entity, with evidence)

Return combined JSON:
{
  "facts": [...],
  "entities": [{"name": "...", "type": "..."}],
  "relationships": [{"source": "...", "target": "...", "type": "...", "evidence": "..."}]
}
```

Heuristic fallback (no LLM needed): structured events like `TaskCompleted` create entities directly from the event data (task title → entity, project → entity, task→project → relationship).

### Entity Deduplication

**In-pipeline** during extraction:
1. New entity extracted → fuzzy-match against existing entities (Levenshtein + embedding similarity via existing fastembed)
2. Confidence ≥ 0.88 → auto-merge
3. Confidence 0.5–0.88 → queue for user confirmation
4. Confidence < 0.5 → create new entity

**User confirmation** (conversational, no separate UI):
> Agent: "I think 'John' and 'your manager John Smith' are the same person — correct?"

### Relationship Strength Evolution

On every co-occurrence (same entities mentioned together):
```rust
new_strength = (current_strength + 0.1).min(1.0);
```

**Decay:** On app startup (or weekly cron), apply time-based decay to all relationship strengths:
```rust
// Decay relationships not seen recently (half-life: 60 days)
let days_since_update = (now - relationship.updated_at).num_days() as f64;
let decay = 0.5_f64.powf(days_since_update / 60.0);
new_strength = (relationship.strength * decay).max(0.05);  // Floor at 0.05 (never fully forget)
```

This prevents stale relationships from permanently dominating graph queries. Frequently reinforced edges stay strong; abandoned ones fade.

### Backfill (One-Time Phase 0 Migration)

**Step 1: Entities from SPO facts.** Extract unique subjects (excluding "user") and infer type from predicate patterns. Case-normalize names to prevent duplicates:

```sql
-- Convert existing SPO facts where subject looks like an entity
INSERT INTO entities (id, name, entity_type, source, first_seen_at, last_seen_at, created_at, updated_at)
  SELECT
    lower(hex(randomblob(16))),
    TRIM(subject),
    CASE
      WHEN predicate LIKE '%works_on%' OR predicate LIKE '%project%' THEN 'project'
      WHEN predicate LIKE '%uses%' OR predicate LIKE '%tool%' THEN 'technology'
      WHEN predicate LIKE '%knows%' OR predicate LIKE '%person%' THEN 'person'
      ELSE 'concept'
    END,
    'backfill',
    MIN(created_at), MAX(created_at),
    MIN(created_at), MIN(created_at)
  FROM semantic_facts
  WHERE subject != 'user'
  GROUP BY LOWER(TRIM(subject));
```

**Step 2: Entities from note_entity_mentions.** `entity_id` in this table is an opaque ID (task UUID, project slug), not a human-readable name. We must JOIN against the source tables to resolve names:

```sql
-- Create entities from task references
INSERT OR IGNORE INTO entities (id, name, entity_type, source, source_id, first_seen_at, last_seen_at, created_at, updated_at)
  SELECT
    lower(hex(randomblob(16))),
    t.title,
    'task',
    'backfill',
    nem.entity_id,
    t.created_at, t.created_at,
    t.created_at, t.created_at
  FROM note_entity_mentions nem
  JOIN tasks t ON nem.entity_id = t.id
  WHERE nem.entity_type = 'task';

-- Create entities from project references
INSERT OR IGNORE INTO entities (id, name, entity_type, source, source_id, first_seen_at, last_seen_at, created_at, updated_at)
  SELECT
    lower(hex(randomblob(16))),
    p.title,
    'project',
    'backfill',
    nem.entity_id,
    p.created_at, p.created_at,
    p.created_at, p.created_at
  FROM note_entity_mentions nem
  JOIN projects p ON nem.entity_id = p.id
  WHERE nem.entity_type = 'project';

-- Create "references" relationships (note → entity)
INSERT INTO entity_relationships (id, source_entity_id, target_entity_id, relationship_type, strength, source, valid_from, created_at, updated_at)
  SELECT
    lower(hex(randomblob(16))),
    note_ent.id,
    target_ent.id,
    'references',
    0.5,
    'backfill',
    n.created_at,
    n.created_at, n.created_at
  FROM note_entity_mentions nem
  JOIN notes n ON nem.note_id = n.id
  JOIN entities note_ent ON note_ent.source_id = n.id AND note_ent.entity_type = 'concept'
  JOIN entities target_ent ON target_ent.source_id = nem.entity_id;
```

Also links `project_id` where possible (from `semantic_facts.project_id` or `notes.project_id`).

**Note:** The backfill script runs once during Phase 0 migration. For entities where source tables don't exist (e.g., area references), they are created as `concept` type with the raw `entity_id` as `source_id` for later manual review.

### EntityRepo API

```rust
impl EntityRepo {
    pub async fn find_by_name(&self, name: &str) -> Result<Vec<EntityRow>>;
    pub async fn get_neighborhood(&self, entity_id: &str, depth: u8) -> Result<GraphNeighborhood>;
    pub async fn find_path(&self, from: &str, to: &str, max_depth: u8) -> Result<Vec<GraphEdge>>;
    pub async fn get_related_entities(&self, entity_id: &str, rel_type: Option<&str>) -> Result<Vec<EntityRow>>;
    pub async fn merge_entities(&self, keep_id: &str, merge_id: &str) -> Result<()>;
    pub async fn get_entities_for_note(&self, note_id: &str) -> Result<Vec<EntityRow>>;
    pub async fn upsert_entity(&self, entity: NewEntity) -> Result<EntityRow>;
    pub async fn upsert_relationship(&self, rel: NewRelationship) -> Result<()>;
}
```

### What This Unlocks in Conversations

User: *"Help me plan the API migration"*
→ InsightForge decomposes + pulls graph neighborhood for "API migration" entity
→ Finds connected: 3 team members, 2 dependent projects, a budget risk, 5 related notes
→ Agent response references all of this naturally: *"The API migration connects to your Q2 OKR, depends on the auth team (who you met with last Tuesday), and has a $5K budget allocation. Want me to simulate what happens if we push it back 2 weeks?"*

---

## 3. InsightForge Context Assembly

### The Problem Today

The current `ContextEngine` retrieves memory via a single query:
```
User message → MemoryRetriever.retrieve(query, limit=5) → 5 facts injected
```
This misses connections. A message about "distributed systems" only finds facts about "distributed systems" — not the related facts about "consensus", "CAP theorem", or "microservice failures."

### Architecture

```
User message arrives
       ↓
  Should decompose?  ──no──→  Single-query retrieval (existing path, unchanged)
       │ yes
       ↓
  Heuristic Decomposer (0ms, free)
       │
       ├── Produced ≥ 3 sub-queries? ──yes──→ Skip LLM
       │ no
       ↓
  Cheap LLM Decomposer (Gemini Flash, 2s timeout, circuit breaker)
       │ fail
       ↓
  Fallback: single-query retrieval (identical to today)
       │ success
       ↓
  Parallel Multi-Source Search (per sub-query, per source)
       ├── MemoryRetriever (cognitive facts + conversation recall)
       ├── NoteSearcher (feature-notes embeddings)
       ├── TaskSearcher (feature-tasks embeddings + keyword)
       ├── FinanceSearcher (feature-finance tx_search + budgets)
       └── GraphSearcher (EntityRepo::get_neighborhood + find_by_name)
       ↓
  RRF Merge (reuse UnifiedMemoryService algorithm, k=60)
       ↓
  Deduplicate by ID → Re-normalize 0.0-1.0 → BudgetAllocator token cap
       ↓
  Enriched context block → injected into every LLM call
```

### Decomposition Gate

Reuses the existing 4-layer intent analyzer — **zero new classification cost**:

| Condition | Decompose? |
|---|---|
| Heuristic Layer 1 matched, Direct mode, confidence ≥ 0.85 | **No** |
| Complexity score ≥ 3 | **Yes** |
| Planning/analysis keywords ("plan", "analyze", "review", "compare") | **Yes** |
| Insight Review or Report generation | **Always** |
| Message < 20 chars OR Clarification mode | **No** |

### Heuristic Decomposer (Always Runs First, Free)

1. Extract noun phrases + domain keywords from the message
2. Cross-reference with entity names from the knowledge graph (cheap `find_by_name` query)
3. Generate sub-queries by combining: `[topic] + [related domain]`, `[topic] + [temporal context]`, `[topic] + [connected entities from graph]`
4. If ≥ 3 meaningful sub-queries → skip the LLM call entirely

### Model Strategy & Fallback Chain

```
1. Heuristic decomposer (0ms, zero cost)
   → works for 60-70% of decomposable queries

2. Cheap LLM (if heuristic insufficient)
   → Config: contextEngine.insightForge.model
   → Default: gemini-2.0-flash ($0.10/MTok input)
   → Timeout: 2 seconds (hard)
   → Circuit breaker: 3 failures per session → skip for 5 minutes

3. Fallback: single-query retrieval
   → Identical to today's behavior
   → User never sees degradation
```

### DomainSearcher Trait (Pluggable Feature Search)

Lives in `context_engine` (L3). Feature crates implement it, injected at startup in `app-core`:

```rust
#[async_trait]
pub trait DomainSearcher: Send + Sync {
    fn domain_name(&self) -> &str;  // "notes", "tasks", "finance", "graph"
    async fn search(&self, query: &str, limit: usize) -> Vec<MemoryEntry>;
}
```

Each searcher is wrapped in its own 800ms timeout via `futures::future::join_all` + error isolation. One slow searcher never blocks others.

| Feature Crate | Searcher | Wraps |
|---|---|---|
| `feature-notes` | `NoteSearcher` | Existing embedding search in `note_suggestions` |
| `feature-tasks` | `TaskSearcher` | Existing embedding search + keyword in `TaskTool::search` |
| `feature-finance` | `FinanceSearcher` | Existing `tx_search` + budget status queries |
| `cognitive` | `GraphSearcher` | `EntityRepo::get_neighborhood()` + `find_by_name()` |

Each searcher converts domain objects into `MemoryEntry`:
- `content`: human-readable summary (e.g., "Task: Fix auth bug (due: Mar 20, high priority)")
- `score`: relevance score normalized to 0.0–1.0
- `source`: `MemorySource::Domain { name: "tasks" }`

### Cross-Query RRF Merge

Same algorithm already used in `UnifiedMemoryService`:
```
rrf_score = Σ (1 / (k + rank_in_subquery + 1))  where k=60
```
Items found by 3/5 sub-queries rank dramatically higher. After merge: deduplicate by ID, re-normalize, apply token budget via existing `BudgetAllocator`.

### Integration Point

```rust
// In ContextEngine builder — add alongside existing with_memory_retriever():
pub fn with_insight_forge(mut self, forge: InsightForge) -> Self {
    self.insight_forge = Some(forge);
    self
}

// Also add domain searcher registration:
pub fn with_domain_searchers(mut self, searchers: Vec<Box<dyn DomainSearcher>>) -> Self {
    if let Some(ref mut forge) = self.insight_forge {
        for s in searchers {
            forge.add_searcher(s);
        }
    }
    self
}

// In assemble_uncached() — replaces the memory retrieval block
// (the self.retrieve_memories() call inside the method, NOT by line number):
let memories = match &self.insight_forge {
    Some(forge) if forge.should_activate(&strategy, &message_text) => {
        forge.retrieve(&message_text, self.memory_retrieval_limit).await
    }
    _ => {
        // Existing single-query path (unchanged)
        self.retrieve_memories(&message_text).await
    }
};
```

**Plumbing path:** `app-core` creates the `DomainSearcher` instances from feature crates (L4) and passes them into `ContextEngine::builder().with_domain_searchers(vec![...])` during app initialization. This follows the same dependency inversion pattern used for `MemoryRetriever` (trait in L3, impl in L5, injected from L7).

### Latency & Cost

| Step | Time | Cost |
|---|---|---|
| Heuristic decomposition | <5ms | Free |
| Cheap LLM decomposition (if needed) | ~500-800ms | ~$0.001 |
| Parallel multi-source search (5 queries × 5 sources) | ~100-200ms | Free (local DB) |
| RRF merge + dedup | <5ms | Free |
| **Total added latency** | **~200ms (heuristic) to ~1s (LLM)** | **~$0.001** |

### Configuration

```json
{
  "contextEngine": {
    "insightForge": {
      "enabled": true,
      "model": "gemini-2.0-flash",
      "timeoutMs": 2000,
      "maxSubQueries": 5,
      "perSourceLimit": 5,
      "perSourceTimeoutMs": 800,
      "circuitBreakerThreshold": 3,
      "circuitBreakerCooldownSecs": 300
    },
    "temporalWeight": 0.05
  }
}
```

All config keys use camelCase per project convention (`#[serde(rename_all = "camelCase")]`).

### Where It Lives

**Crate:** `context_engine` (L3)
**New files:**
- `src/insight_forge/mod.rs` — `InsightForge` struct, `should_activate()`, `retrieve()`
- `src/insight_forge/decomposer.rs` — `QueryDecomposer` trait, `HeuristicDecomposer`, `LlmDecomposer`
- `src/insight_forge/domain_searcher.rs` — `DomainSearcher` trait
- `src/insight_forge/circuit_breaker.rs` — per-session circuit breaker

---

## 4. Temporal Intelligence

### What Changes

**Today:** Consolidation silently supersedes old facts. Archive buries them. Retrieval only sees active facts. The user's knowledge evolution is invisible.

**After:** The system tracks *how* facts change, surfaces contradictions conversationally, and can answer "what changed this week?"

### TemporalService

**Crate:** `cognitive` (L5)
**File:** `src/services/temporal.rs`
**Injection:** Via `app-core` builder + `FeaturePackage` pattern (same as other cognitive services)

```rust
pub struct TemporalService {
    fact_repo: SemanticFactRepo,  // Also handles archive ops (archive_superseded, search_archived, reinstate_archived)
    entity_repo: EntityRepo,
}
```

**Note:** There is no separate `ArchiveRepo` — archive operations are methods on `SemanticFactRepo` itself (`archive_superseded`, `search_archived`, `reinstate_archived`). `TemporalService` uses these existing methods to query both active facts and archived facts.

#### Capability 1: Fact History Queries

```rust
pub async fn get_fact_history(
    &self,
    subject: &str,
    predicate: &str,
) -> Result<Vec<FactVersion>>
```

Queries both `semantic_facts` AND `semantic_facts_archive` for matching `subject+predicate`, ordered by `valid_from DESC`. Returns the full chain: current → superseded → archived.

#### Capability 2: Contradiction Detection (In-Pipeline)

Integrated into `BackgroundConsolidationService` **after** `execute_memory_ops()` completes — not inside `decide_batch()` (which is a trait method with no bus access):

```rust
// In BackgroundConsolidationService, after execute_memory_ops():
for op in &ops {
    if let MemoryOp::Update { old, new } = op {
        if old.object != new.object
            && old.confidence >= 0.7
            && old.source == "user_stated"
            && !is_same_session(&old.recorded_at, &current_session_start)
        {
            self.bus.publish(DomainEvent::ContradictionDetected {
                existing: old.clone(),
                new: new.clone(),
            });
        }
    }
}
```

**Prerequisites:** Add `ContradictionDetected { existing: SemanticFactRow, new: ExtractedFact }` variant to the `DomainEvent` enum in `crates/bus/src/domain_events.rs`. The agent runtime subscribes to this event and surfaces it conversationally.

**When NOT to surface:** Low-confidence facts (< 0.7), non-user-stated facts, same-session updates (compare `recorded_at` vs session start), trivial predicates (timestamps, counts).

#### Capability 3: Change Summaries

```rust
pub async fn change_summary(
    &self,
    since: DateTime<Utc>,
    domains: Option<Vec<String>>,
) -> Result<ChangeSummary>

pub struct ChangeSummary {
    pub period: (DateTime<Utc>, DateTime<Utc>),
    pub new_facts: Vec<SemanticFactRow>,
    pub updated_facts: Vec<(SemanticFactRow, SemanticFactRow)>,  // (old, new)
    pub superseded_facts: Vec<SemanticFactRow>,
    pub by_domain: HashMap<String, usize>,
}
```

No LLM call needed — structured data that the agent or report skill can narrate.

### Temporal Backfill Migration (Phase 0)

**No SQL backfill needed.** The existing `semantic_facts` and `semantic_facts_archive` schemas already declare `valid_from TEXT NOT NULL` — the column is fully populated for all rows. The temporal scoring weight (Phase 3) will work immediately with existing data.

The only Phase 0 action for temporal: ensure the new `entity_relationships` table has `valid_from` populated during extraction (already included in the schema above).

### Temporal Scoring in Retrieval

Adjustment to the existing 5-factor scoring formula (steal 0.05 from situational_boost):

```rust
// Updated weights:
// semantic_sim * 0.30 + retrievability * 0.20 + confidence * 0.15
// + access_frequency * 0.10 + situational_boost * 0.20 + temporal * 0.05

let temporal_score = match fact.valid_from {
    Some(vf) => {
        let age_days = (now - vf).num_days() as f64;
        (1.0 / (1.0 + age_days / 30.0)).max(0.1)
    }
    None => 0.5  // Unknown validity → neutral
};
```

Configurable via `contextEngine.temporal_weight` (default: 0.05). Small weight intentionally — temporal is a tiebreaker, not a dominant factor.

### Integration with Entity Relationships

`entity_relationships` has `valid_from`/`valid_until` from day one. Graph neighborhood queries can optionally filter by time window:

```rust
pub async fn get_neighborhood(
    &self,
    entity_id: &str,
    depth: u8,
    time_window: Option<(DateTime<Utc>, DateTime<Utc>)>,
) -> Result<GraphNeighborhood>
```

Useful for reports: "What was I working on in Q1?"

### How This Surfaces in Conversations

**Contradiction:**
> User: "I think we should use PostgreSQL for the new service"
> Agent: "Noted. Last month you mentioned preferring DynamoDB for new services because of operational simplicity. Has your thinking changed, or is this project different?"

**Weekly review:**
> Agent: "This week: 12 new facts learned, 3 opinions updated, 2 project relationships changed."

**Temporal context:**
> User: "How's the API migration going?"
> Agent: "You started planning this on Feb 15. On Mar 1 it was blocked by the auth team. Last week the blocker was resolved..."

---

## 5. Multi-Perspective Analysis

### Mode 1: Internal (Automatic, Zero Cost) — Phase 0

For complexity score ≥ 4 AND decision-oriented requests (not action-oriented), modify the existing planning prompt in `runtime.rs:899-914`:

```
Updated planning prompt addition:
"Before executing, briefly consider the optimistic, skeptical, and practical angles.
Synthesize into a balanced approach, then create your step-by-step plan."
```

One line change. No new calls, no new module, no cost. The LLM naturally produces more balanced plans.

**Gate:** Only for decision-oriented requests. "Should I migrate?" → yes. "Migrate this database" → no. Distinguished by: (1) absence of imperative action keywords in the heuristic layer, and (2) the `ComplexitySignals.estimated_tool_calls` field — if tool calls ≥ 2 and no explicit action keywords, it's likely a decision/analysis request. If the intent analyzer's `has_sequential_deps` flag is available (from `ComplexitySignals`), use it as an additional signal — sequential deps suggest action-oriented execution, not decision-making.

### Mode 2: Explicit (On Request) — Phase 4

When the user asks for analysis/advice, the general orchestrator detects it (existing keyword matching for "analyze", "trade-offs", "compare", "advice") and injects persona context:

```
The user is asking for analysis. Generate perspectives from these viewpoints:
{selected_personas_from_registry}
For each: 2-3 sentences of focused analysis, then synthesize a recommendation.
```

Persona selection: domain-match against knowledge graph entities in the current query, then select from `PersonaRepo`. Zero extra cost — just system prompt injection.

### Shared Persona Registry

Lives in `cognitive` (L5), shared across the platform:

**File:** `crates/cognitive/src/repos/persona.rs`

Used by:
- Notes Insight Review Tab 5 (Perspectives)
- Agent runtime for explicit analysis
- Future: Productivity coaching personas

6 builtin personas ship with the app (seeded in migration):

| Name | Role | Tone |
|---|---|---|
| The Skeptic | Critical Analyst | skeptical |
| The Practitioner | Industry Professional | pragmatic |
| The Connector | Interdisciplinary Researcher | curious |
| The Student | Curious Learner | inquisitive |
| The Strategist | Systems Thinker | analytical |
| The Devil's Advocate | Contrarian Thinker | provocative |

Schema and full API defined in `2026-03-16-insight-review-design.md` §3.

### Configuration

```json
{
  "perspectives": {
    "internalEnabled": true,
    "defaultPersonaIds": ["builtin-skeptic", "builtin-practitioner", "builtin-strategist"]
  }
}
```

The `defaultPersonaIds` array references persona `id` values (not tone strings). These are the IDs seeded during migration for the 6 builtin personas. When `PersonaRepo::select_for_note()` has no pinned personas and no domain matches, it falls back to these IDs.

---

## 6. Reports as Skill

### Decision

No separate `ReportEngine`. Reports are a **skill configuration** inside the existing ReactiveEngine — zero duplicate code.

### Implementation

**New reference skill:** `skills/task-management/references/reports.md`

Contains templates per report type. When the skill router detects a report trigger, the orchestrator loads the reference, injects the outline as a planning prompt, and the existing ReactiveEngine executes tool calls and synthesizes each section.

Report triggers treated as a special case of "planning/analysis" keywords in the skill router — the intent analyzer's complexity score decides full report vs lightweight summary.

### Report Templates

#### Weekly Review
**Triggers:** "weekly review", "what happened this week", "week summary"
**Tool queries per section:**
1. Accomplishments → `tasks list` (completed this week)
2. In Progress → `tasks list` (status: in_progress)
3. Blockers & Overdue → `tasks list` (overdue or blocked)
4. Patterns → `productivity activity_summary`, focus sessions
5. Knowledge Growth → notes created/modified, `TemporalService::change_summary(7 days)`
6. Next Week → tasks due next week, calendar upcoming

#### Project Retrospective
**Triggers:** "retrospective for {project}", "how did {project} go"
**Tool queries:** tasks by project, OKR progress, time entries, cognitive facts about project, graph neighborhood

#### Financial Health
**Triggers:** "financial health", "money review", "spending report"
**Tool queries:** `finance net_worth`, `report_spending`, `report_income`, `budget_status`, `goal_list`

#### Knowledge Growth
**Triggers:** "what did I learn", "knowledge review"
**Tool queries:** notes created/modified, Insight Review cache history, flashcard performance, `TemporalService::change_summary()`

### Streaming

Each section completes as the ReAct loop progresses. The frontend already shows tool call results incrementally via `PipelineEvent`. No new streaming infrastructure.

### Cron Integration

The automation orchestrator schedules reports via existing cron:
```
"Every Sunday at 6pm, generate my weekly review and send to Telegram"
```
Already works — cron triggers the agent with the prompt.

---

## 7. Scenario Reasoning (Generalized What-If)

### Decision

Not a new tool or engine. Generalize the existing finance `goal_whatif` pattern into a reasoning template using graph neighborhoods.

### Detection

Add "what if", "what happens if", "what would happen" to the planning/analysis keyword list in the intent analyzer. When detected + complexity ≥ 3, the planning prompt includes:

```
The user is exploring a scenario. Structure your response:
1. Identify the change variable and current baseline (use tools to get real data)
2. Trace first-order effects (what changes directly)
3. Trace second-order effects (use knowledge graph neighborhoods, max depth 2)
4. Present best/worst/likely outcomes with specific numbers where possible
5. Synthesize a recommendation
```

### Graph Integration

"What if I deprioritize Project X?" triggers:
1. Agent queries `EntityRepo::get_neighborhood("Project X", depth=2)`
2. Finds: 3 dependent tasks, 2 team members, 1 OKR, 1 budget allocation
3. For each connected entity, queries the relevant tool (tasks, OKR, finance)
4. Synthesizes cascading effects with real data

### Feature-Specific Scenarios

| Feature | Example | Mechanism |
|---|---|---|
| **Tasks** | "What if I push the deadline back 2 weeks?" | Graph: dependent tasks + OKR impact |
| **Finance** | "What if I increase savings by $500/month?" | Existing `goal_whatif` + `fire_sensitivity` + graph context |
| **Productivity** | "What if I block 2 hours for deep work daily?" | Projected score from `ProductivityPatternAnalyzer` |
| **Notes** | "What if this assumption is wrong?" | Perspectives mode: counter-argument generation |

### Configuration

```json
{
  "scenario": {
    "max_graph_depth": 2
  }
}
```

---

## 8. Graceful Degradation (Mandatory)

Every new component has a defined failure mode. The user never sees degradation — the system silently falls back to today's behavior.

### Degradation Table

| Component | Failure | Degradation | User Impact |
|---|---|---|---|
| InsightForge decomposition | LLM timeout / bad output | Single-query retrieval | None — same as today |
| Heuristic decomposer | No meaningful sub-queries | Skip decomposition | None |
| DomainSearcher (any) | Per-searcher 800ms timeout | Other searchers return results | Slightly less context from that domain |
| Entity extraction | LLM fails | Heuristic extraction (existing fallback) | SPO facts still created, entities skipped |
| Entity dedup | Fuzzy match fails | Create separate entity, merge later | Minor duplicate, auto-resolved |
| Contradiction detection | LLM unavailable | Skip check, proceed with Update | Silent |
| Temporal queries | Archive unavailable | Return only active facts | Partial history |
| Graph queries | Entity not found / empty | Fall back to flat semantic facts | Same as today |
| Persona selection | No domain-relevant personas | Use 3 builtin defaults | Less domain-specific |
| Report skill | Tool call fails mid-report | Skip section, note in output | "Couldn't fetch budget data" |
| Scenario reasoning | Graph neighborhood empty | Direct tool queries only | Simpler analysis |

### Implementation Pattern

```rust
let result = tokio::time::timeout(
    Duration::from_millis(config.timeout_ms),
    component.execute(input)
).await;

match result {
    Ok(Ok(data)) => data,
    Ok(Err(e)) => {
        tracing::warn!(session_key = %key, component = "insight_forge", reason = %e, "Using fallback");
        fallback_value
    }
    Err(_timeout) => {
        tracing::warn!(session_key = %key, component = "insight_forge", "Timed out, using fallback");
        circuit_breaker.record_failure(session_key);
        fallback_value
    }
}
```

### Circuit Breaker (Per-Session)

- 3 failures within 5 minutes → component disabled for that session for 5 minutes
- Uses `DashMap<SessionKey, (failure_count, last_failure_time)>`
- On cooldown expiry, reset and try again
- Global disable via config: `"insightForge": { "enabled": false }`
- Structured `tracing::warn!` with session_key + component + fallback reason for auto-tuning

---

## 9. Architecture Diagram

```
┌─────────────────────────────────────────────────────────────────┐
│                      FRONTEND (desktop-ui)                       │
│  ┌──────────┐ ┌──────────┐ ┌──────────┐ ┌───────────────────┐  │
│  │ Chat UI  │ │ Task UI  │ │Finance UI│ │ Notes + Insight   │  │
│  │          │ │          │ │          │ │ Review Panel      │  │
│  └────┬─────┘ └────┬─────┘ └────┬─────┘ └────────┬──────────┘  │
│       │Tauri IPC    │            │                 │             │
└───────┼─────────────┼────────────┼─────────────────┼─────────────┘
        ▼             ▼            ▼                 ▼
┌─────────────────────────────────────────────────────────────────┐
│                    APP-CORE (L7) — Handlers                      │
│  ┌─────────────────────────────────────────────────────────────┐ │
│  │  chat → agent    tasks → feature-tasks    notes → insight   │ │
│  └─────────────────────────────────────────────────────────────┘ │
└─────────────────────────┬───────────────────────────────────────┘
                          ▼
┌─────────────────────────────────────────────────────────────────┐
│                   AGENT RUNTIME (L5)                             │
│                                                                  │
│  ┌──────────────┐  ┌──────────────┐  ┌────────────────────────┐ │
│  │ Skill Router │→ │Intent Analyzer│→ │  Execution Router      │ │
│  │ (5 skills)   │  │ (4 layers)   │  │  ┌────────┐ ┌────────┐ │ │
│  └──────────────┘  └──────────────┘  │  │Direct  │ │Reactive│ │ │
│                                       │  │Engine  │ │Engine  │ │ │
│  Multi-Perspective                    │  └────────┘ └────────┘ │ │
│  (planning prompt)                    │  Reports = skill       │ │
│                                       │  Scenarios = template  │ │
│  Shared PersonaRepo ←────────────────│──────────────────────── │ │
│                                       └────────────────────────┘ │
└─────────────────────────┬───────────────────────────────────────┘
                          ▼
┌─────────────────────────────────────────────────────────────────┐
│                  CONTEXT ENGINE (L3) ★ KEY UPGRADE ★             │
│                                                                  │
│  ┌──────────────────────────────────────────────────────────┐   │
│  │              InsightForge                                 │   │
│  │  ┌────────────┐    ┌──────────────────────────────────┐  │   │
│  │  │Heuristic   │───→│  Parallel Multi-Source Retrieval  │  │   │
│  │  │Decomposer  │    │  ┌────────┐ ┌──────┐ ┌────────┐ │  │   │
│  │  │  + LLM     │    │  │Memory  │ │Notes │ │Tasks   │ │  │   │
│  │  │  fallback  │    │  │Retriever│ │Search│ │Search  │ │  │   │
│  │  └────────────┘    │  └────────┘ └──────┘ └────────┘ │  │   │
│  │                     │  ┌────────┐ ┌──────────────────┐ │  │   │
│  │  Circuit Breaker    │  │Finance │ │Graph Searcher    │ │  │   │
│  │  (per-session)      │  │Search  │ │(neighborhoods)   │ │  │   │
│  │                     │  └────────┘ └──────────────────┘ │  │   │
│  │                     └──────────────────────────────────┘  │   │
│  └──────────────────────────────────────────────────────────┘   │
│                                                                  │
│  ┌────────────┐ ┌──────────────┐ ┌──────────────┐ ┌──────────┐ │
│  │Budget      │ │History       │ │Context       │ │Deferred  │ │
│  │Allocator   │ │Compressor    │ │Sources       │ │Inventory │ │
│  └────────────┘ └──────────────┘ └──────────────┘ └──────────┘ │
└─────────────────────────┬───────────────────────────────────────┘
                          ▼
┌─────────────────────────────────────────────────────────────────┐
│                  COGNITIVE MEMORY (L5) ★ UNIFIED ★               │
│                                                                  │
│  ┌────────────────┐ ┌──────────────┐ ┌────────────────────────┐ │
│  │ Semantic Facts │ │  Episodic    │ │  Procedural Rules      │ │
│  │ (SPO triples) │ │  Memories    │ │  (learned patterns)    │ │
│  └────────┬───────┘ └──────────────┘ └────────────────────────┘ │
│           │                                                      │
│  ┌────────▼───────┐ ┌──────────────┐ ┌────────────────────────┐ │
│  │ Unified        │ │  Temporal    │ │  Persona Registry      │ │
│  │ Knowledge Graph│ │  Service     │ │  (shared)              │ │
│  │ entities +     │ │  evolution + │ │  builtin + auto +      │ │
│  │ relationships  │ │  contradict  │ │  user-created          │ │
│  │ (single source │ │  + change    │ │                        │ │
│  │  of truth)     │ │  summaries   │ │                        │ │
│  └────────────────┘ └──────────────┘ └────────────────────────┘ │
│                                                                  │
│  ┌────────────────────────────────────────────────────────────┐ │
│  │ Storage: SQLite (relational + FTS5) + LanceDB (vectors)    │ │
│  └────────────────────────────────────────────────────────────┘ │
└─────────────────────────────────────────────────────────────────┘
```

---

## 10. Implementation Phases

### Phase 0: Shared Infrastructure (Prerequisite)
- [ ] Add `entities` + `entity_relationships` + `insight_personas` + `insight_persona_pins` tables to cognitive migration
- [ ] Temporal backfill: `UPDATE semantic_facts SET valid_from = created_at WHERE valid_from IS NULL`
- [ ] Entity backfill: convert existing SPO facts + note_entity_mentions into graph entries
- [ ] `PersonaRepo` in cognitive crate (shared between Insight Review + agent runtime)
- [ ] `EntityRepo` in cognitive crate with neighborhood/path/merge queries
- [ ] `DomainSearcher` trait in context_engine (dependency inversion)
- [ ] Multi-perspective planning prompt tweak (one line in runtime.rs)

### Phase 1: InsightForge Context Assembly (Highest ROI)
- [ ] `InsightForge` module in context_engine: `QueryDecomposer` trait, heuristic + LLM decomposers
- [ ] Per-session circuit breaker
- [ ] Integration into `ContextEngine::assemble_uncached()` with `should_activate()` gate
- [ ] Feature crates implement `DomainSearcher`: NoteSearcher, TaskSearcher, FinanceSearcher, GraphSearcher
- [ ] Per-searcher timeout isolation (800ms)
- [ ] Post-RRF BudgetAllocator pass
- [ ] Config: `contextEngine.insightForge.*`

### Phase 2: Notes Insight Review (Most Visible Feature)
- [ ] Full Insight Review implementation (per `2026-03-16-insight-review-design.md` v2)
- [ ] Tabs 1-4 (Synthesis, Gap Analysis, Self-Assessment, Concept Map)
- [ ] InsightForge context assembly (uses Phase 0-1 infrastructure)
- [ ] Deep Dive note generation from gaps
- [ ] Flashcard persistence with FSRS

### Phase 3: Temporal Intelligence
- [ ] `TemporalService` in cognitive (injectable via FeaturePackage pattern)
- [ ] Fact history queries (active + archive)
- [ ] Contradiction detection in ConsolidationHandler (with same-session guard)
- [ ] Change summaries (structured, LLM-free)
- [ ] Temporal scoring weight in retrieval (configurable, default 0.05)
- [ ] Entity extraction in BackgroundConsolidationService (combined fact+entity prompt)

### Phase 4: Perspectives + Personas in Notes
- [ ] Insight Review Tab 5 (Perspectives) using shared PersonaRepo
- [ ] Persona management UI in notes
- [ ] Explicit perspective analysis in agent (system prompt injection)
- [ ] Persona auto-generation from note domains
- [ ] Persona domain-matching against knowledge graph entities

### Phase 5: Reports + Scenarios
- [ ] Report skill reference (`skills/task-management/references/reports.md`)
- [ ] Weekly Review, Project Retrospective, Financial Health, Knowledge Growth templates
- [ ] Scenario reasoning template in planning prompt
- [ ] Graph neighborhood for cascading effect analysis (depth limit: 2)
- [ ] Cron-triggered automated reports

### Phase 6: Polish + Cross-Session Tracking
- [ ] Cross-session insight tracking in notes ("What's Changed" banner)
- [ ] Scenario challenges in Self-Assessment quiz
- [ ] Persona follow-up chat
- [ ] Knowledge growth metrics
- [ ] Entity merge/dedup UI tools

---

## 11. Design Principles

1. **Platform over feature** — Every capability lives at the lowest appropriate layer so all features benefit.
2. **Data, not code** — Personas, entity types, relationship types, report templates — all stored in SQLite, not hardcoded.
3. **Progressive enhancement** — InsightForge falls back to simple retrieval. Knowledge graph falls back to flat facts. Perspectives fall back to single-viewpoint. Nothing breaks.
4. **Transparent complexity** — The user sees better answers, not "InsightForge decomposed your query into 5 sub-queries."
5. **Close the loop** — Every analysis offers an action. Gap → Create Note. Contradiction → Resolve. Scenario → Decision. Weekly Review → Updated Priorities.
6. **Composable** — InsightForge uses the knowledge graph. The knowledge graph uses temporal tracking. Reports use InsightForge. Each component enhances the others.
7. **Conversation-first** — Every feature surfaces through natural dialogue, not separate UIs.

---

## 12. What We're NOT Doing

| MiroFish Feature | Why We Skip It |
|---|---|
| Full OASIS social media simulation | Overkill. Lightweight scenario reasoning achieves the same value |
| Zep Cloud dependency | Klynt is local-first. We use SQLite + LanceDB |
| Dual-platform parallel simulation | Multi-perspective reasoning captures the value without the infrastructure |
| Separate ReportEngine | Reports as skill in existing ReactiveEngine — zero duplicate code |
| Docker-based deployment | Klynt ships as a native Tauri desktop app |
| File-system state management | We have proper SQLite. MiroFish's approach is weaker |

---

## 13. Success Metrics

| Metric | Measurement | Target |
|---|---|---|
| **Context relevance** | User thumbs-up/down on agent responses (sampled) | >80% positive (up from ~60%) |
| **Multi-source responses** | % of complex queries referencing 2+ knowledge sources | >70% |
| **Entity connections** | Avg. entities + relationships per user after 30 days | >50 entities, >100 relationships |
| **Contradiction surfacing** | Contradictions detected and resolved per week | >2 per active week |
| **Report adoption** | Weekly review generation frequency | >1 per week after first use |
| **Decomposition efficiency** | % of decompositions handled by heuristic (no LLM) | >60% |

---

## 14. New & Modified Files (All Phases)

### New files

| File | Crate | Phase | Purpose |
|---|---|---|---|
| `src/repos/entity.rs` | cognitive | 0 | EntityRepo + EntityRow + relationship queries |
| `src/repos/persona.rs` | cognitive | 0 | PersonaRepo (shared) + PersonaRow + selection |
| `src/services/temporal.rs` | cognitive | 3 | TemporalService (history, contradictions, summaries) |
| `src/insight_forge/mod.rs` | context_engine | 1 | InsightForge struct + retrieve + should_activate |
| `src/insight_forge/decomposer.rs` | context_engine | 1 | QueryDecomposer trait + heuristic + LLM impls |
| `src/insight_forge/domain_searcher.rs` | context_engine | 1 | DomainSearcher trait |
| `src/insight_forge/circuit_breaker.rs` | context_engine | 1 | Per-session circuit breaker |
| `skills/.../references/reports.md` | skill-system | 5 | Report templates |

### Modified files

| File | Crate | Phase | Change |
|---|---|---|---|
| `migrations/001_cognitive_tables.sql` | cognitive | 0 | Add entities, entity_relationships, personas, pins tables |
| `src/repos/mod.rs` | cognitive | 0 | Register entity + persona modules |
| `src/services/extraction.rs` | cognitive | 3 | Combined fact+entity extraction prompt |
| `src/services/consolidation.rs` | cognitive | 3 | Contradiction detection hook |
| `src/services/retrieval.rs` | cognitive | 3 | Add temporal_score to 6-factor formula |
| `src/assembler/mod.rs` | context_engine | 1 | InsightForge integration point |
| `src/lib.rs` | context_engine | 1 | Export InsightForge + DomainSearcher |
| `src/agent_runtime/runtime.rs` | agent | 0 | Multi-perspective planning prompt |
| Feature crates (notes, tasks, finance) | various | 1 | Implement DomainSearcher trait |
