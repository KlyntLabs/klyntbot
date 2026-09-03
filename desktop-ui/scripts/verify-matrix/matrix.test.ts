// @vitest-environment node
import { describe, expect, it } from "vitest";
import { mkdtempSync, writeFileSync, rmSync } from "node:fs";
import { dirname, join } from "node:path";
import { tmpdir } from "node:os";
import { fileURLToPath } from "node:url";
import { loadManifest, ManifestError, type ManifestEntry } from "./matrix.ts";

const scriptDir = dirname(fileURLToPath(import.meta.url));

function writeTempManifest(content: string): { dir: string; path: string } {
  const dir = mkdtempSync(join(tmpdir(), "verify-manifest-"));
  const path = join(dir, "manifest.json");
  writeFileSync(path, content, "utf8");
  return { dir, path };
}

function validEntry(overrides: Partial<ManifestEntry> = {}): ManifestEntry {
  return {
    name: "typecheck",
    command: "bun run typecheck",
    cwd: "desktop-ui",
    mode: "hard",
    profiles: ["default"],
    ...overrides,
  };
}

describe("loadManifest", () => {
  it("returns entries in file order for a valid manifest", () => {
    const entries = [
      validEntry({ name: "first", command: "bun run first" }),
      validEntry({ name: "second", command: "bun run second", mode: "report" }),
    ];
    const { dir, path } = writeTempManifest(JSON.stringify(entries));
    try {
      const loaded = loadManifest(path);
      expect(loaded).toEqual(entries);
      expect(loaded.map((e) => e.name)).toEqual(["first", "second"]);
    } finally {
      rmSync(dir, { recursive: true, force: true });
    }
  });

  it("throws ManifestError naming the path when the file is missing", () => {
    const path = join(tmpdir(), "verify-manifest-missing", "no-such.json");
    expect(() => loadManifest(path)).toThrow(ManifestError);
    try {
      loadManifest(path);
    } catch (err) {
      expect(err).toBeInstanceOf(ManifestError);
      expect((err as ManifestError).path).toBe(path);
    }
  });

  it("throws ManifestError naming the path for invalid JSON", () => {
    const { dir, path } = writeTempManifest("{ not json");
    try {
      expect(() => loadManifest(path)).toThrow(ManifestError);
      try {
        loadManifest(path);
      } catch (err) {
        expect(err).toBeInstanceOf(ManifestError);
        expect((err as ManifestError).path).toBe(path);
      }
    } finally {
      rmSync(dir, { recursive: true, force: true });
    }
  });

  it("throws ManifestError when an entry is missing any of the five fields", () => {
    const fields = ["name", "command", "cwd", "mode", "profiles"] as const;
    for (const field of fields) {
      const entry = validEntry() as Record<string, unknown>;
      delete entry[field];
      const { dir, path } = writeTempManifest(JSON.stringify([entry]));
      try {
        expect(() => loadManifest(path)).toThrow(ManifestError);
        try {
          loadManifest(path);
        } catch (err) {
          expect(err).toBeInstanceOf(ManifestError);
          expect((err as ManifestError).path).toBe(path);
        }
      } finally {
        rmSync(dir, { recursive: true, force: true });
      }
    }
  });

  it("throws ManifestError when profiles is empty", () => {
    const { dir, path } = writeTempManifest(
      JSON.stringify([validEntry({ profiles: [] })]),
    );
    try {
      expect(() => loadManifest(path)).toThrow(ManifestError);
      try {
        loadManifest(path);
      } catch (err) {
        expect(err).toBeInstanceOf(ManifestError);
        expect((err as ManifestError).path).toBe(path);
      }
    } finally {
      rmSync(dir, { recursive: true, force: true });
    }
  });

  it("loads the real frontend verify manifest with six hard default checks", () => {
    const path = join(scriptDir, "../../../scripts/verify-frontend.manifest.json");
    const loaded = loadManifest(path);
    expect(loaded.map((e) => e.name)).toEqual([
      "typecheck",
      "lint",
      "test",
      "check:tokens",
      "check:performance",
      "test:e2e",
    ]);
    expect(loaded).toHaveLength(6);
    for (const entry of loaded) {
      expect(entry.mode).toBe("hard");
      expect(entry.profiles).toEqual(["default"]);
      expect(entry.cwd).toBe("desktop-ui");
      expect(entry.command).toBe(`bun run ${entry.name}`);
    }
  });
});
