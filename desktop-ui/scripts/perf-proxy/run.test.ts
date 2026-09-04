// @vitest-environment node
import { EventEmitter } from "node:events";
import {
  existsSync,
  mkdirSync,
  mkdtempSync,
  readFileSync,
  rmSync,
  writeFileSync,
} from "node:fs";
import os from "node:os";
import { join } from "node:path";
import { PassThrough } from "node:stream";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import type { Baseline, OsFacts, RowEnvironment, RowResult } from "./contract.ts";
import {
  MEASUREMENT_CONTRACT_VERSION,
  SCHEMA_VERSION,
} from "./contract.ts";
import { describeEnvironment } from "./env.ts";
import { EXPECTED_ROWS } from "../../tests/perf-proxy/support/rowfile.ts";
import { runProxy } from "./run.ts";

const ADVISORY =
  "This result is an advisory WebKit proxy, not native rendering evidence.";

function rowEnvironment(overrides: Partial<RowEnvironment> = {}): RowEnvironment {
  return {
    webkitVersion: "WebKit/605.1.15",
    headless: true,
    viewport: { width: 1280, height: 800 },
    dpr: 1,
    userAgent: "Mozilla/5.0 (perf-proxy)",
    ...overrides,
  };
}

function rowResult(
  name: string,
  opts: {
    environment?: RowEnvironment;
    rafP95?: number;
    screenshotP50?: number;
  } = {},
): RowResult {
  const rafP95 = opts.rafP95 ?? 11;
  const screenshotP50 = opts.screenshotP50 ?? 40;
  const [scenario, theme] = name.split("×");
  return {
    row: name,
    scenario: scenario ?? name,
    theme: theme ?? "light",
    n: name.includes("200") ? 200 : 20,
    renderPath: name.startsWith("idle-20×") ? "plain" : "virtualized",
    raf: {
      sampleCount: 300,
      p50Ms: rafP95 - 1,
      p95Ms: rafP95,
      maxMs: rafP95 + 1,
    },
    screenshot: {
      captureCount: 10,
      p50Ms: screenshotP50,
      options: { type: "png" },
    },
    environment: opts.environment ?? rowEnvironment(),
    durationMs: 1000,
  };
}

function realOsFacts(): OsFacts {
  return {
    platform: os.platform(),
    arch: os.arch(),
    release: os.release(),
    hostname: os.hostname(),
    cpus: os.cpus().map((cpu) => ({ model: cpu.model })),
  };
}

function writeRows(dir: string, rows: RowResult[]): void {
  mkdirSync(dir, { recursive: true });
  for (const row of rows) {
    writeFileSync(join(dir, `${row.row}.json`), JSON.stringify(row), "utf8");
  }
}

function baselineFor(
  identityKey: string,
  rows: RowResult[],
  keyed: ReturnType<typeof describeEnvironment>["keyed"],
  diagnostic: ReturnType<typeof describeEnvironment>["diagnostic"],
  metricOverride?: { row: string; rafP95Median: number },
): Baseline {
  const mapped: Baseline["rows"] = {};
  for (const row of rows) {
    const rafMedian =
      metricOverride?.row === row.row
        ? metricOverride.rafP95Median
        : row.raf.p95Ms;
    mapped[row.row] = {
      raf: { p95: { median: rafMedian, spread: 1, margin: 3 } },
      screenshot: {
        p50: { median: row.screenshot.p50Ms, spread: 2, margin: 6 },
      },
    };
  }
  return {
    identity: {
      schemaVersion: SCHEMA_VERSION,
      measurementContractVersion: MEASUREMENT_CONTRACT_VERSION,
      environmentKey: identityKey,
    },
    recordedAt: "2026-09-04T00:00:00Z",
    sourceRevision: "abc",
    repetitions: 5,
    rows: mapped,
    environment: { keyed, diagnostic },
  };
}

type FakeChild = EventEmitter & {
  stdout: PassThrough;
  stderr: PassThrough;
};

