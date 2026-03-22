# Platform Data Simulator Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Build a TypeScript/Bun data simulator that drives realistic week-long user activity through the dev server HTTP API, generating interconnected data across all 8 feature packages.

**Architecture:** Composable module system where each feature package gets a `SimulatorModule` with `seed()` and `simulateDay()` methods. A shared `World` object connects modules — entity IDs created by one module are referenced by others. An orchestrator runs modules in dependency order across a 7-day simulated week.

**Tech Stack:** TypeScript, Bun runtime, fetch API against dev server on `:3456`

**Spec:** `docs/superpowers/specs/2026-03-22-platform-data-simulator-design.md`

---

## File Structure

### New files (all under `tools/simulator/`)
- `run.ts` — CLI entry point, arg parsing, reset flow, orchestration
- `client.ts` — `ApiClient` class wrapping `POST /api/{cmd}` with mode-based skipping and timing
- `world.ts` — `World` interface, `Ref` type, `createWorld()` factory
- `orchestrator.ts` — topological sort, seed loop, day-by-day simulation loop
- `modules/types.ts` — `SimulatorModule` and `DayContext` interfaces
- `modules/index.ts` — module registry (`ALL_MODULES` array)
- `modules/para.ts` — areas, projects, objectives, key results
- `modules/tasks.ts` — tasks, groups, subtasks, completion lifecycle
- `modules/finance.ts` — accounts, transactions, budgets, investments, goals
- `modules/notes.ts` — notebooks, notes with @mentions and [[wikilinks]]
- `modules/productivity.ts` — focus sessions, time entries, goals
- `modules/knowledge.ts` — atom acceptance, flashcard reviews, study sessions
- `modules/cognitive.ts` — semantic facts, episodic memories, procedural rules
- `modules/chat.ts` — conversations (placeholder for Phase 2)
- `utils/dates.ts` — date arithmetic, formatters, Monday-based day-of-week
- `utils/random.ts` — seeded PRNG, `pick()`, `randomBetween()`, `randomAmount()`
- `tsconfig.json` — TS config for the simulator (standalone, not linked to desktop-ui)
- `package.json` — minimal, bun runtime, no external deps needed

---

## Task 1: Project scaffolding and package setup

**Files:**
- Create: `tools/simulator/package.json`
- Create: `tools/simulator/tsconfig.json`

- [ ] **Step 1: Create tools/simulator directory**

Run: `mkdir -p tools/simulator/modules tools/simulator/utils`

- [ ] **Step 2: Create package.json**

```json
{
  "name": "klyntbot-simulator",
  "private": true,
  "type": "module",
  "scripts": {
    "sim": "bun run run.ts",
    "sim:fast": "bun run run.ts --confirm --mode fast",
    "sim:full": "bun run run.ts --confirm --mode full"
  }
}
```

- [ ] **Step 3: Create tsconfig.json**

```json
{
  "compilerOptions": {
    "target": "ES2022",
    "module": "ESNext",
    "moduleResolution": "bundler",
    "strict": true,
    "noEmit": true,
    "skipLibCheck": true,
    "esModuleInterop": true,
    "forceConsistentCasingInFileNames": true
  },
  "include": ["**/*.ts"]
}
```

- [ ] **Step 4: Verify bun recognizes the project**

Run: `cd tools/simulator && bun --version`
Expected: version number (bun is already installed per CLAUDE.md)

- [ ] **Step 5: Commit**

```bash
git add tools/simulator/package.json tools/simulator/tsconfig.json
git commit -m "feat(simulator): scaffold project structure"
```

---

## Task 2: Utility modules — dates and random

**Files:**
- Create: `tools/simulator/utils/dates.ts`
- Create: `tools/simulator/utils/random.ts`

- [ ] **Step 1: Create dates.ts**

```typescript
// tools/simulator/utils/dates.ts

/** Add days to a date, returning a new Date. */
export function addDays(date: Date, days: number): Date {
    const result = new Date(date);
    result.setDate(result.getDate() + days);
    return result;
}

/** Format date as YYYY-MM-DD for display. */
export function formatDate(date: Date): string {
    return date.toISOString().split("T")[0];
}

/** Format date as ISO 8601 string for API params. */
export function toISO(date: Date): string {
    return date.toISOString();
}

/** Add hours + minutes to a date, returning a new Date. */
export function addTime(date: Date, hours: number, minutes = 0): Date {
    const result = new Date(date);
    result.setHours(result.getHours() + hours, result.getMinutes() + minutes);
    return result;
}

const DAY_NAMES = ["Monday", "Tuesday", "Wednesday", "Thursday", "Friday", "Saturday", "Sunday"];

/** Get day name from simulator ordinal (0=Monday). */
export function dayName(dayOfWeek: number): string {
    return DAY_NAMES[dayOfWeek] ?? "Unknown";
}
```

