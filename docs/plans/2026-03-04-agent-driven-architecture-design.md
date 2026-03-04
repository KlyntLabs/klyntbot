# Agent-Driven Architecture Design

**Date**: 2026-03-04
**Status**: Approved
**Approach**: Agent Runtime (Approach B)

## Problem Statement

The current AI system is a monolithic agent that handles everything through one pipeline. Skills exist but are just prompt injections — they don't control execution. Subagents exist but are underutilized. Learning collects data but doesn't close the feedback loop.

The goal is to upgrade to an agent-driven architecture where:
- Domain-specific agent profiles shape the LLM's behavior per request
- Agents can delegate to other agents for composed capabilities
- The system auto-decides sync vs async execution
- A unified learning system replaces fragmented memory + learning

## Current State Assessment

### Skills — Fully Active
- 9 built-in skills compiled via `include_str!`, loaded by `SkillManager`
- 3 always-loaded (`todo`, `daily-planning`, `finance`) — injected into every system prompt
- 6 trigger-matched (`cron`, `browser`, `summarize`, `weather`, `weekly-report`, `skill-creator`)
- Two injection paths: `SkillContentSource` (priority 30) + dynamic injection at `pipeline.rs:225`

### Subagents — Fully Implemented, Underutilized
- Complete `SubagentManager` with spawn/cancel/status, 3 profiles, concurrency limiting
- `SpawnTool` registered and LLM-callable — LLM decides when to spawn
- `AgentTaskTool` provides task board with dependency-ordered claiming
- Results route back via message bus
- Gap: No hierarchical spawning, no automatic orchestration

### Learning — Half-Complete
- Strategy records written after every message (working)
- Strategy history feeds LLM classifier (working)
- Emoji reactions → satisfaction scores (working)
- Adaptive threshold in system prompt (working)
- `OutcomeRecorder::record_tool_outcome()` never called in production (broken)
- Threshold adaptation via confidence bands: dead (no data flows in)

### Intent Pipeline — Fully Functional
- 12 context sources (priority 100→30) build the system prompt
- Two-stage classification: heuristic → LLM classifier
- Token budget waterfall
- ReAct loop with fabrication detection, duplicate tool call prevention
- Two memory systems: explicit notes + ANN conversation recall

## Design Decisions

| Decision | Choice | Rationale |
|---|---|---|
| Agent routing model | Single agent, skill-enhanced | Avoids latency of separate processes; one LLM, many hats |
| Agent profile structure | Flat folder with AGENT.md + skills/ | Self-contained, like Claude Code's approach |
| Subagent handling | Smart delegation + auto-async | AI decides sync vs async; delegation for composition |
| Fallback agent | General agent as orchestrator | Handles greetings, ambiguous, cross-domain; can delegate |
| Learning architecture | Unified (replaces memory) | One system: user profile, patterns, agent adaptation, conversation recall |

## Architecture

```
┌─────────────────────────────────────────────────┐
│                   AgentLoop                      │
│  (unchanged: message routing, session mgmt)      │
├─────────────────────────────────────────────────┤
│                  AgentRuntime                     │
│  ┌──────────┐  ┌──────────┐  ┌───────────────┐  │
│  │  Agent    │  │  Intent  │  │   Context     │  │
│  │  Manager  │  │  Analyzer│  │   Engine      │  │
│  │          │  │          │  │  (per-agent)  │  │
│  └──────────┘  └──────────┘  └───────────────┘  │
│  ┌──────────────────────────────────────────┐    │
│  │         ExecutionCore + ReactiveEngine    │    │
│  │  ┌────────────┐  ┌──────────────────┐    │    │
│  │  │  Agent     │  │  Delegation      │    │    │
│  │  │  Tools     │  │  Tool            │    │    │
│  │  └────────────┘  └──────────────────┘    │    │
│  └──────────────────────────────────────────┘    │
├─────────────────────────────────────────────────┤
│              Unified Learning System             │
│  ┌──────────┐ ┌──────────┐ ┌─────────────────┐  │
│  │  User    │ │Behavioral│ │    Agent        │  │
│  │  Profile │ │ Patterns │ │  Adaptation     │  │
│  └──────────┘ └──────────┘ └─────────────────┘  │
│  ┌──────────────────────────────────────────┐    │
│  │       Conversation Memory (existing)      │    │
│  └──────────────────────────────────────────┘    │
├─────────────────────────────────────────────────┤
│  Storage │ Providers │ Channels │ Tools │ Bus    │
│            (all unchanged)                       │
└─────────────────────────────────────────────────┘
```

