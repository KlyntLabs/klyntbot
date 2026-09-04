// @vitest-environment node
import { describe, expect, it } from "vitest";
import type {
  Baseline,
  Identity,
  LatestRun,
  MetricStat,
  RowResult,
} from "./contract.ts";
import { compareRows, identityMatches } from "./compare.ts";

function identity(overrides: Partial<Identity> = {}): Identity {
  return {
    schemaVersion: 1,
    measurementContractVersion: 1,
    environmentKey: "abcdef0123456789",
    ...overrides,
  };
}

function metricStat(overrides: Partial<MetricStat> = {}): MetricStat {
  return { median: 10, spread: 1, margin: 3, ...overrides };
}

function rowResult(name: string, rafP95: number, screenshotP50: number): RowResult {
  return {
    row: name,
    scenario: "idle-20",
    theme: "light",
    n: 20,
    renderPath: "plain",
    raf: { sampleCount: 300, p50Ms: rafP95 - 1, p95Ms: rafP95, maxMs: rafP95 + 1 },
    screenshot: {
      captureCount: 10,
      p50Ms: screenshotP50,
      options: { type: "png" },
    },
    environment: {
      webkitVersion: "WebKit/605.1.15",
      headless: true,
      viewport: { width: 1280, height: 800 },
      dpr: 1,
      userAgent: "Mozilla/5.0",
    },
    durationMs: 1000,
  };
}

function baselineFor(rows: Record<string, { rafP95: MetricStat; screenshotP50: MetricStat }>): Baseline {
  const mapped: Baseline["rows"] = {};
  for (const [name, stats] of Object.entries(rows)) {
    mapped[name] = {
      raf: { p95: stats.rafP95 },
      screenshot: { p50: stats.screenshotP50 },
    };
  }
  return {
    identity: identity(),
    recordedAt: "2026-09-04T00:00:00Z",
    sourceRevision: "abc",
    repetitions: 5,
    rows: mapped,
    environment: {
      keyed: {
        runnerClass: "local:deadbeef",
        platform: "darwin",
        arch: "arm64",
        osMajor: "24",
        webkitVersion: "WebKit/605.1.15",
        headless: true,
        viewport: { width: 1280, height: 800 },
        dpr: 1,
      },
      diagnostic: {
        cpuModel: "Apple M2",
        osRelease: "24.5.0",
        userAgent: "Mozilla/5.0",
      },
    },
  };
}

function latestFor(rows: RowResult[], id: Identity = identity()): LatestRun {
  return {
    identity: id,
    environment: {
      keyed: {
        runnerClass: "local:deadbeef",
        platform: "darwin",
        arch: "arm64",
        osMajor: "24",
        webkitVersion: "WebKit/605.1.15",
        headless: true,
        viewport: { width: 1280, height: 800 },
        dpr: 1,
      },
      diagnostic: {
        cpuModel: "Apple M2",
        osRelease: "24.5.0",
        userAgent: "Mozilla/5.0",
      },
    },
    rows,
    outcome: "HEALTHY",
    recordedAt: "2026-09-04T01:00:00Z",
  };
}

describe("compareRows", () => {
  it("returns HEALTHY when every metric is within its stored margin", () => {
    const baseline = baselineFor({
      "idle-20×light": {
        rafP95: metricStat({ median: 10, margin: 3 }),
        screenshotP50: metricStat({ median: 40, margin: 6 }),
      },
    });
    const latest = latestFor([rowResult("idle-20×light", 12, 45)]);
    const result = compareRows(latest, baseline);
    expect(result.outcome).toBe("HEALTHY");
    expect(result.rows.every((delta) => delta.exceeded === false)).toBe(true);
  });

  it("returns DEGRADED naming row, metric, baseline, current, and delta when one exceeds", () => {
    const baseline = baselineFor({
      "idle-20×light": {
        rafP95: metricStat({ median: 10, margin: 3 }),
        screenshotP50: metricStat({ median: 40, margin: 6 }),
      },
    });
    const latest = latestFor([rowResult("idle-20×light", 14, 42)]);
    const result = compareRows(latest, baseline);
    expect(result.outcome).toBe("DEGRADED");
    const exceeded = result.rows.filter((delta) => delta.exceeded);
    expect(exceeded).toHaveLength(1);
    expect(exceeded[0]).toMatchObject({
      row: "idle-20×light",
      metric: "raf.p95",
      baseline: 10,
      current: 14,
      delta: 4,
      margin: 3,
      exceeded: true,
    });
  });

  it("treats a metric exactly at the margin as within", () => {
    const baseline = baselineFor({
      "idle-20×light": {
        rafP95: metricStat({ median: 10, margin: 3 }),
        screenshotP50: metricStat({ median: 40, margin: 6 }),
      },
    });
    const latest = latestFor([rowResult("idle-20×light", 13, 46)]);
    const result = compareRows(latest, baseline);
    expect(result.outcome).toBe("HEALTHY");
    expect(result.rows.every((delta) => delta.exceeded === false)).toBe(true);
  });

  it("fails closed when a latest row is missing from the baseline", () => {
    const baseline = baselineFor({
      "idle-20×light": {
        rafP95: metricStat({ median: 10, margin: 3 }),
        screenshotP50: metricStat({ median: 40, margin: 6 }),
      },
    });
    const latest = latestFor([
      rowResult("idle-20×light", 12, 45),
      rowResult("idle-200×light", 12, 45),
    ]);
    expect(() => compareRows(latest, baseline)).toThrow(/idle-200×light/);
  });
});

describe("identityMatches", () => {
  it("is false when any of the three identity fields differ", () => {
    const base = identity();
    expect(identityMatches(base, identity({ schemaVersion: 2 }))).toBe(false);
    expect(
      identityMatches(base, identity({ measurementContractVersion: 2 })),
    ).toBe(false);
    expect(
      identityMatches(base, identity({ environmentKey: "ffffffffffffffff" })),
    ).toBe(false);
  });

  it("is true when all three identity fields match", () => {
    expect(identityMatches(identity(), identity())).toBe(true);
  });
});
