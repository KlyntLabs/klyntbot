# Entity Store — Self-Evolving Entity Platform

**Date:** 2026-04-12
**Status:** Approved
**Scope:** Replace fixed-schema feature crates with a Notion-like entity store that self-evolves through AI-user collaboration.

---

## 1. Vision

Reposition the platform from a fixed-schema task manager into a self-evolving workspace where:

- Users can create **any database** (tasks, finance, apartment hunting, job applications — anything)
- Each database has a **fully customizable schema** (add/remove/modify fields freely, like Notion)
- The AI **learns and evolves** each database's schema and behavior over time through Reforge, Mirror, and Autotuner
- Built-in features (Tasks, Finance) ship as **templates** — same flexible foundation, mature starting skills
- A future **marketplace** lets users share templates (schema + skill bundles)

## 2. Key Decisions

| Decision | Choice | Rationale |
|----------|--------|-----------|
| Existing features + user-created | **Both** | Full migration + user can create new databases from scratch |
| Data model | **Entity-centric** | More powerful for AI reasoning than database-centric |
| AI bootstrap | **Skills as seed intelligence** | Templates ship with skills so AI isn't "dumb" on day one |
| Field types | **Notion-parity** (~16 types) | Proven UX, path to AI-native types later |
| Relations | **Explicit + AI-inferred** | User-defined + cognitive system detects implicit connections |
| AI autonomy | **Confidence-tiered** | Low-risk auto-applied, medium suggested, high requires approval. Autotuner calibrates thresholds per-database |
| Templates | **Schema + Skill bundle** | Simple, portable, marketplace-ready. No learned-behavior leakage |
| Architecture | **Hybrid Schema (EntityStore)** | Dynamic SQLite tables with real typed columns via ALTER TABLE |
| Backward compat | **None needed** | Pre-release — clean replacement, no data migration |

## 3. Core Data Model

### 3.1 Foundation Tables

```sql
-- Registry of all databases
CREATE TABLE databases (
    id           TEXT PRIMARY KEY,
    name         TEXT NOT NULL,
    slug         TEXT UNIQUE NOT NULL,
    icon         TEXT,
    description  TEXT,
    template_id  TEXT,
    skill_id     TEXT,
    created_at   TEXT NOT NULL,
    updated_at   TEXT NOT NULL
);

-- Field definitions — the schema of each database
CREATE TABLE database_fields (
    id           TEXT PRIMARY KEY,
    database_id  TEXT NOT NULL REFERENCES databases(id),
    name         TEXT NOT NULL,
    slug         TEXT NOT NULL,
    field_type   TEXT NOT NULL,
    options_json TEXT,
    position     INTEGER NOT NULL,
    required     INTEGER DEFAULT 0,
    hidden       INTEGER DEFAULT 0,
    ai_managed   INTEGER DEFAULT 0,
    ai_config    TEXT,
    default_value TEXT,
    created_at   TEXT NOT NULL,
    UNIQUE(database_id, slug)
);

-- Views per database (table, board, calendar, list, gallery, timeline)
CREATE TABLE database_views (
    id           TEXT PRIMARY KEY,
    database_id  TEXT NOT NULL REFERENCES databases(id),
    name         TEXT NOT NULL,
    view_type    TEXT NOT NULL,
    config_json  TEXT NOT NULL,
    position     INTEGER NOT NULL,
    is_default   INTEGER DEFAULT 0,
    created_at   TEXT NOT NULL,
    updated_at   TEXT NOT NULL
);

-- Custom dashboards with widgets querying any database
CREATE TABLE dashboards (
    id          TEXT PRIMARY KEY,
    name        TEXT NOT NULL,
    config_json TEXT NOT NULL,
    position    INTEGER NOT NULL,
    created_at  TEXT NOT NULL,
    updated_at  TEXT NOT NULL
);

-- Cross-database entity relations
CREATE TABLE entity_relations (
    id            TEXT PRIMARY KEY,
    source_id     TEXT NOT NULL,
    source_db_id  TEXT NOT NULL,
    target_id     TEXT NOT NULL,
    target_db_id  TEXT NOT NULL,
    relation_type TEXT DEFAULT 'related',
    inferred      INTEGER DEFAULT 0,
    confidence    REAL,
    created_at    TEXT NOT NULL
);
```

### 3.2 Dynamic Tables

Each database gets a real SQLite table with typed columns:

```sql
-- On database creation (slug="tasks"):
CREATE TABLE db_tasks (
    id         TEXT PRIMARY KEY,
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL
);

-- On field addition (name="Due Date", type=date, slug="due_date"):
ALTER TABLE db_tasks ADD COLUMN due_date TEXT;
```

Every dynamic table always has `id`, `created_at`, `updated_at`. All other columns are user/AI-defined.

