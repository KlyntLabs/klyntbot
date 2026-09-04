// @vitest-environment node
import { describe, expect, it } from "vitest";
import { mkdtempSync, readFileSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import type { LatestRun, RowDelta } from "./contract.ts";
import { renderSummary, writeStepSummary } from "./summary.ts";

const ADVISORY =
  "This result is an advisory WebKit proxy, not native rendering evidence.";

function sampleRun(overrides: Partial<LatestRun> = {}): LatestRun {
  return {
    identity: {
      schemaVersion: 1,
      measurementContractVersion: 1,
      environmentKey: "abcdef0123456789",
    },
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
    rows: [],
    outcome: "HEALTHY",
    recordedAt: "2026-09-04T01:00:00Z",
    ...overrides,
  };
}

describe("renderSummary", () => {
  it("carries outcome, identity, per-row table, and the fixed advisory sentence", () => {
    const comparison: RowDelta[] = [
      {
        row: "idle-20×light",
        metric: "raf.p95",
        baseline: 10,
        current: 12,
        delta: 2,
        margin: 3,
        exceeded: false,
      },
      {
        row: "idle-20×light",
        metric: "screenshot.p50",
        baseline: 40,
        current: 45,
        delta: 5,
        margin: 6,
        exceeded: false,
      },
    ];
    const markdown = renderSummary(sampleRun(), comparison);
    expect(markdown).toContain("HEALTHY");
    expect(markdown).toContain("abcdef0123456789");
    expect(markdown).toContain("idle-20×light");
    expect(markdown).toContain("raf.p95");
    expect(markdown).toContain("screenshot.p50");
    expect(markdown).toContain("10");
    expect(markdown).toContain("12");
    expect(markdown).toContain(ADVISORY);
  });

  it("includes the subcode when the outcome is COULD_NOT_MEASURE", () => {
    const markdown = renderSummary(
      sampleRun({ outcome: "COULD_NOT_MEASURE", subcode: "NO_BASELINE" }),
      undefined,
      "NO_BASELINE",
    );
    expect(markdown).toContain("COULD_NOT_MEASURE");
    expect(markdown).toContain("NO_BASELINE");
    expect(markdown).toContain("abcdef0123456789");
    expect(markdown).toContain(ADVISORY);
  });
});

describe("writeStepSummary", () => {
  it("appends markdown only when GITHUB_STEP_SUMMARY is set", () => {
    const dir = mkdtempSync(join(tmpdir(), "perf-summary-"));
    const summaryPath = join(dir, "step-summary.md");
    try {
      writeStepSummary("# skipped\n", {});
      writeFileSync(summaryPath, "existing\n", "utf8");
      writeStepSummary("# appended\n", { GITHUB_STEP_SUMMARY: summaryPath });
      expect(readFileSync(summaryPath, "utf8")).toBe("existing\n# appended\n");
    } finally {
      rmSync(dir, { recursive: true, force: true });
    }
  });
});
