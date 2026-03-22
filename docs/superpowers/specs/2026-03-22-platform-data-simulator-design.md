# Platform Data Simulator

**Date:** 2026-03-22
**Status:** Approved
**Phase:** Phase 1 (structural seeding + week-long behavioral simulation)

## Problem

The platform has 8 feature packages, 102 IPC commands, 150+ database tables, and 75+ domain events. Developing and testing features requires realistic, interconnected data across all domains — PARA hierarchy, finance, notes/knowledge, cognitive memory, productivity, coaching, and chat. Manually creating test data is tedious, inconsistent, and doesn't exercise cross-domain connections (entity mentions, knowledge graph links, temporal correlations) that make the system valuable.

## Solution

A TypeScript/Bun data simulator that drives realistic user activity through the dev server HTTP API (`POST /api/{command}` on `:3456`). All operations go through the real handler stack — triggering domain events, embeddings, atom extraction, FSRS scheduling, and coaching signals — providing full behavioral fidelity and natural performance profiling.

## Design

### 1. Architecture

```
tools/simulator/
├── run.ts                    # CLI entry point
├── client.ts                 # HTTP client wrapping POST /api/{cmd}
├── world.ts                  # Shared world definition
├── orchestrator.ts           # Interleaves modules across timeline
├── modules/
│   ├── types.ts              # SimulatorModule interface
│   ├── index.ts              # Module registry (single registration point)
│   ├── para.ts               # Areas, projects, objectives, key results
│   ├── tasks.ts              # Task lifecycle, decomposition, execution
│   ├── finance.ts            # Accounts, transactions, budgets, investments
│   ├── notes.ts              # Notebooks, notes, annotations, versions
│   ├── productivity.ts       # Focus sessions, time entries, goals
│   ├── knowledge.ts          # Flashcard reviews, atom acceptance, study sessions
│   ├── cognitive.ts          # Semantic facts, episodic memories, procedural rules
│   └── chat.ts               # Conversations, tool calls, agent interactions
└── utils/
    ├── dates.ts              # Date helpers for the simulated week
    └── random.ts             # Seeded random for reproducibility
```

### 2. Module Interface

```typescript
interface SimulatorModule {
    name: string;
    description: string;
    dependencies: string[];  // module names that must run first

    // Create structural entities this module owns
    seed(world: World, client: ApiClient): Promise<void>;

    // Simulate a day of activity
    simulateDay(world: World, client: ApiClient, day: DayContext): Promise<void>;
}

interface DayContext {
    date: Date;
    dayOfWeek: number;     // Simulator-internal ordinal: 0=Monday, 6=Sunday
                           // NOT from Date.getDay() (which returns 0=Sunday)
                           // Computed as: (dayIndex % 7) where week starts on Monday
    isWeekend: boolean;    // dayOfWeek >= 5
    dayIndex: number;       // 0-based index within the simulation run
}
```

Adding a new module: create the file implementing `SimulatorModule`, add it to `ALL_MODULES` in `modules/index.ts`. The orchestrator auto-resolves dependency order via topological sort.

### 3. World — Shared Cross-Module Context

The World object is the connective tissue. Created by the `para` module (structural foundation), then enriched by every module. All modules reference the same entity IDs:

```typescript
interface World {
    weekStart: Date;

    // PARA hierarchy (created by para module)
    areas: { personal: Ref, work: Ref, finance: Ref };
    projects: {
        apiRedesign: Ref,         // work: tasks, notes, meetings
        parisTrip: Ref,           // personal: notes, expenses
        fireGoal: Ref,            // finance: investments, goals
        languageLearning: Ref,    // learning: flashcards, practice
    };
    objectives: Record<string, Ref>;

    // Finance (created by finance module)
    accounts: { checking: Ref, savings: Ref, creditCard: Ref, brokerage: Ref };

    // Notes (created by notes module)
    notebooks: {
        workResearch: Ref,    // intent: research
        studyNotes: Ref,      // intent: study
        dailyJournal: Ref,    // intent: capture
    };

    // Accumulated across modules
    createdNotes: Map<string, Ref>;
    createdTasks: Map<string, Ref>;
}

interface Ref { id: string; title: string; }
```

