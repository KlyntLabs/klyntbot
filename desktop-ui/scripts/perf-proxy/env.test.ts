// @vitest-environment node
import { createHash } from "node:crypto";
import { describe, expect, it } from "vitest";
import type { OsFacts, RowEnvironment, RowResult } from "./contract.ts";
import { IdentityMismatch, describeEnvironment } from "./env.ts";

const HOSTNAME = "perf-proxy-test-host.local";

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
  environment: RowEnvironment = rowEnvironment(),
): RowResult {
  return {
    row: name,
    scenario: "idle-20",
    theme: "light",
    n: 20,
    renderPath: "plain",
    raf: { sampleCount: 300, p50Ms: 10, p95Ms: 11, maxMs: 12 },
    screenshot: {
      captureCount: 10,
      p50Ms: 40,
      options: { type: "png", scale: "css" },
    },
    environment,
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

describe("describeEnvironment", () => {
  it("uses github:<ImageOS> as runnerClass when CI and ImageOS are set", () => {
    const result = describeEnvironment({
      env: { CI: "true", ImageOS: "ubuntu24" },
      os: osFacts(),
      rows: [rowResult("idle-20×light")],
    });
    expect(result.keyed.runnerClass).toBe("github:ubuntu24");
  });

  it("uses local:<64-hex hostname digest> as runnerClass off CI", () => {
    const result = describeEnvironment({
      env: {},
      os: osFacts(),
      rows: [rowResult("idle-20×light")],
    });
    const digest = createHash("sha256").update(HOSTNAME).digest("hex");
    expect(digest).toHaveLength(64);
    expect(result.keyed.runnerClass).toBe(`local:${digest}`);
  });

  it("returns a 16-hex environment key", () => {
    const result = describeEnvironment({
      env: {},
      os: osFacts(),
      rows: [rowResult("idle-20×light")],
    });
    expect(result.key).toMatch(/^[0-9a-f]{16}$/);
  });

  it("returns the same key for the same inputs", () => {
    const input = {
      env: {},
      os: osFacts(),
      rows: [rowResult("idle-20×light"), rowResult("idle-20×dark")],
    };
    expect(describeEnvironment(input).key).toBe(describeEnvironment(input).key);
  });

  it("changes the key when any keyed dimension changes", () => {
    const base = describeEnvironment({
      env: {},
      os: osFacts(),
      rows: [rowResult("idle-20×light")],
    }).key;

    const byPlatform = describeEnvironment({
      env: {},
      os: osFacts({ platform: "linux" }),
      rows: [rowResult("idle-20×light")],
    }).key;
    expect(byPlatform).not.toBe(base);

    const byArch = describeEnvironment({
      env: {},
      os: osFacts({ arch: "x64" }),
      rows: [rowResult("idle-20×light")],
    }).key;
    expect(byArch).not.toBe(base);

    const byOsMajor = describeEnvironment({
      env: {},
      os: osFacts({ release: "23.0.0" }),
      rows: [rowResult("idle-20×light")],
    }).key;
    expect(byOsMajor).not.toBe(base);

    const byRunner = describeEnvironment({
      env: { CI: "true", ImageOS: "macos15" },
      os: osFacts(),
      rows: [rowResult("idle-20×light")],
    }).key;
    expect(byRunner).not.toBe(base);

    const byWebkit = describeEnvironment({
      env: {},
      os: osFacts(),
      rows: [rowResult("idle-20×light", rowEnvironment({ webkitVersion: "other" }))],
    }).key;
    expect(byWebkit).not.toBe(base);

    const byHeadless = describeEnvironment({
      env: {},
      os: osFacts(),
      rows: [rowResult("idle-20×light", rowEnvironment({ headless: false }))],
    }).key;
    expect(byHeadless).not.toBe(base);

    const byViewport = describeEnvironment({
      env: {},
      os: osFacts(),
      rows: [
        rowResult(
          "idle-20×light",
          rowEnvironment({ viewport: { width: 1024, height: 768 } }),
        ),
      ],
    }).key;
    expect(byViewport).not.toBe(base);

    const byDpr = describeEnvironment({
      env: {},
      os: osFacts(),
      rows: [rowResult("idle-20×light", rowEnvironment({ dpr: 2 }))],
    }).key;
    expect(byDpr).not.toBe(base);
  });

  it("does not change the key when only the CPU model changes", () => {
    const a = describeEnvironment({
      env: {},
      os: osFacts({ cpus: [{ model: "Apple M2" }] }),
      rows: [rowResult("idle-20×light")],
    }).key;
    const b = describeEnvironment({
      env: {},
      os: osFacts({ cpus: [{ model: "Intel Core i9" }] }),
      rows: [rowResult("idle-20×light")],
    }).key;
    expect(a).toBe(b);
  });

  it("throws IdentityMismatch when row environments disagree", () => {
    expect(() =>
      describeEnvironment({
        env: {},
        os: osFacts(),
        rows: [
          rowResult("idle-20×light", rowEnvironment({ webkitVersion: "a" })),
          rowResult("idle-20×dark", rowEnvironment({ webkitVersion: "b" })),
        ],
      }),
    ).toThrow(IdentityMismatch);
  });

  it("never serializes the raw hostname in its result", () => {
    const result = describeEnvironment({
      env: {},
      os: osFacts({ hostname: HOSTNAME }),
      rows: [rowResult("idle-20×light")],
    });
    expect(JSON.stringify(result)).not.toContain(HOSTNAME);
  });
});
