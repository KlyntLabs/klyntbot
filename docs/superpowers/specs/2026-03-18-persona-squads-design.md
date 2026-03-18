# Persona Squads & Agent Evolution — Design Spec

**Date:** 2026-03-18
**Status:** Approved
**Phase:** 1 (Insight Review), 2 (Chat Mode), 3 (Multi-Agent — future)

## Problem

The current persona system has structural limitations:

1. **Flat list with hidden limits** — 6 builtin personas exist but a hardcoded limit of 4 was applied during perspective generation, confusing users who see all 6 active.
2. **No grouping** — Personas are individually toggled with no concept of purpose-built teams. Users must manually curate which personas are active for each context.
3. **No persona depth** — Personas are defined by 6 flat fields (name, role, expertise, perspective, tone, icon). They cannot carry skills, memory, or analysis frameworks.
4. **Single LLM call** — All perspectives generated in one prompt, diluting quality as persona count increases.

## Solution: Layered Composition

Introduce **Squads** (reusable persona groups) with **Persona Skills** (agent-like depth per persona), using a **layered architecture** where orchestrator skills control execution and persona skills add perspective.

### Architecture Layers

```
┌─────────────────────────────────────────────────┐
│  Squad / Team                                   │
│  User-facing selection unit                     │
│  Links to one orchestrator skill + N personas   │
├──────────────────┬──────────────────────────────┤
│  Orchestrator    │  Persona Skills              │
│  (existing)      │  (new — additive context)    │
│                  │                              │
│  ✅ Tool access  │  ✅ Expertise areas           │
│  ✅ MCP control  │  ✅ Analysis frameworks       │
│  ✅ Delegation   │  ✅ Questioning style/tone    │
│  ✅ Max iters    │  ✅ Cognitive bias            │
│                  │  ✅ Scoped memory             │
│                  │  ❌ No tool access            │
│                  │  ❌ No MCP control            │
│                  │  ❌ No delegation             │
└──────────────────┴──────────────────────────────┘
         │ at runtime
         ▼
┌─────────────────────────────────────────────────┐
│  Execution                                      │
│  Orchestrator handles tools; each persona gets  │
│  its own LLM call with skill + scoped memory    │
└─────────────────────────────────────────────────┘
```

### Why Layered Composition

- **Tool permissions & execution** remain controlled by the orchestrator skill — no permission leaks.
- **Persona skills** only inject additional system prompt sections (like `always_skills` and existing `ContextSource` behavior).
- **Delegation, SubagentManager, and cognitive scoping** continue unchanged.
- **80–90% code reuse** of existing infrastructure.

## Data Model

### New Tables

#### `squads`

| Column             | Type    | Notes                                      |
|--------------------|---------|--------------------------------------------|
| id                 | TEXT PK | UUID                                       |
| name               | TEXT    | UNIQUE for user squads                     |
| description        | TEXT    | Purpose description                        |
| icon               | TEXT    | Emoji or icon name                         |
| orchestrator_skill | TEXT    | Links to existing skill name               |
| source             | TEXT    | `"builtin"` or `"user"`                    |
| domains            | TEXT    | JSON array of domain strings               |
| is_active          | INTEGER | 1 = active, 0 = inactive                  |
| created_at         | TEXT    | RFC3339                                    |
| updated_at         | TEXT    | RFC3339                                    |

#### `squad_members`

| Column       | Type    | Notes                                         |
|--------------|---------|-----------------------------------------------|
| squad_id     | TEXT    | FK → squads.id                                |
| persona_id   | TEXT    | FK → personas.id                              |
| role_in_squad| TEXT    | e.g., "lead", "reviewer", "advisor"           |
| sort_order   | INTEGER | Display/execution order                       |
| PRIMARY KEY  |         | (squad_id, persona_id)                        |

### Extended PersonaRow

Existing fields unchanged. New fields added:

| Field               | Type           | Notes                                    |
|---------------------|----------------|------------------------------------------|
| skill_path          | Option<String> | Path to PERSONA.md skill file            |
| questioning_style   | String         | "interrogative", "socratic", "narrative" |
| cognitive_bias      | String         | What the persona optimizes for           |
| analysis_frameworks | TEXT (JSON)    | Array of framework names                 |

### New Types

| Type                    | Crate        | Purpose                                           |
|-------------------------|--------------|----------------------------------------------------|
| `SquadRow`              | cognitive    | DB row for squads table                            |
| `SquadMemberRow`        | cognitive    | DB row for squad_members join table                |
| `SquadRepo`             | cognitive    | CRUD + resolve_squad() + seed_builtins()           |
| `PersonaSkillMetadata`  | cognitive    | Parsed persona skill metadata                      |
| `SkillType::Persona`    | skill-system | New variant alongside Skill and Orchestrator       |
| `PersonaPerspectiveSource` | agent     | ContextSource impl for persona skill injection     |
| `SquadExecutor`         | agent        | Phase 2: fan-out to persona calls + synthesis      |