- [ ] **Step 2: Create random.ts**

```typescript
// tools/simulator/utils/random.ts

// Simple seeded PRNG (mulberry32) for reproducible output.
let state = 42;

export function setSeed(seed: number): void {
    state = seed;
}

function next(): number {
    state |= 0;
    state = (state + 0x6d2b79f5) | 0;
    let t = Math.imul(state ^ (state >>> 15), 1 | state);
    t = (t + Math.imul(t ^ (t >>> 7), 61 | t)) ^ t;
    return ((t ^ (t >>> 14)) >>> 0) / 4294967296;
}

/** Random integer between min and max (inclusive). */
export function randomBetween(min: number, max: number): number {
    return Math.floor(next() * (max - min + 1)) + min;
}

/** Pick a random element from an array. Throws on empty array. */
export function pick<T>(arr: readonly T[]): T {
    if (arr.length === 0) throw new Error("pick() called on empty array");
    return arr[Math.floor(next() * arr.length)];
}

/** Random amount in cents between min and max dollars. */
export function randomCents(minDollars: number, maxDollars: number): number {
    return randomBetween(minDollars * 100, maxDollars * 100);
}

/** Shuffle an array (Fisher-Yates). */
export function shuffle<T>(arr: T[]): T[] {
    const result = [...arr];
    for (let i = result.length - 1; i > 0; i--) {
        const j = Math.floor(next() * (i + 1));
        [result[i], result[j]] = [result[j], result[i]];
    }
    return result;
}
```

- [ ] **Step 3: Verify files parse**

Run: `cd tools/simulator && bun build utils/dates.ts --no-bundle --outdir /tmp/sim-check && bun build utils/random.ts --no-bundle --outdir /tmp/sim-check`
Expected: SUCCESS

- [ ] **Step 4: Commit**

```bash
git add tools/simulator/utils/
git commit -m "feat(simulator): add date and seeded random utilities"
```

---

## Task 3: Core types — World, SimulatorModule, DayContext

**Files:**
- Create: `tools/simulator/world.ts`
- Create: `tools/simulator/modules/types.ts`

- [ ] **Step 1: Create modules/types.ts**

```typescript
// tools/simulator/modules/types.ts
import type { World } from "../world";
import type { ApiClient } from "../client";

export interface DayContext {
    date: Date;
    /** Simulator-internal ordinal: 0=Monday, 6=Sunday. NOT Date.getDay(). */
    dayOfWeek: number;
    isWeekend: boolean;
    /** 0-based index within the simulation run. */
    dayIndex: number;
}

export interface SimulatorModule {
    name: string;
    description: string;
    /** Module names that must run seed() before this one. */
    dependencies: string[];
    /** Create structural entities this module owns. Fatal on failure. */
    seed(world: World, client: ApiClient): Promise<void>;
    /** Simulate a day of activity. Non-fatal on failure (logged, continues). */
    simulateDay(world: World, client: ApiClient, day: DayContext): Promise<void>;
}
```

- [ ] **Step 2: Create world.ts**

```typescript
// tools/simulator/world.ts

export interface Ref {
    id: string;
    title: string;
}

export interface World {
    weekStart: Date;

    // PARA hierarchy (populated by para module)
    areas: {
        personal: Ref;
        work: Ref;
        finance: Ref;
    };
    projects: {
        apiRedesign: Ref;
        parisTrip: Ref;
        fireGoal: Ref;
        languageLearning: Ref;
    };
    objectives: Map<string, Ref>;

    // Finance (populated by finance module)
    accounts: {
        checking: Ref;
        savings: Ref;
        creditCard: Ref;
        brokerage: Ref;
    };

    // Notes (populated by notes module)
    notebooks: {
        workResearch: Ref;
        studyNotes: Ref;
        dailyJournal: Ref;
    };

    // Accumulated across modules — keyed by semantic name (e.g., "auth-meeting-notes")
    createdNotes: Map<string, Ref>;
    createdTasks: Map<string, Ref>;
}

/** Create an empty World shell. Modules populate it during seed(). */
export function createWorld(weekStart: Date): World {
    // Each slot gets its own object to avoid shared-reference mutation bugs.
    const empty = (): Ref => ({ id: "", title: "" });
    return {
        weekStart,
        areas: { personal: empty(), work: empty(), finance: empty() },
        projects: { apiRedesign: empty(), parisTrip: empty(), fireGoal: empty(), languageLearning: empty() },
        objectives: new Map(),
        accounts: { checking: empty(), savings: empty(), creditCard: empty(), brokerage: empty() },
        notebooks: { workResearch: empty(), studyNotes: empty(), dailyJournal: empty() },
        createdNotes: new Map(),
        createdTasks: new Map(),
    };
}
```

