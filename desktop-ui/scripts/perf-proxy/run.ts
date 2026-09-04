import { spawn as defaultSpawn, type ChildProcess } from "node:child_process";
import {
  existsSync,
  mkdirSync,
  readFileSync,
  renameSync,
  rmSync,
  writeFileSync,
} from "node:fs";
import os from "node:os";
import { dirname, join } from "node:path";
import {
  MEASUREMENT_CONTRACT_VERSION,
  SCHEMA_VERSION,
  type Baseline,
  type Identity,
  type KeyedDims,
  type Diagnostics,
  type LatestRun,
  type Outcome,
  type OsFacts,
  type RowDelta,
  type RowResult,
  type Subcode,
} from "./contract.ts";
import { compareRows } from "./compare.ts";
import { IdentityMismatch, describeEnvironment } from "./env.ts";
import { renderSummary, writeStepSummary } from "./summary.ts";
import {
  EXPECTED_ROWS,
  ROWS_DIR,
  readRowFiles,
} from "../../tests/perf-proxy/support/rowfile.ts";

const PERF_CONFIG = "playwright.perf-proxy.config.ts";
const DEFAULT_LATEST = "test-results/perf-proxy/latest.json";
const DEFAULT_BASELINES = "tests/perf-proxy/baselines";

export type SpawnFn = typeof defaultSpawn;

export type RunProxyOpts = {
  spawn?: SpawnFn;
  rowsDir?: string;
  baselinesDir?: string;
  latestPath?: string;
  env?: NodeJS.ProcessEnv;
};

export type RunProxyResult = {
  outcome: Outcome;
  subcode?: Subcode;
  exitCode: 0 | 1 | 2;
};

type MeasureRaw = {
  rows: RowResult[];
  output: string;
  exitCode: number | null;
};

function realOsFacts(): OsFacts {
  return {
    platform: os.platform(),
    arch: os.arch(),
    release: os.release(),
    hostname: os.hostname(),
    cpus: os.cpus().map((cpu) => ({ model: cpu.model })),
  };
}

function emptyEnvironment(): { keyed: KeyedDims; diagnostic: Diagnostics } {
  return {
    keyed: {
      runnerClass: "unknown",
      platform: os.platform(),
      arch: os.arch(),
      osMajor: os.release().split(".")[0] ?? "",
      webkitVersion: "",
      headless: true,
      viewport: { width: 0, height: 0 },
      dpr: 0,
    },
    diagnostic: {
      cpuModel: os.cpus()[0]?.model ?? "",
      osRelease: os.release(),
      userAgent: "",
    },
  };
}

function unknownIdentity(): Identity {
  return {
    schemaVersion: SCHEMA_VERSION,
    measurementContractVersion: MEASUREMENT_CONTRACT_VERSION,
    environmentKey: "",
  };
}

function exitFor(outcome: Outcome): 0 | 1 | 2 {
  if (outcome === "HEALTHY") return 0;
  if (outcome === "DEGRADED") return 1;
  return 2;
}

function printOutcome(outcome: Outcome, subcode?: Subcode, identity?: Identity): void {
  if (outcome === "COULD_NOT_MEASURE" && subcode) {
    const id =
      identity?.environmentKey && identity.environmentKey.length > 0
        ? ` identity=${identity.environmentKey}`
        : "";
    console.log(`COULD_NOT_MEASURE / ${subcode}${id}`);
    return;
  }
  console.log(outcome);
}

function cleanStart(rowsDir: string, latestPath: string): void {
  rmSync(rowsDir, { recursive: true, force: true });
  if (existsSync(latestPath)) {
    rmSync(latestPath, { force: true });
  }
}