Cross-domain connections emerge naturally:
- Finance transactions reference `world.projects.parisTrip`
- Notes contain `@task:{id}` entity mentions using `world.createdTasks`
- Notes create `[[wikilinks]]` to other `world.createdNotes`
- Focus sessions tie to `world.createdTasks` IDs
- Cognitive facts reference entities from all modules

### 4. Module Execution Order

```
1. para        → areas, projects, objectives, key results
2. tasks       → tasks, groups, subtasks, dependencies
3. finance     → accounts, transactions, budgets, investments
4. notes       → notebooks, notes with @mentions and [[wikilinks]]
5. productivity → focus sessions, time entries, goals
6. knowledge   → atom acceptance, flashcard reviews, study sessions
7. cognitive   → facts, episodic memories, rules
8. chat        → conversations with tool calls
```

Auto-resolved from `dependencies` field via topological sort with deduplication — shared dependencies (e.g., `para` needed by both `tasks` and `finance`) run exactly once. Running a subset (e.g., `--modules finance`) auto-includes transitive dependencies (para).

**Error handling:** each module's `simulateDay` is wrapped in try/catch. A failed module logs the error and continues to the next module — partial days are better than aborting the entire simulation. `seed()` failures are fatal (abort the run) since subsequent modules depend on structural entities.

### 5. Simulated Week

| Day | Theme | Key activities |
|-----|-------|---------------|
| Monday | Planning & deep work | Morning briefing, task review, project planning, focus sessions on API Redesign, meeting notes, expense logging |
| Tuesday | Research & learning | Research notes (Paris trip), flashcard review, language practice, finance review, investment check |
| Wednesday | Collaboration & review | Task decomposition, project retrospective notes, coaching check-in, OKR progress update |
| Thursday | Deep work & creation | Long focus session, create study notes, generate flashcards, log time entries, budget review |
| Friday | Wrap-up & reflection | Complete tasks, weekly finance summary, capture journal entry, review productivity goals |
| Saturday | Light personal | Personal project notes, casual expense logging, flashcard review, journal |
| Sunday | Rest & planning | Weekly reflection journal, review upcoming tasks, light study session |

### 6. Cross-Domain Scenario Examples

**Finance <-> Notes <-> Projects:**
- Create research note "Paris Trip Budget Analysis" in `workResearch` notebook with `@project:paris-trip` entity mention
- Log Paris trip expenses referencing the same project
- Finance review surfaces spending linked to the project

**Tasks <-> Productivity <-> Notes:**
- Start focus session on task "Implement auth layer" under API Redesign
- Create meeting note "Auth design discussion with Sarah" with `@task:{id}` mention
- Log 2h time entry against the same task

**Knowledge <-> Notes <-> Cognitive:**
- Create study note "OAuth 2.0 Patterns" in `studyNotes` notebook
- Accept knowledge atoms, review generated flashcards
- Cognitive module creates "user is learning OAuth" semantic fact

**Chat <-> Everything:**
- "What's the status of API Redesign?" triggers task list tool call
- "How much did I spend on the Paris trip?" triggers finance query

### 7. API Client

```typescript
class ApiClient {
    constructor(
        private baseUrl: string = "http://localhost:3456",
        private mode: "fast" | "selective" | "full" = "fast",
    ) {}

    // Returns T when the call executes, or undefined when skipped by mode.
    // Modules MUST guard skippable calls: use `client.maybe("cmd", params)` which
    // returns undefined for skipped calls, or `client.post("cmd", params)` which
    // throws if the command would be skipped (use for required calls).
    async maybe<T>(command: string, params?: Record<string, unknown>): Promise<T | undefined> {
        if (this.shouldSkip(command)) {
            console.log(`  skip ${command} (${this.mode} mode)`);
            return undefined;
        }
        return this.doPost<T>(command, params);
    }

    async post<T>(command: string, params?: Record<string, unknown>): Promise<T> {
        return this.doPost<T>(command, params);
    }

    private async doPost<T>(command: string, params?: Record<string, unknown>): Promise<T> {

        const start = performance.now();
        const res = await fetch(`${this.baseUrl}/api/${command}`, {
            method: "POST",
            headers: { "Content-Type": "application/json" },
            body: params ? JSON.stringify(params) : undefined,
        });
        const elapsed = performance.now() - start;

        if (elapsed > 1000) console.warn(`  SLOW: ${command} took ${elapsed.toFixed(0)}ms`);
        if (!res.ok) throw new Error(`${command} failed: ${res.status} ${await res.text()}`);
        return res.json();
    }
}
```

