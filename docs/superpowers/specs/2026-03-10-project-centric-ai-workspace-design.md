# Project-Centric AI Workspace — Design Spec

## Overview

Transform Klyntbot's project management from basic CRUD into a **Project-Centric AI Workspace** where every project is a living dashboard. All features (tasks, notes, conversations, productivity, memories) are unified within project scope, and the AI operates with full project context — instructions, sources, role awareness, and scoped memory.

### Key Constraints

- **No backward compatibility needed** — pre-release, breaking changes acceptable. Edit existing migrations directly and reset database.
- **Single-user system** — no multi-user collaboration. "Roles" describe the user's identity within each project for AI behavior adjustment.

### Priority Order

1. **Cross-Feature Unification** — entity linking, project-scoped queries
2. **Project as AI Context** — instructions, sources, scoped memory, RAG
3. **Enhanced Project Detail UI** — 3-panel dashboard timeline
4. **Role Clarification & Intelligence** — role-aware AI, memory views, insights

---

## 1. Data Model

### 1.1 Project Enhancement

Existing `projects` table gains new columns (edit `001_initial.sql` directly):

```sql
-- New columns on projects table
instructions    TEXT,           -- JSON: {context, guidelines, constraints, persona}
ai_personality  TEXT,           -- Tone/style override
user_role       TEXT,           -- Freeform "who am I in this project"
start_date      TEXT,           -- ISO date
target_end_date TEXT,           -- ISO date
settings        TEXT            -- JSON: project-specific configs
```

`instructions` is a single JSON column with layered sections:

```json
{
  "context": "React Native mobile app for personal finance...",
  "guidelines": "Prioritize performance and UX...",
  "constraints": "Budget 50M VND, 3-month timeline...",
  "persona": "Communicate in Vietnamese, technical terms in English..."
}
```

The UI presents this as one editor with collapsible sections. Not separate rows.

`user_role` is freeform text — no enum, no dropdown. Example: "I'm the backend lead. I focus on API architecture, database design, and code review."

### 1.2 Entity Links Table (New)

Generic table connecting any entity to any other entity.

**ID semantics by entity kind:**
- `task`, `note`, `objective`, `key_result`, `source` → use the entity's `id` (UUID string)
- `conversation` → use the session `key` (the primary key of the `sessions` table, not a UUID)

```sql
CREATE TABLE entity_links (
    id          TEXT PRIMARY KEY,
    source_kind TEXT NOT NULL,   -- 'task', 'note', 'conversation', 'objective', 'key_result', 'source'
    source_id   TEXT NOT NULL,   -- entity id, or session key for conversations
    target_kind TEXT NOT NULL,
    target_id   TEXT NOT NULL,
    link_type   TEXT NOT NULL DEFAULT 'related',  -- 'related', 'created_from', 'discusses', 'implements'
    metadata    TEXT,            -- JSON, optional context
    created_at  TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%SZ', 'now')),
    UNIQUE(source_kind, source_id, target_kind, target_id, link_type)
);

CREATE INDEX idx_entity_links_source ON entity_links(source_kind, source_id);
CREATE INDEX idx_entity_links_target ON entity_links(target_kind, target_id);
```

### 1.3 Project Sources Table (New)

External knowledge base entries (notes auto-included separately):

```sql
CREATE TABLE project_sources (
    id          TEXT PRIMARY KEY,
    project_id  TEXT NOT NULL REFERENCES projects(id) ON DELETE CASCADE,
    source_type TEXT NOT NULL,   -- 'document', 'link', 'file', 'snippet'
    title       TEXT NOT NULL,
    content     TEXT,            -- Extracted/raw content for RAG
    url         TEXT,            -- For links
    file_path   TEXT,            -- For uploaded files
    embedding_id TEXT,           -- LanceDB vector ref
    metadata    TEXT,            -- JSON (file size, type, etc.)
    tags        TEXT DEFAULT '[]',
    created_at  TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%SZ', 'now')),
    updated_at  TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%SZ', 'now'))
);

CREATE INDEX idx_project_sources_project ON project_sources(project_id);
```

### 1.4 Cognitive Memory Extensions

