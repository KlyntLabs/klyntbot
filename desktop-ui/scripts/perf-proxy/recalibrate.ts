import { spawnSync } from "node:child_process";
import { existsSync, mkdirSync, writeFileSync } from "node:fs";
import { join } from "node:path";
import {
  MARGIN_POLICY,
  R_REPETITIONS,
  type Baseline,
  type Identity,
  type MetricStat,
  type OsFacts,
  type Outcome,
  type RowResult,
  type Subcode,
} from "./contract.ts";
import { identityMatches } from "./compare.ts";
import { describeEnvironment } from "./env.ts";
import {
  measureOnce as defaultMeasureOnce,
  realOsFacts,
} from "./run.ts";

const DEFAULT_BASELINES_ROOT = "tests/perf-proxy/baselines";

export type MeasureOnceFn = () => Promise<{
  rows: RowResult[];
  identity: Identity;
}>;

export type RecalibrateOpts = {
  measureOnce?: MeasureOnceFn;
  candidateDir?: string;
  baselinesRoot?: string;
  env?: NodeJS.ProcessEnv;
  os?: OsFacts;
  sourceRevision?: string;
  recordedAt?: string;
};

export type RecalibrateResult = {
  outcome: Outcome;
  subcode?: Subcode;
  exitCode: 0 | 1 | 2;
  baselinePath?: string;
};

function identityFailure(): RecalibrateResult {
  console.log("COULD_NOT_MEASURE / IDENTITY");
  return {
    outcome: "COULD_NOT_MEASURE",
    subcode: "IDENTITY",
    exitCode: 2,
  };
}

function median(values: number[]): number {
  const sorted = [...values].sort((a, b) => a - b);
  const mid = Math.floor(sorted.length / 2);
  if (sorted.length % 2 === 0) {
    return (sorted[mid - 1] + sorted[mid]) / 2;
  }
  return sorted[mid];
}

function metricStat(
  values: number[],
  floor: number,
  multiplier: number,
): MetricStat {
  const med = median(values);
  const spread = Math.max(...values) - Math.min(...values);
  const margin = Math.max(multiplier * spread, floor);
  return { median: med, spread, margin };
}

export function aggregate(reps: RowResult[][]): Baseline["rows"] {
  if (reps.length === 0) {
    return {};
  }
  const rowNames = reps[0].map((row) => row.row);
  const out: Baseline["rows"] = {};
  for (const name of rowNames) {
    const rafValues: number[] = [];
    const shotValues: number[] = [];
    for (const rep of reps) {
      const row = rep.find((entry) => entry.row === name);
      if (!row) {
        throw new Error(`missing row ${name} in a repetition`);
      }
      rafValues.push(row.raf.p95Ms);
      shotValues.push(row.screenshot.p50Ms);
    }
    const shotMedian = median(shotValues);
    out[name] = {
      raf: {
        p95: metricStat(
          rafValues,
          MARGIN_POLICY.rafP95.floorMs,
          MARGIN_POLICY.rafP95.multiplier,
        ),
      },
      screenshot: {
        p50: metricStat(
          shotValues,
          MARGIN_POLICY.screenshotP50.floorRatio * shotMedian,
          MARGIN_POLICY.screenshotP50.multiplier,
        ),
      },
    };
  }
  return out;
}

function gitHead(): string {
  const result = spawnSync("git", ["rev-parse", "HEAD"], {
    encoding: "utf8",
  });
  if (result.status !== 0) {
    throw new Error(
      `git rev-parse HEAD failed: ${result.stderr || result.stdout}`,
    );
  }
  return result.stdout.trim();
}

function parseCandidateDir(argv: string[]): string | undefined {
  const idx = argv.indexOf("--candidate");
  if (idx === -1) {
    return undefined;
  }
  const value = argv[idx + 1];
  if (!value || value.startsWith("-")) {
    throw new Error("--candidate requires a directory argument");
  }
  return value;
}

export async function recalibrate(
  opts: RecalibrateOpts = {},
): Promise<RecalibrateResult> {
  const measure = opts.measureOnce ?? defaultMeasureOnce;
  const env = opts.env ?? process.env;
  const osFacts = opts.os ?? realOsFacts();
  const baselinesRoot = opts.baselinesRoot ?? DEFAULT_BASELINES_ROOT;
  const candidateDir = opts.candidateDir;

  const repRows: RowResult[][] = [];
  let identity: Identity | undefined;

  for (let i = 0; i < R_REPETITIONS; i++) {
    const once = await measure();
    if (identity === undefined) {
      identity = once.identity;
    } else if (!identityMatches(identity, once.identity)) {
      return identityFailure();
    }
    repRows.push(once.rows);
  }

  if (!identity) {
    return identityFailure();
  }

  const lastRows = repRows[repRows.length - 1];
  const described = describeEnvironment({
    env,
    os: osFacts,
    rows: lastRows,
  });

  const baseline: Baseline = {
    identity,
    recordedAt: opts.recordedAt ?? new Date().toISOString(),
    sourceRevision: opts.sourceRevision ?? gitHead(),
    repetitions: R_REPETITIONS,
    rows: aggregate(repRows),
    environment: {
      keyed: described.keyed,
      diagnostic: described.diagnostic,
    },
  };

  const outDir = candidateDir ?? baselinesRoot;
  if (!existsSync(outDir)) {
    mkdirSync(outDir, { recursive: true });
  }
  const baselinePath = join(outDir, `${identity.environmentKey}.json`);
  writeFileSync(baselinePath, `${JSON.stringify(baseline, null, 2)}\n`, "utf8");

  return {
    outcome: "HEALTHY",
    exitCode: 0,
    baselinePath,
  };
}

async function main(): Promise<void> {
  const argv = process.argv.slice(2);
  const candidateDir = parseCandidateDir(argv);
  const result = await recalibrate(
    candidateDir === undefined ? {} : { candidateDir },
  );
  process.exit(result.exitCode);
}

if (import.meta.main) {
  await main();
}