**Terminology:** The model is entity-centric (entities are the universal primitive), but organized into databases (the user-facing container). A "database" is a schema + collection of entities. An "entity" is a row in a database with dynamic properties. Users think in databases; the AI reasons about entities.

### 3.3 Field Type System

| Field Type | SQLite Type | Value Format |
|-----------|-------------|--------------|
| text | TEXT | plain string |
| number | REAL | numeric |
| select | TEXT | single option string |
| multi_select | TEXT | JSON array of strings |
| date | TEXT | ISO 8601 |
| checkbox | INTEGER | 0 or 1 |
| url | TEXT | URL string |
| email | TEXT | email string |
| phone | TEXT | phone string |
| relation | TEXT | JSON array of entity IDs |
| rollup | *(computed)* | evaluated at query time (aggregates over a relation field) |
| formula | TEXT | expression string (evaluated in Rust, not SQL — supports field references and basic math) |
| created_time | TEXT | auto-set, immutable |
| last_edited | TEXT | auto-updated |
| files | TEXT | JSON array of file refs |
| person | TEXT | user identifier |

### 3.4 Query Generation

The system generates SQL dynamically from `database_fields` metadata:

```sql
-- "show me tasks due this week with high priority"
SELECT id, title, due_date, priority, status
FROM db_tasks
WHERE due_date BETWEEN '2026-04-12' AND '2026-04-18'
  AND priority = 'high'
ORDER BY due_date ASC
```

No JOINs for field access. No json_extract. Native SQL on real columns.

## 4. AI Evolution Loop

### 4.1 Three-Layer Architecture

```
MIRROR (Observe)
  → Watches per-database field usage patterns
  → Tracks which fields are filled, filtered, sorted, ignored
  → Emits SchemaObservation events

REFORGE (Reason)
  → Analyzes observations + skill knowledge
  → Detects unused fields, missing fields, type mismatches, relation opportunities
  → Rewrites database skills based on learnings
  → Emits SchemaEvolution proposals

AUTOTUNER (Act)
  → Applies confidence-tiered autonomy:
    Low-risk  (auto):    reorder fields, hide unused, adjust select options, update skill
    Medium-risk (suggest): add field, create relation, change field config
    High-risk (approve):  remove field, change field type, merge/split databases
  → Thresholds self-calibrate per-database based on user acceptance rates
```

### 4.2 Schema Evolution Storage

```sql
CREATE TABLE schema_evolutions (
    id            TEXT PRIMARY KEY,
    database_id   TEXT NOT NULL REFERENCES databases(id),
    action_type   TEXT NOT NULL,
    action_json   TEXT NOT NULL,
    confidence    REAL NOT NULL,
    reasoning     TEXT NOT NULL,
    status        TEXT NOT NULL,
    source        TEXT NOT NULL,
    created_at    TEXT NOT NULL,
    resolved_at   TEXT
);

CREATE TABLE schema_autonomy (
    database_id      TEXT PRIMARY KEY REFERENCES databases(id),
    auto_threshold   REAL DEFAULT 0.9,
    suggest_threshold REAL DEFAULT 0.6,
    acceptance_rate  REAL DEFAULT 0.5,
    total_proposed   INTEGER DEFAULT 0,
    total_accepted   INTEGER DEFAULT 0,
    updated_at       TEXT NOT NULL
);
```

### 4.3 New Domain Events

```
SchemaFieldUsed { database_id, field_id, usage_type }
SchemaFieldIgnored { database_id, field_id, days_unused }
SchemaPatternDetected { database_id, pattern_type, evidence }
EntityRelationInferred { source_db, target_db, confidence, evidence }
SchemaEvolutionProposed { database_id, action, confidence, reasoning }
SchemaEvolutionApplied { database_id, action, auto_applied }
SchemaEvolutionSuggested { database_id, suggestion_id }
```

### 4.4 Concrete Example

User creates "Job Applications" from template. Template seeds fields: Company, Role, Status (select: Applied/Interview/Offer/Rejected), Salary, Applied Date, Notes.

**Week 1 — Mirror observes:** User always fills Company, Role, Status. Never uses Notes. Types "LinkedIn" in conversations. Asks "which companies haven't responded?"

**Week 2 — Reforge reasons:** Notes unused 14 days → propose hide (0.85). User mentions source channel → propose add Source field (0.72). Response timing questions → propose add Last Contact Date (0.68). Rewrites skill to ask about source on creation.

**Week 2 — Autotuner acts:** Hide Notes: 0.85 < auto_threshold 0.9 → suggest (approved). Add Source: 0.72 → suggest (approved). After 3 acceptances, auto_threshold drops → next similar action auto-applies.

## 5. Cognitive Memory Integration

### 5.1 Per-Database Embeddings

Each database gets its own embedding table in LanceDB:

```
db_{database_id}_embeddings
  (id, entity_id, vector[384], field_context, updated_at)

db_schema_embeddings
  (id, database_id, field_id, vector[384], description, updated_at)
```

Entity embedding text is composed from field values:
```
"Job Applications entity: Company is Anthropic. Role is Backend Engineer.
 Status is Interview. Salary is 180000."
```

Schema embeddings enable cross-database field discovery ("Due Date" in Tasks ↔ "Deadline" in Job Applications).

### 5.2 Skill-Driven Salience

Replace hardcoded salience match arms with skill declarations:

```yaml
# In the database's skill frontmatter:
salience:
  extract_on:
    - field: status
      to_values: ["Offer", "Rejected"]
      importance: 0.9
  accumulate_on:
    - event: entity_created
      importance: 0.3
    - field: notes
      importance: 0.2
```

The generic salience engine reads the skill to classify events. Reforge evolves these declarations based on user behavior.

### 5.3 Fact Extraction

Semantic facts get `domain = database_id`. The existing extraction pipeline (episodic → semantic → consolidation) works unchanged — facts from all databases flow into the same Synthesize phase.

The `cognitive_bridge.rs` functions (extract_energy_profile, extract_estimation_bias, etc.) are replaced by a single schema-aware extractor that reads the skill for domain context.

### 5.4 Unified Retrieval

Replace `TaskSearcher`, `NoteSearcher`, `FinanceSearcher` with a single `DatabaseSearcher` that:
1. Embeds the query
2. Searches relevant database embedding tables (all or filtered)
3. Keyword search on dynamic tables using schema-aware column selection
4. RRF merge (same hybrid strategy as today)
5. Returns results with database name, field labels, and values

### 5.5 Context Injection

Replace `TodoSource` and other feature-specific context sources with `DatabaseContextSource`:
- Iterates all user databases
- Reads each skill's `context_rules` to determine active/relevant items
- Queries dynamic tables with skill-defined filters
- Formats with field labels
- Token-budget aware: prioritizes most-used databases (from Mirror data)

### 5.6 Per-Database Brain Versioning

```sql
CREATE TABLE database_brain_versions (
    version       INTEGER,
    database_id   TEXT NOT NULL REFERENCES databases(id),
    skill_version TEXT NOT NULL,
    schema_hash   TEXT NOT NULL,
    params_json   TEXT NOT NULL,
    metrics_json  TEXT NOT NULL,
    promoted_at   TEXT NOT NULL,
    reason        TEXT NOT NULL,
    parent_version INTEGER,
    reverted      INTEGER DEFAULT 0,
    PRIMARY KEY (database_id, version)
);
```

Each database has an independent evolution timeline. Revert one database's brain without affecting others.

## 6. Skill Integration

### 6.1 Database-Skill Binding

Every database has exactly one skill at `~/.klyntbot/skills/db-{database_id}/SKILL.md`.

### 6.2 Skill Frontmatter Extensions

```yaml
metadata:
  klyntbot:
    # Existing fields (type, tools, triggers, summary, etc.)

    # NEW: Schema-aware fields
    schema_hints:
      status:
        lifecycle: true
        completion_values: ["done", "completed"]
        active_values: ["todo", "doing"]
      due_date:
        temporal: true
        urgency_source: true
      priority:
        ranking: true

    # NEW: Salience declarations
    salience:
      extract_on:
        - field: status
          to_values: ["done"]
          importance: 0.7
      accumulate_on:
        - event: entity_created
          importance: 0.3

    # NEW: Context injection rules
    context_rules:
      active_filter: "status NOT IN ('done', 'archived')"
      sort_by: "due_date ASC"
      max_items: 15
      format: "{title} — {status}, due {due_date}"
```

### 6.3 Three Skill Origins

**From Template:** Mature skill shipped with the template. Knows field semantics, behavioral patterns, suggestions.

**From Scratch:** Auto-generated minimal skill from schema. Lists fields, basic context rules, generic salience. Functional but basic.

**Reforge Evolution:** Nightly cycle rewrites skill based on observed usage. Adds schema_hints, adjusts salience declarations, enriches behavioral instructions. The skill becomes a living document unique to each user.

### 6.4 Skill Hot-Reload on Schema Change

When a field is added/removed (by user or AI):
1. ALTER TABLE or mark field hidden
2. Update `database_fields` table
3. Update skill body (field list, context_rules format string)
4. Record version via `skill_version_repo`
5. Trigger `SkillStore.reload()`

### 6.5 Workspace Orchestrator

A system-level skill for cross-database queries:

```yaml
name: workspace
description: Cross-database queries and connections
metadata:
  klyntbot:
    type: orchestrator
    triggers: ["across all", "connect", "between", "compare"]
    always_skills: [database-list]
```