Edit existing cognitive `001_cognitive_tables.sql`:

```sql
-- Add to semantic_facts table
project_id    TEXT REFERENCES projects(id),
memory_type   TEXT DEFAULT 'fact'   -- 'fact', 'decision', 'insight', 'pattern', 'milestone'

-- Add to episodic_memories table
project_id    TEXT REFERENCES projects(id)

-- Add to procedural_rules table
project_id    TEXT REFERENCES projects(id)
```

**Rust struct updates required:**
- `SemanticFact` in `crates/cognitive/src/types.rs`: add `pub project_id: Option<String>` and `pub memory_type: String`
- `EpisodicMemory`: add `pub project_id: Option<String>`
- `ProceduralRule`: add `pub project_id: Option<String>`
- `SessionRow` in `crates/storage/src/rows/session.rs`: add `pub project_id: Option<String>`, `pub conversation_type: Option<String>`, `pub pinned: bool`

Indexes:

```sql
CREATE INDEX idx_semantic_facts_project ON semantic_facts(project_id);
CREATE INDEX idx_episodic_memories_project ON episodic_memories(project_id);
CREATE INDEX idx_procedural_rules_project ON procedural_rules(project_id);
```

### 1.5 Session Enhancement

Edit existing session schema:

```sql
-- Add to sessions table
project_id        TEXT REFERENCES projects(id),
conversation_type TEXT DEFAULT 'general',  -- 'general', 'task_discussion', 'review', 'planning', 'retrospective'
pinned            INTEGER DEFAULT 0
```

---

## 2. Context Engine & AI Pipeline

### 2.1 ProjectContextSource

New `ContextSource` implementation, priority 80 (above AreaSource at 75).

**Propagating `project_id` into the context pipeline:** `SourceContext` currently has `channel`, `chat_id`, `message`, `intent_summary`. Add a new field:

```rust
pub struct SourceContext {
    // ...existing fields...
    pub project_id: Option<String>,  // NEW — set when conversation is within a project
}
```

This is populated by the caller (agent runtime) when the session has a `project_id`. The `ProjectContextSource` reads it to scope all queries.

```rust
pub struct ProjectContextSource {
    repos: Repos,
    source_retriever: SourceRetriever,  // wraps Arc<lancedb::Connection> (from VectorStore)
}

impl ContextSource for ProjectContextSource {
    fn name(&self) -> &str { "project" }
    fn priority(&self) -> u8 { 80 }

    async fn provide(&self, ctx: &SourceContext) -> Option<String> {
        let project_id = ctx.project_id.as_deref()?;
        // 1. Load project instructions (JSON → formatted sections)
        // 2. Load user_role description
        // 3. RAG search project_sources + project-linked notes
        // 4. Load project-scoped semantic_facts
        // 5. Assemble context block
    }
}
```

Note: AreaSource (priority 75) runs after ProjectContextSource (priority 80), so area context is still available in the prompt. ProjectContextSource does not depend on area context — it queries project data directly.

Context assembly order in the prompt:
1. Project instructions (context → guidelines → constraints)
2. User role description
3. AI personality override (if set)
4. Relevant project sources (RAG, max ~2000 tokens)
5. Project-scoped memories (max ~500 tokens)

### 2.2 Source Retriever

RAG over project sources + project-linked notes using LanceDB:

```rust
pub struct SourceRetriever {
    vector_store: VectorStore,  // existing type, wraps Arc<lancedb::Connection>
}

impl SourceRetriever {
    pub async fn search(
        &self,
        project_id: &str,
        query: &str,
        top_k: usize,
    ) -> Vec<RetrievedSource>;
}
```

On source/note add or update:
1. Extract text content
2. Embed via existing pipeline
3. Store vector in LanceDB with `project_id` metadata
4. Query: filter by `project_id`, vector similarity, return top-k

Global LanceDB index with `project_id` filter (not per-project indexes).

### 2.3 Memory Extraction Enhancement

Extend existing cognitive pipeline:
- Extracted `SemanticFact` entries inherit `project_id` from the conversation
- Decision detection: phrases like "decided to", "let's go with" → `memory_type: 'decision'`
- Milestone detection: completions, releases → `memory_type: 'milestone'`
- `CognitiveContextSource` gains project filter for dynamic retrieval

