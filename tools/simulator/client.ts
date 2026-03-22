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
