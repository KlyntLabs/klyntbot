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