### 2.4 Conversation Context Flow

```
User message in project context
    → ContextEngine assembles prompt:
        1. IdentitySource (agent persona)
        2. ProjectContextSource (instructions, role, sources, memories)
        3. AreaSource (area context)
        4. CognitiveContextSource (global + project-filtered)
        5. TodoSource, ProductivitySource, etc.
    → LLM generates response
    → Post-conversation pipeline:
        1. Extract facts → store with project_id
        2. Detect decisions/milestones → tag memory_type
        3. Auto-link conversation to discussed entities
        4. Update source relevance scores
```

### 2.5 Token Budget

The existing `BudgetAllocator` uses a `Priority` enum with waterfall allocation — not percentage-based. The numbers below are **illustrative size guidance** for each component, not a new API. `ProjectContextSource` should use `Priority::High` to ensure project context is included before lower-priority sources.

```
Approximate token sizes per component:
├── Agent persona: ~500 tokens
├── Project instructions: ~800 tokens (hard cap via truncation)
├── Project sources (RAG): ~1500 tokens (top 3-5 results)
├── Project memories: ~400 tokens (top 10 facts)
├── User role: ~200 tokens
└── Other context sources: as budget allows
```

---

## 3. Repository & Handler Layer

### 3.1 New Repositories

**EntityLinkRepo** — generic storage, typed API:

```rust
pub struct EntityLinkRepo { pool: SqlitePool }

impl EntityLinkRepo {
    // Generic
    pub async fn create(&self, link: EntityLink) -> Result<EntityLink>;
    pub async fn delete(&self, id: &str) -> Result<()>;
    pub async fn list_by_entity(&self, kind: EntityKind, id: &str) -> Result<Vec<EntityLink>>;

    // Typed convenience — all default to link_type "related"
    pub async fn link_task_to_note(&self, task_id: &str, note_id: &str) -> Result<EntityLink>;
    pub async fn link_conversation_to_task(&self, session_key: &str, task_id: &str) -> Result<EntityLink>;
    pub async fn link_conversation_to_note(&self, session_key: &str, note_id: &str) -> Result<EntityLink>;
    // For specific link types (e.g. "created_from", "discusses"), use create() directly

    // Contextual surfacing
    pub async fn get_linked_entities(&self, kind: EntityKind, id: &str) -> Result<LinkedEntities>;

    // Project-wide
    pub async fn get_project_links(&self, project_id: &str) -> Result<Vec<EntityLink>>;
}

/// Summary types for cross-entity linking — lightweight versions of full entities.
pub struct ActionSummary {
    pub id: String,
    pub title: String,
    pub status: String,
    pub priority: Option<String>,
}

pub struct NoteSummary {
    pub id: String,
    pub title: String,
    pub updated_at: String,
}

pub struct SessionSummary {
    pub key: String,  // session primary key
    pub title: Option<String>,
    pub conversation_type: Option<String>,
    pub updated_at: String,
}

pub struct ObjectiveSummary {
    pub id: String,
    pub title: String,
    pub progress: f64,
    pub status: String,
}

pub struct KeyResultSummary {
    pub id: String,
    pub title: String,
    pub progress: f64,
}

pub struct LinkedEntities {
    pub tasks: Vec<ActionSummary>,
    pub notes: Vec<NoteSummary>,
    pub conversations: Vec<SessionSummary>,
    pub sources: Vec<ProjectSource>,
    pub objectives: Vec<ObjectiveSummary>,
    pub key_results: Vec<KeyResultSummary>,
}
```

**ProjectSourceRepo:**

```rust
pub struct ProjectSourceRepo { pool: SqlitePool }

impl ProjectSourceRepo {
    pub async fn create(&self, source: ProjectSource) -> Result<ProjectSource>;
    pub async fn get(&self, id: &str) -> Result<ProjectSource>;
    pub async fn list_by_project(&self, project_id: &str) -> Result<Vec<ProjectSource>>;
    pub async fn update(&self, id: &str, patch: ProjectSourcePatch) -> Result<ProjectSource>;
    pub async fn delete(&self, id: &str) -> Result<()>;
    pub async fn search(&self, project_id: &str, query: &str) -> Result<Vec<ProjectSource>>;
}
```

