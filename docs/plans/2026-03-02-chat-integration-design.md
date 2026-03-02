# Chat Integration Design

**Date:** 2026-03-02
**Status:** Approved

## Overview

Integrate the AI agent directly into the desktop chat, replacing the current mock implementation with a fully functional system. The desktop app becomes the single entry point — embedding the AgentLoop, channels, and all background services in one Tauri process.

## Decisions

| Decision | Choice |
|---|---|
| Agent hosting | Embedded in Tauri process (replaces `klyntbot serve`) |
| Breaking changes | Acceptable — no backward compatibility required |
| Session scoping | Optional selector + auto-detect fallback |
| Sidebar sessions | Ephemeral by default, pin to promote to full thread |
| Thread grouping | PARA hierarchy (Area > Project), generic `entity_kind` for extensibility |
| Personas & skills storage | `.md` files on disk (`~/.klyntbot/personas/`, `~/.klyntbot/skills/`) |
| Channel integrations | Run in same Tauri process alongside desktop chat |

## 1. Core Architecture: Embedded AgentLoop

### Single process replaces `klyntbot serve`

The Tauri `AppCore` hosts everything that `serve.rs` currently starts:

```
AppCore {
    repos: Repos,                          // SQLite repos
    agent: Arc<AgentLoop>,                 // full agent with intent pipeline
    bus: Arc<MessageBus>,                  // inbound/outbound channels
    channel_manager: Arc<Mutex<ChannelManager>>,  // Telegram, Discord, etc.
    cron_service: CronService,             // scheduled jobs
    vector_store: Option<VectorStore>,     // LanceDB
    persona_manager: Arc<PersonaManager>,  // .md file personas
}
```

### Two message paths, one agent

**Desktop chat** (fast, in-process):
```
chat_send IPC → agent.process_direct_streaming(content, session_key, context)
  → StreamingHandle.event_rx
  → Tauri emit("agent:content_chunk") per token
  → Tauri emit("agent:done") on completion
  → React useEvent("agent:content_chunk") → render
```

**Channel messages** (bus-driven, same as today):
```
Telegram/Discord/etc → bus.publish_inbound()
  → AgentLoop::run_with_rx() → IntentPipeline
  → bus.publish_outbound() → ChannelManager → channel.send()
```

Both paths share the same AgentLoop, ToolRegistry, sessions, and storage.

### `klyntbot serve` removed

Its startup logic (MessageBus creation, AgentLoop building, ChannelManager init, CronService registration) moves into `AppCore::initialize()`. The `serve` CLI subcommand is deleted.

## 2. Session Categorization

### Generic entity context

Sessions are categorized via a `session_context` table using generic entity references — no feature-specific columns:

```sql
CREATE TABLE session_context (
    session_key TEXT PRIMARY KEY REFERENCES sessions(key) ON DELETE CASCADE,
    context_type TEXT NOT NULL DEFAULT 'general',
    entity_kind TEXT,         -- 'area' | 'project' | 'objective' | 'task' | 'finance.budgets' | etc.
    entity_id TEXT,           -- ID within that entity's table
    area_id TEXT,             -- cached for fast grouping (resolved from entity hierarchy)
    project_id TEXT,          -- cached for grouping (NULL if area-level or non-PARA)
    is_ephemeral INTEGER NOT NULL DEFAULT 0,
    is_pinned INTEGER NOT NULL DEFAULT 0,
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL
);
```

### Extensibility

New features (habits, journal, etc.) only need to:
1. Pick an `entity_kind` string (e.g., `habit`, `journal.entries`)
2. Optionally set `area_id` if they link into PARA
3. No schema migration, no new columns

### Thread list grouping

```
Area: Work
  Project: CryptoGuard
    "Security audit checklist"         2m ago
    "Smart contract review"            1d ago
  Project: Klynt
    "UI improvements"                  3w ago
  "Task automation logic"              1w ago    ← area-level, no project

Area: Personal
  "Integration with calendar"          2d ago

Finance
  Budgets
    "Monthly budget review"            5m ago
  Investments
    "Portfolio rebalancing"            1d ago

General
  "Help me plan my week"              3m ago
```

Grouping rules:
- `area_id` set → group under area (→ project if `project_id` set)
- `area_id` NULL → group under top-level feature bucket (parsed from `entity_kind` prefix)
- No context → "General" bucket

### Auto-detection

When user creates a thread without selecting a scope:
1. First message processed normally
2. If agent calls entity-specific tools, `session_context` auto-updates
3. If multiple entities touched, stays `general`

## 3. Sidebar Context Chat

### Cursor-style slide-in panel

A sidebar that appears on any page, auto-detecting the current entity from route params:

| Page | `entity_kind` | `entity_id` |
|---|---|---|
| `/project/:id` | `project` | route param |
| `/task/:id` | `task` | route param |
| `/objective/:id` | `objective` | route param |
| `/finance/budgets` | `finance.budgets` | null |
| `/` (dashboard) | null | null |
| `/chat` | — | sidebar hidden |

### Session lifecycle

- Session key format: `sidebar:{entity_kind}:{entity_id}`
- Created on first message, `is_ephemeral = 1`
- Pin button promotes to full thread (`is_pinned = 1, is_ephemeral = 0`)
- Ephemeral + unpinned sessions auto-deleted after 7 days

