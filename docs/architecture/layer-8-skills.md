# Layer 8: Built-in Orchestrator Skills

## Overview

Klyntbot uses a skill-based architecture to route user messages to specialized domain agents. Five built-in orchestrator skills live in the `skills/` directory at the workspace root. Each skill is a directory containing a `SKILL.md` file (with YAML frontmatter + Markdown instructions) and a `references/` folder of detailed sub-workflows.

The `skill-system` crate (`crates/skill-system/`) compiles these files into the binary via `include_str!`, parses them at startup, and routes incoming messages to the appropriate orchestrator using keyword + semantic scoring.

## Skill Architecture

### Skill Types

| Type | Purpose | Routing |
|---|---|---|
| **Orchestrator** | Domain-specific agent that owns a set of tools and can delegate to other orchestrators | Selected per-message by `SkillRouter` |
| **Skill** | Non-orchestrator skill activated alongside an orchestrator for supplemental context | Activated by keyword/semantic threshold |
| **Persona** | User-defined personality overlay parsed from `PERSONA.md` files | Applied on top of the active orchestrator |

### Skill Scopes

| Scope | Priority | Source | Trusted |
|---|---|---|---|
| `BuiltIn` | 0 (lowest) | `skills/` compiled into binary | Yes |
| `User` | 1 | `~/.klyntbot/skills/` on filesystem | Yes |
| `Project` | 2 (highest) | Project-local `.klyntbot/skills/` | No (untrusted) |

Higher-priority scopes shadow lower-priority skills with the same name.

## SKILL.md Format Specification

Each skill is defined by a `SKILL.md` file following the Agent Skills spec:

```markdown
---
name: <skill-name>              # Required. Lowercase alphanumeric + hyphens, max 64 chars
description: >                  # Required. Multi-line description used for routing
  Description text used for keyword matching and semantic scoring.
license: MIT                    # Optional
compatibility: <text>           # Optional. External dependency notes
metadata:
  author: klyntbot              # Custom metadata (preserved in SkillMetadata.custom)
  version: "2.0.0"             # Custom metadata
  klyntbot:                     # Klyntbot-specific configuration block
    type: orchestrator          # "orchestrator" or omit for regular skill
    tools: [tool1, tool2]       # Allowed internal tools (null = all, [] = none)
    mcp_tools: ["*"]            # MCP servers this skill can access ("*" = all, [] = none)
    max_iterations: 15          # ReAct loop iteration limit (default: 10)
    can_delegate_to: [skill-a]  # Other orchestrators this one may delegate to
    always_skills: [sub-skill]  # Reference files auto-loaded with this orchestrator
    invokes: [skill-b]          # Skills this one may chain to (informational)
    triggers:                   # Keyword triggers for routing
      - keyword phrase 1
      - keyword phrase 2
---

Markdown body with agent instructions, decision flowcharts, routing tables,
red flags, response style guidance, and references to sub-workflows.
```

### Frontmatter Fields

| Field | Required | Description |
|---|---|---|
| `name` | Yes | Unique skill identifier. Must be lowercase alphanumeric + hyphens |
| `description` | Yes | Used for keyword tokenization and semantic embedding during routing |
| `license` | No | License identifier |
| `compatibility` | No | Notes on external dependencies (e.g., "Requires Google Calendar MCP") |
| `metadata.klyntbot.type` | No | `"orchestrator"` for domain agents; omit for regular skills |
| `metadata.klyntbot.tools` | No | Whitelist of internal tools. `null`/omitted = all tools. `[]` = no tools. `ask_user` is always injected |
| `metadata.klyntbot.mcp_tools` | No | MCP server access. `["*"]` = all servers. `["google-calendar"]` = specific server. `[]` = none |
| `metadata.klyntbot.max_iterations` | No | Maximum ReAct loop iterations (default: 10) |
| `metadata.klyntbot.can_delegate_to` | No | Orchestrator names this skill can delegate work to |
| `metadata.klyntbot.always_skills` | No | Reference file names loaded alongside this orchestrator (resolved to `references/<name>.md`) |
| `metadata.klyntbot.invokes` | No | Skills this one may chain to (informational, used for dependency tracking) |
| `metadata.klyntbot.triggers` | No | Keyword phrases for routing. Matched against user messages during skill selection |