## Component 1: Agent Profile System

### On-Disk Structure

```
agents/                          # built-in (compiled via include_str!)
  general/
    AGENT.md                     # fallback orchestrator
    skills/
      memory.md
      search.md
  task/
    AGENT.md
    skills/
      todo.md
      planning.md
      project-management.md
  finance/
    AGENT.md
    skills/
      budgeting.md
      investments.md
      spending-analysis.md
  calendar/
    AGENT.md
    skills/
      scheduling.md
      daily-planning.md

~/.klyntbot/agents/              # user-defined (override or extend)
  custom-agent/
    AGENT.md
    skills/
      custom-skill.md
```

### AGENT.md Format

```yaml
---
name: task
description: Task and project management specialist
tools: [task, area, project, okr, memory]
triggers: [todo, task, create a task, my tasks, focus, project, area, objective]
max_iterations: 10
can_delegate_to: [calendar, finance]
always_skills: [todo]
---

You are the task management agent. You help users create, organize, and track tasks,
projects, areas, and objectives using the OKR+PARA framework.

## Behavior
- When creating tasks, always use the enrichment system to auto-infer priority and due dates
- For "plan my day" requests, check both tasks and calendar
- When a task relates to finance, delegate to the finance agent for budget context

## Response Style
- Be concise and action-oriented
- Confirm task creation with a brief summary
- Suggest next actions when relevant
```

### AgentManager

Replaces `SkillManager`. Responsibilities:
- Load built-in agents from `agents/` at compile time via `include_str!`
- Load workspace agents from `~/.klyntbot/agents/` at runtime (override built-in by name)
- Parse AGENT.md frontmatter + body + per-agent skills
- `match_agent(message) -> AgentProfile` — keyword trigger matching
- `get_agent(name) -> AgentProfile` — for delegation lookup
- Filter by enabled packs (like current `filter_by_skills`)

### Agent Selection Flow

```
User message arrives
  → AgentManager::match_agent(message)
    → Keyword scan across all agent triggers
    → If high-confidence match: return that agent
    → If ambiguous/no match: return GeneralAgent
  → AgentProfile loaded with: instructions, tool filter, skills, delegation targets
```

`IntentAnalyzer` determines execution mode (Direct vs Reactive + iteration budget). `AgentManager` determines which agent profile. These are orthogonal concerns.

## Component 2: Agent Runtime + Delegation

### AgentRuntime (replaces IntentPipeline)

```
AgentRuntime::process_message(msg, session_history, system_prompt)
  │
  ├─ Step 1: Agent Selection
  │    AgentManager::match_agent(msg) → AgentProfile
  │
  ├─ Step 2: Intent Analysis (reuse existing IntentAnalyzer)
  │    Determines: Direct vs Reactive, iteration budget
  │    Uses: strategy history from learning (existing)
  │
  ├─ Step 3: Context Assembly
  │    Build system prompt from:
  │      - Base identity (IdentitySource)
  │      - Agent instructions (AGENT.md body)
  │      - Agent's always-loaded skills
  │      - Learning context (unified, replaces MemorySource)
  │      - Matched trigger skills (non-always)
  │      - Confidence threshold
  │    Tool definitions filtered to agent's allowed tools
  │    + DelegationTool (if agent has can_delegate_to)
  │
  ├─ Step 4: Execution (reuse ExecutionCore + ReactiveEngine)
  │    If Direct: single LLM call, no tools
  │    If Reactive: ReAct loop with agent-scoped tools
  │      - LLM can call DelegationTool mid-loop
  │      - Auto-async check after iteration 3
  │
  ├─ Step 5: Validation + Cost Tracking (reuse existing)
  │
  └─ Step 6: Learning Record
       Strategy record + outcome recording (now actually wired)
```

