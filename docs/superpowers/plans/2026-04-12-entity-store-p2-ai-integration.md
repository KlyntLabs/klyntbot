# Entity Store — Plan 2: AI Integration

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Connect the entity store to the AI subsystems — skill binding, Mirror schema observation, Reforge schema evolution, cognitive memory generalization, unified retrieval, and context injection.

**Architecture:** Each database gets a bound skill at `~/.klyntbot/skills/db-{id}/SKILL.md`. Mirror adds a `SchemaMirrorSubscriber` that tracks field usage. Reforge gets Phase 2.5 for schema evolution proposals. Cognitive salience, fact extraction, and retrieval become schema-aware via skill declarations. `DatabaseContextSource` replaces feature-specific context sources. `DatabaseSearcher` replaces domain-specific searchers.

**Tech Stack:** Rust, async-trait, serde, sqlx, LanceDB (lancedb crate), skill-system

**Spec:** `docs/superpowers/specs/2026-04-12-flexible-database-engine-design.md` (Sections 4-7)

**Depends on:** Plan 1 (entity-store and database-tool crates must exist)

---

## File Structure

### New Files

| File | Responsibility |
|------|----------------|
| `crates/entity-store/src/skill_binding.rs` | Generate/update skill files when databases are created/modified |
| `crates/cognitive/src/mirror/subscribers/schema.rs` | `SchemaMirrorSubscriber` — tracks field usage from entity events |
| `crates/cognitive/src/services/reforge/schema_evolution.rs` | Phase 2.5 — LLM-driven schema evolution proposals |
| `crates/agent/src/adapters/database_embedding.rs` | `DatabaseEmbeddingAdapter` — embeds entities from any database |
| `crates/agent/src/context_sources/database.rs` | `DatabaseContextSource` — injects active entities from all databases |
| `crates/agent/src/domain_searchers/database_searcher.rs` | `DatabaseSearcher` — keyword + semantic search across all databases |

### Modified Files

| File | Change |
|------|--------|
| `crates/skill-system/src/types.rs` | Add `schema_hints`, `salience`, `context_rules` to `KlyntbotMeta` |
| `crates/skill-system/src/parser.rs` | Parse new frontmatter fields |
| `crates/cognitive/src/mirror/engine.rs` | Start `SchemaMirrorSubscriber` alongside existing subscribers |
| `crates/cognitive/src/mirror/mod.rs` | Add `pub mod subscribers::schema;` |
| `crates/cognitive/src/services/reforge/service.rs` | Insert Phase 2.5 call between Synthesize and Review |
| `crates/cognitive/src/services/reforge/collector.rs` | Collect schema observations in Phase 1 |
| `crates/cognitive/src/services/salience.rs` | Add entity event handling with skill-driven classification |
| `crates/cognitive/src/services/background.rs` | Add entity event → observation mapping |
| `crates/cognitive/migrations/` | Add migration for `mirror_schema_observations` table |
| `crates/agent/src/agent_loop/builder.rs` | Wire DatabaseContextSource and DatabaseSearcher |
| `crates/storage/src/vector_store/schemas.rs` | Add dynamic per-database embedding schema |

---

## Task 1: Extend skill frontmatter with schema_hints, salience, context_rules

**Files:**
- Modify: `crates/skill-system/src/types.rs`
- Modify: `crates/skill-system/src/parser.rs`

- [ ] **Step 1: Add new fields to `KlyntbotMeta` struct**

```rust
// Add to KlyntbotMeta:
pub schema_hints: Option<HashMap<String, SchemaHint>>,
pub salience: Option<SalienceDeclaration>,
pub context_rules: Option<ContextRules>,
```

Define supporting types:

```rust
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct SchemaHint {
    pub lifecycle: Option<bool>,
    pub completion_values: Option<Vec<String>>,
    pub active_values: Option<Vec<String>>,
    pub temporal: Option<bool>,
    pub urgency_source: Option<bool>,
    pub ranking: Option<bool>,
    pub behavioral: Option<bool>,
    pub grouping: Option<bool>,
    pub budget_field: Option<bool>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct SalienceDeclaration {
    #[serde(default)]
    pub extract_on: Vec<SalienceRule>,
    #[serde(default)]
    pub accumulate_on: Vec<SalienceRule>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SalienceRule {
    pub field: Option<String>,
    pub event: Option<String>,
    pub to_values: Option<Vec<String>>,
    pub importance: Option<f64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ContextRules {
    pub active_filter: Option<String>,
    pub sort_by: Option<String>,
    pub max_items: Option<usize>,
    pub format: Option<String>,
}
```

- [ ] **Step 2: Write tests for parsing the new frontmatter fields**
- [ ] **Step 3: Run tests and commit**

---

## Task 2: Implement skill binding — auto-generate skill on database creation

**Files:**
- Create: `crates/entity-store/src/skill_binding.rs`

- [ ] **Step 1: Implement `generate_skill(schema: &DatabaseSchema) -> String` that produces a SKILL.md with frontmatter + body**
- [ ] **Step 2: Implement `update_skill_on_field_change(schema: &DatabaseSchema, skill_dir: &Path)` that updates the field list in an existing skill**
- [ ] **Step 3: Wire into EntityStore — call generate_skill after create_database, update_skill after add_field/remove_field**
- [ ] **Step 4: Write tests and commit**