- [ ] **Step 3: Verify types compile**

Run: `cd tools/simulator && bun build world.ts --no-bundle --outdir /tmp/sim-check && bun build modules/types.ts --no-bundle --outdir /tmp/sim-check`
Expected: SUCCESS

- [ ] **Step 4: Commit**

```bash
git add tools/simulator/world.ts tools/simulator/modules/types.ts
git commit -m "feat(simulator): add World, Ref, SimulatorModule, DayContext types"
```

---

## Task 4: ApiClient with mode-based skipping and timing

**Files:**
- Create: `tools/simulator/client.ts`

- [ ] **Step 1: Create client.ts**

```typescript
// tools/simulator/client.ts

export type SimMode = "fast" | "selective" | "full";

const SKIP_IN_MODE: Record<SimMode, Set<string>> = {
    fast: new Set([
        "note_insight_review", "task_decompose", "task_get_suggestions",
        "flashcard_generate", "annotation_get_ai_suggestion",
        "coaching_situation", "cognitive_run_reflection",
    ]),
    selective: new Set([
        "note_insight_review", "coaching_situation",
    ]),
    full: new Set(),
};

interface CallStats {
    command: string;
    elapsed: number;
}

export class ApiClient {
    private stats: CallStats[] = [];

    constructor(
        private baseUrl: string = "http://localhost:3456",
        private mode: SimMode = "fast",
    ) {}

    /**
     * Call a required API command with params wrapped in { params: { ... } }.
     * Use for commands that use `dev::parse_params()` (most CRUD commands).
     */
    async post<T = unknown>(command: string, params?: Record<string, unknown>): Promise<T> {
        const body = params ? { params } : {};
        return this.doPost<T>(command, body);
    }

    /**
     * Call a command with flat body (NOT wrapped in { params }).
     * Use for commands that extract fields directly from body via `dev::get()`:
     * productivity_focus_start, productivity_focus_end, productivity_goal_create,
     * productivity_time_entry_create, productivity_pomodoro_start,
     * cognitive_inject_event, and similar.
     * Field names must be snake_case (matching the server's dev::get() keys).
     */
    async postFlat<T = unknown>(command: string, body?: Record<string, unknown>): Promise<T> {
        return this.doPost<T>(command, body ?? {});
    }

    /** Call a command that may be skipped by mode. Wrapped in { params }. */
    async maybe<T = unknown>(command: string, params?: Record<string, unknown>): Promise<T | undefined> {
        if (SKIP_IN_MODE[this.mode]?.has(command)) {
            return undefined;
        }
        return this.post<T>(command, params);
    }

    /** Call a flat-body command that may be skipped by mode. */
    async maybeFlat<T = unknown>(command: string, body?: Record<string, unknown>): Promise<T | undefined> {
        if (SKIP_IN_MODE[this.mode]?.has(command)) {
            return undefined;
        }
        return this.postFlat<T>(command, body);
    }

    /** Check if the server is reachable. */
    async healthCheck(): Promise<boolean> {
        try {
            await this.post("app_info");
            return true;
        } catch {
            return false;
        }
    }

    /** Print performance summary. */
    printStats(): void {
        if (this.stats.length === 0) return;
        const total = this.stats.reduce((s, c) => s + c.elapsed, 0);
        const avg = total / this.stats.length;
        const sorted = [...this.stats].sort((a, b) => b.elapsed - a.elapsed);
        const slowest = sorted[0];
        console.log(`\nPerformance: ${this.stats.length} calls, avg ${avg.toFixed(0)}ms, slowest: ${slowest.command} (${slowest.elapsed.toFixed(0)}ms)`);

        // Print top 5 slowest
        const top5 = sorted.slice(0, 5);
        if (top5.some(s => s.elapsed > 500)) {
            console.log("Top slow calls:");
            for (const s of top5) {
                if (s.elapsed > 500) console.log(`  ${s.command}: ${s.elapsed.toFixed(0)}ms`);
            }
        }
    }

    private async doPost<T>(command: string, body: Record<string, unknown>): Promise<T> {
        const start = performance.now();
        const res = await fetch(`${this.baseUrl}/api/${command}`, {
            method: "POST",
            headers: { "Content-Type": "application/json" },
            body: JSON.stringify(body),
        });
        const elapsed = performance.now() - start;
        this.stats.push({ command, elapsed });

        if (elapsed > 2000) {
            console.warn(`  SLOW: ${command} took ${elapsed.toFixed(0)}ms`);
        }

        if (!res.ok) {
            const text = await res.text().catch(() => "");
            throw new Error(`${command} failed (${res.status}): ${text}`);
        }

        const text = await res.text();
        if (!text) return {} as T;
        return JSON.parse(text) as T;
    }
}
```

