// @vitest-environment node
import { describe, expect, it } from "vitest";
import { spawnSync } from "node:child_process";
import {
  existsSync,
  mkdtempSync,
  readFileSync,
  rmSync,
  writeFileSync,
} from "node:fs";
import { dirname, join } from "node:path";
import { tmpdir } from "node:os";
import { fileURLToPath } from "node:url";

const scriptDir = dirname(fileURLToPath(import.meta.url));
const mainPath = join(scriptDir, "main.ts");
const realManifestPath = join(
  scriptDir,
  "../../../scripts/verify-frontend.manifest.json",
);

function runMain(
  args: string[],
  opts: { cwd?: string } = {},
): { status: number | null; stdout: string; stderr: string } {
  const result = spawnSync("bun", [mainPath, ...args], {
    encoding: "utf8",
    cwd: opts.cwd ?? process.cwd(),
  });
  return {
    status: result.status,
    stdout: result.stdout ?? "",
    stderr: result.stderr ?? "",
  };
}

function writeTempManifest(
  build: (marker: string) => unknown[],
): { dir: string; path: string; marker: string } {
  const dir = mkdtempSync(join(tmpdir(), "verify-main-"));
  const path = join(dir, "manifest.json");
  const marker = join(dir, "marker");
  writeFileSync(path, JSON.stringify(build(marker)), "utf8");
  return { dir, path, marker };
}

describe("verify-matrix CLI", () => {
  it("lists every entry field and exits 0 without running checks", () => {
    const { dir, path, marker } = writeTempManifest((markerPath) => [
      {
        name: "writes-marker",
        command: `sh -c "touch '${markerPath}'"`,
        cwd: ".",
        mode: "hard",
        profiles: ["default"],
      },
    ]);
    try {
      const result = runMain(["--manifest", path, "--list"], { cwd: dir });
      expect(result.status).toBe(0);
      expect(result.stdout).toContain("writes-marker");
      expect(result.stdout).toContain("hard");
      expect(result.stdout).toContain("default");
      expect(result.stdout).toContain(".");
      expect(result.stdout).toContain(`sh -c "touch '${marker}'"`);
      expect(existsSync(marker)).toBe(false);
    } finally {
      rmSync(dir, { recursive: true, force: true });
    }
  });

  it("lists the six default check names from the real manifest", () => {
    const result = runMain(["--list"]);
    expect(result.status).toBe(0);
    for (const name of [
      "typecheck",
      "lint",
      "test",
      "check:tokens",
      "check:performance",
      "test:e2e",
    ]) {
      expect(result.stdout).toContain(name);
    }
    const real = JSON.parse(readFileSync(realManifestPath, "utf8")) as Array<{
      name: string;
    }>;
    expect(real.map((e) => e.name)).toEqual([
      "typecheck",
      "lint",
      "test",
      "check:tokens",
      "check:performance",
      "test:e2e",
    ]);
  });

  it("runs an appended extra-profile entry only when that profile is selected", () => {
    const { dir, path, marker } = writeTempManifest((markerPath) => [
      {
        name: "noop",
        command: 'sh -c "true"',
        cwd: ".",
        mode: "hard",
        profiles: ["default"],
      },
      {
        name: "extra-check",
        command: `sh -c "touch '${markerPath}'"`,
        cwd: ".",
        mode: "hard",
        profiles: ["extra"],
      },
    ]);
    try {
      const defaultRun = runMain(["--manifest", path], { cwd: dir });
      expect(defaultRun.status).toBe(0);
      expect(existsSync(marker)).toBe(false);

      const extraRun = runMain(["--manifest", path, "--profile", "extra"], {
        cwd: dir,
      });
      expect(extraRun.status).toBe(0);
      expect(existsSync(marker)).toBe(true);
      expect(extraRun.stdout).toContain("extra-check");
    } finally {
      rmSync(dir, { recursive: true, force: true });
    }
  });

  it("exits 1 and lists registered names for an unknown check", () => {
    const { dir, path } = writeTempManifest(() => [
      {
        name: "alpha",
        command: 'sh -c "true"',
        cwd: ".",
        mode: "hard",
        profiles: ["default"],
      },
      {
        name: "beta",
        command: 'sh -c "true"',
        cwd: ".",
        mode: "hard",
        profiles: ["default"],
      },
    ]);
    try {
      const result = runMain(["--manifest", path, "nope"], { cwd: dir });
      expect(result.status).toBe(1);
      expect(result.stderr).toContain("alpha");
      expect(result.stderr).toContain("beta");
    } finally {
      rmSync(dir, { recursive: true, force: true });
    }
  });

  it("exits 1 and names the path for an unparsable manifest", () => {
    const dir = mkdtempSync(join(tmpdir(), "verify-main-bad-"));
    const path = join(dir, "broken.json");
    writeFileSync(path, "{ not json", "utf8");
    try {
      const result = runMain(["--manifest", path], { cwd: dir });
      expect(result.status).toBe(1);
      expect(result.stderr).toContain(path);
    } finally {
      rmSync(dir, { recursive: true, force: true });
    }
  });

  it("prints the launch error when a check cannot be started", () => {
    const { dir, path } = writeTempManifest(() => [
      {
        name: "missing",
        command: "definitely-not-a-command-xyz",
        cwd: ".",
        mode: "report",
        profiles: ["default"],
      },
    ]);
    try {
      const result = runMain(["--manifest", path], { cwd: dir });
      expect(result.status).toBe(1);
      const combined = `${result.stdout}\n${result.stderr}`;
      expect(combined).toMatch(/launch error \(missing\):/);
      expect(combined).toMatch(/ENOENT|not found|definitely-not-a-command-xyz/i);
    } finally {
      rmSync(dir, { recursive: true, force: true });
    }
  });
});