---

## Task 3: Add SchemaMirrorSubscriber

**Files:**
- Create: `crates/cognitive/src/mirror/subscribers/schema.rs`
- Add migration for `mirror_schema_observations` table
- Modify: `crates/cognitive/src/mirror/engine.rs`

- [ ] **Step 1: Write migration SQL for mirror_schema_observations table**
- [ ] **Step 2: Implement SchemaMirrorSubscriber that listens for EntityCreated, EntityUpdated, EntityDeleted events**
- [ ] **Step 3: On each event, upsert usage counts in mirror_schema_observations (which fields were filled, which were left empty)**
- [ ] **Step 4: Start subscriber in MirrorEngine::start() alongside existing 4 subscribers**
- [ ] **Step 5: Write tests and commit**

---

## Task 4: Add Reforge Phase 2.5 — Schema Evolution

**Files:**
- Create: `crates/cognitive/src/services/reforge/schema_evolution.rs`
- Modify: `crates/cognitive/src/services/reforge/service.rs`
- Modify: `crates/cognitive/src/services/reforge/collector.rs`

- [ ] **Step 1: In collector.rs, add schema observation collection — read mirror_schema_observations and database_fields**
- [ ] **Step 2: Create schema_evolution.rs with `SchemaEvolutionHandler` trait and `SchemaEvolutionInput` / `SchemaEvolutionOutput` types**
- [ ] **Step 3: In service.rs, insert Phase 2.5 call after Synthesize (Phase 2) and before Review (Phase 3)**
- [ ] **Step 4: Phase 5 (Apply) — execute accepted proposals via EntityStore::add_field/remove_field with Autotuner confidence gating**
- [ ] **Step 5: Write tests and commit**

---

## Task 5: Generalize salience for entity events

**Files:**
- Modify: `crates/cognitive/src/services/salience.rs`

- [ ] **Step 1: For EntityCreated/Updated/Deleted events, load the database's skill and read its salience declarations**
- [ ] **Step 2: Match changed fields against extract_on/accumulate_on rules**
- [ ] **Step 3: Return appropriate SalienceVerdict with skill-defined importance**
- [ ] **Step 4: Fallback: if no skill found, use Accumulate with importance 0.3**
- [ ] **Step 5: Write tests and commit**

---

## Task 6: Generalize background event→observation mapping

**Files:**
- Modify: `crates/cognitive/src/services/background.rs`

- [ ] **Step 1: Add match arms for EntityCreated, EntityUpdated, EntityDeleted**
- [ ] **Step 2: Build observation content from entity fields (e.g., "Created entity in 'Job Applications': Company=Anthropic, Role=Backend")**
- [ ] **Step 3: Use skill-defined importance or default 0.3**
- [ ] **Step 4: Write tests and commit**

---

## Task 7: Implement DatabaseEmbeddingAdapter

**Files:**
- Create: `crates/agent/src/adapters/database_embedding.rs`
- Modify: `crates/storage/src/vector_store/schemas.rs`

- [ ] **Step 1: Add dynamic embedding table creation in vector store — `ensure_database_embedding_table(database_id)`**
- [ ] **Step 2: Implement DatabaseEmbeddingAdapter that composes entity text from field values and embeds via EmbeddingEngine**
- [ ] **Step 3: Wire into EntityStore or DatabaseTool — embed on entity create/update**
- [ ] **Step 4: Write tests and commit**

---

## Task 8: Implement DatabaseContextSource

**Files:**
- Create: `crates/agent/src/context_sources/database.rs`
- Modify: `crates/agent/src/agent_loop/builder.rs`

- [ ] **Step 1: Implement ContextSource trait for DatabaseContextSource**
- [ ] **Step 2: For each database, read skill's context_rules to determine active filter, sort, max items, format string**
- [ ] **Step 3: Query EntityStore with those filters and format results**
- [ ] **Step 4: Token-budget aware — prioritize most-used databases**
- [ ] **Step 5: Register in builder.rs, replacing TodoSource and other feature-specific context sources**
- [ ] **Step 6: Write tests and commit**

---

## Task 9: Implement DatabaseSearcher

**Files:**
- Create: `crates/agent/src/domain_searchers/database_searcher.rs`
- Modify: `crates/agent/src/agent_loop/builder.rs`

- [ ] **Step 1: Implement DomainSearcher trait for DatabaseSearcher**
- [ ] **Step 2: Search across all database embedding tables + keyword search on text fields**
- [ ] **Step 3: Return MemoryEntry results with database name and field labels**
- [ ] **Step 4: Register in builder.rs, replacing TaskSearcher and other feature-specific searchers**
- [ ] **Step 5: Write tests and commit**

---

## Task 10: Workspace-level orchestrator skill

**Files:**
- Create: `skills/workspace/SKILL.md`

- [ ] **Step 1: Write the workspace orchestrator skill that handles cross-database queries**
- [ ] **Step 2: Add auto-update logic — regenerate database-list reference when databases are created/deleted**
- [ ] **Step 3: Commit**

---

## Task 11: Verify full AI integration

- [ ] **Step 1: Run full workspace tests**

Run: `cargo nextest run --workspace`

- [ ] **Step 2: Run clippy**

Run: `cargo clippy --workspace --all-targets --all-features`

- [ ] **Step 3: Final commit with any fixes**