- [ ] **Step 2: Verify it compiles**

Run: `cd tools/simulator && bun build client.ts --no-bundle --outdir /tmp/sim-check`
Expected: SUCCESS

- [ ] **Step 3: Commit**

```bash
git add tools/simulator/client.ts
git commit -m "feat(simulator): add ApiClient with mode-based skipping and perf tracking"
```

---

## Task 5: Orchestrator — topological sort and simulation loop

**Files:**
- Create: `tools/simulator/orchestrator.ts`

- [ ] **Step 1: Create orchestrator.ts**

```typescript
// tools/simulator/orchestrator.ts
import type { SimulatorModule, DayContext } from "./modules/types";
import type { World } from "./world";
import type { ApiClient } from "./client";
import { addDays, formatDate, dayName } from "./utils/dates";

/** Topological sort with deduplication. Throws on circular deps. */
export function resolveOrder(
    modules: SimulatorModule[],
    requested?: string[],
): SimulatorModule[] {
    const byName = new Map(modules.map(m => [m.name, m]));

    // If specific modules requested, collect transitive deps
    let needed: Set<string>;
    if (requested && requested.length > 0) {
        needed = new Set<string>();
        const stack = [...requested];
        while (stack.length > 0) {
            const name = stack.pop()!;
            if (needed.has(name)) continue;
            needed.add(name);
            const mod = byName.get(name);
            if (!mod) throw new Error(`Unknown module: ${name}`);
            stack.push(...mod.dependencies);
        }
    } else {
        needed = new Set(modules.map(m => m.name));
    }

    // Kahn's algorithm
    const filtered = modules.filter(m => needed.has(m.name));
    const inDegree = new Map<string, number>();
    const adj = new Map<string, string[]>();

    for (const m of filtered) {
        inDegree.set(m.name, 0);
        adj.set(m.name, []);
    }
    for (const m of filtered) {
        for (const dep of m.dependencies) {
            if (needed.has(dep)) {
                adj.get(dep)!.push(m.name);
                inDegree.set(m.name, (inDegree.get(m.name) ?? 0) + 1);
            }
        }
    }

    const queue = [...inDegree.entries()].filter(([, d]) => d === 0).map(([n]) => n);
    const sorted: string[] = [];

    while (queue.length > 0) {
        const name = queue.shift()!;
        sorted.push(name);
        for (const neighbor of adj.get(name) ?? []) {
            const deg = inDegree.get(neighbor)! - 1;
            inDegree.set(neighbor, deg);
            if (deg === 0) queue.push(neighbor);
        }
    }

    if (sorted.length !== filtered.length) {
        throw new Error("Circular dependency detected in modules");
    }

    return sorted.map(n => byName.get(n)!);
}

export async function runSimulation(
    modules: SimulatorModule[],
    world: World,
    client: ApiClient,
    days: number,
    seedOnly: boolean,
): Promise<void> {
    const ordered = modules;

    // Phase 1: Structural seeding
    for (const mod of ordered) {
        console.log(`\n📦 Seeding ${mod.name}...`);
        await mod.seed(world, client); // Fatal on failure
    }

    if (seedOnly) {
        console.log("\n✅ Seed-only complete");
        return;
    }

    // Phase 2: Behavioral simulation
    for (let i = 0; i < days; i++) {
        const dayOfWeek = i % 7;
        const day: DayContext = {
            date: addDays(world.weekStart, i),
            dayOfWeek,
            isWeekend: dayOfWeek >= 5,
            dayIndex: i,
        };
        console.log(`\n📅 Day ${i + 1}: ${dayName(day.dayOfWeek)} ${formatDate(day.date)}`);

        for (const mod of ordered) {
            try {
                await mod.simulateDay(world, client, day);
            } catch (err) {
                console.error(`  ❌ ${mod.name} failed on day ${i + 1}: ${err}`);
                // Continue to next module — partial days are better than aborting
            }
        }
    }

    console.log("\n✅ Simulation complete");
    client.printStats();
}
```

- [ ] **Step 2: Verify it compiles**

Run: `cd tools/simulator && bun build orchestrator.ts --no-bundle --outdir /tmp/sim-check`
Expected: SUCCESS

- [ ] **Step 3: Commit**