## Storage Layout

### Builtin (compiled via `include_str!`)

```
squads/
├── general-analysis/
│   ├── SQUAD.md
│   └── personas/
│       ├── skeptic/
│       │   ├── PERSONA.md
│       │   └── references/
│       ├── connector/
│       │   └── PERSONA.md
│       ├── student/
│       │   └── PERSONA.md
│       └── devils-advocate/
│           └── PERSONA.md
├── research-academic/
│   ├── SQUAD.md
│   └── personas/
│       ├── skeptic/          (symlink or shared reference)
│       ├── academic-reviewer/
│       │   └── PERSONA.md
│       ├── methodologist/
│       │   └── PERSONA.md
│       └── student/
├── finance-analysis/
│   ├── SQUAD.md
│   └── personas/
│       ├── deep-analyst/
│       │   ├── PERSONA.md
│       │   └── references/
│       │       ├── dcf-guide.md
│       │       └── ratio-cheatsheet.md
│       ├── risk-reviewer/
│       │   └── PERSONA.md
│       └── strategist/
└── strategy-planning/
    ├── SQUAD.md
    └── personas/
        ├── strategist/
        ├── practitioner/
        │   └── PERSONA.md
        └── devils-advocate/
```

### User-created

```
~/.klyntbot/
├── personas/{id}/
│   ├── PERSONA.md
│   └── references/*.md
└── data.db              (squads + squad_members tables)
```

## File Formats

### SQUAD.md

```yaml
---
name: finance-analysis
description: >
  Multi-perspective financial analysis team.
  Combines quantitative rigor, risk assessment,
  and strategic thinking.
icon: 💰
orchestrator_skill: finance-management
domains: [finance, investing, budgeting]
personas:
  - deep-analyst
  - risk-reviewer
  - strategist
---

# Finance Analysis Squad

This squad provides comprehensive financial analysis by combining
three complementary perspectives: quantitative depth, risk assessment,
and strategic positioning.
```

### PERSONA.md

```yaml
---
name: deep-analyst
persona_only: true
version: "1.0.0"
icon: 📊
domains: [finance, productivity]
metadata:
  expertise_areas:
    - DCF valuation
    - ratio analysis
    - scenario modeling
  analysis_frameworks:
    - bottom-up
    - comparative
    - sensitivity analysis
  questioning_style: interrogative
  tone: rigorous
  cognitive_bias: precision
  references:
    - dcf-guide
    - ratio-cheatsheet
---

You are a rigorous financial analyst who always starts with
first-principles DCF modeling, cross-checks every assumption
with sensitivity analysis, and presents findings in a clear,
data-backed tone. You are skeptical of optimistic projections
and always highlight downside risks.
```

**Parser behavior:** Reuses the existing skill markdown parser. When `persona_only: true`, the parser ignores `tools`, `mcp_tools`, `max_iterations`, `can_delegate_to`, and `triggers`. Extracts `metadata` into `PersonaSkillMetadata`. The body becomes the persona's system prompt section.

## Three-Tier Memory Scoping

```
Global (scope_type = "system")
  └── Squad (scope_type = "squad", scope_id = squad.id)
       └── Persona (scope_type = "persona", scope_id = persona.id)
```

**Visibility rules:**
- Each persona sees: **Global + its current Squad's memory + its own Persona memory**
- Squad-level synthesis sees: **Global + Squad**
- Memory promotion (Phase 3): Persona → Squad → Global via reflection

**Implementation:** Extends existing `scope_type`/`scope_id` columns in `semantic_facts`, `episodic_memories`, and `procedural_rules`. `MemoryRetriever` accepts a scope chain `[("system", None), ("squad", Some(squad_id)), ("persona", Some(persona_id))]` and queries all tiers.

## Builtin Squads

### 1. General Analysis Squad
- **Orchestrator:** `general`
- **Members:** Skeptic, Connector, Student, Devil's Advocate
- **Purpose:** Default squad for Insight Review. Covers critical thinking, cross-domain synthesis, simplification, and contrarian challenge.
- **Note:** Default squad shown when no squad is explicitly selected.

### 2. Research & Academic Squad
- **Orchestrator:** `general`
- **Members:** Skeptic, Academic Reviewer (new), Methodologist (new), Student
- **Purpose:** Deep academic analysis — methodology review, citation checking, gap identification, accessible summarization.

