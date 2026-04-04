# Flat Skill System — Design Spec

> **Goal:** Replace the routed/delegated skill system with a flat architecture inspired by Claude Code. All tools available to every message, skills as user-editable markdown files, KLYNTBOT.md as the platform soul file. Eliminate delegation entirely.

---

## Problem

The current skill system wastes tokens through unnecessary routing and delegation:

1. **SkillRouter** picks an orchestrator skill per message → filters tools to that skill's allowlist
2. **IntentAnalyzer** classifies intent with heuristic + embedding layers → decides complexity
3. When the router picks the wrong skill, the model must **delegate** to a sub-agent — spawning a full sub-runtime with different tools at ~3-4K token overhead
4. False multi-domain detection (e.g., "Work **and** Personal" triggering cross-domain) forces General orchestrator → delegation even for single-domain requests
5. The delegation tool counts against the execution budget, wasting turns and tokens

**Measured impact:** The word "and" in area names triggers orchestration. A simple "create area" request costs ~7K tokens through General → delegate → task-management, when direct execution would cost ~2.5K.

## Solution: Claude Code Architecture

Adopt Claude Code's proven pattern:

- **Flat tool pool** — all tools available to every message, model picks
- **Skills as context documents** — markdown files with YAML frontmatter, not routing rules
- **Two-layer always-loaded context** — KLYNTBOT.md (soul) + skill YAML listing (capabilities)
- **On-demand skill loading** — full skill body loaded via `skill_reference` tool when the model needs domain instructions
- **No delegation** — single execute loop handles everything

---

## Architecture

### File Structure

```
~/.klyntbot/
├── KLYNTBOT.md              ← Soul file (always loaded)
├── config.json
├── skills/                  ← User-editable skills
│   ├── task-management.md
│   ├── finance-management.md
│   ├── automation.md
│   ├── notebook.md
│   └── learning.md
├── data.db
└── ...
```

### Skill File Format

Markdown with YAML frontmatter. Matches Claude Code's skill format:

```markdown
---
name: task-management
description: Create, organize, and track tasks, projects, areas using OKR+PARA
whenToUse: When the user mentions todos, tasks, projects, areas, planning, reviews, or goals
---

You are the task management specialist...

## Core Workflow
Every task belongs to an **area**...
```

**YAML fields:**

| Field | Required | Purpose |
|---|---|---|
| `name` | Yes | Skill identifier |
| `description` | Yes | Short description (max 250 chars, always in system prompt) |
| `whenToUse` | No | Guidance for when model should load this skill |
| `references` | No | List of sub-files for on-demand loading |

### KLYNTBOT.md — The Soul File

Always loaded into system prompt. User-editable. Controls personality, tone, voice, persona, and platform-wide preferences.

```markdown
# Klyntbot

You are Klyntbot, a personal AI assistant.

## Personality
- Helpful, concise, and proactive
- Speak naturally, not robotically
- Match the user's language (if they write in Vietnamese, respond in Vietnamese)

## Preferences
- Use metric units
- Currency: VND
- Timezone: auto-detect from system
```

### Default Skills (5)

| Skill | Domain knowledge |
|---|---|
| `task-management.md` | OKR+PARA framework, area/project/objective conventions, planning workflows |
| `finance-management.md` | Currency handling, VND formatting, account types, budgeting patterns |
| `automation.md` | Cron syntax, reminder patterns, recurring job setup |
| `notebook.md` | Note-taking conventions, tagging, linking, knowledge capture workflows |
| `learning.md` | Flashcard generation, spaced repetition, study patterns, knowledge review |

**Removed skills:**
- `general` — its job was fallback routing + delegation, both eliminated
- `communication` — messaging is a tool capability, not domain expertise

---

## System Prompt Assembly

Three layers, always sent with every API call:

```
┌─────────────────────────────────────────────┐
│  Layer 1: Base System Prompt                │
│  (hardcoded, compiled into binary)          │
│  - Core identity & capabilities             │
│  - Safety rules, output format              │
│  - Tool usage instructions                  │
│  - ~2000 tokens                             │
├─────────────────────────────────────────────┤
│  Layer 2: KLYNTBOT.md                       │
│  (user-editable soul file, always loaded)   │
│  - Personality, tone, persona               │
│  - User preferences                         │
│  - ~200-1000 tokens                         │
├─────────────────────────────────────────────┤
│  Layer 3: Skill Listing                     │
│  (YAML frontmatter only, always loaded)     │
│  - name + description + whenToUse per skill │
│  - Budget-capped at 1% of context window    │
│  - ~30-50 tokens per skill × 5 = ~200 toks  │
│  - Includes: "Use skill_reference tool to   │
│    load full instructions when needed"      │
└─────────────────────────────────────────────┘
```