### Parser Behavior

The parser (`crates/skill-system/src/parser.rs`) is intentionally lenient:

- **YAML colon fix** -- Unquoted colons in values (common cross-client issue) are auto-quoted
- **Name validation** -- Violations (uppercase, consecutive hyphens, length > 64) produce warnings but skills still load
- **Missing description** -- The only hard failure; description is required for routing
- **Empty body** -- Allowed; the skill is metadata-only

## Built-in Orchestrator Skills

### 1. General (`skills/general/`)

**Purpose:** Default catch-all orchestrator for greetings, casual conversation, factual questions, web search, memory operations, and multi-domain orchestration.

| Property | Value |
|---|---|
| **Tools** | `ask_user`, `memory`, `web_search`, `web_fetch`, `grep`, `glob`, `read_file`, `list_dir`, `spawn`, `learning` |
| **MCP Access** | `["*"]` (all MCP servers) |
| **Max Iterations** | 15 |
| **Delegates To** | `task-management`, `finance-management`, `automation`, `communication` |
| **Always Skills** | (none) |

**Trigger Keywords:** hello, hi, hey, thanks, how are you, good morning, what is, how does, look up, find out, search for, remember this, do you recall, summarize, help me, what should I do today, catch me up

**Key Behavior:** Routes to specialist skills whenever possible. Handles multi-domain requests by decomposing into discrete steps, delegating each, and synthesizing a unified response. Never attempts domain-specific tool calls directly.

**Reference Files:**
- `references/search.md` -- Web search and information retrieval
- `references/memory.md` -- Storing and recalling user facts
- `references/browser.md` -- Browser automation
- `references/summarize.md` -- Summarizing URLs, articles, content
- `references/skill-creator.md` -- Creating new skills

### 2. Task Management (`skills/task-management/`)

**Purpose:** Task and project management specialist using OKR+PARA framework. Handles task creation, daily/weekly planning, project health, retrospectives, decomposition, and goal tracking.

| Property | Value |
|---|---|
| **Tools** | `task`, `tasks`, `area`, `project`, `okr`, `notes`, `productivity`, `ask_user`, `memory`, `grep`, `glob`, `read_file`, `list_dir` |
| **MCP Access** | `["google-calendar"]` (calendar operations only) |
| **Max Iterations** | 12 |
| **Delegates To** | `finance-management` |
| **Always Skills** | `todo`, `daily-planner` |

**Trigger Keywords:** create a task, add todo, plan my day, break this down, decompose, weekly review, weekly report, project retrospective, monthly review, score OKRs, project status, what's next, prioritize, daily plan, overdue, backlog, notes, estimate, forecast

**Key Behavior:** Every task belongs to an area. Always follows the todo skill workflow for creation (ask-first, enrichment, confidence scoring). Uses daily-planner for morning/evening planning. Distinguishes "weekly review" (interactive GTD) from "weekly report" (passive summary). Never creates tasks without an `area_id`.

**Reference Files:**
- `references/todo.md` -- Complete task creation workflow
- `references/daily-planner.md` -- Morning/evening planning
- `references/task-decompose.md` -- Breaking down complex goals
- `references/project-management.md` -- Project health and tracking
- `references/weekly-review.md` -- Interactive GTD-style review
- `references/retrospective.md` -- Monthly/quarterly OKR scoring
- `references/reports.md` -- Data-driven reports (weekly, retro, knowledge)

### 3. Finance Management (`skills/finance-management/`)