### 3. Finance Analysis Squad
- **Orchestrator:** `finance-management`
- **Members:** Deep Analyst (new), Risk Reviewer (new), Strategist
- **Purpose:** Financial decisions — quantitative analysis, downside risk assessment, long-term strategic positioning.

### 4. Strategy & Planning Squad
- **Orchestrator:** `task-management`
- **Members:** Strategist, Practitioner, Devil's Advocate
- **Purpose:** Planning and decision-making — long-term strategy, practical implementation, stress-testing assumptions.

### Persona Migration (pre-release)

Current 6 builtins redistributed:
- **Skeptic** → General Analysis + Research (shared)
- **Connector** → General Analysis
- **Student** → General Analysis + Research (shared)
- **Devil's Advocate** → General Analysis + Strategy (shared)
- **Practitioner** → Strategy & Planning
- **Strategist** → Finance Analysis + Strategy (shared)

4 new personas added: Academic Reviewer, Methodologist, Deep Analyst, Risk Reviewer.

Pre-release: drop existing persona table, recreate with new schema. No migration scripts needed.

## Execution Architecture

### Phase 1 — Insight Review (Parallel Persona Calls)

```
User selects squad
    → resolve_squad() loads squad + all members
    → Orchestrator does shared analysis pass (tool calls OK)
    → tokio::join! fans out to N persona LLM calls (parallel)
        Each call gets:
        - Note content
        - Orchestrator context
        - Persona skill body (from PERSONA.md)
        - Scoped memory (Global + Squad + Persona)
        No tool access for persona calls
    → Each persona card streams independently (SSE per persona)
    → PerspectivesTab renders N cards with independent progress
```

**Key changes from current system:**
- `select_personas()` → `resolve_squad()` — loads squad + all members
- Single prompt → N parallel LLM calls via `tokio::join!`
- Each call includes persona skill body + three-tier scoped memory
- SSE streaming per persona card (independent progress)
- `SquadExecutor` reuses existing `DirectEngine` for each persona call

### Phase 2 — Chat Mode (Squad Conversations)

**New concept: SquadSession** — a chat session bound to a squad.

```
User sends message to squad
    → AgentRuntime::process_message(squad_id: Some("..."))
    → Orchestrator routes:
        1. Tool call → orchestrator handles directly
        2. Fan-out → each persona responds with perspective
        3. Synthesize → combine persona outputs
    → User toggles: multi-voice (all personas) or synthesized (merged)
    → Persona outputs stored as Message rows with persona_id field
```

**Integration:** `AgentRuntime::process_message()` gains `squad_id: Option<String>`. When set, runtime loads squad, resolves orchestrator + personas, executes layered. Orchestrator still controls tool filtering, MCP access, delegation.

### Phase 3 — Multi-Agent Collaboration (Future)

Intentionally left high-level — designed after Phase 1+2 usage data:
- Inter-persona invocation within a squad (debate mode)
- Shared working memory (blackboard pattern)
- Memory promotion: persona observations → squad knowledge → global facts
- Consensus detection by orchestrator
- Squad-level learning via FSRS-based skill improvement per persona

## Crate Impact

### Major Changes

**cognitive (L5):**
- New `SquadRow`, `SquadMemberRow` tables
- New `SquadRepo` — CRUD, `resolve_squad()`, `seed_builtins()` (loads from `squads/{name}/SQUAD.md` at startup, same pattern as `status_workflows`)
- New `PersonaSkillMetadata` struct
- Extend `PersonaRow` — add `skill_path`, `questioning_style`, `cognitive_bias`, `analysis_frameworks`
- Extend `PersonaRepo::seed_builtins()` for squad-aware seeding
- Extend `MemoryRetriever` — accept scope chain `[global, squad, persona]`
- Extend `SemanticFactRepo` — queries with `scope_type IN ('system','squad','persona')`

**app-core (L7):**
- Rewrite `InsightHandler` — `resolve_squad()` + parallel LLM calls replacing single-prompt approach
- New `SquadHandler` — CRUD operations (AppCore methods)
- New squad Tauri commands — list/create/update/delete squads, add/remove members
- Extend `insight_prompts` — per-persona prompt with skill body injection

**desktop + desktop-ui (L7):**
- New `commands/squads.rs` Tauri command module (+ `DEV_COMMANDS` constant)
- New `SquadPicker` component — replaces "Manage Personas" button in Insight Review
- New `SquadManager` component — full CRUD UI for squads
- New `useSquads` hook — IPC bindings for squad operations
- Modify `ManagePersonasModal` — nested inside squad management context
- Modify `PerspectivesTab` — squad header + independent streaming card grid
- Modify `useInsightReview` — `squad_id` param + N parallel SSE streams