**Skill listing format** (injected as system-reminder):

```
Available skills (use skill_reference tool to load full instructions):
- task-management: Create, organize, and track tasks, projects, areas using OKR+PARA — When user mentions todos, tasks, projects, areas, planning, reviews, or goals
- finance-management: Personal finance tracking with multi-currency support — When user mentions expenses, budget, accounts, transactions, or spending
- automation: Reminders, cron jobs, and recurring automations — When user mentions remind, schedule, every day, recurring, or automate
- notebook: Note-taking, knowledge capture, and idea organization — When user mentions notes, jot down, write down, or capture
- learning: Flashcard generation, spaced repetition, and study workflows — When user mentions study, flashcards, review, learn, or quiz
```

### Tool Pool — Flat

All registered tools from all `FeaturePackage`s sent to every API call. No filtering by skill. The model picks what it needs.

```rust
// Before: filter tools per skill
let filtered_tools = filter_tools_for_profile(tool_definitions, &profile);

// After: send everything
let tools = tool_definitions; // all tools, always
```

### `skill_reference` Tool

New tool that loads a skill's full body by name. Replaces SkillRouter + progressive loading.

```
Tool: skill_reference
Parameters: { "name": "task-management" }
Returns: Full markdown body of the skill (everything after YAML frontmatter)
```

The model calls this when it recognizes (from the skill listing) that a skill has relevant instructions for the current task. This is an explicit model decision — no routing heuristics.

---

## Simplified Pipeline

### Before (6 phases with routing):
```
Route → Prepare → Execute → Enrich → Record → Adapt
```

### After (3 phases, no routing):
```
Prepare → Execute → Record
```

**Phase 1: Prepare**
1. Load KLYNTBOT.md (cached, hot-reloaded on file change)
2. Load skill listing (YAML frontmatter, cached)
3. Build system prompt (base + KLYNTBOT.md + skill listing)
4. Collect ALL tool definitions (flat, no filtering)
5. Assemble context (memory retrieval, history)
6. Create `ExecutionBudget` from depth mode

No routing. No classification. No skill selection. No tool filtering.

**Phase 2: Execute**
1. `execute_loop()` with all tools + budget
2. Model calls tools as needed (including `skill_reference`)
3. Budget gates, wrap-up injection, cancellation — unchanged from budget-bounded execution

**Phase 3: Record**
1. Cost tracking (unchanged)
2. Usage report event (unchanged)
3. Interaction recording (simplified — no strategy repo, no mode tracking)

Enrich and Adapt remain as depth-gated placeholders for DeepThink/Ultra mode.

### Simplified `process_message()` Signature

```rust
pub async fn process_message(
    &self,
    message: &str,
    history: Vec<Message>,
    tool_definitions: &[serde_json::Value],  // ALL tools, unfiltered
    ctx: &RoutingContext,
    event_tx: Option<tokio::sync::mpsc::Sender<AgentEvent>>,
    cancel_token: Option<tokio_util::sync::CancellationToken>,
    depth: DepthMode,
) -> Result<RuntimeResult>
```

**Removed parameters:**
- `tool_names: &[&str]` — was for IntentAnalyzer
- `system_prompt: Option<&str>` — now built internally from KLYNTBOT.md + skills
- `correction: Option<CorrectionContext>` — folds into context assembly

### Slimmed `AgentRuntime` Struct

| Field | Status |
|---|---|
| `skill_catalog` | **Remove** |
| `skill_router` | **Remove** |
| `analyzer` | **Remove** |
| `context_engine` | **Keep** |
| `core` | **Keep** |
| `validator` | **Keep** |
| `cost_tracker` | **Keep** |
| `config` | **Simplify** — remove PipelineConfig, keep model/provider info |
| `active_profile` | **Remove** |
| `activated_skills` | **Remove** |
| `delegation_self_ref` | **Remove** |
| `autotuner_hook` | **Keep** |
| `hot_config` | **Keep** |
| `skill_store` | **New** — loaded YAML frontmatter + full bodies |

---