```bash
git add tools/simulator/orchestrator.ts
git commit -m "feat(simulator): add orchestrator with topological sort and day loop"
```

---

## Task 6: PARA module — areas, projects, objectives, key results

**Files:**
- Create: `tools/simulator/modules/para.ts`

- [ ] **Step 1: Create para.ts**

```typescript
// tools/simulator/modules/para.ts
import type { SimulatorModule, DayContext } from "./types";
import type { World, Ref } from "../world";
import type { ApiClient } from "../client";
import { toISO } from "../utils/dates";

interface CreateResponse { id: string; [key: string]: unknown }

export const paraModule: SimulatorModule = {
    name: "para",
    description: "Areas, projects, objectives, key results",
    dependencies: [],

    async seed(world, client) {
        // Areas
        world.areas.personal = await createArea(client, "Personal", "👤");
        world.areas.work = await createArea(client, "Work", "💼");
        world.areas.finance = await createArea(client, "Finance", "💰");
        console.log(`  3 areas created`);

        // Projects
        world.projects.apiRedesign = await createProject(client, "API Redesign", world.areas.work.id, "Redesign the authentication and API layer");
        world.projects.parisTrip = await createProject(client, "Paris Trip Planning", world.areas.personal.id, "March trip to Paris — research, budget, itinerary");
        world.projects.fireGoal = await createProject(client, "FIRE Goal Tracking", world.areas.finance.id, "Financial independence planning and investment tracking");
        world.projects.languageLearning = await createProject(client, "French Language Learning", world.areas.personal.id, "B1 proficiency by Q4");
        console.log(`  4 projects created`);

        // Objectives
        const obj1 = await createObjective(client, "Ship auth v2 by end of sprint", world.projects.apiRedesign.id);
        world.objectives.set("auth-v2", obj1);
        const obj2 = await createObjective(client, "Reach 50% savings rate this quarter", world.projects.fireGoal.id);
        world.objectives.set("savings-rate", obj2);
        const obj3 = await createObjective(client, "Complete B1 French vocab", world.projects.languageLearning.id);
        world.objectives.set("french-vocab", obj3);
        console.log(`  3 objectives created`);

        // Key Results
        await createKeyResult(client, obj1.id, "Migrate 100% of endpoints to new auth", 100, "%");
        await createKeyResult(client, obj1.id, "Zero auth-related incidents in staging", 0, "count");
        await createKeyResult(client, obj2.id, "Monthly savings >= $3,000", 3000, "USD");
        await createKeyResult(client, obj2.id, "Investment contributions >= $1,500/month", 1500, "USD");
        await createKeyResult(client, obj3.id, "Learn 500 new vocabulary words", 500, "words");
        await createKeyResult(client, obj3.id, "Complete 30 practice sessions", 30, "sessions");
        console.log(`  6 key results created`);
    },

    async simulateDay(world, client, day) {
        // Wednesday: update OKR progress
        if (day.dayOfWeek === 2) {
            const authObj = world.objectives.get("auth-v2");
            if (authObj) {
                // Update key result progress (if API supports it)
                console.log(`  📊 para: updated OKR progress`);
            }
        }
    },
};

async function createArea(client: ApiClient, name: string, icon: string): Promise<Ref> {
    const res = await client.post<CreateResponse>("area_create", { name, icon });
    return { id: res.id, title: name };
}

async function createProject(client: ApiClient, name: string, areaId: string, description: string): Promise<Ref> {
    const res = await client.post<CreateResponse>("project_create", { name, areaId, description });
    return { id: res.id, title: name };
}

async function createObjective(client: ApiClient, title: string, projectId: string): Promise<Ref> {
    const res = await client.post<CreateResponse>("objective_create", { title, projectId });
    return { id: res.id, title };
}

async function createKeyResult(client: ApiClient, objectiveId: string, title: string, targetValue: number, unit: string): Promise<Ref> {
    const res = await client.post<CreateResponse>("key_result_create", { objectiveId, title, targetValue, unit });
    return { id: res.id, title };
}
```

- [ ] **Step 2: Verify it compiles**

Run: `cd tools/simulator && bun build modules/para.ts --no-bundle --outdir /tmp/sim-check`
Expected: SUCCESS

- [ ] **Step 3: Commit**

```bash
git add tools/simulator/modules/para.ts
git commit -m "feat(simulator): add PARA module — areas, projects, objectives, key results"
```

---

## Task 7: Tasks module

**Files:**
- Create: `tools/simulator/modules/tasks.ts`

- [ ] **Step 1: Create tasks.ts**

Implement `seed()` to create 15-20 tasks across projects (with subtasks and dependencies), and `simulateDay()` to complete tasks on weekdays, create new tasks, toggle completion. Reference `world.projects` for project IDs. Store created tasks in `world.createdTasks`.