Auto-updated when databases are created/deleted.

## 7. AI Subsystem Adaptations

### 7.1 Mirror — Add SchemaMirrorSubscriber

New subscriber listens for `EntityCreated`, `EntityUpdated`, `EntityFieldAccessed` events. Tracks per-database field usage patterns in `mirror_schema_observations` table:

```sql
CREATE TABLE mirror_schema_observations (
    id           TEXT PRIMARY KEY,
    database_id  TEXT NOT NULL,
    field_id     TEXT NOT NULL,
    usage_type   TEXT NOT NULL,
    count         INTEGER DEFAULT 1,
    last_used_at TEXT NOT NULL,
    UNIQUE(database_id, field_id, usage_type)
);
```

No changes to: routing subscriber, meta-rule detector, brain versioning, trial preview, facade API, weekly narratives.

### 7.2 Reforge — Add Phase 2.5 (Schema Evolution)

Insert between Synthesize and Review:
1. Read `mirror_schema_observations` (field usage)
2. Read `database_fields` (current schema)
3. Read database skill (domain context)
4. LLM call: "Given schema, usage, skill — what fields should be added/removed/modified?"
5. Output `SchemaEvolutionProposal` structs
6. Phase 5 (Apply) executes via Autotuner confidence tiers

Skill rewriting in existing Phase 5b handles updating skills when schemas change.

### 7.3 Autotuner — Per-Database Parameter Overrides

Extend `TrialParams` with `Map<database_id, database_overrides>` for per-database retrieval tuning (vector_top_k, min_similarity, relevance weights).

Champion params become `global_params + per_database_overrides`. Constraint evaluation stays the same.

### 7.4 Cognitive Memory — Generalize Three Hardcoded Points

**A. Salience:** Replace hardcoded event→verdict match arms with skill-driven classification via `schema_hints` and `salience` declarations.

**B. Importance:** Replace hardcoded per-event importance values with skill-defined `importance_map`.

**C. Fact domain:** Set `SemanticFact.domain = database_id`. Scope chain: `system → database → entity`.

Everything else in the cognitive pipeline is already generic (FSRS, relevance scoring, consolidation, compaction).

## 8. Cross-Domain

All cross-domain scenarios work naturally:

| Scenario | Mechanism |
|----------|-----------|
| Cross-DB semantic search | Retrieval searches all embedding tables, ranks by relevance |
| Cross-DB fact extraction | Reforge Synthesize reads all domains in same pass |
| Cross-DB entity relations | `entity_relations` table + Reforge Phase 6.5 graph consolidation |
| Cross-DB context injection | `DatabaseContextSource` iterates all databases |
| Cross-DB AI reasoning | LLM sees all database context in same prompt |
| Cross-DB skill routing | Workspace orchestrator skill handles multi-database queries |
| Cross-DB autotuner | Metrics are per-message, not per-database |

## 9. Feature Migration Plan

### 9.1 Tier 1: Full Migration to Flexible Database

| Current Crate | Becomes |
|--------------|---------|
| `feature-tasks` (43 fixed columns, 9 tables) | Template: "Task Management" |
| `feature-finance` (9 sub-repos, 10 tables) | Templates: "Accounts", "Transactions", "Budgets", etc. |
| `feature-learning` (235 lines, no tables) | Deleted — inline prompt building into app-core |

### 9.2 Tier 2: Kept as Specialized System

| Crate | Why | Integration |
|-------|-----|-------------|
| `feature-notes` | Long-form documents, wiki-link graph, FTS5, versioning, structural hole detection. Not a "row of fields" problem. | Links to flexible database entities via `entity_relations`. Note metadata could use flexible fields. |

### 9.3 Tier 3: Kept as Engines (Not Databases)

| Crate | Why | Integration |
|-------|-----|-------------|
| `feature-productivity` | Real-time OS monitoring, 4-tier rule engine, time-series bucketing, auto-focus FSM, distraction detection, nudge system. Signal processing pipeline, not user data. | Feeds computed metrics into cognitive system. Daily summaries surface as context. |
| `feature-coaching` | Behavioral intervention engine — signal accumulation, trigger evaluation, dismissal backoff, receptivity adjustment. Stateful event processor with 2 small tables. | Reads `UserSituation` which incorporates flexible database activity signals. |
| `feature-language-learning` | Audio analysis pipeline — phoneme FSRS-5, tone analysis, adaptive feedback. Hooks into voice-engine transcription. 80% algorithm, 20% storage. | Flashcard generation can source from any flexible database. |

### 9.4 New Crate Structure