### Pre-loaded entity context

`PageContextSource` (priority 90) injects entity-specific data for sidebar sessions:
- Entity details (name, status, description)
- Related tasks (with IDs, priorities, due dates)
- Related OKRs (with progress)
- Entity IDs pre-loaded so tool calls work without searching

This means many questions can be answered without any tool calls — the data is already in the system prompt.

### Shared components

`MessageList` extracted from `Chat.tsx` into a shared component used by both `/chat` page and sidebar.

## 4. Persona System

### File-based personas

Personas are `.md` files with YAML frontmatter stored at `~/.klyntbot/personas/`:

```markdown
---
name: CryptoGuard Expert
scope:
  project: CryptoGuard       # matched by entity name, resolved to ID at load time
skills: [todo, browser]
tone: technical
---

You are a blockchain security expert specializing in smart contract
auditing and DeFi protocol analysis.

## Behavior
- Always check for common vulnerability patterns
- Reference CWE/SWC IDs when discussing security issues
```

### Scope declaration

```yaml
scope:
  area: Work                  # attaches to area by name
scope:
  project: CryptoGuard        # attaches to project by name
scope:
  feature: finance             # attaches to feature entity_kind
scope:
  feature: finance.investments # attaches to feature sub-category
# (omit scope for global fallback persona)
```

### Cascading inheritance

For a session scoped to Project "CryptoGuard" under Area "Work":

```
1. IdentitySource (base identity — always present)
2. + Area persona "Work" (work.md instructions)
3. + Project persona "CryptoGuard" (cryptoguard.md instructions)

Result:
  instructions = concatenated (general → specific)
  skills = union of all persona skill lists
  tone = most specific wins
```

### Skill scoping

Skills gain an optional `scope` field in frontmatter:

```yaml
---
name: finance
scope:
  feature: finance            # only injected in finance-context sessions
---
```

- Skills without `scope`: global (current behavior, filtered by packs)
- Skills with `scope`: only injected when session matches
- Saves tokens — finance instructions no longer in every conversation

### PersonaManager

```
PersonaManager (mirrors SkillManager):
  load_all()              → reads ~/.klyntbot/personas/*.md
  resolve_scope(name)     → DB lookup: name → entity ID
  for_session(context)    → returns cascaded PersonaChain
  reload()                → hot-reload without restart
```

### PersonaContextSource (priority 95)

New context source that resolves the persona chain for the current session and injects the concatenated instructions. Falls back to no-op when no persona matches.

## 5. Context Assembly Order

```
Priority 100: IdentitySource         — base agent identity, date/time
Priority  95: PersonaContextSource   — cascaded persona instructions from .md files
Priority  90: PageContextSource      — pre-loaded entity data (sidebar only)
Priority  80: MemorySource           — relevant memory (SQL + LanceDB)
Priority  70: TodoSource             — today's tasks (scoped when context set)
Priority  40: SkillSummarySource     — skill list overview
Priority  30: SkillContentSource     — full skill content (persona-scoped)
```

## 6. New/Modified Components

### New (Rust)

| Component | Crate | Purpose |
|---|---|---|
| `AppCore::initialize()` | `desktop` | Replaces `serve.rs` — builds agent, bus, channels, cron |
| `PersonaManager` | `agent` | Load, parse, resolve, cascade `.md` personas |
| `PersonaContextSource` | `agent` | Context source injecting persona instructions |
| `PageContextSource` | `agent` | Context source injecting pre-loaded entity data |
| `SessionContextRepo` | `storage` | CRUD for `session_context` table |
| Migration `002_session_context.sql` | `storage` | New table |

### Modified (Rust)

| Component | Change |
|---|---|
| `AppCore` | Add AgentLoop, MessageBus, ChannelManager, CronService |
| `chat_send` command | Call `agent.process_direct_streaming()`, emit streaming events |
| `chat_threads` command | Join with `session_context` for grouping data |
| `SkillContentSource` | Accept persona skill filter |
| `SkillManager` | Add `skills_for_persona()` method |
| `AgentLoop::process_direct_streaming()` | Accept optional `SessionContext` for page context |

### Removed (Rust)

| Component | Reason |
|---|---|
| `crates/cli/src/serve.rs` | Merged into desktop AppCore |
| `serve` CLI subcommand | No longer needed |

### New (Frontend)

| Component | Purpose |
|---|---|
| `SidebarChat` | Cursor-style slide-in panel with page context |
| `usePageContext()` | Hook: reads current route → entity_kind + entity_id |
| `MessageList` (extracted) | Shared message renderer for /chat and sidebar |
| Thread list grouping | Groups by PARA hierarchy + feature buckets |
| Scope selector | Optional dropdown when creating new thread |

### Modified (Frontend)

| Component | Change |
|---|---|
| `Chat.tsx` | Real streaming via `useEvent("agent:*")`, remove mocks |
| `ChatPanel.tsx` | Replace with SidebarChat |
| Thread sidebar | Group by area/project instead of flat list |

## 7. File Structure Summary

```
~/.klyntbot/
  config.json                    # app config (unchanged)
  data.db                        # SQLite (sessions, entities, context)
  lance/                         # LanceDB vectors
  skills/                        # skill .md files (unchanged pattern)
    {name}/SKILL.md
  personas/                      # NEW — persona .md files
    {name}.md
```