**Purpose:** Personal finance specialist for accounts, transactions, budgets, investments, FIRE planning, spending intelligence, portfolio analytics, and net worth tracking.

| Property | Value |
|---|---|
| **Tools** | `finance`, `ask_user`, `memory`, `web_search`, `web_fetch` |
| **MCP Access** | `[]` (no MCP servers) |
| **Max Iterations** | 10 |
| **Delegates To** | `task-management` |
| **Always Skills** | `budgeting` |

**Trigger Keywords:** how much did I spend, add expense, budget, spending, transaction, net worth, bitcoin price, crypto, stock price, FIRE, savings, investment, portfolio, financial report, debt, loan, mortgage, retirement, financial independence, spending anomaly, portfolio drift, rebalance, spending trend, net worth snapshot, change currency

**Key Behavior:** Amounts are always in smallest currency unit (e.g., $50 = 5000 cents). Auto-account selection when `account_id` is omitted. Supports multi-currency with auto-conversion. Never gives investment advice. FIRE planning includes traditional, coast, lean, and fat FIRE calculations with Monte Carlo simulation.

**Reference Files:**
- `references/budgeting.md` -- Complete action routing table
- `references/spending-intelligence.md` -- Spending analysis workflows
- `references/analytics-actions.md` -- All 19 analytical actions
- `references/fire-planning.md` -- FIRE planning workflow
- `references/portfolio-analysis.md` -- Portfolio analytics
- `references/financial-health.md` -- Financial health reports
- `references/currency-engine.md` -- Multi-currency handling

### 4. Automation (`skills/automation/`)

**Purpose:** Scheduling specialist for reminders, recurring tasks, and automated workflows using the cron system.

| Property | Value |
|---|---|
| **Tools** | `cron`, `spawn`, `ask_user`, `memory`, `productivity` |
| **MCP Access** | `[]` (no MCP servers) |
| **Max Iterations** | 10 |
| **Delegates To** | (none) |
| **Always Skills** | `cron` |

**Trigger Keywords:** remind me, reminder, schedule, every day, every hour, every week, recurring, cron, automate, automation, set an alarm, daily at, every morning, timer, repeat, periodically

**Key Behavior:** Two modes: **Reminder** (sends a message at scheduled time) and **Task** (agent executes a command on schedule and sends the result). Converts natural language time expressions to cron parameters. Warns about very frequent schedules (under 1 minute). Task mode has cost implications (LLM tokens per execution).

**Reference Files:**
- `references/cron.md` -- Complete time expression guide

### 5. Communication (`skills/communication/`)

**Purpose:** Cross-channel messaging specialist for sending messages, notifications, and broadcasts across Telegram, Discord, Slack, and Email.

| Property | Value |
|---|---|
| **Tools** | `message`, `ask_user`, `memory` |
| **MCP Access** | `[]` (no MCP servers) |
| **Max Iterations** | 10 |
| **Delegates To** | (none) |
| **Always Skills** | `messaging` |

**Trigger Keywords:** send a message, notify, tell, dm, ping, broadcast, announce, alert, email, reply to, forward this, post in, share with, reach out, contact

**Key Behavior:** Always confirms target channel, recipient, and message content before sending. Respects channel-specific formatting (MarkdownV2 for Telegram, Markdown for Discord, Block Kit mrkdwn for Slack, HTML for Email). Splits long messages for channels with length limits (Discord: 2000 chars). Never sends without explicit user confirmation.

**Reference Files:**
- `references/messaging.md` -- Channel formatting rules and examples
- `references/notification.md` -- Alert routing and batching

## Skill Compilation

Skills are compiled into the binary at build time in `crates/skill-system/src/discovery.rs`:

### Main Skill Content

```rust
macro_rules! include_skill {
    ($name:expr) => {
        ($name, include_str!(concat!("../../../skills/", $name, "/SKILL.md")))
    };
}

pub const BUILTIN_SKILLS: &[(&str, &str)] = &[
    include_skill!("general"),
    include_skill!("task-management"),
    include_skill!("finance-management"),
    include_skill!("automation"),
    include_skill!("communication"),
];
```