function spawnPlaywright(
  spawnFn: SpawnFn,
  args: string[],
): Promise<{ exitCode: number | null; output: string }> {
  return new Promise((resolve) => {
    let output = "";
    let exitCode: number | null = null;
    let launchError: string | undefined;
    let settled = false;

    const finish = () => {
      if (settled) return;
      settled = true;
      resolve({ exitCode, output });
    };

    const child: ChildProcess = spawnFn("playwright", args, {
      stdio: ["inherit", "pipe", "pipe"],
    });

    const onPipeData =
      (relay: (chunk: string) => void) => (chunk: Buffer | string) => {
        const text = typeof chunk === "string" ? chunk : chunk.toString();
        relay(text);
        output += text;
      };

    child.stdout?.on("data", onPipeData((text) => process.stdout.write(text)));
    child.stderr?.on("data", onPipeData((text) => process.stderr.write(text)));
    child.on("error", (err: Error) => {
      launchError = err.message;
      exitCode = null;
      output += `launch error: ${err.message}\n`;
      finish();
    });
    child.on("close", (code: number | null) => {
      if (launchError !== undefined) {
        finish();
        return;
      }
      exitCode = code;
      finish();
    });
  });
}

async function collectMeasurement(opts: {
  spawn: SpawnFn;
  rowsDir: string;
}): Promise<MeasureRaw> {
  const { exitCode, output } = await spawnPlaywright(opts.spawn, [
    "test",
    '--config',
    PERF_CONFIG,
  ]);

  let rows: RowResult[] = [];
  if (existsSync(opts.rowsDir)) {
    rows = readRowFiles(opts.rowsDir);
  }
  return { rows, output, exitCode };
}

function missingExpectedRows(rows: RowResult[]): boolean {
  const have = new Set(rows.map((row) => row.row));
  return EXPECTED_ROWS.some((name) => !have.has(name));
}

function loadBaseline(
  baselinesDir: string,
  environmentKey: string,
): Baseline | null {
  const path = join(baselinesDir, `${environmentKey}.json`);
  if (!existsSync(path)) {
    return null;
  }
  return JSON.parse(readFileSync(path, "utf8")) as Baseline;
}

function writeLatestAtomic(latestPath: string, run: LatestRun): void {
  const dir = dirname(latestPath);
  mkdirSync(dir, { recursive: true });
  const tempPath = join(
    dir,
    `.latest.${process.pid}.${Date.now()}.json.tmp`,
  );
  writeFileSync(tempPath, `${JSON.stringify(run, null, 2)}\n`, "utf8");
  renameSync(tempPath, latestPath);
}

export async function finalizeOutcome(input: {
  outcome: Outcome;
  subcode?: Subcode;
  rows: RowResult[];
  identity?: Identity;
  environment?: { keyed: KeyedDims; diagnostic: Diagnostics };
  comparison?: RowDelta[];
  latestPath: string;
  env: NodeJS.ProcessEnv;
}): Promise<0 | 1 | 2> {
  let outcome = input.outcome;
  let subcode = input.subcode;
  const identity = input.identity ?? unknownIdentity();
  const environment = input.environment ?? emptyEnvironment();

  const run: LatestRun = {
    identity,
    environment,
    rows: input.rows,
    outcome,
    recordedAt: new Date().toISOString(),
  };
  if (subcode) {
    run.subcode = subcode;
  }
  if (input.comparison) {
    run.comparison = input.comparison;
  }

  try {
    writeLatestAtomic(input.latestPath, run);
  } catch {
    outcome = "COULD_NOT_MEASURE";
    subcode = "WRITE_FAILED";
    run.outcome = outcome;
    run.subcode = subcode;
  }

  printOutcome(outcome, subcode, identity);
  const markdown = renderSummary(run, input.comparison, subcode);
  writeStepSummary(markdown, input.env);
  return exitFor(outcome);
}