## Migration

### Skill Conversion

Current compiled skills (`skills/*/SKILL.md`) become runtime files. On first run, if `~/.klyntbot/skills/` is empty, write defaults from embedded templates:

```rust
const DEFAULT_SKILLS: &[(&str, &str)] = &[
    ("task-management.md", include_str!("../../skills/task-management.md")),
    ("finance-management.md", include_str!("../../skills/finance-management.md")),
    ("automation.md", include_str!("../../skills/automation.md")),
    ("notebook.md", include_str!("../../skills/notebook.md")),
    ("learning.md", include_str!("../../skills/learning.md")),
];
```

### Crate Changes

| Crate | Change |
|---|---|
| `skill-system` | **Gut** — remove `SkillRouter`, `SkillCatalog`, `SkillPackage`, progressive loading. Replace with `SkillStore` (load `.md` files, parse YAML, serve frontmatter + full body) |
| `agent` | **Simplify** — remove `IntentAnalyzer`, delegation handler, orchestration override. Slim `AgentRuntime` to 3-phase pipeline |
| `tools` | **Remove** `DelegationTool`. **Add** `SkillReferenceTool` |
| `config` | **Remove** `OrchestratorConfig`, `SkillConfig`. Keep `ExecutionConfig` |
| `app-core` | **Simplify** builder — no skill catalog, no router, no analyzer. Load KLYNTBOT.md + skills on startup |
| `context-engine` | **Simplify** — remove `SkillContextSource`, `AgentContextSource`. System prompt built from KLYNTBOT.md + skill listing |
| `simulator` | **Simplify** — remove skill catalog/router setup, just pass all tools |

### What Stays Unchanged

- `ExecutionBudget`, `DepthMode`, `execute_loop` (budget-bounded execution)
- `ExecutionCore`, `CycleOutcome`, tool execution mechanics
- All feature crates and tool implementations
- Memory/cognitive system
- MCP server/client
- Desktop UI / Tauri adapter
- Channel integrations

### What Gets Deleted

- `crates/agent/src/intent_pipeline/analysis.rs` — all classification logic
- `crates/agent/src/intent_pipeline/engines/` — already deleted (budget-bounded work)
- `crates/tools/src/domain/delegation.rs` — DelegationTool
- `crates/agent/src/autotuner/shadow_classifier.rs` — classified against removed IntentAnalyzer
- Orchestration override logic in `runtime.rs`
- Tool filtering functions (`filter_tools_for_profile`, `ORCHESTRATOR_ALLOWED_TOOLS`)

**Estimated impact:** Delete ~4000 lines, add ~500 lines.

---

## Token Budget Comparison

### Before (routed + delegated):
```
Single-domain "create area":
  Routing/classification:    ~500 tokens
  General orchestrator call: ~2500 tokens (system prompt + tools + LLM response)
  Delegation overhead:       ~500 tokens (tool call + sub-runtime setup)
  Sub-agent execution:       ~2500 tokens (task-mgmt system prompt + tools + LLM)
  Total:                     ~6000 tokens

Cross-domain "create task and log expense":
  Routing/classification:    ~500 tokens
  General orchestrator:      ~3000 tokens (2 delegation decisions)
  Delegation 1 (tasks):      ~3000 tokens
  Delegation 2 (finance):    ~3000 tokens
  Total:                     ~9500 tokens
```

### After (flat + skill_reference):
```
Single-domain "create area":
  System prompt (base + KLYNTBOT.md + listing): ~2500 tokens
  skill_reference("task-management"):           ~800 tokens (one tool call)
  area:create tool call:                        ~200 tokens
  LLM response:                                ~500 tokens
  Total:                                        ~4000 tokens (33% reduction)

Cross-domain "create task and log expense":
  System prompt:                                ~2500 tokens
  skill_reference("task-management"):           ~800 tokens
  area:create + task tool calls:                ~400 tokens
  skill_reference("finance-management"):        ~800 tokens
  finance tool calls:                           ~400 tokens
  LLM response:                                ~500 tokens
  Total:                                        ~5400 tokens (43% reduction)
```

---

## Non-Goals

- **Skill marketplace / sharing** — future concern, not in this spec
- **Per-skill tool restrictions** — intentionally removed, model chooses freely
- **Skill versioning** — files on disk, user manages via git or manual backup
- **Debate engine changes** — stays as a separate engine, not affected by this redesign
