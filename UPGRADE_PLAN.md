# Klyntbot Major Upgrade Plan — Context Hub Integration

> **Status**: Pre-production. Breaking changes accepted.
> **Scope**: 7 upgrades across 12 crates, ~40 files, ~4000 LOC net new.
> **Inspired by**: [context-hub](https://github.com/andrewyng/context-hub) patterns + Agent Skills spec.

---

## Table of Contents

1. [Overview](#1-overview)
2. [Upgrade 1: BM25 Full-Text Search](#2-upgrade-1-bm25-full-text-search)
3. [Upgrade 2: Annotation System](#3-upgrade-2-annotation-system)
4. [Upgrade 3: Progressive Context Loading](#4-upgrade-3-progressive-context-loading)
5. [Upgrade 4: Tool Registry with Rich Metadata](#5-upgrade-4-tool-registry-with-rich-metadata)
6. [Upgrade 5: Agent Skills Spec Compatibility](#6-upgrade-5-agent-skills-spec-compatibility)
7. [Upgrade 6: MCP Server Enhancement](#7-upgrade-6-mcp-server-enhancement)
8. [Upgrade 7: Multi-Source Content Registry](#8-upgrade-7-multi-source-content-registry)
9. [Implementation Order & Dependencies](#9-implementation-order--dependencies)
10. [Migration Checklist](#10-migration-checklist)

---

## 1. Overview

### Design Philosophy

Replace custom-only patterns with **industry-standard + klyntbot extensions**. Every upgrade follows:

1. **Adopt the standard first** (Agent Skills spec, BM25, FTS5)
2. **Extend where klyntbot needs more** (triggers, FSRS decay, trust filtering)
3. **Replace old patterns entirely** (no backward compat wrappers — we're pre-production)

### Architecture After Upgrade

```
L0: common                         — (unchanged)
L1: config, bus, tools-core        — +ToolMetadata, +ToolCategory, +AgentSkillsMeta
L2: storage, domain                — +FTS5 tables, +annotation tables, +content registry tables
L3: providers, session, scheduling, context_engine — +ContextInventory, +progressive loading, +context_request tool
L4: tools, features, plugin-runtime — +rich metadata on all tools
L5: channels, agent, cognitive     — +annotation CRUD, +runtime skill loader, +BM25 retrieval
L6: mcp                            — +expanded server (10+ tools exposed)
L7: app-core, desktop-shared       — +init for new subsystems, +IPC types
L8: klyntbot                       — (unchanged facade)
```

---

## 2. Upgrade 1: BM25 Full-Text Search

### Problem

Current keyword search uses `LIKE '%query%'` — no ranking, no tokenization, full table scan. Vector search requires pre-computed embeddings (expensive). No lightweight text search for tools, skills, knowledge.

### Solution

Replace `LIKE` with **SQLite FTS5** virtual tables + BM25 ranking. Add as a third signal alongside existing vector search and keyword matching.

### Changes

#### 2.1 New migration: `storage/migrations/NNNN_add_fts5_tables.sql`

```sql
-- Full-text index for semantic facts
CREATE VIRTUAL TABLE IF NOT EXISTS semantic_facts_fts USING fts5(
    id UNINDEXED,
    domain,
    subject,
    predicate,
    object,
    memory_type,
    content='semantic_facts',
    content_rowid='rowid',
    tokenize='porter unicode61'
);

-- Triggers to keep FTS in sync
CREATE TRIGGER semantic_facts_ai AFTER INSERT ON semantic_facts BEGIN
    INSERT INTO semantic_facts_fts(rowid, id, domain, subject, predicate, object, memory_type)
    VALUES (new.rowid, new.id, new.domain, new.subject, new.predicate, new.object, new.memory_type);
END;

CREATE TRIGGER semantic_facts_ad AFTER DELETE ON semantic_facts BEGIN
    INSERT INTO semantic_facts_fts(semantic_facts_fts, rowid, id, domain, subject, predicate, object, memory_type)
    VALUES ('delete', old.rowid, old.id, old.domain, old.subject, old.predicate, old.object, old.memory_type);
END;

CREATE TRIGGER semantic_facts_au AFTER UPDATE ON semantic_facts BEGIN
    INSERT INTO semantic_facts_fts(semantic_facts_fts, rowid, id, domain, subject, predicate, object, memory_type)
    VALUES ('delete', old.rowid, old.id, old.domain, old.subject, old.predicate, old.object, old.memory_type);
    INSERT INTO semantic_facts_fts(rowid, id, domain, subject, predicate, object, memory_type)
    VALUES (new.rowid, new.id, new.domain, new.subject, new.predicate, new.object, new.memory_type);
END;

-- FTS for episodic memories
CREATE VIRTUAL TABLE IF NOT EXISTS episodic_memories_fts USING fts5(
    id UNINDEXED,
    domain,
    content,
    summary,
    content='episodic_memories',
    content_rowid='rowid',
    tokenize='porter unicode61'
);

-- (Similar triggers for episodic_memories)

-- FTS for procedural rules
CREATE VIRTUAL TABLE IF NOT EXISTS procedural_rules_fts USING fts5(
    id UNINDEXED,
    domain,
    rule_text,
    content='procedural_rules',
    content_rowid='rowid',
    tokenize='porter unicode61'
);

-- (Similar triggers for procedural_rules)

-- FTS for annotations (see Upgrade 2)
CREATE VIRTUAL TABLE IF NOT EXISTS annotations_fts USING fts5(
    id UNINDEXED,
    target_type,
    target_id,
    content,
    tags,
    tokenize='porter unicode61'
);
```

#### 2.2 New module: `crates/cognitive/src/search/bm25.rs`

```rust
/// BM25 search result with score
pub struct Bm25Result {
    pub id: String,
    pub score: f64,
    pub snippet: String,
}

/// Query all FTS5 tables and merge results using RRF
pub async fn bm25_search(
    pool: &SqlitePool,
    query: &str,
    limit: usize,
) -> Result<Vec<Bm25Result>> {
    // Uses: SELECT id, rank FROM semantic_facts_fts WHERE semantic_facts_fts MATCH ?1
    //       ORDER BY rank LIMIT ?2
    // FTS5 rank is negative BM25 (lower = better match)
}
```

#### 2.3 Replace in `crates/cognitive/src/repos/semantic_fact.rs`

**Current** (line ~195-240):
```rust
// LIKE-based search — replace entirely
pub async fn search_archived(&self, query: &str, ...) -> Result<Vec<SemanticFact>> {
    sqlx::query_as("... WHERE (subject LIKE ?1 OR predicate LIKE ?1 OR object LIKE ?1) ...")
}
```

**New**:
```rust
pub async fn search_fts(&self, query: &str, domain: Option<&str>, limit: usize) -> Result<Vec<SemanticFact>> {
    let sql = r#"
        SELECT f.* FROM semantic_facts f
        INNER JOIN semantic_facts_fts fts ON f.id = fts.id
        WHERE semantic_facts_fts MATCH ?1
        AND (?2 IS NULL OR f.domain = ?2)
        AND f.superseded_at IS NULL
        ORDER BY fts.rank
        LIMIT ?3
    "#;
    sqlx::query_as(sql).bind(query).bind(domain).bind(limit as i64)
        .fetch_all(&*self.pool).await.map_err(Into::into)
}
```

#### 2.4 Upgrade hybrid retrieval in `crates/cognitive/src/retrieval.rs`

**Current**: `rrf_merge(keyword_results, vector_results)` — 2 signals.

**New**: `rrf_merge_triple(bm25_results, vector_results, decay_results)` — 3 signals.

```rust
/// Triple-source Reciprocal Rank Fusion
pub fn rrf_merge_triple(
    bm25: &[ScoredMemory],
    vector: &[ScoredMemory],
    decay: &[ScoredMemory],  // Sorted by FSRS retrievability
    k: f64,  // Default 60.0
) -> Vec<ScoredMemory> {
    let mut scores: HashMap<String, f64> = HashMap::new();
    for (rank, item) in bm25.iter().enumerate() {
        *scores.entry(item.id.clone()).or_default() += 1.0 / (k + rank as f64 + 1.0);
    }
    for (rank, item) in vector.iter().enumerate() {
        *scores.entry(item.id.clone()).or_default() += 1.0 / (k + rank as f64 + 1.0);
    }
    for (rank, item) in decay.iter().enumerate() {
        *scores.entry(item.id.clone()).or_default() += 0.5 / (k + rank as f64 + 1.0); // Lower weight for decay
    }
    // Sort by RRF score descending, return top N
}
```

### Files Changed

| File | Action |
|------|--------|
| `crates/storage/migrations/NNNN_fts5.sql` | **New** — FTS5 virtual tables + triggers |
| `crates/cognitive/src/search/bm25.rs` | **New** — BM25 query functions |
| `crates/cognitive/src/search/mod.rs` | **New** — Module declaration |
| `crates/cognitive/src/repos/semantic_fact.rs` | **Replace** `search_archived` with `search_fts` |
| `crates/cognitive/src/repos/episodic_memory.rs` | **Add** `search_fts` method |
| `crates/cognitive/src/repos/procedural_rule.rs` | **Add** `search_fts` method |
| `crates/cognitive/src/retrieval.rs` | **Replace** `rrf_merge` → `rrf_merge_triple` |
| `crates/tools-core/src/search.rs` | **Update** `rrf_merge` to support 3 signals |

---

## 3. Upgrade 2: Annotation System

### Problem

Cognitive system has structured memory (S-P-O facts, FSRS) but no **ad-hoc, human-readable notes** attached to specific contexts. Context-hub shows this pattern works: agents annotate gotchas after tasks, annotations appear on next retrieval.

### Solution

New `annotations` table. Annotations attach to any entity (tool, fact, rule, skill, API, project). Agent auto-creates annotations after task completion. Annotations surface during context assembly.

### Changes

#### 3.1 New migration: `storage/migrations/NNNN_add_annotations.sql`

```sql
CREATE TABLE IF NOT EXISTS annotations (
    id TEXT PRIMARY KEY,
    target_type TEXT NOT NULL,     -- 'tool', 'fact', 'rule', 'skill', 'api', 'project', 'custom'
    target_id TEXT NOT NULL,       -- Tool name, fact ID, skill name, etc.
    content TEXT NOT NULL,         -- Plain text annotation
    tags TEXT DEFAULT '',          -- Comma-separated tags
    author TEXT NOT NULL DEFAULT 'agent',  -- 'agent' or 'user'
    priority INTEGER DEFAULT 0,   -- 0=normal, 1=important, 2=critical
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL,
    expires_at TEXT,               -- Optional TTL
    access_count INTEGER DEFAULT 0,
    UNIQUE(target_type, target_id, content)  -- Prevent exact duplicates
);

CREATE INDEX idx_annotations_target ON annotations(target_type, target_id);
CREATE INDEX idx_annotations_tags ON annotations(tags);
```

#### 3.2 New repo: `crates/storage/src/repos/annotation.rs`

```rust
pub struct AnnotationRepo { pool: SqlitePool }

impl AnnotationRepo {
    pub async fn upsert(&self, annotation: &Annotation) -> Result<()>;
    pub async fn get_for_target(&self, target_type: &str, target_id: &str) -> Result<Vec<Annotation>>;
    pub async fn search(&self, query: &str) -> Result<Vec<Annotation>>;  // FTS5
    pub async fn list_all(&self) -> Result<Vec<Annotation>>;
    pub async fn delete(&self, id: &str) -> Result<bool>;
    pub async fn delete_expired(&self) -> Result<u64>;
    pub async fn increment_access(&self, id: &str) -> Result<()>;
}
```

#### 3.3 New type: `crates/cognitive/src/types.rs`

```rust
#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct Annotation {
    pub id: String,
    pub target_type: String,
    pub target_id: String,
    pub content: String,
    pub tags: String,
    pub author: String,
    pub priority: i32,
    pub created_at: String,
    pub updated_at: String,
    pub expires_at: Option<String>,
    pub access_count: i64,
}
```

#### 3.4 New tool: `crates/tools/src/annotate.rs`

```rust
#[derive(Tool)]
#[tool(
    name = "annotate",
    description = "Create, read, or manage annotations — persistent notes attached to tools, APIs, skills, or any context. Use after completing a task to record gotchas, workarounds, or learnings for future sessions."
)]
pub struct AnnotateTool { repo: AnnotationRepo }

// Actions: create, get, list, delete, search
```

#### 3.5 Inject annotations into context: `crates/agent/src/context_sources/`

New `AnnotationContextSource`:
- At context assembly time, query annotations for:
  - Active tools (by tool name)
  - Active skills (by skill name)
  - Current project (by project ID)
  - Critical priority annotations (priority >= 2)
- Format as system message section: `"[Active Annotations]\n- {target}: {content}\n..."`
- Priority: between `RetrievedMemory` and `CompressedHistory`

#### 3.6 Self-improving loop: auto-annotate after task completion

In `AgentRuntime::process_message()`, after successful execution:
- If the response mentions a workaround, gotcha, or version-specific behavior
- And the agent used tools during execution
- Generate annotation suggestion via lightweight LLM call
- Auto-create annotation (agent can also create manually via `annotate` tool)

### Files Changed

| File | Action |
|------|--------|
| `crates/storage/migrations/NNNN_annotations.sql` | **New** |
| `crates/storage/src/repos/annotation.rs` | **New** |
| `crates/storage/src/repos/mod.rs` | **Add** module |
| `crates/cognitive/src/types.rs` | **Add** `Annotation` struct |
| `crates/tools/src/annotate.rs` | **New** — annotate tool |
| `crates/tools/src/lib.rs` | **Add** module |
| `crates/agent/src/context_sources/annotation.rs` | **New** — AnnotationContextSource |
| `crates/agent/src/context_sources/mod.rs` | **Add** module |
| `crates/agent/src/agent_runtime/runtime.rs` | **Add** post-execution annotation logic |
| `crates/app-core/src/lib.rs` | **Add** AnnotationRepo init + wire to tools |

---

## 4. Upgrade 3: Progressive Context Loading

### Problem

Context assembled **once** before execution. All 11 sources run in parallel via `join_all`, stuffed into system prompt. If budget runs out, lower-priority sources silently dropped. Agent has no way to request more context mid-execution.

### Solution

Two-phase context: **initial assembly** (lightweight) + **on-demand expansion** (agent-driven via tool).

### Changes

#### 4.1 New struct: `ContextInventory`

Add to `crates/context_engine/src/assembler.rs`:

```rust
/// Tracks what context is loaded vs. available but not loaded.
#[derive(Clone, Debug)]
pub struct ContextInventory {
    pub items: Vec<ContextInventoryItem>,
}

#[derive(Clone, Debug)]
pub struct ContextInventoryItem {
    pub source_name: String,
    pub priority: Priority,
    pub status: ContextItemStatus,
    pub token_estimate: usize,
    pub summary: Option<String>,  // One-line description of what this source provides
}

#[derive(Clone, Debug)]
pub enum ContextItemStatus {
    Loaded { tokens_used: usize },
    Deferred { reason: String },     // Budget insufficient, will load on request
    Available { description: String }, // Source exists but wasn't queried
}
```

#### 4.2 Extend `AssembledContext`

```rust
pub struct AssembledContext {
    pub messages: Vec<Message>,
    pub token_count: usize,
    pub budget_report: BudgetReport,
    pub inventory: ContextInventory,          // NEW
    pub budget_remaining: usize,              // NEW — tokens still available
    pub version: u32,                         // NEW — incremented on refresh
}
```

#### 4.3 New method: `ContextEngine::expand()`

```rust
impl ContextEngine {
    /// Expand context by loading a deferred source or refreshing a stale one.
    pub async fn expand(
        &self,
        current: &AssembledContext,
        source_name: &str,
        request: &ContextRequest,
    ) -> Result<AssembledContext> {
        // 1. Find the source by name
        // 2. Call source.provide() with current SourceContext
        // 3. Allocate from budget_remaining
        // 4. Insert into messages at correct priority position
        // 5. Update inventory item status → Loaded
        // 6. Increment version
        // 7. Return new AssembledContext
    }

    /// Retrieve additional memories mid-execution based on new query.
    pub async fn retrieve_additional_memory(
        &self,
        current: &AssembledContext,
        query: &str,
        limit: usize,
    ) -> Result<AssembledContext> {
        // Re-run memory retrieval with new query
        // Append new memories (deduplicate against existing)
        // Update budget
    }
}
```

#### 4.4 New tool: `context_request`

```rust
#[derive(Tool)]
#[tool(
    name = "context_request",
    description = "Request additional context mid-execution. Use when you need more information from a specific context source (e.g., project details, additional memories, user history) to complete the current task."
)]
pub struct ContextRequestTool {
    context_engine: Arc<ContextEngine>,
    assembled: Arc<RwLock<AssembledContext>>,
}

// Params: { source: String, query: Option<String> }
// Returns: The newly loaded context content, or "already loaded" / "budget insufficient"
```

#### 4.5 Modify `ReactiveEngine` loop

In `crates/agent/src/intent_pipeline/engines/reactive.rs`:

```rust
// Before each iteration:
// 1. Check if context_version changed (another tool may have triggered expansion)
// 2. If changed, rebuild messages from latest AssembledContext
// 3. Pass assembled context Arc to tools so context_request can mutate it
```

#### 4.6 Inject inventory into system prompt

Add to `build_system_prompt()`:

```
[Available Context - request via context_request tool if needed]
- Project details (deferred - 2.1k tokens estimated)
- Recent episodic memories (loaded - 1.8k tokens)
- Active annotations (loaded - 0.4k tokens)
- User behavioral patterns (deferred - 0.9k tokens)
Budget remaining: 12.4k tokens
```

### Files Changed

| File | Action |
|------|--------|
| `crates/context_engine/src/assembler.rs` | **Major** — add ContextInventory, expand(), retrieve_additional_memory() |
| `crates/context_engine/src/budget.rs` | **Add** remaining budget tracking |
| `crates/context_engine/src/source.rs` | **Add** `estimated_tokens()` to ContextSource trait |
| `crates/tools/src/context_request.rs` | **New** — context_request tool |
| `crates/agent/src/intent_pipeline/engines/reactive.rs` | **Modify** — check context version per iteration |
| `crates/agent/src/agent_runtime/runtime.rs` | **Modify** — pass Arc<RwLock<AssembledContext>> to execution |
| `crates/agent/src/context_sources/*.rs` | **Add** `estimated_tokens()` impl to each source |

---

## 5. Upgrade 4: Tool Registry with Rich Metadata

### Problem

ToolRegistry is `HashMap<String, DynTool>`. No category, tags, examples, usage tracking, search. Agent discovers tools only by scanning all tool definitions in system prompt.

### Solution

Extend `Tool` trait with optional metadata. Add `ToolMetadata` struct. BM25-index tool metadata for agent discovery.

### Changes

#### 5.1 New types in `crates/tools-core/src/lib.rs`

```rust
/// Rich metadata for tool discovery and categorization.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ToolMetadata {
    pub category: ToolCategory,
    pub tags: Vec<String>,
    pub author: String,
    pub version: String,
    pub source: ToolSource,
    pub examples: Vec<ToolExample>,
    pub related_tools: Vec<String>,
    pub cost_hint: CostHint,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub enum ToolCategory {
    #[default]
    General,
    FileSystem,
    Search,
    Web,
    Communication,
    TaskManagement,
    Memory,
    Finance,
    Productivity,
    System,
    Mcp,
    Plugin,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub enum ToolSource {
    #[default]
    Native,
    Feature(String),    // Feature package name
    Mcp(String),        // MCP server name
    Plugin(String),     // Plugin name
    External,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolExample {
    pub description: String,
    pub params: serde_json::Value,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub enum CostHint {
    #[default]
    Free,         // No external API calls
    Low,          // < 1 cent per call
    Medium,       // 1-10 cents per call
    High,         // > 10 cents per call
    Variable,     // Depends on input
}
```

#### 5.2 Extend `Tool` trait

```rust
pub trait Tool: Send + Sync {
    fn name(&self) -> &str;
    fn description(&self) -> &str;
    fn parameters(&self) -> Value;
    async fn execute(&self, args: Value, ctx: &RoutingContext) -> Result<String>;
    fn permission_level(&self) -> PermissionLevel { PermissionLevel::Standard }
    fn custom_timeout(&self) -> Option<Duration> { None }

    // NEW — rich metadata for discovery
    fn metadata(&self) -> ToolMetadata { ToolMetadata::default() }
}
```

#### 5.3 Extend `ToolRegistry`

```rust
pub struct ToolRegistry {
    tools: HashMap<String, DynTool>,
    metadata: HashMap<String, ToolMetadata>,     // NEW
    usage_counts: HashMap<String, u64>,           // NEW
    cached_definitions: Mutex<Option<Arc<Vec<Value>>>>,
    permissions: Option<ToolPermissions>,
}

impl ToolRegistry {
    /// Search tools by query using BM25 over name + description + tags
    pub fn search(&self, query: &str, limit: usize) -> Vec<ToolSearchResult>;

    /// Get tools by category
    pub fn by_category(&self, category: ToolCategory) -> Vec<&str>;

    /// Record tool usage (called after execution)
    pub fn record_usage(&mut self, name: &str);

    /// Get top-N most used tools
    pub fn top_used(&self, n: usize) -> Vec<(&str, u64)>;

    /// Export registry as JSON (for UI inspection)
    pub fn export_json(&self) -> Value;
}
```

#### 5.4 Update `#[derive(Tool)]` macro

In `crates/tools-core-macros/src/lib.rs`, extend the derive macro to support:

```rust
#[derive(Tool)]
#[tool(
    name = "read_file",
    description = "Read file contents",
    category = "FileSystem",
    tags = "file,read,content",
    cost = "Free",
)]
pub struct ReadFileTool { ... }
```

### Files Changed

| File | Action |
|------|--------|
| `crates/tools-core/src/lib.rs` | **Add** ToolMetadata, ToolCategory, ToolSource, CostHint, extend Tool trait |
| `crates/tools-core/src/registry.rs` | **Major rewrite** — add metadata, usage tracking, search |
| `crates/tools-core-macros/src/lib.rs` | **Extend** derive(Tool) to parse metadata attributes |
| `crates/tools/src/*.rs` | **Update** all ~24 tools with `#[tool(category, tags, cost)]` |
| `crates/mcp/src/client/tool.rs` | **Add** MCP metadata mapping |
| `crates/plugin-runtime/src/*.rs` | **Add** plugin metadata mapping |

---

## 6. Upgrade 5: Agent Skills Spec Compatibility

### Problem

Klyntbot skills use custom format (name, description, always, triggers). External skills in `~/.klyntbot/.agents/skills/` exist but aren't auto-loaded. Not compatible with Agent Skills spec (30+ tools).

### Solution

Replace custom skill format with **Agent Skills spec + klyntbot extensions**. Add runtime skill loading from filesystem.

### Changes

#### 6.1 New skill frontmatter format

Replace current:
```yaml
---
name: todo
description: Task creation with confidence scoring
always: true
triggers: []
---
```

With Agent Skills spec + extensions:
```yaml
---
name: todo
description: Task creation with confidence scoring
license: MIT
metadata:
  author: klyntbot
  version: "1.0.0"
  updated-on: "2026-03-10"
  source: official
  tags: "task,todo,productivity"
  # Klyntbot extensions (ignored by other tools)
  always: true
  triggers: "create task,add todo,new task"
  agent: task
---
```

#### 6.2 Update AgentSkill struct

In `crates/agent/src/agent_profile/types.rs`:

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentSkill {
    pub name: String,
    pub description: String,
    pub content: String,

    // Agent Skills spec fields
    pub license: Option<String>,
    pub author: Option<String>,
    pub version: Option<String>,
    pub updated_on: Option<String>,
    pub source: Option<String>,       // "official", "maintainer", "community"
    pub tags: Vec<String>,

    // Klyntbot extensions
    pub always: bool,
    pub triggers: Vec<String>,
    pub agent: Option<String>,        // Which agent owns this skill
}
```

#### 6.3 New: Runtime skill loader

New file: `crates/agent/src/skill_loader.rs`

```rust
pub struct SkillLoader {
    builtin_skills: Vec<AgentSkill>,     // From include_str! (compiled)
    external_skills: Vec<AgentSkill>,    // From filesystem (runtime)
    skills_dir: PathBuf,                 // ~/.klyntbot/.agents/skills/
}

impl SkillLoader {
    /// Load all skills from filesystem at startup
    pub fn load_external_skills(&mut self) -> Result<Vec<AgentSkill>>;

    /// Hot-reload: watch for filesystem changes
    pub fn watch(&self, tx: mpsc::Sender<SkillEvent>) -> Result<()>;

    /// Get all skills for a given agent (builtin + external filtered by agent tag)
    pub fn skills_for_agent(&self, agent_name: &str) -> Vec<&AgentSkill>;

    /// Search skills by query (BM25 over name + description + tags)
    pub fn search(&self, query: &str) -> Vec<&AgentSkill>;
}
```

#### 6.4 Update all builtin skills

All 14 skills in `agents/*/skills/*.md` updated to new format. All 5 AGENT.md files updated.

#### 6.5 Update AgentProfile parser

`crates/agent/src/agent_profile/parser.rs` — parse new frontmatter fields:

```rust
fn parse_skill_frontmatter(content: &str) -> Result<AgentSkill> {
    // Parse standard Agent Skills spec fields
    // Parse klyntbot extension fields from metadata.*
    // Fallback: if no metadata block, treat as legacy format
}
```

### Files Changed

| File | Action |
|------|--------|
| `agents/*/skills/*.md` (14 files) | **Rewrite** frontmatter to Agent Skills spec |
| `agents/*/AGENT.md` (5 files) | **Update** frontmatter with metadata |
| `crates/agent/src/agent_profile/types.rs` | **Replace** AgentSkill struct |
| `crates/agent/src/agent_profile/parser.rs` | **Replace** skill parser |
| `crates/agent/src/skill_loader.rs` | **New** — runtime skill loader |
| `crates/agent/src/context_sources/agent.rs` | **Update** — use SkillLoader for skill injection |
| `crates/config/src/lib.rs` | **Add** `skills_dir` config field |
| `crates/app-core/src/lib.rs` | **Add** SkillLoader init |

---

## 7. Upgrade 6: MCP Server Enhancement

### Problem

MCP server only exposes `get_status()`. External agents (Claude Code, Cursor) can't use klyntbot's tools.

### Solution

Expose curated tool subset via MCP server. Adopt context-hub defensive patterns.

### Changes

#### 7.1 Expand exposed tools

In `crates/mcp/src/server/`:

```rust
// Expose these tools via MCP:
const MCP_EXPOSED_TOOLS: &[&str] = &[
    "task",           // Task CRUD
    "memory",         // Memory read/write
    "annotate",       // Annotation CRUD
    "search",         // Unified search (BM25 + vector)
    "project",        // Project management
    "area",           // PARA areas
    "okr",            // OKR management
    "context_request", // Context expansion
    "learning",       // User profile learning
    "web_search",     // Web search
];
```

#### 7.2 Security patterns from context-hub

```rust
// Path traversal protection
fn validate_path(path: &str) -> Result<PathBuf> {
    let resolved = PathBuf::from(path).canonicalize()?;
    let allowed = PathBuf::from(&config.data_dir);
    if !resolved.starts_with(&allowed) {
        return Err(McpError::PathTraversal);
    }
    Ok(resolved)
}

// Stderr redirect for stdio transport
fn redirect_console() {
    // Redirect tracing/log output to stderr
    // Keep stdout clean for JSON-RPC
}
```

#### 7.3 Dynamic tool list updates

```rust
// When tools change (MCP reconnect, plugin load), notify clients
impl McpServer {
    async fn on_tool_registry_changed(&self) {
        self.notification_sender
            .send(Notification::ToolListChanged)
            .await;
    }
}
```

### Files Changed

| File | Action |
|------|--------|
| `crates/mcp/src/server/mod.rs` | **Major rewrite** — expose 10+ tools |
| `crates/mcp/src/server/handlers.rs` | **New** — handler per exposed tool |
| `crates/mcp/src/server/security.rs` | **New** — path validation, rate limiting |
| `crates/mcp/src/server/transport.rs` | **Add** stderr redirect |

---

## 8. Upgrade 7: Multi-Source Content Registry

### Problem

Agent profiles and skills are compile-time only. No way to add community or internal knowledge sources at runtime.

### Solution

New `ContentRegistry` subsystem — loads docs and skills from multiple sources (builtin, local folders, remote CDN). Uses context-hub's registry format.

### Changes

#### 8.1 New crate or module: `crates/agent/src/content_registry/`

```rust
pub struct ContentRegistry {
    sources: Vec<ContentSource>,
    docs: Vec<DocEntry>,
    skills: Vec<SkillEntry>,
    bm25_index: Bm25Index,   // In-memory BM25 for search
}

pub enum ContentSource {
    Builtin,                          // include_str! agents
    Local { name: String, path: PathBuf },  // ~/.klyntbot/content/
    Remote { name: String, url: String, cache_dir: PathBuf },  // CDN
}

pub struct DocEntry {
    pub id: String,              // "author/name"
    pub name: String,
    pub description: String,
    pub source: String,          // "official", "maintainer", "community"
    pub tags: Vec<String>,
    pub content_source: String,  // Which ContentSource
    pub languages: Vec<LanguageEntry>,
}

pub struct SkillEntry {
    pub id: String,
    pub name: String,
    pub description: String,
    pub source: String,
    pub tags: Vec<String>,
    pub content_source: String,
    pub path: PathBuf,
    pub files: Vec<String>,
}

impl ContentRegistry {
    /// Load from all configured sources
    pub async fn load(config: &ContentConfig) -> Result<Self>;

    /// Search docs + skills by query
    pub fn search(&self, query: &str, limit: usize) -> Vec<SearchResult>;

    /// Get a specific doc/skill by ID
    pub fn get(&self, id: &str) -> Option<ContentEntry>;

    /// Refresh from remote sources
    pub async fn refresh(&mut self) -> Result<()>;
}
```

#### 8.2 Config extension

In `crates/config/src/lib.rs`:

```rust
#[derive(Serialize, Deserialize)]
pub struct ContentConfig {
    pub sources: Vec<ContentSourceConfig>,
    pub trust_policy: String,           // "official,maintainer,community"
    pub refresh_interval_secs: u64,     // Cache TTL
    pub content_dir: PathBuf,           // ~/.klyntbot/content/
}

#[derive(Serialize, Deserialize)]
pub struct ContentSourceConfig {
    pub name: String,
    pub url: Option<String>,     // Remote CDN
    pub path: Option<String>,    // Local folder
}
```

#### 8.3 New tool: `docs`

```rust
#[derive(Tool)]
#[tool(
    name = "docs",
    description = "Search and fetch documentation for APIs, SDKs, and libraries from the content registry. Use before writing code against external services to get current, accurate API reference.",
    category = "Search",
    tags = "documentation,api,sdk,reference",
)]
pub struct DocsTool { registry: Arc<RwLock<ContentRegistry>> }

// Actions: search, get, list
```

### Files Changed

| File | Action |
|------|--------|
| `crates/agent/src/content_registry/mod.rs` | **New** — ContentRegistry |
| `crates/agent/src/content_registry/loader.rs` | **New** — multi-source loader |
| `crates/agent/src/content_registry/search.rs` | **New** — BM25 search |
| `crates/agent/src/content_registry/types.rs` | **New** — DocEntry, SkillEntry |
| `crates/config/src/lib.rs` | **Add** ContentConfig |
| `crates/tools/src/docs.rs` | **New** — docs tool |
| `crates/app-core/src/lib.rs` | **Add** ContentRegistry init |

---

## 9. Implementation Order & Dependencies

```
Week 1-2: Upgrade 1 (BM25) + Upgrade 2 (Annotations)
  ├── BM25 has no deps — start immediately
  └── Annotations uses BM25 FTS5 — can build in parallel, integrate at end

Week 3-4: Upgrade 5 (Agent Skills Spec) + Upgrade 4 (Tool Metadata)
  ├── Skills spec is format-only — no runtime deps
  ├── Tool metadata extends Tool trait — independent of BM25
  └── Both feed into search infrastructure from Week 1-2

Week 5-6: Upgrade 3 (Progressive Context)
  ├── Depends on: ContextInventory aware of annotations (Upgrade 2)
  ├── Depends on: context_request tool registered with metadata (Upgrade 4)
  └── Most architecturally complex — needs BM25 + annotations stable first

Week 7: Upgrade 6 (MCP Server) + Upgrade 7 (Content Registry)
  ├── MCP server wraps existing tools — just exposure layer
  └── Content registry is additive — loads from config, provides search
```

### Dependency Graph

```
BM25 (1) ─────────────────────────┐
                                   ├─→ Progressive Context (3)
Annotations (2) ──────────────────┘         │
                                            │
Agent Skills (5) ─────────────────────┐     │
                                      ├─→ Content Registry (7)
Tool Metadata (4) ───────────────────┘     │
                                           │
MCP Server (6) ←───────────────────────────┘
```

---

## 10. Migration Checklist

### Before Starting

- [ ] Create branch: `feat/context-hub-integration`
- [ ] Ensure all current tests pass: `cargo nextest run --workspace`
- [ ] Document current test count as baseline

### Per Upgrade

- [ ] **Upgrade 1**: BM25
  - [ ] Add FTS5 migration
  - [ ] Implement `search_fts` on all repos
  - [ ] Replace `rrf_merge` → `rrf_merge_triple`
  - [ ] Update cognitive retrieval tests
  - [ ] Verify: `cargo nextest run -p cognitive`

- [ ] **Upgrade 2**: Annotations
  - [ ] Add annotations migration
  - [ ] Implement AnnotationRepo
  - [ ] Implement annotate tool
  - [ ] Add AnnotationContextSource
  - [ ] Add post-execution annotation logic
  - [ ] Tests: annotation CRUD, FTS search, context injection
  - [ ] Verify: `cargo nextest run -p storage -p cognitive -p tools -p agent`

- [ ] **Upgrade 3**: Progressive Context
  - [ ] Add ContextInventory to AssembledContext
  - [ ] Implement `expand()` and `retrieve_additional_memory()`
  - [ ] Add `estimated_tokens()` to all ContextSources
  - [ ] Implement context_request tool
  - [ ] Modify ReactiveEngine for context versioning
  - [ ] Inject inventory into system prompt
  - [ ] Tests: progressive loading, budget tracking, mid-execution expansion
  - [ ] Verify: `cargo nextest run -p context_engine -p agent`

- [ ] **Upgrade 4**: Tool Metadata
  - [ ] Add ToolMetadata types
  - [ ] Extend Tool trait with `metadata()`
  - [ ] Update derive(Tool) macro
  - [ ] Add metadata to all 24 built-in tools
  - [ ] Extend ToolRegistry with search, usage tracking
  - [ ] Tests: metadata parsing, search, usage counting
  - [ ] Verify: `cargo nextest run -p tools-core -p tools`

- [ ] **Upgrade 5**: Agent Skills Spec
  - [ ] Define new frontmatter format
  - [ ] Rewrite all 14 skill files
  - [ ] Rewrite all 5 AGENT.md files
  - [ ] Update AgentSkill struct and parser
  - [ ] Implement SkillLoader with filesystem discovery
  - [ ] Add skills_dir config
  - [ ] Tests: parsing both formats, runtime loading, skill search
  - [ ] Verify: `cargo nextest run -p agent`

- [ ] **Upgrade 6**: MCP Server
  - [ ] Expose 10+ tools via MCP server
  - [ ] Add path traversal protection
  - [ ] Add stderr redirect for stdio transport
  - [ ] Implement dynamic tool list notifications
  - [ ] Tests: MCP protocol compliance, security checks
  - [ ] Verify: `cargo nextest run -p mcp`

- [ ] **Upgrade 7**: Content Registry
  - [ ] Implement ContentRegistry with multi-source
  - [ ] Add ContentConfig
  - [ ] Implement docs tool
  - [ ] Add BM25 index for content search
  - [ ] Tests: registry loading, search, source merging
  - [ ] Verify: `cargo nextest run -p agent -p tools`

### After All Upgrades

- [ ] Full test suite: `cargo nextest run --workspace`
- [ ] Doc tests: `cargo test --workspace --doc`
- [ ] Clippy: `cargo clippy --workspace --all-targets --all-features`
- [ ] Format: `cargo fmt --all --check`
- [ ] Desktop UI: `cd desktop-ui && bun run build && bun run lint:fix`
- [ ] Manual test: `cargo tauri dev` — verify all features work end-to-end

---

## 11. Frontend Changes (Desktop UI)

> FE stack: React 19 + TypeScript + Tailwind v4 + React Router + Tauri IPC (via `useIpc` / `useQuery` / `useMutation` hooks).
> Architecture: `features/` (domain-sliced) + `shared/` (hooks, types, ui, composites).

### 11.1 Shared Types — New & Modified

All BE↔FE contracts flow through `desktop-shared/src/types.rs` (Rust) → `desktop-ui/src/shared/types/` (TS).

#### `desktop-shared/src/types.rs` — Rust side

```rust
// ADD to existing EntityKind enum:
pub enum EntityKind {
    // ... existing variants ...
    Annotation,     // NEW
    ContentDoc,     // NEW
    ContentSkill,   // NEW
}

// ADD impl EntityKind::parse():
// "annotation" => Some(Self::Annotation),
// "content_doc" | "contentdoc" => Some(Self::ContentDoc),
// "content_skill" | "contentskill" => Some(Self::ContentSkill),
```

#### `shared/types/annotations.ts` — **NEW FILE**

```typescript
export interface Annotation {
  id: string;
  targetType: 'tool' | 'fact' | 'rule' | 'skill' | 'api' | 'project' | 'custom';
  targetId: string;
  content: string;
  tags: string;
  author: 'agent' | 'user';
  priority: 0 | 1 | 2;  // normal, important, critical
  createdAt: string;
  updatedAt: string;
  expiresAt?: string;
  accessCount: number;
}

export interface AnnotationCreateParams {
  targetType: string;
  targetId: string;
  content: string;
  tags?: string;
  priority?: number;
  expiresAt?: string;
}

export interface AnnotationSearchResult {
  annotation: Annotation;
  relevanceScore: number;
  snippet: string;
}
```

#### `shared/types/tools.ts` — **NEW FILE**

```typescript
export type ToolCategory =
  | 'General' | 'FileSystem' | 'Search' | 'Web'
  | 'Communication' | 'TaskManagement' | 'Memory'
  | 'Finance' | 'Productivity' | 'System' | 'Mcp' | 'Plugin';

export type ToolSource =
  | { type: 'Native' }
  | { type: 'Feature'; name: string }
  | { type: 'Mcp'; server: string }
  | { type: 'Plugin'; name: string };

export type CostHint = 'Free' | 'Low' | 'Medium' | 'High' | 'Variable';

export interface ToolMetadata {
  category: ToolCategory;
  tags: string[];
  author: string;
  version: string;
  source: ToolSource;
  examples: { description: string; params: Record<string, unknown> }[];
  relatedTools: string[];
  costHint: CostHint;
}

export interface ToolRegistryEntry {
  name: string;
  description: string;
  metadata: ToolMetadata;
  usageCount: number;
}

export interface ToolSearchResult {
  name: string;
  description: string;
  metadata: ToolMetadata;
  score: number;
}
```

#### `shared/types/content.ts` — **NEW FILE**

```typescript
export interface ContentSource {
  name: string;
  type: 'builtin' | 'local' | 'remote';
  url?: string;
  path?: string;
  docCount: number;
  skillCount: number;
  lastRefreshed?: string;
}

export interface DocEntry {
  id: string;
  name: string;
  description: string;
  source: string;
  tags: string[];
  languages: { language: string; recommendedVersion: string }[];
}

export interface SkillEntry {
  id: string;
  name: string;
  description: string;
  source: string;
  tags: string[];
  agent?: string;
  always: boolean;
}

export interface ContentSearchResult {
  entry: DocEntry | SkillEntry;
  type: 'doc' | 'skill';
  score: number;
}
```

#### `shared/types/context.ts` — **NEW FILE**

```typescript
export type ContextItemStatus =
  | { status: 'loaded'; tokensUsed: number }
  | { status: 'deferred'; reason: string }
  | { status: 'available'; description: string };

export interface ContextInventoryItem {
  sourceName: string;
  priority: string;
  status: ContextItemStatus;
  tokenEstimate: number;
  summary?: string;
}

export interface ContextInventory {
  items: ContextInventoryItem[];
  budgetTotal: number;
  budgetUsed: number;
  budgetRemaining: number;
  version: number;
}
```

#### `shared/types/chat.ts` — Extend existing

```typescript
// ADD to TransparencyData interface:
export interface TransparencyData {
  // ... existing fields ...
  contextInventory?: ContextInventory;           // NEW — progressive context
  annotations?: { target: string; content: string; priority: number }[];  // NEW — active annotations
  searchScores?: { source: string; score: number }[];  // NEW — BM25/vector/RRF scores
}

// ADD to SkillLoadedPayload:
export interface SkillLoadedPayload {
  sessionKey: string;
  name: string;
  trigger: string;
  agent?: string;
  source?: string;      // NEW — "builtin" | "external" | "community"
  version?: string;     // NEW — Agent Skills spec version
  tags?: string[];      // NEW
}

// NEW event payloads:
export interface AnnotationCreatedPayload {
  sessionKey: string;
  targetType: string;
  targetId: string;
  content: string;
  author: string;
}

export interface ContextExpandedPayload {
  sessionKey: string;
  sourceName: string;
  tokensLoaded: number;
  budgetRemaining: number;
}
```

#### `shared/types/index.ts` — Add re-exports

```typescript
export * from "./annotations";
export * from "./tools";
export * from "./content";
export * from "./context";
```

---

### 11.2 New Tauri Commands — Rust side

#### `crates/desktop/src/commands/annotations.rs` — **NEW FILE**

```rust
#[tauri::command]
pub async fn list_annotations(state: State<'_, AppState>, target_type: Option<String>, target_id: Option<String>) -> Result<Vec<AnnotationResponse>, String>;

#[tauri::command]
pub async fn create_annotation(state: State<'_, AppState>, params: AnnotationCreateParams) -> Result<AnnotationResponse, String>;

#[tauri::command]
pub async fn delete_annotation(state: State<'_, AppState>, id: String) -> Result<bool, String>;

#[tauri::command]
pub async fn search_annotations(state: State<'_, AppState>, query: String, limit: Option<usize>) -> Result<Vec<AnnotationSearchResult>, String>;
```

#### `crates/desktop/src/commands/search.rs` — **NEW FILE**

```rust
/// Unified search: BM25 + vector + RRF merge across all entity types
#[tauri::command]
pub async fn global_search(state: State<'_, AppState>, query: String, limit: Option<usize>, entity_types: Option<Vec<String>>) -> Result<Vec<GlobalSearchResult>, String>;
```

#### `crates/desktop/src/commands/tool_registry.rs` — **NEW FILE**

```rust
#[tauri::command]
pub async fn get_tool_registry(state: State<'_, AppState>) -> Result<Vec<ToolRegistryEntry>, String>;

#[tauri::command]
pub async fn search_tools(state: State<'_, AppState>, query: String) -> Result<Vec<ToolSearchResult>, String>;

#[tauri::command]
pub async fn get_tool_usage_stats(state: State<'_, AppState>) -> Result<Vec<ToolUsageStat>, String>;
```

#### `crates/desktop/src/commands/content_registry.rs` — **NEW FILE**

```rust
#[tauri::command]
pub async fn get_content_sources(state: State<'_, AppState>) -> Result<Vec<ContentSourceResponse>, String>;

#[tauri::command]
pub async fn search_content(state: State<'_, AppState>, query: String, limit: Option<usize>) -> Result<Vec<ContentSearchResult>, String>;

#[tauri::command]
pub async fn get_content_doc(state: State<'_, AppState>, id: String, lang: Option<String>) -> Result<String, String>;

#[tauri::command]
pub async fn refresh_content(state: State<'_, AppState>, source_name: Option<String>) -> Result<(), String>;
```

#### `crates/desktop/src/commands/mod.rs` — Add modules

```rust
pub mod annotations;       // NEW
pub mod search;            // NEW (global_search)
pub mod tool_registry;     // NEW
pub mod content_registry;  // NEW
```

---

### 11.3 New FE Components & Pages

#### A. Annotations Panel — `features/chat/components/AnnotationPanel.tsx`

Inline panel below TransparencyPanel in chat, shows annotations created during the conversation.

```
┌──────────────────────────────────┐
│ 📌 Annotations (3)         [+]  │
├──────────────────────────────────┤
│ 🔧 search tool                  │
│   "Use BM25 for keyword queries, │
│    vector for semantic"          │
│                       — agent    │
├──────────────────────────────────┤
│ 📋 stripe/api                   │
│   "Webhook requires raw body"   │
│                       — agent    │
└──────────────────────────────────┘
```

#### B. Context Inventory Viewer — `features/chat/components/ContextInventoryView.tsx`

Shows loaded vs deferred context sources in TransparencyPanel.

```
┌──────────────────────────────────┐
│ 🧠 Context Budget  8.2k / 12k   │
│ ▓▓▓▓▓▓▓▓▓▓░░░░░  68%           │
├──────────────────────────────────┤
│ ✅ Recent history      3.1k tok  │
│ ✅ Episodic memories   1.8k tok  │
│ ✅ Active annotations  0.4k tok  │
│ ⏸️  Project details    2.1k tok  │
│ ⏸️  Behavioral patterns 0.9k tok │
└──────────────────────────────────┘
```

#### C. Tool Registry Browser — `features/system/components/ToolRegistryTab.tsx`

New tab in SystemPage (alongside existing Events, Pipeline, Memory, Coaching tabs).

```
┌──────────────────────────────────────────────┐
│ 🔍 [Search tools...]                        │
├──────────────────────────────────────────────┤
│ FileSystem (3)  │ Memory (4)  │ MCP (8)     │
├──────────────────────────────────────────────┤
│ 📁 read_file          Free    used: 142x    │
│    Read file contents                        │
│    Tags: file, read, content                 │
│    Related: list_dir, write_file             │
├──────────────────────────────────────────────┤
│ 📁 list_dir           Free    used: 98x     │
│ 📁 write_file         Free    used: 45x     │
└──────────────────────────────────────────────┘
```

#### D. Content Registry Settings — `features/settings/components/ContentRegistrySettings.tsx`

New settings page at `/settings/content`.

```
┌──────────────────────────────────────────────┐
│ Content Sources                              │
├──────────────────────────────────────────────┤
│ ✅ builtin        5 docs, 14 skills         │
│    Compiled agent profiles                   │
│                                              │
│ ✅ community      68 docs, 12 skills        │
│    cdn.aichub.org/v1                         │
│    Last refreshed: 2h ago    [Refresh]       │
│                                              │
│ ⚪ internal       — not configured —         │
│    [Add local source...]                     │
├──────────────────────────────────────────────┤
│ Trust policy: [official] [maintainer] [comm] │
│ Refresh interval: [24h ▼]                    │
└──────────────────────────────────────────────┘
```

#### E. Skills Manager — `features/settings/components/SkillsSettings.tsx`

New settings page at `/settings/skills`.

```
┌──────────────────────────────────────────────┐
│ Agent Skills                                 │
├──────────────────────────────────────────────┤
│ Built-in (14)                                │
│   📋 todo (task)          always  v1.0.0     │
│   📋 daily-planner (task) always  v1.0.0     │
│   📋 memory (general)     trigger v1.0.0     │
│                                              │
│ External (2)                  [+ Add Skill]  │
│   📋 vercel-react         trigger v1.0.0     │
│      ~/.klyntbot/.agents/skills/             │
│   📋 web-design           trigger v1.0.0     │
└──────────────────────────────────────────────┘
```

---

### 11.4 Modified FE Components

#### `features/chat/components/TransparencyPanel.tsx` — EXTEND

Add 3 new CollapsibleBox sections:

1. **Context Inventory** — show budget bar + loaded/deferred sources (uses `ContextInventoryView`)
2. **Active Annotations** — show annotations that were injected into this message's context
3. **Search Scores** — show BM25/vector/RRF scores for memory retrieval (debug info)

Also extend existing sections:
- **Skills** section: add `source` badge (builtin/external), version tag
- **Tools** section: add category grouping, cost hint icon

#### `features/debug/components/tabs/MemoryTab.tsx` — EXTEND

- Add "Annotations" sub-tab alongside existing memory types
- Add search bar with BM25 indicator (show relevance scores)
- Add annotation CRUD (create/edit/delete from debug panel)

#### `features/settings/components/McpServersSettings.tsx` — EXTEND

- Add expandable tool list per MCP server (show name, description, category)
- Add "Exposed Tools" section showing which klyntbot tools are exposed via MCP server

#### `app/router.tsx` — ADD ROUTES

```typescript
// New lazy imports:
const ContentRegistrySettings = lazy(() =>
  import("../features/settings").then((m) => ({ default: m.ContentRegistrySettings })),
);
const SkillsSettings = lazy(() =>
  import("../features/settings").then((m) => ({ default: m.SkillsSettings })),
);

// New routes inside SettingsLayout:
{ path: "/settings/content", element: <SettingsLayout><ContentRegistrySettings /></SettingsLayout> },
{ path: "/settings/skills", element: <SettingsLayout><SkillsSettings /></SettingsLayout> },
```

#### `app/layouts/Sidebar.tsx` — ADD NAV ITEMS

Add "Content" and "Skills" links to Settings section in sidebar navigation.

---

### 11.5 New Hooks

| Hook | Purpose | File |
|------|---------|------|
| `useAnnotations(targetType?, targetId?)` | CRUD + list annotations | `shared/hooks/useAnnotations.ts` |
| `useGlobalSearch(query)` | Unified BM25+vector search | `shared/hooks/useGlobalSearch.ts` |
| `useToolRegistry()` | Browse/search tool registry | `shared/hooks/useToolRegistry.ts` |
| `useContentRegistry()` | Browse/search/refresh content sources | `shared/hooks/useContentRegistry.ts` |
| `useContextInventory(sessionKey)` | Track context budget in real-time | `shared/hooks/useContextInventory.ts` |

All hooks follow existing pattern: `useQuery` for reads, `useMutation` for writes, `useEvent` for real-time updates.

---

### 11.6 New Event Subscriptions

The FE listens to Tauri events via `useEvent()` hook. New events to subscribe:

| Event Name | Payload | Consumed By |
|------------|---------|-------------|
| `annotation:created` | `AnnotationCreatedPayload` | `AnnotationPanel`, `MemoryTab` |
| `annotation:deleted` | `{ id: string }` | `AnnotationPanel`, `MemoryTab` |
| `context:expanded` | `ContextExpandedPayload` | `ContextInventoryView` |
| `context:inventory_updated` | `ContextInventory` | `ContextInventoryView` |
| `tool:registry_changed` | `{ toolCount: number }` | `ToolRegistryTab` |
| `content:refreshed` | `{ source: string, docs: number, skills: number }` | `ContentRegistrySettings` |

These emit via existing `app.emit()` pattern in `crates/desktop/src/commands/mod.rs`.

---

### 11.7 FE Files Summary — All Changes

| File | Action | Upgrade |
|------|--------|---------|
| **New Type Files** | | |
| `shared/types/annotations.ts` | **New** | Annotations |
| `shared/types/tools.ts` | **New** | Tool Registry |
| `shared/types/content.ts` | **New** | Content Registry |
| `shared/types/context.ts` | **New** | Progressive Context |
| `shared/types/chat.ts` | **Extend** | All — add fields to TransparencyData, new payloads |
| `shared/types/index.ts` | **Extend** | All — re-exports |
| **New Hooks** | | |
| `shared/hooks/useAnnotations.ts` | **New** | Annotations |
| `shared/hooks/useGlobalSearch.ts` | **New** | BM25 |
| `shared/hooks/useToolRegistry.ts` | **New** | Tool Registry |
| `shared/hooks/useContentRegistry.ts` | **New** | Content Registry |
| `shared/hooks/useContextInventory.ts` | **New** | Progressive Context |
| **New Components** | | |
| `features/chat/components/AnnotationPanel.tsx` | **New** | Annotations |
| `features/chat/components/ContextInventoryView.tsx` | **New** | Progressive Context |
| `features/system/components/ToolRegistryTab.tsx` | **New** | Tool Registry |
| `features/settings/components/ContentRegistrySettings.tsx` | **New** | Content Registry |
| `features/settings/components/SkillsSettings.tsx` | **New** | Agent Skills |
| **Modified Components** | | |
| `features/chat/components/TransparencyPanel.tsx` | **Extend** | All — 3 new sections |
| `features/debug/components/tabs/MemoryTab.tsx` | **Extend** | BM25 + Annotations |
| `features/settings/components/McpServersSettings.tsx` | **Extend** | MCP Server |
| `app/router.tsx` | **Add routes** | Content + Skills settings |
| `app/layouts/Sidebar.tsx` | **Add nav items** | Content + Skills settings |
| **New Tauri Commands (Rust)** | | |
| `crates/desktop/src/commands/annotations.rs` | **New** | Annotations |
| `crates/desktop/src/commands/search.rs` | **New** | BM25 |
| `crates/desktop/src/commands/tool_registry.rs` | **New** | Tool Registry |
| `crates/desktop/src/commands/content_registry.rs` | **New** | Content Registry |
| `crates/desktop/src/commands/mod.rs` | **Extend** | All — add modules |
| `crates/desktop-shared/src/types.rs` | **Extend** | Annotations + Content |
| `crates/desktop-shared/src/events.rs` | **Extend** | New event constants |

**Total FE changes: ~15 new files, ~8 modified files.**
**Estimated LOC: ~2000 lines TypeScript/TSX.**

---

### 11.8 FE Implementation Order

Follow the same phase order as backend:

| Phase | Backend | FE Tasks |
|-------|---------|----------|
| **Week 1-2** | BM25 + Annotations | Type files, `useAnnotations`, `useGlobalSearch`, `AnnotationPanel`, extend `MemoryTab` |
| **Week 3-4** | Skills + Tool Registry | `useToolRegistry`, `ToolRegistryTab`, `SkillsSettings`, extend `TransparencyPanel` skills section |
| **Week 5-6** | Progressive Context | `useContextInventory`, `ContextInventoryView`, extend `TransparencyPanel` context section |
| **Week 7** | MCP + Content Registry | `useContentRegistry`, `ContentRegistrySettings`, extend `McpServersSettings`, routes + sidebar |

Each phase's FE can be developed in parallel with its backend once the Tauri commands are defined (type-first approach).
