// @vitest-environment node
import { spawnSync } from "node:child_process";
import {
  existsSync,
  mkdtempSync,
  readdirSync,
  readFileSync,
  rmSync,
  writeFileSync,
} from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { afterEach, describe, expect, it } from "vitest";
import {
  MEASUREMENT_CONTRACT_VERSION,
  MARGIN_POLICY,
  R_REPETITIONS,
  SCHEMA_VERSION,
  type Identity,
  type OsFacts,
  type RowEnvironment,
  type RowResult,
} from "./contract.ts";
import { describeEnvironment } from "./env.ts";
import { aggregate, recalibrate } from "./recalibrate.ts";

const HOSTNAME = "perf-proxy-recalibrate-host.local";

const temps: string[] = [];

afterEach(() => {
  while (temps.length > 0) {
    const dir = temps.pop();
    if (dir) {
      rmSync(dir, { recursive: true, force: true });
    }
  }
});

function tempDir(prefix: string): string {
  const dir = mkdtempSync(join(tmpdir(), prefix));
  temps.push(dir);
  return dir;
}

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

function osFacts(overrides: Partial<OsFacts> = {}): OsFacts {
  return {
    platform: "darwin",
    arch: "arm64",
    release: "24.5.0",
    hostname: HOSTNAME,
    cpus: [{ model: "Apple M2" }],
    ...overrides,
  };
}

function sixRows(rafP95: number, screenshotP50: number): RowResult[] {
  const names = [
    "idle-20×light",
    "idle-20×dark",
    "idle-200×light",
    "idle-200×dark",
    "scroll-200×light",
    "scroll-200×dark",
  ];
  return names.map((name) => rowResult(name, { rafP95, screenshotP50 }));
}

function identityFor(rows: RowResult[], os: OsFacts = osFacts()): Identity {
  const described = describeEnvironment({ env: {}, os, rows });
  return {
    schemaVersion: SCHEMA_VERSION,
    measurementContractVersion: MEASUREMENT_CONTRACT_VERSION,
    environmentKey: described.key,
  };
}

describe("aggregate", () => {
  it("computes per-row median, spread, and policy margins across repetitions", () => {
    const rafValues = [10, 12, 11, 10, 14];
    const shotValues = [40, 42, 41, 40, 50];
    const reps = rafValues.map((raf, i) => sixRows(raf, shotValues[i]));

    const rows = aggregate(reps);
    const sample = rows["idle-20×light"];
    expect(sample).toBeDefined();

    // sorted raf: 10,10,11,12,14 → median 11, spread 4 → max(3×4, 2) = 12
    expect(sample.raf.p95.median).toBe(11);
    expect(sample.raf.p95.spread).toBe(4);
    expect(sample.raf.p95.margin).toBe(12);

    // sorted shot: 40,40,41,42,50 → median 41, spread 10 → max(3×10, 0.15×41) = 30
    expect(sample.screenshot.p50.median).toBe(41);
    expect(sample.screenshot.p50.spread).toBe(10);
    expect(sample.screenshot.p50.margin).toBe(30);
  });

  it("uses the absolute floor when multiplier × spread is smaller", () => {
    const reps = Array.from({ length: R_REPETITIONS }, () => sixRows(11, 40));
    const rows = aggregate(reps);
    const sample = rows["idle-20×light"];
    expect(sample.raf.p95.spread).toBe(0);
    expect(sample.raf.p95.margin).toBe(MARGIN_POLICY.rafP95.floorMs);
    expect(sample.screenshot.p50.margin).toBe(
      MARGIN_POLICY.screenshotP50.floorRatio * 40,
    );
  });
});