```
NEW:
  crates/entity-store/             — Schema registry, dynamic tables, field types, query builder, views
    src/schema.rs                  — DatabaseSchema, FieldDefinition, FieldType
    src/entity.rs                  — Entity (dynamic property map)
    src/query.rs                   — Dynamic SQL builder
    src/evolution.rs               — Schema evolution (add/remove/modify fields)
    src/relations.rs               — Cross-database entity relations
    src/views.rs                   — View and dashboard definitions
    src/templates.rs               — Template loading and instantiation
    migrations/                    — Foundation tables (databases, fields, views, dashboards, relations, evolutions, autonomy)

  crates/database-tool/            — Unified tool exposed to AI
    src/tool.rs                    — DatabaseTool (replaces TaskTool, FinanceTool)
    src/actions/                   — CRUD, search, schema ops, AI-powered actions
    src/handlers.rs                — Generic handler traits

  templates/                       — Built-in template bundles (marketplace-ready)
    task-management/
      manifest.json + skill/
    finance/
      manifest.json + skill/

REMOVED:
  crates/feature-tasks/
  crates/feature-finance/
  crates/feature-learning/

KEPT (unchanged):
  crates/feature-notes/
  crates/feature-productivity/
  crates/feature-coaching/
  crates/feature-language-learning/

ADAPTED:
  crates/agent/src/handlers/       — Generic handler implementations
  crates/agent/src/adapters/       — DatabaseEmbeddingAdapter
  crates/agent/src/context_sources/— DatabaseContextSource
  crates/agent/src/domain_searchers/— DatabaseSearcher
  crates/cognitive/src/mirror/     — SchemaMirrorSubscriber
  crates/cognitive/src/services/reforge/ — Phase 2.5 schema evolution
  crates/app-core/                 — Generic db_* Tauri commands
```

## 10. Tool & MCP Surface

### 10.1 Unified Database Tool

One tool replaces all migrated feature tools:

```
Actions:
  Database management:  create_database, list_databases, get_schema, add_field, remove_field, modify_field
  Entity CRUD:          create, get, list, update, delete, search
  Relations:            link, unlink, list_relations
  AI-powered:           suggest_fields, evolve_schema (skill-delegated)
```

### 10.2 MCP Exposure

```rust
fn default_exposed_tools() -> Vec<String> {
    vec!["database", "agent", "memory", "cron", "mirror", "notes", ...]
}
```

External clients call `mcp__klyntbot__database` with `database_id` parameter.

### 10.3 Desktop Commands

Generic `db_*` commands replace all feature-specific Tauri commands:

```
db_list, db_get_schema, db_query, db_create_entity, db_update_entity,
db_delete_entity, db_add_field, db_remove_field, db_get_suggestions, etc.
```

One `DatabaseView` component renders any database based on its schema.

## 11. Template Format

```json
{
  "name": "Task Management",
  "description": "Personal task tracking with priorities, deadlines, and focus management",
  "icon": "✓",
  "version": "1.0.0",
  "databases": [
    {
      "name": "Tasks",
      "slug": "tasks",
      "fields": [
        { "name": "Title", "slug": "title", "type": "text", "required": true },
        { "name": "Status", "slug": "status", "type": "select",
          "options": ["todo", "doing", "done", "someday"] },
        { "name": "Priority", "slug": "priority", "type": "select",
          "options": ["low", "medium", "high", "urgent"] },
        { "name": "Due Date", "slug": "due_date", "type": "date" },
        { "name": "Tags", "slug": "tags", "type": "multi_select" },
        { "name": "Estimated Minutes", "slug": "estimated_minutes", "type": "number" },
        { "name": "Parent", "slug": "parent_id", "type": "relation", "target": "self" }
      ]
    }
  ],
  "relations": [],
  "skill_dir": "skill/"
}
```

## 12. Frontend Architecture

### 12.1 Design Principle

Every database — including Tasks and Finance — renders through the same generic components. No feature-specific React components for data display. The UI is fully schema-driven: read the `DatabaseSchema`, render the fields, let the user configure views.

### 12.2 What Gets Removed

```
REMOVED (feature-specific UI):
  features/tasks/          — 58 files, ~5,750 LOC (IssueLine, IssueBoard, SidebarProperties, etc.)
  features/finance/        — 25+ files, ~3,000 LOC (HealthScoreRing, SpendingHeatmap, NetWorthCard, etc.)
  shared/types/tasks.ts    — fixed Task interface
  shared/types/finance.ts  — fixed FinanceAccount, Transaction, etc.

KEPT (specialized systems, not database views):
  features/notes/          — TipTap editor, Vim mode, graph visualization (document system, not rows)
  features/productivity/   — Real-time activity timeline, focus timer (engine output, not user data)
  features/coaching/       — Intervention cards (behavioral system, not database)
  features/dashboard/      — Becomes the custom dashboard builder (see 12.8)
  features/chat/           — Agent conversation (unchanged)
  features/settings/       — App configuration (unchanged)
  features/brain/          — Mirror/Reforge UI (unchanged)
```