### Reference Files

```rust
macro_rules! include_skill_reference {
    ($skill:expr, $ref_name:expr) => {
        ($skill, $ref_name, include_str!(concat!(
            "../../../skills/", $skill, "/references/", $ref_name, ".md"
        )))
    };
}

pub const BUILTIN_SKILL_REFERENCES: &[(&str, &str, &str)] = &[
    // 21 reference files across all 5 skills
    include_skill_reference!("general", "search"),
    include_skill_reference!("general", "skill-creator"),
    // ... (see discovery.rs for full list)
];
```

At startup, `builtin_reference_map()` builds a `HashMap<String, String>` keyed by `"builtin::{skill}/references/{name}.md"` for runtime lookup.

### Discovery Pipeline

```
BUILTIN_SKILLS (compile-time)
    |
    v
SkillCatalog::discover([BuiltIn, Directory, Personas])
    |
    +-- parse_skill_md() per entry
    |       |-- split_frontmatter()
    |       |-- serde_yaml parse (with lenient colon fix)
    |       |-- validate_skill_name() (warn-only)
    |       +-- build SkillPackage
    |
    +-- Filesystem scan (User/Project scopes)
    |       |-- Recursive up to MAX_SCAN_DEPTH=4, MAX_SCAN_DIRS=2000
    |       |-- Skips node_modules, target, dist, etc.
    |       +-- enumerate_resources() for scripts/, references/, assets/
    |
    +-- Scope shadowing (Project > User > BuiltIn)
    |
    v
SkillCatalog { skills: HashMap<String, Arc<SkillPackage>> }
```

## Skill Routing

The `SkillRouter` (`crates/skill-system/src/router.rs`) selects the best orchestrator for each user message using blended keyword + semantic scoring.

### Scoring Algorithm

1. **Tokenization** -- Both the message and each skill's description are tokenized: lowercased, hyphens replaced with spaces, words under 3 chars filtered, stop words removed
2. **Keyword scoring** -- For each skill, count description tokens that appear in the message tokens. Normalize by `max(desc_tokens.len() / 3, 1)` and cap at 1.0
3. **Semantic scoring** -- If query embeddings are available, compute cosine similarity between the query embedding and precomputed skill description embeddings
4. **Blending** -- `blended = keyword * 0.7 + semantic * 0.3`
5. **Candidacy gate** -- A skill must have `keyword_score > 0` OR `semantic_score >= 0.5` to be considered
6. **Fallback** -- If no orchestrator qualifies, the `general` skill is always selected

### Constants

| Constant | Value | Purpose |
|---|---|---|
| `SKILL_ACTIVATION_THRESHOLD` | 0.4 | Minimum blended score for non-orchestrator skill activation |
| `MAX_ACTIVATED_SKILLS` | 3 | Maximum supplemental skills activated per message |
| `GENERAL_SKILL_NAME` | "general" | Fallback orchestrator name |

### Trigger Keywords vs Description Keywords

Each skill has two sources of routing signal:

- **`triggers` list** -- Explicit keyword phrases in the YAML frontmatter. These are NOT directly used by `SkillRouter` for scoring; instead they are part of the description-based matching since the description captures the same domain language
- **`description` field** -- The primary input for `SkillRouter`'s keyword tokenization. Skills should include domain-relevant terms in their description for effective routing

The triggers list serves as documentation and may be used by other routing mechanisms (e.g., `IntentAnalyzer` in the agent runtime).

## Context Injection

The `SkillContextSource` (`crates/skill-system/src/context.rs`) implements the `ContextSource` trait to inject skill instructions into the LLM system prompt:

1. **Tier 1 (Catalog)** -- `catalog_prompt()` generates an XML listing of all skills for the system prompt, providing the LLM with awareness of available capabilities
2. **Tier 2 (Full SKILL.md)** -- When an orchestrator is selected, its full body is injected wrapped in `<skill_content>` tags
3. **Always-loaded skills** -- Reference files listed in `always_skills` are loaded alongside the orchestrator (e.g., task-management always loads `todo.md` and `daily-planner.md`)
4. **Activated skills** -- Non-orchestrator skills that pass the activation threshold are injected with deduplication tracking
5. **Tier 3 (Resources)** -- Bundled resource file paths are listed in `<skill_resources>` XML for on-demand file-read activation

Key properties:
- **Protected from compaction** -- Skill context is marked `protected: true`, preventing the context engine from dropping it during token budget management
- **Deduplicated** -- Each skill is injected at most once per session, tracked via `activated_names` set
- **Dynamic token estimation** -- `estimated_tokens()` computes from actual content length (`body.len() / 4`) rather than using a fixed estimate

## Delegation and Handoff Graph

```
general (catch-all, multi-domain orchestrator)
  |-- delegates to --> task-management
  |-- delegates to --> finance-management
  |-- delegates to --> automation
  |-- delegates to --> communication

task-management (OKR+PARA)
  |-- delegates to --> finance-management

finance-management (personal finance)
  |-- delegates to --> task-management

automation (cron, reminders)
  |-- (no delegation)

communication (cross-channel messaging)
  |-- (no delegation)
```

Note: `general` can delegate to all other orchestrators and handles multi-domain orchestration by decomposing requests, delegating each part, and synthesizing results. `task-management` and `finance-management` have bidirectional delegation for cross-domain requests (e.g., "create a budget task" or "check transactions then create a task").

## MCP Tool Access Matrix

| Skill | MCP Access | Servers |
|---|---|---|
| general | `["*"]` | All connected MCP servers |
| task-management | `["google-calendar"]` | Google Calendar only |
| finance-management | `[]` | None |
| automation | `[]` | None |
| communication | `[]` | None |

Access is enforced by `SkillPackage::allows_mcp_server()` which checks for wildcard (`"*"`) or exact server name match.

## Adding a New Built-in Skill

1. Create `skills/<name>/SKILL.md` with YAML frontmatter and Markdown body
2. Add reference files to `skills/<name>/references/` as needed
3. Add `include_skill!("<name>")` to `BUILTIN_SKILLS` in `crates/skill-system/src/discovery.rs`
4. Add `include_skill_reference!("<name>", "<ref>")` entries for each reference file
5. Write a description with domain keywords for effective routing
6. Set `metadata.klyntbot.type: orchestrator` if it should be a primary routing target
7. Define `tools`, `mcp_tools`, `can_delegate_to`, `always_skills`, and `triggers`
8. Run `cargo nextest run -p skill-system` to verify parsing and routing

## Source Files

| File | Purpose |
|---|---|
| `skills/general/SKILL.md` | General-purpose orchestrator definition |
| `skills/task-management/SKILL.md` | Task management orchestrator definition |
| `skills/finance-management/SKILL.md` | Finance management orchestrator definition |
| `skills/automation/SKILL.md` | Automation orchestrator definition |
| `skills/communication/SKILL.md` | Communication orchestrator definition |
| `skills/*/references/*.md` | 22 reference files with detailed sub-workflows |
| `crates/skill-system/src/discovery.rs` | Skill compilation, discovery, catalog building |
| `crates/skill-system/src/parser.rs` | SKILL.md frontmatter parsing |
| `crates/skill-system/src/types.rs` | `SkillPackage`, `SkillCatalog`, `KlyntbotMeta` types |
| `crates/skill-system/src/router.rs` | `SkillRouter` with keyword + semantic scoring |
| `crates/skill-system/src/context.rs` | `SkillContextSource` for LLM context injection |
| `crates/skill-system/src/persona.rs` | Persona skill parsing |