### 8. LLM Mode Control

| Mode | Skips | Use case |
|------|-------|----------|
| `fast` | `note_insight_review`, `task_decompose`, `task_get_suggestions`, `flashcard_generate`, `annotation_get_ai_suggestion`, `coaching_situation`, `cognitive_run_reflection` | Quick dev iteration |
| `selective` | `note_insight_review`, `coaching_situation` | Test AI features selectively |
| `full` | Nothing | Full integration testing |

The mode controls which API calls the simulator makes. Server-side background processing (atom extraction, embeddings) fires based on server config independently.

### 9. CLI

```bash
bun run tools/simulator/run.ts [options]

Options:
  --confirm              Required safety check
  --mode <fast|selective|full>  LLM mode (default: fast)
  --modules <list>       Comma-separated module names (default: all)
  --days <n>             Days to simulate (default: 7)
  --seed-only            Only structural seeding, skip behavioral simulation
  --base-url <url>       Dev server URL (default: http://localhost:3456)
  --dry-run              Print plan without calling APIs
```

### 10. Reset Flow

The dev server's `AppCore` is a long-lived singleton — it holds an open `SqlitePool` that won't reconnect after the DB file is deleted. Therefore, the simulator cannot delete the DB while the server is running and expect it to reinitialize. Instead:

1. Verify `--confirm` flag
2. Print instructions: "Stop the dev server, then press Enter to continue"
3. Wait for user confirmation
4. Delete `~/.klyntbot-dev/data.db` and `~/.klyntbot-dev/lancedb/`
5. Print: "Start the dev server (`cargo tauri dev`), then press Enter"
6. Wait for user confirmation
7. Verify server is reachable (`POST /api/app_info` with empty body — the dev server only accepts POST, not GET)
8. Run simulation

Alternative (future improvement): add a `POST /api/dev_reset_db` endpoint to the dev server that re-runs migrations on a cleared data dir, allowing fully automated reset without server restart. This is deferred — the manual restart flow is acceptable for a dev tool.

### 11. Progress Output

```
Wiping dev database...
Waiting for dev server to reinitialize...
Server ready (2.3s)

Seeding para... 3 areas, 4 projects, 6 objectives, 12 key results
Seeding tasks... 24 tasks, 8 groups, 3 decompositions
Seeding finance... 4 accounts, 3 budgets, 2 portfolios, 3 goals
...

Day 1: Monday 2026-03-16
  tasks: reviewed 8 tasks, completed 2, created 3
  finance: logged 4 transactions ($342 spent)
  notes: created "Auth Design Meeting Notes" with @task mention
  productivity: 2h focus session on "Implement auth layer"
  SLOW: note_create took 1,240ms
...

Simulation complete (7 days, 847 API calls, 4m 32s)
Performance: avg 312ms/call, slowest: note_create (1,240ms)
```

## Phase 2 (deferred)

- Probabilistic activity variations (randomized daily patterns)
- Multiple user personas with different engagement profiles
- Load testing mode (concurrent API calls, large data volumes)
- Automated performance regression tracking (compare run times across builds)
- Calendar event mocking (synthetic CalDAV data)
- Activity log ingestion simulation (OS/browser/IDE events)

## Non-goals

- Mocking LLM responses — the simulator uses real LLM or skips the call entirely
- Replacing unit/integration tests — the simulator is a dev tool, not a test suite
- Production data generation — this is dev-only, never used in production
- Simulating external integrations (Telegram, Discord, Slack, email)

## Testing Strategy

- Verify each module's `seed()` creates expected entity counts via list API calls
- Verify cross-module references: after notes module, check entity mentions reference valid task/project IDs
- Verify `--modules` flag correctly auto-includes transitive dependencies
- Verify `--mode fast` skips LLM-heavy endpoints
- Verify reset flow: DB deleted, server reinitializes, simulation runs cleanly on empty DB