### 12.3 Core TypeScript Types

```typescript
// Replaces Task, FinanceAccount, and all fixed entity types
interface Entity {
  id: string;
  databaseId: string;
  fields: Record<string, unknown>;  // slug → value
  createdAt: string;
  updatedAt: string;
}

interface DatabaseSchema {
  id: string;
  name: string;
  slug: string;
  icon?: string;
  description?: string;
  templateId?: string;
  fields: FieldDefinition[];
  views: ViewDefinition[];
}

interface FieldDefinition {
  id: string;
  name: string;
  slug: string;
  fieldType: FieldType;
  options?: unknown;       // type-specific config (select choices, number format, etc.)
  position: number;
  required: boolean;
  hidden: boolean;
  aiManaged: boolean;
  defaultValue?: unknown;
}

type FieldType =
  | "text" | "number" | "select" | "multi_select" | "date"
  | "checkbox" | "url" | "email" | "phone" | "relation"
  | "rollup" | "formula" | "created_time" | "last_edited"
  | "files" | "person";

interface ViewDefinition {
  id: string;
  name: string;
  viewType: ViewType;
  config: ViewConfig;       // filters, sorts, visible fields, grouping, etc.
  position: number;
  isDefault: boolean;
}

type ViewType = "table" | "board" | "calendar" | "list" | "gallery" | "timeline";

interface ViewConfig {
  filters?: FilterRule[];
  sorts?: SortRule[];
  visibleFields?: string[];  // field IDs to show (order matters)
  groupBy?: string;          // field ID for board/timeline grouping
  calendarField?: string;    // field ID for calendar date source
  galleryField?: string;     // field ID for gallery cover image
  cardFields?: string[];     // field IDs shown on board/gallery cards
  layout?: Record<string, unknown>;  // view-specific layout options
}
```

### 12.4 View System — 6 View Types

Each database can have multiple named views. Users create/switch views via tabs (like Notion).

**Table View** — spreadsheet-style with sortable/resizable columns
- Builds on existing `DataTable` composite (7,653 LOC, already supports dynamic columns)
- Each column rendered by `FieldRenderer` based on field type
- Inline editing via `FieldEditor`
- Column reorder via drag-and-drop
- Column visibility toggle

**Board View** — kanban grouped by any select field
- User picks which select field to group by (e.g., Status, Priority)
- Cards show configurable `cardFields`
- Drag-and-drop between columns updates the grouping field value
- Swimlanes for optional secondary grouping

**Calendar View** — monthly/weekly view by any date field
- User picks which date field to display
- Entities shown as events on the calendar
- Drag to reschedule updates the date field
- Reuses existing calendar infrastructure from dashboard feature

**List View** — compact single-column list
- Title + configurable inline fields
- Checkbox for select fields marked as `lifecycle: true` (from skill schema_hints)
- Grouped by any field or ungrouped

**Gallery View** — card grid
- Cover image from files field (or first text field as preview)
- Configurable card fields
- Filterable/sortable

**Timeline View** — Gantt-style horizontal timeline
- Requires two date fields (start + end) or one date field
- Entities as horizontal bars
- Drag to reschedule
- Grouped by any select field

### 12.5 View Storage

```sql
CREATE TABLE database_views (
    id           TEXT PRIMARY KEY,
    database_id  TEXT NOT NULL REFERENCES databases(id),
    name         TEXT NOT NULL,
    view_type    TEXT NOT NULL,
    config_json  TEXT NOT NULL,      -- ViewConfig serialized
    position     INTEGER NOT NULL,
    is_default   INTEGER DEFAULT 0,
    created_at   TEXT NOT NULL,
    updated_at   TEXT NOT NULL
);
```

Templates seed default views. For example, the Task Management template creates:
- "All Tasks" (table view, all fields visible)
- "Board" (board view, grouped by status)
- "Calendar" (calendar view, by due_date)

Users add/remove/configure views freely. AI can suggest views via schema evolution.

### 12.6 Component Architecture