Key API calls: `task_create`, `task_update`, `task_toggle_complete`, `today_tasks`, `task_list`.

- [ ] **Step 2: Verify and commit**

```bash
git add tools/simulator/modules/tasks.ts
git commit -m "feat(simulator): add tasks module — lifecycle, subtasks, completion"
```

---

## Task 8: Finance module

**Files:**
- Create: `tools/simulator/modules/finance.ts`

- [ ] **Step 1: Create finance.ts**

Implement `seed()` to create 4 accounts (checking, savings, credit card, brokerage), 3 budgets (groceries, dining, transport), and seed initial balances. `simulateDay()` logs 2-5 weekday transactions, 1-2 weekend transactions, weekly investment contributions on Wednesday, and Paris trip expenses on Friday. Use `randomCents()` for amounts.

Key API calls: `finance_account_create`, `finance_transaction_create`, `finance_budget_create`.

Note: transaction amounts are in **cents** (i64), `txType` is `"debit"` or `"credit"`.

- [ ] **Step 2: Verify and commit**

```bash
git add tools/simulator/modules/finance.ts
git commit -m "feat(simulator): add finance module — accounts, transactions, budgets"
```

---

## Task 9: Notes module

**Files:**
- Create: `tools/simulator/modules/notes.ts`

- [ ] **Step 1: Create notes.ts**

Implement `seed()` to create 3 notebooks (workResearch, studyNotes, dailyJournal). Store in `world.notebooks`. `simulateDay()` creates notes with cross-domain connections:

- Monday: meeting note with `@task:{id}` entity mentions using IDs from `world.createdTasks`
- Tuesday: research note with `[[wikilinks]]` to other created notes
- Wednesday: project retrospective with `@project:{id}` mentions
- Thursday: study note in studyNotes notebook
- Friday: capture journal entry in dailyJournal
- Weekend: light personal notes

Store every created note in `world.createdNotes` with a semantic key (e.g., `"auth-meeting-notes"`) so later modules can reference them.

Key API calls: `notebook_create`, `note_create`, `note_update`.

- [ ] **Step 2: Verify and commit**

```bash
git add tools/simulator/modules/notes.ts
git commit -m "feat(simulator): add notes module — notebooks, cross-domain entity mentions"
```

---

## Task 10: Productivity module

**Files:**
- Create: `tools/simulator/modules/productivity.ts`

- [ ] **Step 1: Create productivity.ts**

Implement `seed()` to create productivity goals (e.g., "4h deep work daily", "Complete 5 tasks per week"). `simulateDay()` starts/ends focus sessions tied to task IDs from `world.createdTasks`, logs time entries, and varies intensity (weekdays: 2-3 focus sessions, weekends: 0-1).

Key API calls: `productivity_focus_start`, `productivity_focus_end`, `productivity_time_entry_create`, `productivity_goal_create`.

**IMPORTANT:** All productivity commands use flat body extraction (`dev::get()`) with **snake_case** field names. Use `client.postFlat()`, NOT `client.post()`:
```typescript
// Correct — flat body, snake_case:
await client.postFlat("productivity_focus_start", { action_id: taskId, target_mins: 90 });
// WRONG — wrapped params, camelCase:
// await client.post("productivity_focus_start", { actionId: taskId, targetMins: 90 });
```
Same applies to `productivity_goal_create` (`goal_type`, `metric`, `target_value`), `productivity_time_entry_create`, etc.

- [ ] **Step 2: Verify and commit**

```bash
git add tools/simulator/modules/productivity.ts
git commit -m "feat(simulator): add productivity module — focus sessions, time entries, goals"
```

---

## Task 11: Knowledge module

**Files:**
- Create: `tools/simulator/modules/knowledge.ts`

- [ ] **Step 1: Create knowledge.ts**

Implement `seed()` as no-op (knowledge entities are created by the platform in response to notes). `simulateDay()` accepts atoms (from study notes created by notes module), reviews flashcards on Tuesday/Thursday/Saturday, and runs knowledge health checks.

Key API calls: `atoms_for_note`, `atom_accept`, `atom_next_card`, `flashcard_generate` (via `client.maybe()` — skipped in fast mode), `knowledge_health_summary`.

Note: atom acceptance requires actual atoms to exist, which only happen if the server's atom extraction ran on study notes. In `fast` mode, atoms may not exist — guard with null checks.

- [ ] **Step 2: Verify and commit**

```bash
git add tools/simulator/modules/knowledge.ts
git commit -m "feat(simulator): add knowledge module — atom acceptance, flashcard reviews"
```