### DelegationTool

Lets agents call other agents synchronously within a ReAct loop:

```rust
// LLM calls:
delegate(agent: "calendar", query: "what meetings do I have this week?")

// Internally:
// 1. Load CalendarAgent profile
// 2. Build agent-scoped context (calendar instructions + skills)
// 3. Run a mini ReAct loop (max 5 iterations, scoped tools)
// 4. Return the result as tool output to the calling agent
```

Constraints:
- Max delegation depth: 2 (A → B → C, no deeper)
- Delegated agent inherits session but gets own system prompt
- Only agents in `can_delegate_to` are callable
- Reduced iteration budget for delegated calls

### Auto-Async

System decides to go async based on complexity signals:

```
After Step 2 (Intent Analysis):
  If estimated_tool_calls > 8 OR complexity_score > "high":
    → Send immediate response: "I'll work on this and message you when ready."
    → Spawn async task using evolved SubagentManager
    → Async task uses same AgentProfile + ExecutionCore
    → Result routes back via message bus (existing mechanism)
```

Threshold is tunable in config and adapts via learning. User never decides.

### Removals

- `IntentPipeline` → replaced by `AgentRuntime`
- `ToolGroup` enum → replaced by agent profile tool lists
- `SkillManager` → absorbed into `AgentManager`
- `SkillSummarySource` + `SkillContentSource` → replaced by `AgentContextSource`
- Feature pack skill filtering → replaced by agent-level pack association

### Reuse

- `ExecutionCore` + `ReactiveEngine` — unchanged execution backbone
- `IntentAnalyzer` — still classifies Direct vs Reactive + budget
- `ContextEngine` — still handles token budgeting and history compression
- `ResponseValidator` + `CostTracker` — unchanged
- All tools — unchanged
- `SubagentManager` — evolved for auto-async

## Component 3: Unified Learning System

### Architecture

Merges `MemoryStore`, `ConversationEmbeddingStore`, and partial `LearningService` into one system with four layers:

```
LearningSystem
  ├── UserProfile          # explicit facts about the user
  ├── BehavioralPatterns   # observed patterns from interactions
  ├── AgentAdaptation      # per-agent preference tuning
  └── ConversationMemory   # semantic recall (existing embeddings, kept)
```

### Layer 1: UserProfile (replaces MemoryStore)

```sql
CREATE TABLE user_profile (
    id INTEGER PRIMARY KEY,
    category TEXT NOT NULL,  -- "projects" | "preferences" | "context" | "habits"
    key TEXT NOT NULL,
    value TEXT NOT NULL,     -- JSON
    source TEXT NOT NULL,    -- "user_explicit" | "system_inferred" | "agent_observed"
    confidence REAL NOT NULL DEFAULT 1.0,
    agent_name TEXT,
    last_confirmed DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP,
    created_at DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP,
    UNIQUE(category, key)
);
```

Population:
- User says "I'm working on Project X" → `{ category: "projects", key: "active_project", value: "Project X", source: "user_explicit", confidence: 1.0 }`
- System infers patterns → writes with `source: "system_inferred"`, lower confidence
- Evolved `MemoryTool` lets LLM read/write profile entries

### Layer 2: BehavioralPatterns

```sql
CREATE TABLE behavioral_patterns (
    id INTEGER PRIMARY KEY,
    pattern_type TEXT NOT NULL,  -- "time_of_day" | "day_of_week" | "agent_usage" | "tool_sequence"
    pattern_key TEXT NOT NULL,
    pattern_value TEXT NOT NULL, -- JSON
    sample_count INTEGER NOT NULL DEFAULT 0,
    last_updated DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP,
    UNIQUE(pattern_type, pattern_key)
);

CREATE TABLE interaction_log (
    id INTEGER PRIMARY KEY,
    timestamp DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP,
    agent_name TEXT NOT NULL,
    tool_names TEXT,            -- JSON array
    channel TEXT NOT NULL,
    duration_ms INTEGER
);
```

- Lightweight recorder (no LLM call) logs each interaction
- Hourly background analyzer computes patterns from raw logs
- Patterns with `sample_count >= 10` are considered reliable