### Moderate Changes

**skill-system (L3):**
- New `SkillType::Persona` variant
- New `PersonaSkillParser` — reuses `SkillParser`, extracts `PersonaSkillMetadata`
- Extend `SkillPackage` — `persona_only: bool` field, skip tool/mcp when true
- Extend `SkillCatalog::discover()` — load persona skills from `squads/` dir

**agent (L5) — Phase 2 only:**
- New `SquadExecutor` — orchestrate parallel persona calls, collect results, synthesize. Reuses existing `DirectEngine` for each persona call.
- Extend `AgentRuntime::process_message()` — `squad_id: Option<String>` parameter
- New `PersonaPerspectiveSource` — `ContextSource` impl for persona skill injection

### Zero Changes

- `SkillRouter` — still routes to orchestrator skills (squads layer above)
- `ExecutionRouter` — Direct/Reactive modes unchanged
- `ToolRegistry` — tool access still via orchestrator
- `CostTracker` — already per-call, naturally tracks N persona calls
- `ContextEngine` — existing sources unchanged
- `config`, `bus`, `storage`, `providers`, `session` — zero changes
- All `feature-*` crates — zero changes
- `channels` — zero changes (Phase 1+2 only affects desktop)

## UI/UX Changes (Phase 1)

### Insight Review — Before → After

**Before:** Flat "Manage Personas" modal with 6 checkboxes. All 6 active but only 4 generated. No grouping, no purpose context.

**After:** Squad Picker dropdown — select a squad, all its members generate perspectives. Clear, purposeful grouping with member previews.

### PerspectivesTab Redesign

- Squad header with icon, name, and member count
- 2-column grid of `PersonaCard` components
- Each card streams independently with status: "done" / "streaming..." / "queued"
- Animated cursor on actively streaming cards

### Squad Management (new screen)

- Full CRUD for squads
- Drag-to-reorder members (sort_order)
- Per-member role display (lead, reviewer, advisor)
- "Add Persona" opens persona picker (existing + create new)
- Builtin squads customizable (add/remove members) but not deletable
- User squads have full CRUD

### New & Changed Components

| Component            | Change  | Description                                           |
|----------------------|---------|-------------------------------------------------------|
| `SquadPicker`        | New     | Dropdown in Insight Review toolbar                    |
| `SquadManager`       | New     | Full management screen for squads                     |
| `useSquads`          | New     | Hook: listSquads, getSquad, createSquad, etc.         |
| `PerspectivesTab`    | Changed | Squad header + independent streaming card grid        |
| `ManagePersonasModal`| Changed | Nested inside SquadManager                            |
| `useInsightReview`   | Changed | squad_id + N parallel SSE streams                     |

## Testing Strategy

### Unit Tests (cognitive crate)

- `SquadRepo` CRUD: create, list, get, update, delete
- `SquadRepo::seed_builtins()` — idempotent seeding of 4 builtin squads
- `SquadRepo::resolve_squad()` — returns squad + ordered members with persona details
- Squad membership: add/remove members, multi-squad membership
- `PersonaSkillMetadata` parsing from PERSONA.md frontmatter
- Memory scope chain queries: verify correct tier filtering

### Unit Tests (skill-system crate)

- `PersonaSkillParser` — parse PERSONA.md, extract metadata, ignore tool fields
- `SkillType::Persona` — discovery from squads/ directory
- `SkillCatalog` includes persona skills alongside orchestrator/regular skills

### Integration Tests (app-core)

- `InsightHandler` with squad: resolve squad → parallel calls → collect results
- Squad CRUD through AppCore handlers
- Persona streaming: verify N independent SSE streams

### Frontend Tests (desktop-ui)

- `SquadPicker` — renders squads, selection updates state
- `useSquads` — IPC mock tests for all operations
- `PerspectivesTab` — renders N persona cards with independent stream state

## Phasing

### Phase 1: Insight Review (this spec)
- Squad data model + storage
- Persona skill format + parser
- Builtin squads seeded
- Parallel persona LLM calls
- Squad Picker + PerspectivesTab UI
- Squad Management UI
- Three-tier memory scoping

### Phase 2: Chat Mode (future spec)
- `SquadSession` concept
- `SquadExecutor` in agent crate
- `squad_id` on `process_message()`
- Multi-voice vs. synthesized toggle
- Chat UI for squad conversations

### Phase 3: Multi-Agent (future spec)
- Inter-persona invocation
- Blackboard working memory
- Memory promotion (persona → squad → global)
- Consensus detection
- FSRS-based persona learning
