# Subsystem 1: Skill Format & Discovery

> Adopt the Agent Skills spec as Klyntbot's native skill format, replacing the current hardcoded agent/skill system with runtime-discoverable, cross-platform-compatible skill packages.

**Date**: 2026-03-14
**Status**: Approved
**Subsystem**: 1 of 5 (Skill Format & Discovery → Declarative Feature Modules → Agent-as-Orchestrator → Skill Lifecycle → Marketplace)

## Context

Klyntbot currently has 5 built-in agents (general, task, finance, automation, communication) with `AGENT.md` files compiled via `include_str!` and 14 sub-skills as markdown files in `agents/{name}/skills/`. This system is rigid: adding, removing, or customizing agents/skills requires recompilation. Skills are not portable to other AI platforms.

The [Agent Skills specification](https://agentskills.io/specification) defines a lightweight, open format for extending AI agent capabilities. It uses `SKILL.md` files with YAML frontmatter, progressive disclosure (metadata → instructions → resources), and a cross-platform discovery convention (`.agents/skills/`). Skills authored in this format work across Claude Code, Claude API, Claude.ai, Agent SDK, Codex, and other compliant clients.

## Design Decisions

| Decision | Choice | Rationale |
|----------|--------|-----------|
| Direction | Both export and import | Follow the community; skills usable in Klyntbot and other platforms |
| Skill model | Unified package | SKILL.md (portable) + manifest.json (Klyntbot runtime extensions) |
| Tool runtime | Declarative by default, WASM/scripts fallback | Auto-generate tools from entity declarations; custom code for advanced cases |
| Storage | Shared schema, namespaced tables | Cross-skill queries essential for AI agent; single DB simplicity |
| Agent identity | Agents become orchestrator skills | Installed/discovered like any skill, with orchestration metadata |
| Cross-platform | Graceful degradation | Full package in Klyntbot, only SKILL.md + scripts in other platforms |

## 1. Skill Package Format

Every skill follows the Agent Skills spec with optional Klyntbot extensions:

```
skill-name/
├── SKILL.md              # Required — Agent Skills spec (portable)
├── manifest.json         # Optional — Klyntbot extensions (declarative tools, storage, permissions)
├── scripts/              # Optional — executable code
├── references/           # Optional — documentation loaded on demand
├── assets/               # Optional — templates, resources
└── migrations/           # Optional — SQL migration files (for skills with custom storage)
```

### SKILL.md Format

Spec-compliant `SKILL.md` with Klyntbot metadata in the `metadata` field:

```yaml
---
name: task-management
description: >
  Create, organize, and track tasks, projects, and areas using OKR+PARA.
  Use when the user mentions todos, tasks, projects, areas, objectives,
  planning, reviews, or goal tracking.
metadata:
  author: klyntbot
  version: "1.0.0"
  klyntbot:
    type: orchestrator        # "skill" (default) or "orchestrator"
    tools: [tasks, project, area, okr, notes]
    can_delegate_to: [finance-management]
    max_iterations: 12
    always_skills: [todo, daily-planner]
---

You are the task management specialist...
```

**Frontmatter fields** (per Agent Skills spec):

| Field | Required | Purpose |
|-------|----------|---------|
| `name` | Yes | 1-64 chars, lowercase + hyphens, matches directory name |
| `description` | Yes | 1-1024 chars, describes what + when to use |
| `license` | No | License name or file reference |
| `compatibility` | No | Environment requirements |
| `metadata` | No | Arbitrary key-value pairs; `metadata.klyntbot` for platform extensions |
| `allowed-tools` | No | Pre-approved tools (experimental) |

**`metadata.klyntbot` fields** (Klyntbot-specific, ignored by other platforms):

| Field | Type | Default | Purpose |
|-------|------|---------|---------|
| `type` | `"skill"` \| `"orchestrator"` | `"skill"` | Orchestrators replace current agents |
| `tools` | `string[]` | `[]` (all) | Tool access allowlist |
| `mcp_tools` | `string[]` | `[]` | MCP server access (`["*"]` = all) |
| `can_delegate_to` | `string[]` | `[]` | Orchestrator delegation targets |
| `max_iterations` | `u32` | `10` | ReAct loop iteration budget |
| `always_skills` | `string[]` | `[]` | Skills always loaded with this orchestrator |

### manifest.json Format

Optional companion file for Klyntbot-specific runtime features. Other platforms ignore this file.

```json
{
  "schema_version": "1.0",
  "entities": {
    "bookmarks": {
      "fields": {
        "url": { "type": "text", "required": true },
        "title": { "type": "text" },
        "tags": { "type": "text[]" },
        "notes": { "type": "text" },
        "is_favorite": { "type": "boolean", "default": false }
      },
      "indexes": ["tags"],
      "enable_vector_search": true,
      "vector_fields": ["title", "notes"]
    }
  },
  "tools": {
    "mode": "declarative",
    "custom": []
  },
  "permissions": ["storage"],
  "cron_jobs": []
}
```

Entity declarations in `manifest.json` drive auto-provisioning (Subsystem 2):
- Storage tables generated from field definitions
- CRUD tools generated from entity schemas
- Vector search indexes provisioned from `vector_fields`

## 2. Skill Discovery & Scanning

### Discovery Scopes

| Scope | Path | Purpose | Priority |
|-------|------|---------|----------|
| Built-in | Compiled via `include_str!` | Core skills shipped with Klyntbot | Lowest (0) |
| User-level | `{data_dir}/skills/` | User-installed skills | Medium (1) |
| Project-level | `.agents/skills/` | Cross-client interop | Highest (2) |

**Priority rule**: Higher-priority scopes shadow lower-priority skills with the same `name`. A warning is logged when shadowing occurs.

### Scanning Algorithm

```
1. Load built-in skills (compiled, always available)
2. Scan {data_dir}/skills/*/SKILL.md
3. Scan .agents/skills/*/SKILL.md (if project context exists)
4. For each SKILL.md found:
   a. Parse YAML frontmatter → extract name, description, metadata
   b. Check for manifest.json in same directory → parse if present
   c. Validate name matches directory name (warn if not, load anyway)
   d. Apply priority rules for name collisions (log shadowing)
5. Build SkillCatalog: HashMap<String, Arc<SkillEntry>>
6. Pre-compute description embeddings for semantic matching
7. Record loaded_at timestamp for each entry
```

**Scanning constraints** (per Agent Skills spec):
- Skip `.git/`, `node_modules/`, and similar non-skill directories
- Max depth: 4 levels from scan root
- Max directories: 2000 per scan root

### Trust Model

| Scope | Trust level | Behavior |
|-------|-------------|----------|
| Built-in | Always trusted | No confirmation needed |
| User-level | Trusted by default | User explicitly installed these |
| Project-level | Requires confirmation | Prompt user before first activation (prevents untrusted repo injection) |

### Hot Reload

`SkillCatalog` supports hot-reload for development and marketplace updates:
- `loaded_at: SystemTime` on each `SkillEntry`
- `SkillCatalog::reload() -> Result<Vec<SkillChange>>` re-scans all sources, returns diff
- File watcher on skill directories triggers reload (optional, debounced)

## 3. Progressive Disclosure & Activation

### Tier 1: Catalog (always loaded, ~50-100 tokens per skill)

At startup, the system prompt includes a compact XML catalog:

```xml
<available_skills>
  <skill name="task-management" type="orchestrator">
    Create, organize, and track tasks, projects, and areas using OKR+PARA.
  </skill>
  <skill name="finance-management" type="orchestrator">
    Track expenses, budgets, and financial goals.
  </skill>
  <skill name="search" type="skill">
    Web search and information retrieval.
  </skill>
</available_skills>
```

### Tier 2: Activation (on-demand, < 5,000 tokens per skill)

Two activation paths:

**Orchestrator selection** (replaces current `AgentManager` routing):

1. User message arrives
2. `SkillRouter` runs matching cascade:
   - Keyword matching against skill descriptions + `metadata.klyntbot` hints
   - Semantic matching (embedding cosine similarity, 70% keyword + 30% semantic blend)
   - LLM classifier fallback (if confidence below threshold)
3. Best orchestrator skill selected → SKILL.md body injected as system context
4. Orchestrator's `always_skills` references loaded
5. Per-message skill activation: non-orchestrator skills whose descriptions match are also injected

**Direct skill activation** (fallback):

1. No orchestrator matches strongly
2. Falls back to "general" orchestrator (default)
3. General orchestrator activates relevant skills by description match

### Tier 3: Resources (as-needed)

When activated skill instructions reference files:
- Referenced markdown (`references/*.md`) → injected into context when LLM requests them
- Scripts (`scripts/*.py`) → executed via `run_script` tool, only output enters context
- Assets (`assets/*`) → read by tools as needed

### Mapping to Current Code

| Current | New |
|---------|-----|
| `AgentManager::match_agent()` | `SkillRouter::select_orchestrator()` |
| `AgentContextSource::provide()` | `SkillContextSource::provide()` |
| `AgentProfile` struct | `SkillEntry` with `type: orchestrator` |
| `AgentSkill` struct | `SkillEntry` with `type: skill` |
| `IntentAnalyzer` 4-layer cascade | `SkillRouter` matching cascade (same algorithm, new inputs) |
| `ContentRegistry` | Retired — unified into `SkillCatalog` |
| `always_skills` frontmatter | `metadata.klyntbot.always_skills` |
| `triggers` frontmatter | Replaced by description-based matching (spec-compliant) |

## 4. Rust Data Model

### Core Types

```rust
// crates/skill-system/src/types.rs

pub enum SkillType {
    Skill,          // Regular skill — knowledge, workflows, tools
    Orchestrator,   // Replaces current "agent" — persona, routing, delegation
}

pub enum SkillScope {
    BuiltIn,        // Compiled via include_str!
    User,           // {data_dir}/skills/
    Project,        // .agents/skills/
}

pub struct SkillEntry {
    pub name: String,
    pub description: String,
    pub skill_type: SkillType,
    pub scope: SkillScope,
    pub location: PathBuf,
    pub body: Option<String>,
    pub manifest: Option<SkillManifest>,
    pub metadata: SkillMetadata,
    pub loaded_at: SystemTime,
}

pub struct SkillMetadata {
    pub license: Option<String>,
    pub compatibility: Option<String>,
    pub custom: HashMap<String, serde_json::Value>,
    pub klyntbot: Option<KlyntbotMeta>,
}

pub struct KlyntbotMeta {
    pub tools: Vec<String>,
    pub mcp_tools: Vec<String>,
    pub can_delegate_to: Vec<String>,
    pub max_iterations: Option<u32>,
    pub always_skills: Vec<String>,
}

pub struct SkillManifest {
    pub schema_version: String,
    pub entities: HashMap<String, EntityDef>,
    pub tools: ToolsConfig,
    pub permissions: Vec<Permission>,
    pub cron_jobs: Vec<CronJobDef>,
}

pub struct SkillCatalog {
    skills: HashMap<String, Arc<SkillEntry>>,
    embeddings: HashMap<String, Vec<f32>>,
    loaded_at: SystemTime,
}
```

### SkillCatalog API

```rust
impl SkillCatalog {
    pub fn discover(sources: &[SkillSource]) -> Result<Self>;
    pub fn reload(&mut self) -> Result<Vec<SkillChange>>;
    pub fn orchestrators(&self) -> Vec<&SkillEntry>;
    pub fn regular_skills(&self) -> Vec<&SkillEntry>;
    pub fn get(&self, name: &str) -> Option<&Arc<SkillEntry>>;
    pub fn catalog_prompt(&self) -> String;
}
```

### Crate Placement

```
crates/
  skill-system/        # NEW crate at L3 (same layer as session, scheduling)
    src/
      lib.rs
      types.rs          # SkillEntry, SkillManifest, SkillCatalog
      discovery.rs      # Scanning, parsing, priority resolution
      router.rs         # Orchestrator selection, skill activation matching
      context.rs        # SkillContextSource (replaces AgentContextSource)
      manifest.rs       # manifest.json parsing, entity/tool declarations
```

**Layer placement rationale**: `skill-system` at L3 has no dependency on `agent` (L5). The `agent` crate consumes `skill-system` types, keeping dependency flow strictly upward. Feature crates (L4) and context engine (L3) can directly work with skill types without circular dependencies.

### Dependency Graph

```
L3: skill-system → depends on: common, config, storage (for embeddings)
L5: agent → depends on: skill-system (uses SkillCatalog, SkillRouter, SkillContextSource)
```

## 5. Migration Path

### Directory Restructure

```
# Current (agents/)                    →  New (skills/)
agents/general/AGENT.md               →  skills/general/SKILL.md (orchestrator)
agents/general/skills/search.md       →  skills/general/references/search.md
agents/general/skills/skill-creator.md → skills/general/references/skill-creator.md
agents/general/skills/browser.md      →  skills/general/references/browser.md
agents/general/skills/memory.md       →  skills/general/references/memory.md
agents/general/skills/summarize.md    →  skills/general/references/summarize.md

agents/task/AGENT.md                  →  skills/task-management/SKILL.md (orchestrator)
agents/task/skills/todo.md            →  skills/task-management/references/todo.md
agents/task/skills/daily-planner.md   →  skills/task-management/references/daily-planner.md
agents/task/skills/weekly-review.md   →  skills/task-management/references/weekly-review.md
agents/task/skills/task-decompose.md  →  skills/task-management/references/task-decompose.md
agents/task/skills/project-management.md → skills/task-management/references/project-management.md
agents/task/skills/retrospective.md   →  skills/task-management/references/retrospective.md

agents/finance/AGENT.md               →  skills/finance-management/SKILL.md (orchestrator)
agents/finance/skills/spending-analysis.md → skills/finance-management/references/spending-analysis.md
agents/finance/skills/budgeting.md    →  skills/finance-management/references/budgeting.md

agents/automation/AGENT.md            →  skills/automation/SKILL.md (orchestrator)
agents/automation/skills/cron.md      →  skills/automation/references/cron.md

agents/communication/AGENT.md        →  skills/communication/SKILL.md (orchestrator)
agents/communication/skills/messaging.md → skills/communication/references/messaging.md
agents/communication/skills/notification.md → skills/communication/references/notification.md
```

### Code Changes

| File/Module | Action | Details |
|---|---|---|
| `crates/skill-system/` | **Create** | New L3 crate with types, discovery, router, context, manifest |
| `crates/agent/src/agent_profile/` | **Replace** | Logic moves to `skill-system/`; delete `types.rs`, `manager.rs` |
| `crates/agent/src/context_sources/agent.rs` | **Replace** | Becomes `skill-system/context.rs` (same injection pattern, new types) |
| `crates/agent/src/content_registry/` | **Delete** | Subsumed by `SkillCatalog` |
| `crates/agent/src/agent_runtime/runtime.rs` | **Modify** | Swap `AgentManager` refs → `SkillCatalog` + `SkillRouter` |
| `crates/agent/src/intent_pipeline/analysis.rs` | **Modify** | Feed `SkillRouter` instead of `AgentManager` |
| `crates/agent/src/agent_loop/builder.rs` | **Modify** | Wire `SkillCatalog` instead of `AgentManager` |
| `crates/config/src/schema/` | **Add** | `SkillConfig` section (discovery paths, trust settings) |
| `agents/` directory | **Delete** | Replaced by `skills/` |

### What Doesn't Change

- `ToolRegistry`, `Tool` trait, `FeaturePackage` — untouched (Subsystem 2 evolves these)
- `ExecutionRouter`, `ReactiveEngine` — untouched (consume tool definitions, not skill types)
- MCP server/client — untouched (operates on `ToolRegistry`)
- `IntentAnalyzer` cascade logic — reused, wired to `SkillRouter`
- All `feature-*` crates — untouched until Subsystem 2
- WASM plugin system — untouched until Subsystem 2

### Testing Strategy

- **Unit tests**: `SkillCatalog::discover()` with mock filesystem, SKILL.md frontmatter parsing (including malformed YAML tolerance per spec), `SkillRouter` matching (port existing `AgentManager` tests)
- **Integration test**: Full message → orchestrator selection → skill activation → context assembly
- **Regression**: All existing `agent` crate tests adapted to new types
- **Cross-platform**: Verify SKILL.md files validate against Agent Skills spec (`skills-ref validate`)

## Appendix: Subsystem Roadmap

This spec covers Subsystem 1. The remaining subsystems build on this foundation:

| # | Subsystem | Depends on | Scope |
|---|-----------|------------|-------|
| 1 | **Skill Format & Discovery** (this spec) | — | SKILL.md format, scanning, progressive disclosure, migration |
| 2 | **Declarative Feature Module System** | 1 | manifest.json processing, auto-generated tools + storage, entity framework |
| 3 | **Agent-as-Orchestrator-Skill** | 1, 2 | Dynamic orchestrator routing, delegation, persona management |
| 4 | **Skill Lifecycle & Installation** | 1, 2, 3 | Install/uninstall/upgrade flow, dependency resolution, versioning |
| 5 | **Marketplace** | 1, 2, 3, 4 | Publishing, distribution, discovery, ratings |