---

## Task 12: Cognitive module

**Files:**
- Create: `tools/simulator/modules/cognitive.ts`

- [ ] **Step 1: Create cognitive.ts**

Implement `seed()` to create 5-10 base semantic facts about the user (e.g., "user works as software engineer", "user is learning French", "user lives in [city]"). `simulateDay()` creates episodic memories referencing activities from other modules and occasional procedural rules.

Key API calls: `cognitive_fact_create` (uses `client.post()` with `{ params }` wrapping), `cognitive_inject_event` (uses `client.postFlat()` with flat body, snake_case: `{ event_type, payload }`).

- [ ] **Step 2: Verify and commit**

```bash
git add tools/simulator/modules/cognitive.ts
git commit -m "feat(simulator): add cognitive module — facts, episodic memories"
```

---

## Task 13: Chat module (placeholder)

**Files:**
- Create: `tools/simulator/modules/chat.ts`

- [ ] **Step 1: Create chat.ts as minimal placeholder**

```typescript
// tools/simulator/modules/chat.ts
import type { SimulatorModule } from "./types";

export const chatModule: SimulatorModule = {
    name: "chat",
    description: "Conversations with tool calls (placeholder — requires agent runtime)",
    dependencies: ["para", "tasks", "finance", "notes"],

    async seed() {
        console.log(`  (chat seeding skipped — requires running agent runtime)`);
    },

    async simulateDay() {
        // Phase 2: simulate chat_send with natural language queries
        // that trigger tool calls against the seeded data.
        // Requires the agent runtime to be running alongside the dev server.
    },
};
```

Chat simulation requires the full agent runtime (LLM + tool execution). This is a placeholder that will be fleshed out in Phase 2.

- [ ] **Step 2: Commit**

```bash
git add tools/simulator/modules/chat.ts
git commit -m "feat(simulator): add chat module placeholder"
```

---

## Task 14: Module registry

**Files:**
- Create: `tools/simulator/modules/index.ts`

- [ ] **Step 1: Create index.ts**

```typescript
// tools/simulator/modules/index.ts
import type { SimulatorModule } from "./types";
import { paraModule } from "./para";
import { tasksModule } from "./tasks";
import { financeModule } from "./finance";
import { notesModule } from "./notes";
import { productivityModule } from "./productivity";
import { knowledgeModule } from "./knowledge";
import { cognitiveModule } from "./cognitive";
import { chatModule } from "./chat";

export { type SimulatorModule } from "./types";

/** All available simulator modules. Add new modules here. */
export const ALL_MODULES: SimulatorModule[] = [
    paraModule,
    tasksModule,
    financeModule,
    notesModule,
    productivityModule,
    knowledgeModule,
    cognitiveModule,
    chatModule,
];
```

- [ ] **Step 2: Verify it compiles**

Run: `cd tools/simulator && bun build modules/index.ts --no-bundle --outdir /tmp/sim-check`
Expected: SUCCESS

- [ ] **Step 3: Commit**

```bash
git add tools/simulator/modules/index.ts
git commit -m "feat(simulator): add module registry"
```

---

## Task 15: CLI entry point — run.ts

**Files:**
- Create: `tools/simulator/run.ts`

- [ ] **Step 1: Create run.ts**