### 3.2 Extended Repositories

**ProjectRepo** — new fields in `ProjectRow`:

```rust
pub struct ProjectRow {
    // ...existing...
    pub instructions: Option<String>,
    pub ai_personality: Option<String>,
    pub user_role: Option<String>,
    pub start_date: Option<String>,
    pub target_end_date: Option<String>,
    pub settings: Option<String>,
}
```

**SemanticFactRepo** — project-scoped queries:

```rust
pub async fn list_by_project(&self, project_id: &str) -> Result<Vec<SemanticFact>>;
pub async fn list_by_project_and_type(&self, project_id: &str, memory_type: &str) -> Result<Vec<SemanticFact>>;
```

**SessionRepo** — project-scoped queries:

```rust
pub async fn list_by_project(&self, project_id: &str) -> Result<Vec<SessionRow>>;
```

### 3.3 Repos Aggregate

```rust
pub struct Repos {
    // ...existing 22 repos...
    pub entity_links: EntityLinkRepo,
    pub project_sources: ProjectSourceRepo,
}
```

### 3.4 AppCore Handlers

**Project (extended):**
- `project_update_instructions(id, instructions) -> HandlerResult<ProjectResponse>`
- `project_update_role(id, role) -> HandlerResult<ProjectResponse>`
- `project_get_context_preview(id) -> Result<String>`

**Entity links:**
- `entity_link_create(params) -> HandlerResult<EntityLink>`
- `entity_link_delete(id) -> HandlerResult<bool>`
- `entity_links_for_entity(kind, id) -> Result<LinkedEntities>`

**Project sources:**
- `project_source_create(params) -> HandlerResult<ProjectSource>`
- `project_source_update(params) -> HandlerResult<ProjectSource>`
- `project_source_delete(id) -> HandlerResult<bool>`
- `project_source_list(project_id) -> Result<Vec<ProjectSource>>`

**Project memories:**
- `project_memories_list(project_id) -> Result<Vec<SemanticFact>>`
- `project_memories_by_type(project_id, type) -> Result<Vec<SemanticFact>>`

**Project conversations:**
- `project_conversations_list(project_id) -> Result<Vec<SessionSummary>>`
- `project_conversation_create(params) -> HandlerResult<SessionSummary>`

### 3.5 IPC Types

```rust
pub struct ProjectResponse {
    // ...existing...
    pub instructions: Option<ProjectInstructions>,
    pub ai_personality: Option<String>,
    pub user_role: Option<String>,
    pub start_date: Option<String>,
    pub target_end_date: Option<String>,
    pub settings: Option<serde_json::Value>,
}

pub struct ProjectInstructions {
    pub context: Option<String>,
    pub guidelines: Option<String>,
    pub constraints: Option<String>,
    pub persona: Option<String>,
}

// EntityKind gains new variants. Update EntityKind::parse() mapping:
//   "source" → EntityKind::Source
//   "conversation" → EntityKind::Conversation
pub enum EntityKind {
    // ...existing (Task, Project, Objective, Area, KeyResult, FocusSession, Productivity, Note, Notebook, Finance)...
    Source,
    Conversation,
}
```

### 3.6 Tauri Commands

Thin wrappers following existing pattern (delegate to AppCore, emit entity updates):
- `project_update_instructions`, `project_update_role`
- `entity_link_create`, `entity_link_delete`, `entity_links_for_entity`
- `project_source_create`, `project_source_update`, `project_source_delete`, `project_source_list`
- `project_memories_list`, `project_memories_by_type`
- `project_conversations_list`, `project_conversation_create`
- `project_context_preview`

---

## 4. UI Design

### 4.1 Layout: 3-Panel Dashboard

The project detail page is a **living dashboard** with three panels:

```
┌──────────────────────────────────────────────────────────────────┐
│  ← Back  ● Mobile App Project  [Active]  Day | Week | Month     │
├────────────┬─────────────────────────────────────┬───────────────┤
│            │                                     │               │
│  LEFT      │  CENTER — Timeline Dashboard        │  RIGHT        │
│  PANEL     │                                     │  PANEL        │
│            │  Time │ Activity │ Tasks │ Notes │   │               │
│ ⚙ Instr.  │  9AM  │          │       │       │   │ 🎯 OKRs ▾    │
│ 📚 Sources │ 10AM  │ ██ dev   │ ✓task │ 📓note│   │ ☑ Tasks ▾    │
│ 👤 Role    │ 11AM  │          │       │       │   │ 📓 Notes ▾   │
│            │ 12PM  │          │       │       │   │ 🧠 Memory ▸  │
│ ──────     │  1PM  │ ██ docs  │ □task │       │   │ 📚 Sources ▸ │
│ Convos     │  2PM  │ ██ impl  │       │ 📓note│   │ 📊 Prod ▸    │
│  thread1   │  3PM  │          │ □task │       │   │               │
│  thread2   │  4PM  │ ██ review│       │       │   │               │
│  + New     │       │          │       │       │   │               │
│            │                                     │               │
├────────────┴─────────────────────────────────────┴───────────────┤
│  ⚙  Ask anything...                              ⚙ Expert ▾  ↑  │
└──────────────────────────────────────────────────────────────────┘
```

**Left panel** (Grok-style, ~230px):
- Instructions card → opens layered section editor (Tiptap)
- Sources card → opens source management panel
- My Role card → opens freeform text editor
- Separator
- Conversations list → chronological threads, click to open in main chat
- "+ New conversation" button

**Center panel** (Timeline dashboard):
- Column headers: Activity, Tasks, Notes, Memories
- Time grid (hourly rows) — items placed at creation/update time
- Day/Week/Month toggle in header
- Current time indicator (red line)
- Reuses `DayCalendarView` patterns from existing dashboard

**Right panel** (Entity inventory, ~230px):
- Expandable/collapsible sections: OKRs, Tasks, Notes, Memories, Sources, Productivity
- OKRs: progress bars per objective
- Tasks: status checkboxes, priority dots, overflow count
- Notes: title, timestamps, link counts
- Memories: type-tagged (Decision, Insight, Milestone, Pattern)
- Sources: external files/links/snippets
- Productivity: velocity, focus score, AI insights

**Bottom bar** (persistent):
- Chat input always visible — messages go to selected conversation or create new one
- Project context automatically active

### 4.2 Panel Interactions

- **Left panel cards** (Instructions, Sources, Role) → click opens a slide panel or modal editor over the center area
- **Conversation threads** → click switches the center area to show conversation (or opens in existing sidebar chat)
- **Timeline items** → click opens entity detail (slide panel or navigation)
- **Right panel items** → click navigates to entity or opens inline detail
- **Entity link indicators** (🔗) → click shows linked entities popover

### 4.3 Conversations Model (Hybrid)

Two ways to have conversations:
1. **Project chat space** — threads in left panel (General, Planning, Retrospective). For project-wide discussions.
2. **Contextual conversations** — started from a specific entity (task, note, OKR). Linked to that entity via `entity_links`. Visible in both the left panel thread list and the entity's link popover.

All conversations within a project automatically get full project context (instructions, sources, memories, role).

---

## 5. Component Architecture