```
features/database/                    — THE universal database feature
├── pages/
│   └── DatabasePage.tsx              — loads schema + active view, renders ViewShell
│
├── components/
│   ├── ViewShell.tsx                 — view tabs + toolbar + active view renderer
│   ├── ViewToolbar.tsx               — filter bar, sort, search, group-by, new entity button
│   ├── ViewTabBar.tsx                — view tabs with add/rename/delete
│   │
│   ├── views/
│   │   ├── TableView.tsx             — schema-driven DataTable
│   │   ├── BoardView.tsx             — kanban with dnd-kit
│   │   ├── CalendarView.tsx          — monthly/weekly calendar
│   │   ├── ListView.tsx              — compact list
│   │   ├── GalleryView.tsx           — card grid
│   │   └── TimelineView.tsx          — Gantt-style timeline
│   │
│   ├── fields/
│   │   ├── FieldRenderer.tsx         — display any field type (read-only)
│   │   ├── FieldEditor.tsx           — edit any field type (inline or modal)
│   │   ├── renderers/
│   │   │   ├── TextRenderer.tsx
│   │   │   ├── NumberRenderer.tsx
│   │   │   ├── SelectRenderer.tsx
│   │   │   ├── MultiSelectRenderer.tsx
│   │   │   ├── DateRenderer.tsx
│   │   │   ├── CheckboxRenderer.tsx
│   │   │   ├── UrlRenderer.tsx
│   │   │   ├── RelationRenderer.tsx
│   │   │   ├── FormulaRenderer.tsx
│   │   │   └── FilesRenderer.tsx
│   │   └── editors/
│   │       ├── TextEditor.tsx
│   │       ├── NumberEditor.tsx
│   │       ├── SelectEditor.tsx       — dropdown with option management
│   │       ├── MultiSelectEditor.tsx  — tag-style multi-select
│   │       ├── DateEditor.tsx         — date picker
│   │       ├── RelationEditor.tsx     — entity picker across databases
│   │       └── ... (one per editable type)
│   │
│   ├── entity/
│   │   ├── EntityDetail.tsx          — full entity view (slide panel or page)
│   │   ├── PropertyList.tsx          — all fields rendered dynamically
│   │   └── CreateEntityModal.tsx     — dynamic form from schema
│   │
│   ├── schema/
│   │   ├── SchemaEditor.tsx          — add/remove/reorder fields (Notion property panel)
│   │   ├── FieldTypeSelector.tsx     — dropdown of 16 field types
│   │   └── FieldConfigEditor.tsx     — per-type config (select options, number format, etc.)
│   │
│   └── suggestions/
│       ├── SchemaSuggestionBar.tsx   — AI schema evolution suggestions
│       └── SuggestionCard.tsx        — accept/dismiss AI-proposed changes
│
├── hooks/
│   ├── useDatabase.ts                — fetch schema, fields, views
│   ├── useEntities.ts                — query entities with filters/sorts
│   ├── useEntity.ts                  — single entity CRUD
│   ├── useViews.ts                   — view CRUD
│   └── useSchemaSuggestions.ts       — AI evolution suggestions
│
└── lib/
    ├── query-builder.ts              — build filter/sort params from ViewConfig
    ├── field-utils.ts                — format, validate, compare field values
    └── view-defaults.ts              — default ViewConfig per view type
```

### 12.7 Sidebar — Dynamic Navigation

```tsx
// Sidebar items built from:
// 1. Fixed system items (Chat, Dashboard, Notes, Brain, Settings, etc.)
// 2. Dynamic database items from db_list query
// 3. User can reorder, pin, group into sections

const { data: databases } = useQuery("db_list");

const sidebarItems = [
  // Fixed
  { type: "fixed", label: "Chat", icon: MessageSquare, href: "/chat" },
  { type: "fixed", label: "Dashboard", icon: LayoutDashboard, href: "/dashboard" },
  { type: "separator" },

  // Dynamic — one entry per database
  ...databases.map(db => ({
    type: "database",
    label: db.name,
    icon: db.icon || Database,
    href: `/db/${db.id}`,
  })),
  { type: "separator" },

  // Fixed system
  { type: "fixed", label: "Notes", icon: FileText, href: "/notes" },
  { type: "fixed", label: "Brain", icon: Brain, href: "/brain" },
  { type: "fixed", label: "Settings", icon: Settings, href: "/settings" },
];
```

Users can create new databases directly from the sidebar ("+ New Database" button).

### 12.8 Custom Dashboards

Users build dashboards by placing **widgets** that query data from any database.

```typescript
interface DashboardDefinition {
  id: string;
  name: string;
  widgets: WidgetDefinition[];
}

interface WidgetDefinition {
  id: string;
  widgetType: WidgetType;
  databaseId: string;        // which database to query
  config: WidgetConfig;       // query + display options
  position: GridPosition;     // row, col, width, height
}

type WidgetType =
  | "count"        // single number (e.g., "12 open tasks")
  | "list"         // filtered entity list
  | "chart_bar"    // bar chart by field
  | "chart_pie"    // pie chart by select field
  | "chart_line"   // line chart over time
  | "progress"     // progress toward a numeric target
  | "heatmap"      // activity heatmap by date field
  | "table"        // mini table view
  | "metric"       // computed metric (sum, avg, min, max of a number field)
  | "calendar"     // mini calendar view
  | "custom"       // marketplace widget (loaded from template)

interface WidgetConfig {
  title: string;
  filters?: FilterRule[];
  groupBy?: string;           // field slug for chart grouping
  valueField?: string;        // field slug for aggregation
  dateField?: string;         // field slug for time-based widgets
  aggregation?: "count" | "sum" | "avg" | "min" | "max";
  limit?: number;
  colorField?: string;        // field slug for color coding
}
```

