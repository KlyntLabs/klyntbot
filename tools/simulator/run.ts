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