describe("recalibrate", () => {
  it("writes a baseline under the baselines root when no candidate dir is set", async () => {
    const baselinesRoot = tempDir("perf-proxy-baselines-");
    const os = osFacts();
    const reps = [
      sixRows(10, 40),
      sixRows(11, 41),
      sixRows(12, 42),
      sixRows(11, 40),
      sixRows(10, 41),
    ];
    let call = 0;
    const identity = identityFor(reps[0], os);

    const result = await recalibrate({
      measureOnce: async () => {
        const rows = reps[call++];
        return { rows, identity };
      },
      baselinesRoot,
      os,
      env: {},
      sourceRevision: "deadbeef",
      recordedAt: "2026-09-04T12:00:00.000Z",
    });

    expect(result.exitCode).toBe(0);
    expect(result.outcome).toBe("HEALTHY");
    expect(call).toBe(R_REPETITIONS);

    const path = join(baselinesRoot, `${identity.environmentKey}.json`);
    expect(result.baselinePath).toBe(path);
    expect(existsSync(path)).toBe(true);

    const baseline = JSON.parse(readFileSync(path, "utf8"));
    expect(baseline.identity).toEqual(identity);
    expect(baseline.repetitions).toBe(R_REPETITIONS);
    expect(baseline.sourceRevision).toBe("deadbeef");
    expect(baseline.recordedAt).toBe("2026-09-04T12:00:00.000Z");
    expect(baseline.rows["idle-20×light"].raf.p95).toMatchObject({
      median: expect.any(Number),
      spread: expect.any(Number),
      margin: expect.any(Number),
    });
    expect(baseline.environment.keyed).toBeDefined();
    expect(baseline.environment.diagnostic).toBeDefined();
    expect(JSON.stringify(baseline)).not.toContain(HOSTNAME);
  });

  it("writes only under the candidate dir and leaves the baselines root untouched", async () => {
    const baselinesRoot = tempDir("perf-proxy-baselines-keep-");
    const candidateDir = tempDir("perf-proxy-candidate-");
    const sentinel = join(baselinesRoot, "should-not-appear.json");
    writeFileSync(sentinel, "{}", "utf8");
    const before = new Set(readdirSync(baselinesRoot));

    const os = osFacts();
    const rows = sixRows(11, 40);
    const identity = identityFor(rows, os);

    const result = await recalibrate({
      measureOnce: async () => ({ rows, identity }),
      candidateDir,
      baselinesRoot,
      os,
      env: {},
      sourceRevision: "abc123",
      recordedAt: "2026-09-04T12:00:00.000Z",
    });

    expect(result.exitCode).toBe(0);
    const candidatePath = join(candidateDir, `${identity.environmentKey}.json`);
    expect(result.baselinePath).toBe(candidatePath);
    expect(existsSync(candidatePath)).toBe(true);
    expect(new Set(readdirSync(baselinesRoot))).toEqual(before);
    expect(existsSync(sentinel)).toBe(true);
  });

  it("exits 2 with COULD_NOT_MEASURE / IDENTITY when repetition identities differ", async () => {
    const baselinesRoot = tempDir("perf-proxy-baselines-id-");
    const os = osFacts();
    const rowsA = sixRows(11, 40);
    const rowsB = sixRows(11, 40).map((row) =>
      rowResult(row.row, {
        rafP95: 11,
        screenshotP50: 40,
        environment: rowEnvironment({ webkitVersion: "WebKit/999" }),
      }),
    );
    const identityA = identityFor(rowsA, os);
    const identityB = identityFor(rowsB, os);
    expect(identityA.environmentKey).not.toBe(identityB.environmentKey);

    let call = 0;
    const printed: string[] = [];
    const originalLog = console.log;
    console.log = (msg?: unknown) => {
      printed.push(String(msg));
    };

    try {
      const result = await recalibrate({
        measureOnce: async () => {
          call += 1;
          if (call === 1) {
            return { rows: rowsA, identity: identityA };
          }
          return { rows: rowsB, identity: identityB };
        },
        baselinesRoot,
        os,
        env: {},
        sourceRevision: "abc",
        recordedAt: "2026-09-04T12:00:00.000Z",
      });

      expect(result.exitCode).toBe(2);
      expect(result.outcome).toBe("COULD_NOT_MEASURE");
      expect(result.subcode).toBe("IDENTITY");
      expect(printed.some((line) => line.includes("COULD_NOT_MEASURE / IDENTITY"))).toBe(
        true,
      );
      expect(readdirSync(baselinesRoot)).toEqual([]);
    } finally {
      console.log = originalLog;
    }
  });
});

describe("routine-run flag surface", () => {
  it("lists only --list as a double-quoted flag token in run.ts", () => {
    const result = spawnSync(
      "bash",
      [
        "-lc",
        `grep -o -- '"--[a-z-]*"' scripts/perf-proxy/run.ts | sort -u`,
      ],
      { cwd: join(import.meta.dirname, "../.."), encoding: "utf8" },
    );
    expect(result.status).toBe(0);
    expect(result.stdout.trim()).toBe('"--list"');
  });
});