export async function measureOnce(
  opts: {
    spawn?: SpawnFn;
    rowsDir?: string;
    env?: NodeJS.ProcessEnv;
  } = {},
): Promise<{ rows: RowResult[]; identity: Identity }> {
  const spawnFn = opts.spawn ?? defaultSpawn;
  const rowsDir = opts.rowsDir ?? ROWS_DIR;
  const env = opts.env ?? process.env;

  rmSync(rowsDir, { recursive: true, force: true });
  const raw = await collectMeasurement({ spawn: spawnFn, rowsDir });
  if (raw.exitCode !== 0) {
    throw new Error(
      `playwright exited ${raw.exitCode ?? "null"}: ${raw.output.slice(0, 500)}`,
    );
  }
  if (missingExpectedRows(raw.rows)) {
    throw new Error("ROW_MISSING");
  }
  const described = describeEnvironment({
    env,
    os: realOsFacts(),
    rows: raw.rows,
  });
  return {
    rows: raw.rows,
    identity: {
      schemaVersion: SCHEMA_VERSION,
      measurementContractVersion: MEASUREMENT_CONTRACT_VERSION,
      environmentKey: described.key,
    },
  };
}

export async function runProxy(opts: RunProxyOpts = {}): Promise<RunProxyResult> {
  const spawnFn = opts.spawn ?? defaultSpawn;
  const rowsDir = opts.rowsDir ?? ROWS_DIR;
  const baselinesDir = opts.baselinesDir ?? DEFAULT_BASELINES;
  const latestPath = opts.latestPath ?? DEFAULT_LATEST;
  const env = opts.env ?? process.env;

  cleanStart(rowsDir, latestPath);

  const raw = await collectMeasurement({ spawn: spawnFn, rowsDir });

  let outcome: Outcome;
  let subcode: Subcode | undefined;
  let identity: Identity | undefined;
  let environment: { keyed: KeyedDims; diagnostic: Diagnostics } | undefined;
  let comparison: RowDelta[] | undefined;
  const rows = raw.rows;

  if (raw.exitCode !== 0) {
    outcome = "COULD_NOT_MEASURE";
    subcode =
      raw.output.includes("is already used") ? "PORT_BUSY" : "CHILD_FAILED";
  } else if (missingExpectedRows(rows)) {
    outcome = "COULD_NOT_MEASURE";
    subcode = "ROW_MISSING";
  } else {
    try {
      const described = describeEnvironment({
        env,
        os: realOsFacts(),
        rows,
      });
      identity = {
        schemaVersion: SCHEMA_VERSION,
        measurementContractVersion: MEASUREMENT_CONTRACT_VERSION,
        environmentKey: described.key,
      };
      environment = {
        keyed: described.keyed,
        diagnostic: described.diagnostic,
      };

      const baseline = loadBaseline(baselinesDir, described.key);
      if (!baseline) {
        outcome = "COULD_NOT_MEASURE";
        subcode = "NO_BASELINE";
      } else {
        const provisional: LatestRun = {
          identity,
          environment,
          rows,
          outcome: "HEALTHY",
          recordedAt: new Date().toISOString(),
        };
        const compared = compareRows(provisional, baseline);
        outcome = compared.outcome;
        comparison = compared.rows;
      }
    } catch (err) {
      outcome = "COULD_NOT_MEASURE";
      subcode =
        err instanceof IdentityMismatch ? "IDENTITY" : "CHILD_FAILED";
    }
  }

  const exitCode = await finalizeOutcome({
    outcome,
    subcode,
    rows,
    identity,
    environment,
    comparison,
    latestPath,
    env,
  });

  if (!existsSync(latestPath)) {
    return {
      outcome: "COULD_NOT_MEASURE",
      subcode: "WRITE_FAILED",
      exitCode: 2,
    };
  }

  const latest = JSON.parse(readFileSync(latestPath, "utf8")) as LatestRun;
  const result: RunProxyResult = {
    outcome: latest.outcome,
    exitCode,
  };
  if (latest.subcode) {
    result.subcode = latest.subcode;
  }
  return result;
}

async function main(): Promise<void> {
  const argv = process.argv.slice(2);
  if (argv.includes("--list")) {
    const { exitCode } = await spawnPlaywright(defaultSpawn, [
      "test",
      '--config',
      PERF_CONFIG,
      "--list",
    ]);
    process.exit(exitCode ?? 1);
  }

  const result = await runProxy();
  process.exit(result.exitCode);
}

if (import.meta.main) {
  await main();
}