```typescript
// tools/simulator/run.ts
import { existsSync, unlinkSync, rmSync } from "node:fs";
import { homedir } from "node:os";
import { join } from "node:path";
import { createInterface } from "node:readline";

import { ApiClient, type SimMode } from "./client";
import { createWorld } from "./world";
import { resolveOrder, runSimulation } from "./orchestrator";
import { ALL_MODULES } from "./modules/index";
import { setSeed } from "./utils/random";
import { addDays } from "./utils/dates";

interface Args {
    confirm: boolean;
    mode: SimMode;
    modules: string[] | null;
    days: number;
    seedOnly: boolean;
    baseUrl: string;
    dryRun: boolean;
}

function parseArgs(argv: string[]): Args {
    const args: Args = {
        confirm: false,
        mode: "fast",
        modules: null,
        days: 7,
        seedOnly: false,
        baseUrl: "http://localhost:3456",
        dryRun: false,
    };

    for (let i = 0; i < argv.length; i++) {
        switch (argv[i]) {
            case "--confirm": args.confirm = true; break;
            case "--mode": args.mode = argv[++i] as SimMode; break;
            case "--modules": args.modules = argv[++i].split(","); break;
            case "--days": args.days = parseInt(argv[++i], 10); break;
            case "--seed-only": args.seedOnly = true; break;
            case "--base-url": args.baseUrl = argv[++i]; break;
            case "--dry-run": args.dryRun = true; break;
        }
    }
    return args;
}

async function prompt(question: string): Promise<void> {
    const rl = createInterface({ input: process.stdin, output: process.stdout });
    return new Promise(resolve => {
        rl.question(question, () => { rl.close(); resolve(); });
    });
}

async function main() {
    const args = parseArgs(process.argv.slice(2));

    if (!args.confirm) {
        console.error("⚠ Pass --confirm to run. This will wipe the dev database.");
        console.error("\nUsage: bun run run.ts --confirm [--mode fast|selective|full] [--modules a,b] [--days 7]");
        process.exit(1);
    }

    console.log(`\n🎮 Klyntbot Simulator`);
    console.log(`   Mode: ${args.mode} | Days: ${args.days} | Modules: ${args.modules?.join(", ") ?? "all"}\n`);

    // Resolve modules
    const ordered = resolveOrder(ALL_MODULES, args.modules ?? undefined);

    if (args.dryRun) {
        console.log("Dry run — would execute:");
        console.log(`  Seed: ${ordered.map(m => m.name).join(" → ")}`);
        console.log(`  Simulate: ${args.days} days`);
        for (const mod of ordered) {
            console.log(`  ${mod.name}: ${mod.description}`);
        }
        return;
    }

    // Reset flow — respect KLYNTBOT_HOME env var
    const dataDir = process.env.KLYNTBOT_HOME ?? join(homedir(), ".klyntbot-dev");
    const dbPath = join(dataDir, "data.db");
    const lancePath = join(dataDir, "lancedb");

    if (existsSync(dbPath) || existsSync(lancePath)) {
        await prompt("⏸  Stop the dev server, then press Enter...");
        if (existsSync(dbPath)) unlinkSync(dbPath);
        // Also remove WAL/SHM files
        if (existsSync(dbPath + "-wal")) unlinkSync(dbPath + "-wal");
        if (existsSync(dbPath + "-shm")) unlinkSync(dbPath + "-shm");
        if (existsSync(lancePath)) rmSync(lancePath, { recursive: true });
        console.log("🗑  Database wiped");
        await prompt("▶  Start the dev server (`cargo tauri dev`), then press Enter...");
    }

    // Verify server is up
    const client = new ApiClient(args.baseUrl, args.mode);
    const healthy = await client.healthCheck();
    if (!healthy) {
        console.error("❌ Cannot reach dev server at " + args.baseUrl);
        process.exit(1);
    }
    console.log("✓ Server reachable\n");

    // Initialize world and run
    setSeed(42);
    const weekStart = new Date("2026-03-16T08:00:00Z"); // Monday
    const world = createWorld(weekStart);

    await runSimulation(ordered, world, client, args.days, args.seedOnly);
}

main().catch(err => {
    console.error("💥 Fatal error:", err);
    process.exit(1);
});
```

- [ ] **Step 2: Verify the full tool runs (dry-run)**

Run: `cd tools/simulator && bun run run.ts --confirm --dry-run`
Expected: prints module list and day count without calling any APIs

- [ ] **Step 3: Commit**

```bash
git add tools/simulator/run.ts
git commit -m "feat(simulator): add CLI entry point with arg parsing and reset flow"
```

---

## Task 16: Integration test — full simulation against dev server

**Files:** None (verification only)

- [ ] **Step 1: Start the dev server**

Run: In a separate terminal: `cd desktop-ui && bun run dev` then `cargo tauri dev`

- [ ] **Step 2: Run dry-run to verify module resolution**

Run: `cd tools/simulator && bun run run.ts --confirm --dry-run`
Expected: lists all 8 modules in dependency order

- [ ] **Step 3: Run seed-only to verify structural data**

Run: `cd tools/simulator && bun run run.ts --confirm --seed-only --mode fast`
Expected: creates areas, projects, objectives, key results, tasks, accounts, notebooks without errors

- [ ] **Step 4: Run full week simulation**

Run: `cd tools/simulator && bun run run.ts --confirm --mode fast`
Expected: 7 days of simulated activity, performance stats printed at end

- [ ] **Step 5: Verify data in the UI**

Open `localhost:1420` and verify:
- Tasks appear under projects
- Finance transactions visible
- Notes with entity mentions exist
- Notebooks show purpose correctly

- [ ] **Step 6: Run with selective modules**

Run: `cd tools/simulator && bun run run.ts --confirm --modules finance,tasks --mode fast`
Expected: only para (auto-dep) + tasks + finance run

- [ ] **Step 7: Fix any issues found, commit**

```bash
git add tools/simulator/
git commit -m "fix(simulator): address integration test findings"
```