**Storage:**

```sql
CREATE TABLE dashboards (
    id         TEXT PRIMARY KEY,
    name       TEXT NOT NULL,
    config_json TEXT NOT NULL,    -- DashboardDefinition serialized
    position   INTEGER NOT NULL,  -- sidebar order
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL
);
```

**Example: User builds a "Work Overview" dashboard with:**
- Count widget: "Open Tasks" → database=tasks, filter=status!="done", aggregation=count
- Bar chart: "Tasks by Priority" → database=tasks, groupBy=priority, aggregation=count
- List widget: "Due This Week" → database=tasks, filter=due_date between this week, sort=due_date ASC
- Metric widget: "Monthly Spend" → database=transactions, filter=date=this month, valueField=amount, aggregation=sum
- Progress widget: "Savings Goal" → database=goals, filter=name="Emergency Fund", valueField=current_amount, target from field

Dashboards are sharable in the marketplace — a "Freelancer Dashboard" template might include widgets configured for a freelancer project tracker, invoice tracker, and client database.

### 12.9 Routing

```tsx
// New routes
{ path: "/db/:databaseId",              element: <DatabasePage /> },
{ path: "/db/:databaseId/:entityId",    element: <EntityDetailPage /> },
{ path: "/dashboard",                   element: <DashboardPage /> },
{ path: "/dashboard/:dashboardId",      element: <DashboardPage /> },

// Kept routes
{ path: "/chat",      element: <ChatPage /> },
{ path: "/notes",     element: <KnowledgeBasePage /> },
{ path: "/brain",     element: <BrainPage /> },
{ path: "/settings",  element: <SettingsPage /> },

// Removed routes (replaced by /db/:id)
// /tasks, /finance/*, /project/:id (projects become a database)
```

### 12.10 Template Manifest — View Definitions

Templates include default views so databases are immediately usable:

```json
{
  "name": "Task Management",
  "databases": [{
    "name": "Tasks",
    "slug": "tasks",
    "fields": [ ... ],
    "views": [
      {
        "name": "All Tasks",
        "viewType": "table",
        "isDefault": true,
        "config": {
          "visibleFields": ["title", "status", "priority", "due_date", "tags"],
          "sorts": [{ "field": "created_at", "direction": "desc" }]
        }
      },
      {
        "name": "Board",
        "viewType": "board",
        "config": {
          "groupBy": "status",
          "cardFields": ["priority", "due_date", "tags"]
        }
      },
      {
        "name": "Calendar",
        "viewType": "calendar",
        "config": {
          "calendarField": "due_date",
          "cardFields": ["title", "priority", "status"]
        }
      }
    ]
  }],
  "dashboards": [
    {
      "name": "Task Overview",
      "widgets": [
        { "widgetType": "count", "databaseSlug": "tasks",
          "config": { "title": "Open Tasks", "filters": [{"field": "status", "op": "not_in", "value": ["done"]}] } },
        { "widgetType": "chart_pie", "databaseSlug": "tasks",
          "config": { "title": "By Priority", "groupBy": "priority" } }
      ]
    }
  ]
}
```

### 12.11 Marketplace Impact

Templates are fully self-contained bundles:

```
template-freelancer-project-tracker/
├── manifest.json          — databases, fields, views, dashboards, relations
└── skill/
    ├── SKILL.md           — AI behavior for this domain
    └── references/
        └── workflows.md   — domain-specific workflows
```

A marketplace entry = one downloadable bundle. Installing it:
1. Creates databases with fields from manifest
2. Creates views from manifest
3. Creates dashboard widgets from manifest
4. Installs skill to `~/.klyntbot/skills/db-{id}/`
5. AI immediately understands the domain via the skill

Users can fork and customize any template. AI evolves the fork independently via Reforge.

## 13. What Doesn't Change

| System | Why |
|--------|-----|
| Mirror routing/meta-rule/narratives | Operates on skill/routing data, database-agnostic |
| Reforge Phases 1,3,4,5,6,7 | Fact/rule extraction, skill rewriting, compaction — all generic |
| Autotuner constraint evaluator | Math on metrics, entity-agnostic |
| FSRS-5 spaced repetition | Pure math on stability/retrievability |
| Relevance scoring (14 weights) | Already parameterized and autotuner-tuned |
| LanceDB vector store | Already decoupled from SQLite |
| Skill progressive loading | Tier 1 listing + Tier 2 on-demand body |
| MCP stdio transport | Tool names change, protocol unchanged |