### Layer 3: AgentAdaptation

```sql
CREATE TABLE agent_adaptations (
    id INTEGER PRIMARY KEY,
    agent_name TEXT NOT NULL,
    preference_key TEXT NOT NULL,
    preference_value TEXT NOT NULL, -- JSON
    source TEXT NOT NULL,          -- "user_feedback" | "satisfaction_signal" | "explicit_request"
    confidence REAL NOT NULL DEFAULT 0.5,
    last_updated DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP,
    UNIQUE(agent_name, preference_key)
);
```

- Emoji reactions feed satisfaction signals per-agent
- Consistent positive feedback on a behavior → stored as preference
- Per-agent preferences injected into that agent's context

### Layer 4: ConversationMemory (existing, kept)

`ConversationEmbeddingStore` + `ConversationMemoryRetriever` stay as-is. ANN search with time-decay scoring. Working well, no changes needed.

### Context Injection

Single `LearningContextSource` (replaces `MemorySource` + `ConfidenceSource`):

```
Priority 60

Output:
# About the User
- Working on: Project X (high confidence)
- Timezone: EST
- Prefers morning planning sessions

# Behavioral Patterns
- Usually creates tasks on Mondays
- Checks finance on Fridays

# Agent Preferences (for current agent)
- Prefers detailed task breakdowns
- Likes confirmation after task creation

# Relevant Past Context
[ANN-retrieved conversation snippets]
```

### Removals/Evolutions

- `MemoryStore` → absorbed into UserProfile layer
- `MemoryTool` → evolved to read/write UserProfile
- `ConfidenceSource` → absorbed into LearningContextSource
- `LearningService` background loop → kept, expanded for behavioral patterns
- `OutcomeRecorder` → finally wired to record per-tool outcomes in production
- `AdaptiveThresholds` → kept, now fed by actual outcome data

## Migration Strategy

Since breaking changes are acceptable (pre-production), this is a replace-not-migrate approach, phased to keep the system buildable and testable at each step.

### Phase 1: Agent Profile System (foundation)
- Create `agents/` directory with built-in agent definitions
- Build `AgentManager` (load, parse, match, filter)
- Migrate existing skills into agent-owned skills folders
- Wire `AgentManager` into existing pipeline alongside `SkillManager` (parallel run)

### Phase 2: Agent Runtime (core swap)
- Build `AgentRuntime` to replace `IntentPipeline`
- Refactor `ContextEngine` to build per-agent context
- Remove `ToolGroup` enum — agent profiles own tool filtering
- Remove `SkillManager`, `SkillSummarySource`, `SkillContentSource`
- `IntentAnalyzer` evolves: classifies Direct vs Reactive + selects agent

### Phase 3: Delegation (agent composition)
- Build `DelegationTool`
- Add delegation depth tracking to `ExecutionCore`
- Wire `can_delegate_to` from agent profiles
- Evolve `SubagentManager` for auto-async

### Phase 4: Unified Learning (intelligence layer)
- Build UserProfile storage + tool
- Build BehavioralPatterns recorder + analyzer
- Build AgentAdaptation feedback loop
- Build LearningContextSource
- Wire OutcomeRecorder into ExecutionCore
- Migrate existing memory data to new schema

## Crate-Level Impact

| Crate | Changes |
|---|---|
| `agent` | Major: new `AgentRuntime`, `AgentManager`, `DelegationTool`, evolved `LearningSystem`. Remove `IntentPipeline`, `SkillManager` |
| `tools-core` | Minor: add `DelegationHandler` trait |
| `tools` | Minor: add `DelegationTool`, evolve `MemoryTool`, evolve `SpawnTool` |
| `storage` | Medium: new tables (`user_profile`, `behavioral_patterns`, `agent_adaptations`, `interaction_log`). New repos |
| `context_engine` | Medium: refactor `build_system_prompt()` to accept agent profile. New `LearningContextSource` |
| `config` | Minor: new agent config section |
| `common`, `bus`, `providers`, `channels`, `session`, `scheduling`, `calendar`, `domain`, `cli` | No changes |
