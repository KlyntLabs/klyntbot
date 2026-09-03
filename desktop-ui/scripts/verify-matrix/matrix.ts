import { readFileSync } from "node:fs";

export type ManifestEntry = {
  name: string;
  command: string;
  cwd: string;
  mode: "hard" | "report";
  profiles: string[];
};

export class ManifestError extends Error {
  path: string;

  constructor(path: string, reason: string) {
    super(`${path}: ${reason}`);
    this.name = "ManifestError";
    this.path = path;
  }
}

const MODES = new Set(["hard", "report"]);

function isNonEmptyString(value: unknown): value is string {
  return typeof value === "string" && value.length > 0;
}

function validateEntry(raw: unknown, path: string, index: number): ManifestEntry {
  if (raw === null || typeof raw !== "object" || Array.isArray(raw)) {
    throw new ManifestError(path, `entry ${index} must be an object`);
  }

  const entry = raw as Record<string, unknown>;
  const required = ["name", "command", "cwd", "mode", "profiles"] as const;
  for (const field of required) {
    if (!(field in entry) || entry[field] === undefined || entry[field] === null) {
      throw new ManifestError(path, `entry ${index} missing field "${field}"`);
    }
  }

  if (!isNonEmptyString(entry.name)) {
    throw new ManifestError(path, `entry ${index} missing field "name"`);
  }
  if (!isNonEmptyString(entry.command)) {
    throw new ManifestError(path, `entry ${index} missing field "command"`);
  }
  if (!isNonEmptyString(entry.cwd)) {
    throw new ManifestError(path, `entry ${index} missing field "cwd"`);
  }
  if (typeof entry.mode !== "string" || !MODES.has(entry.mode)) {
    throw new ManifestError(
      path,
      `entry ${index} mode must be "hard" or "report"`,
    );
  }
  if (!Array.isArray(entry.profiles)) {
    throw new ManifestError(path, `entry ${index} missing field "profiles"`);
  }
  if (entry.profiles.length === 0) {
    throw new ManifestError(path, `entry ${index} profiles must be non-empty`);
  }
  if (!entry.profiles.every((p) => typeof p === "string" && p.length > 0)) {
    throw new ManifestError(
      path,
      `entry ${index} profiles must be non-empty strings`,
    );
  }

  return {
    name: entry.name,
    command: entry.command,
    cwd: entry.cwd,
    mode: entry.mode as "hard" | "report",
    profiles: entry.profiles as string[],
  };
}

export function loadManifest(path: string): ManifestEntry[] {
  let text: string;
  try {
    text = readFileSync(path, "utf8");
  } catch {
    throw new ManifestError(path, "cannot read file");
  }

  let parsed: unknown;
  try {
    parsed = JSON.parse(text);
  } catch {
    throw new ManifestError(path, "invalid JSON");
  }

  if (!Array.isArray(parsed)) {
    throw new ManifestError(path, "manifest must be a JSON array");
  }

  return parsed.map((entry, index) => validateEntry(entry, path, index));
}