function makeFakeSpawn(opts: {
  exitCode: number;
  stdout?: string;
  stderr?: string;
  onSpawn: () => void;
}): typeof import("node:child_process").spawn {
  return ((() => {
    const child = new EventEmitter() as FakeChild;
    child.stdout = new PassThrough();
    child.stderr = new PassThrough();
    queueMicrotask(() => {
      opts.onSpawn();
      if (opts.stdout) {
        child.stdout.write(opts.stdout);
      }
      if (opts.stderr) {
        child.stderr.write(opts.stderr);
      }
      child.stdout.end();
      child.stderr.end();
      child.emit("close", opts.exitCode);
    });
    return child;
  }) as unknown) as typeof import("node:child_process").spawn;
}

type CaseName =
  | "HEALTHY"
  | "DEGRADED"
  | "NO_BASELINE"
  | "CHILD_FAILED"
  | "PORT_BUSY"
  | "ROW_MISSING"
  | "IDENTITY"
  | "WRITE_FAILED";

describe("runProxy consistency table", () => {
  let tempRoot: string;
  let rowsDir: string;
  let baselinesDir: string;
  let latestPath: string;
  let summaryPath: string;
  let printed: string[];

  beforeEach(() => {
    tempRoot = mkdtempSync(join(os.tmpdir(), "perf-run-"));
    rowsDir = join(tempRoot, "rows");
    baselinesDir = join(tempRoot, "baselines");
    latestPath = join(tempRoot, "latest.json");
    summaryPath = join(tempRoot, "step-summary.md");
    mkdirSync(rowsDir, { recursive: true });
    mkdirSync(baselinesDir, { recursive: true });
    printed = [];
    vi.spyOn(console, "log").mockImplementation((...args: unknown[]) => {
      printed.push(args.map(String).join(" "));
    });
    vi.spyOn(console, "error").mockImplementation(() => {});
    vi.spyOn(process.stdout, "write").mockImplementation(() => true);
    vi.spyOn(process.stderr, "write").mockImplementation(() => true);
  });

  afterEach(() => {
    vi.restoreAllMocks();
    rmSync(tempRoot, { recursive: true, force: true });
  });

  async function runCase(name: CaseName): Promise<{
    result: Awaited<ReturnType<typeof runProxy>>;
    summary: string;
    latest: { outcome: string; subcode?: string } | null;
    printedText: string;
  }> {
    const env: NodeJS.ProcessEnv = {
      GITHUB_STEP_SUMMARY: summaryPath,
    };
    const allRows = EXPECTED_ROWS.map((rowName) => rowResult(rowName));
    const desc = describeEnvironment({
      env,
      os: realOsFacts(),
      rows: allRows,
    });

    writeFileSync(join(rowsDir, "stale.json"), '{"stale":true}', "utf8");
    writeFileSync(latestPath, '{"outcome":"STALE"}', "utf8");

    let caseLatestPath = latestPath;
    if (name === "WRITE_FAILED") {
      const blocker = join(tempRoot, "not-a-dir");
      writeFileSync(blocker, "x", "utf8");
      caseLatestPath = join(blocker, "latest.json");
    }

    let spawnSawClean = false;

    const spawn = makeFakeSpawn({
      exitCode:
        name === "CHILD_FAILED" || name === "PORT_BUSY" ? 1 : 0,
      stderr:
        name === "PORT_BUSY"
          ? "Error: http://localhost:1420 is already used\n"
          : undefined,
      onSpawn: () => {
        const rowsClean =
          !existsSync(rowsDir) || !existsSync(join(rowsDir, "stale.json"));
        const latestClean =
          name === "WRITE_FAILED"
            ? !existsSync(caseLatestPath)
            : !existsSync(latestPath);
        spawnSawClean = rowsClean && latestClean;
        if (name === "CHILD_FAILED" || name === "PORT_BUSY") {
          return;
        }
        if (name === "ROW_MISSING") {
          writeRows(rowsDir, allRows.slice(0, 5));
          return;
        }
        if (name === "IDENTITY") {
          const mismatched = allRows.map((row, index) =>
            index === 0
              ? rowResult(row.row, {
                  environment: rowEnvironment({ dpr: 2 }),
                })
              : row,
          );
          writeRows(rowsDir, mismatched);
          return;
        }
        writeRows(rowsDir, allRows);
      },
    });

    if (name === "HEALTHY" || name === "DEGRADED" || name === "WRITE_FAILED") {
      const baseline = baselineFor(
        desc.key,
        allRows,
        desc.keyed,
        desc.diagnostic,
        name === "DEGRADED"
          ? { row: "idle-20×light", rafP95Median: 5 }
          : undefined,
      );
      writeFileSync(
        join(baselinesDir, `${desc.key}.json`),
        JSON.stringify(baseline),
        "utf8",
      );
    }

    const result = await runProxy({
      spawn,
      rowsDir,
      baselinesDir,
      latestPath: caseLatestPath,
      env,
    });

    expect(spawnSawClean).toBe(true);

    const printedText = printed.join("\n");
    const summary = existsSync(summaryPath)
      ? readFileSync(summaryPath, "utf8")
      : "";

    let latest: { outcome: string; subcode?: string } | null = null;
    if (name !== "WRITE_FAILED" && existsSync(caseLatestPath)) {
      latest = JSON.parse(readFileSync(caseLatestPath, "utf8")) as {
        outcome: string;
        subcode?: string;
      };
    }

    return { result, summary, latest, printedText };
  }

  const table: Array<{
    name: CaseName;
    outcome: string;
    subcode?: string;
    exitCode: number;
  }> = [
    { name: "HEALTHY", outcome: "HEALTHY", exitCode: 0 },
    { name: "DEGRADED", outcome: "DEGRADED", exitCode: 1 },
    {
      name: "NO_BASELINE",
      outcome: "COULD_NOT_MEASURE",
      subcode: "NO_BASELINE",
      exitCode: 2,
    },
    {
      name: "CHILD_FAILED",
      outcome: "COULD_NOT_MEASURE",
      subcode: "CHILD_FAILED",
      exitCode: 2,
    },
    {
      name: "PORT_BUSY",
      outcome: "COULD_NOT_MEASURE",
      subcode: "PORT_BUSY",
      exitCode: 2,
    },
    {
      name: "ROW_MISSING",
      outcome: "COULD_NOT_MEASURE",
      subcode: "ROW_MISSING",
      exitCode: 2,
    },
    {
      name: "IDENTITY",
      outcome: "COULD_NOT_MEASURE",
      subcode: "IDENTITY",
      exitCode: 2,
    },
    {
      name: "WRITE_FAILED",
      outcome: "COULD_NOT_MEASURE",
      subcode: "WRITE_FAILED",
      exitCode: 2,
    },
  ];

  for (const entry of table) {
    it(`${entry.name} prints, exits, summarizes, and records the same outcome`, async () => {
      const { result, summary, latest, printedText } = await runCase(entry.name);

      expect(result.outcome).toBe(entry.outcome);
      expect(result.exitCode).toBe(entry.exitCode);
      if (entry.subcode) {
        expect(result.subcode).toBe(entry.subcode);
      }

      expect(printedText).toContain(entry.outcome);
      if (entry.subcode) {
        expect(printedText).toContain(entry.subcode);
      }

      expect(summary).toContain(entry.outcome);
      if (entry.subcode) {
        expect(summary).toContain(entry.subcode);
      }
      expect(summary).toContain(ADVISORY);

      if (entry.name === "WRITE_FAILED") {
        expect(latest).toBeNull();
      } else {
        expect(latest).not.toBeNull();
        expect(latest?.outcome).toBe(entry.outcome);
        if (entry.subcode) {
          expect(latest?.subcode).toBe(entry.subcode);
        }
      }
    });
  }
});