```
desktop-ui/src/features/tasks/pages/
├── ProjectDetailPage.tsx          (rewritten — 3-panel dashboard)

desktop-ui/src/features/tasks/components/
├── project-detail/
│   ├── ProjectHeader.tsx          (back, color, name, status, time toggle)
│   ├── ProjectLeftPanel.tsx       (setup cards + conversation list)
│   ├── ProjectTimeline.tsx        (center timeline grid)
│   ├── ProjectEntityPanel.tsx     (right panel — expand/collapse sections)
│   ├── ProjectChatInput.tsx       (bottom persistent chat bar)
│   │
│   ├── panels/
│   │   ├── InstructionsPanel.tsx  (layered section editor, Tiptap reuse)
│   │   ├── SourcesPanel.tsx      (upload, link, snippet management)
│   │   ├── RolePanel.tsx         (freeform text editor)
│   │   └── ContextPreview.tsx    ("What AI sees" preview modal)
│   │
│   ├── timeline/
│   │   ├── ActivityColumn.tsx    (work context blocks)
│   │   ├── TasksColumn.tsx       (task cards on timeline)
│   │   ├── NotesColumn.tsx       (note cards on timeline)
│   │   └── MemoriesColumn.tsx    (memory cards on timeline)
│   │
│   └── entity-sections/
│       ├── OkrSection.tsx        (expand/collapse, progress bars)
│       ├── TaskSection.tsx       (task list with status/priority)
│       ├── NoteSection.tsx       (note list with link counts)
│       ├── MemorySection.tsx     (filterable by type)
│       ├── SourceSection.tsx     (external sources list)
│       └── ProductivitySection.tsx (stats + AI insights)
```

**Reuse from existing code:**
- Timeline grid → adapts `DayCalendarView` patterns
- Instructions editor → reuses Tiptap from notes feature
- Task/OKR rendering → reuses existing components
- Chat input → adapts `ChatInput` from chat feature

**New shared hooks:**
- `useEntityLinks(kind, id)` → `LinkedEntities`
- `useProjectSources(projectId)` → CRUD
- `useProjectMemories(projectId, type?)` → filtered memories
- `useProjectConversations(projectId)` → conversation threads

---

## 6. Implementation Phases

### Phase 1: Cross-Feature Unification

**Storage:**
- Edit `001_initial.sql` — add `entity_links`, `project_sources` tables, new columns on `projects`, `sessions`
- Edit cognitive `001_cognitive_tables.sql` — add `project_id`, `memory_type`
- Reset database

**Repos:**
- `EntityLinkRepo` with typed convenience methods + `get_linked_entities()`
- `ProjectSourceRepo` with CRUD
- Extend `ProjectRepo`, `SemanticFactRepo`, `SessionRepo` with project-scoped queries
- Add to `Repos` aggregate

**Handlers + Commands:**
- Entity link CRUD handlers + Tauri commands
- Project source CRUD handlers + Tauri commands
- Enhanced `ProjectResponse` with new fields

**Frontend:**
- `useEntityLinks` hook
- Link indicators on task/note list items
- Entity link popover component

### Phase 2: Project as AI Context

**Context Engine:**
- `ProjectContextSource` (priority 80)
- `SourceRetriever` — LanceDB RAG over project sources + project-linked notes
- Embed project sources on create/update (async)

**Cognitive Pipeline:**
- Project-scoped fact extraction (inherit `project_id` from conversation)
- Decision/milestone detection → `memory_type` tagging
- Project filter in `CognitiveContextSource` dynamic retrieval

**Handlers:**
- `project_update_instructions`, `project_update_role`
- `project_get_context_preview`
- `project_memories_list`, `project_memories_by_type`

**Frontend:**
- Instructions panel (Tiptap, layered sections)
- Sources panel (upload, link, snippet)
- Role panel (freeform text)
- Context Preview modal

### Phase 3: Enhanced Project Detail UI

**Frontend (major):**
- Rewrite `ProjectDetailPage` → 3-panel layout
- `ProjectLeftPanel` — setup cards + conversations
- `ProjectTimeline` — multi-column time grid (adapts DayCalendarView)
- `ProjectEntityPanel` — expand/collapse entity sections
- `ProjectChatInput` — persistent bottom bar
- Timeline columns: Activity, Tasks, Notes, Memories
- Entity sections: OKRs, Tasks, Notes, Memories, Sources, Productivity

**Handlers:**
- `project_conversations_list`, `project_conversation_create`
- Timeline data aggregation endpoint

### Phase 4: Role & Intelligence

**Backend:**
- Role-aware context injection — adjust AI tone/suggestions based on `user_role`
- Project memory type views (Decisions, Milestones, Insights)
- AI insights generation for productivity section
- Auto-linking conversations to discussed entities

**Frontend:**
- Memory type filter in entity panel
- Productivity stats + AI insights section
- Decision timeline view
- Coaching adjustments per project role
