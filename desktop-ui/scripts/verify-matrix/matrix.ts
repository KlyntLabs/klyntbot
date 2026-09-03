import { spawn as defaultSpawn } from "node:child_process";
import { readFileSync } from "node:fs";
import { join } from "node:path";

export type ManifestEntry = {
  name: string;
  command: string;
  cwd: string;
  mode: "hard" | "report";
  profiles: string[];
};

export type Row = {
  name: string;
  mode: "hard" | "report";
  result: "pass" | "fail";
  exitStatus: number | null;
  durationSec: number;
  output: string;
  launchError?: string;
};

export type Relay = {
  stdout: (chunk: string) => void;
  stderr: (chunk: string) => void;
};

export class ManifestError extends Error {
  path: string;

  constructor(path: string, reason: string) {
    super(`${path}: ${reason}`);
    this.name = "ManifestError";
    this.path = path;
  }
}

export class SelectionError extends Error {
  registeredNames: string[];
  registeredProfiles: string[];

  constructor(registeredNames: string[], registeredProfiles: string[]) {
    super(
      `unknown selection; registered names: ${registeredNames.join(", ")}; registered profiles: ${registeredProfiles.join(", ")}`,
    );
    this.name = "SelectionError";
    this.registeredNames = registeredNames;
    this.registeredProfiles = registeredProfiles;
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

function collectRegistry(entries: ManifestEntry[]): {
  registeredNames: string[];
  registeredProfiles: string[];
} {
  const registeredNames = entries.map((entry) => entry.name);
  const registeredProfiles: string[] = [];
  const seenProfiles = new Set<string>();
  for (const entry of entries) {
    for (const profile of entry.profiles) {
      if (!seenProfiles.has(profile)) {
        seenProfiles.add(profile);
        registeredProfiles.push(profile);
      }
    }
  }
  return { registeredNames, registeredProfiles };
}

export function selectChecks(
  entries: ManifestEntry[],
  sel: { names?: string[]; profile?: string } = {},
): ManifestEntry[] {
  const { registeredNames, registeredProfiles } = collectRegistry(entries);

  if (sel.names !== undefined) {
    const known = new Set(registeredNames);
    for (const name of sel.names) {
      if (!known.has(name)) {
        throw new SelectionError(registeredNames, registeredProfiles);
      }
    }
    const wanted = new Set(sel.names);
    return entries.filter((entry) => wanted.has(entry.name));
  }

  const profile = sel.profile ?? "default";
  if (!registeredProfiles.includes(profile)) {
    throw new SelectionError(registeredNames, registeredProfiles);
  }
  return entries.filter((entry) => entry.profiles.includes(profile));
}

export function splitCommand(command: string): string[] {
  const parts: string[] = [];
  let current = "";
  let inQuotes = false;
  for (const ch of command) {
    if (ch === '"') {
      inQuotes = !inQuotes;
      continue;
    }
    if (!inQuotes && /\s/.test(ch)) {
      if (current.length > 0) {
        parts.push(current);
        current = "";
      }
      continue;
    }
    current += ch;
  }
  if (current.length > 0) {
    parts.push(current);
  }
  return parts;
}

const defaultRelay: Relay = {
  stdout: (chunk) => {
    process.stdout.write(chunk);
  },
  stderr: (chunk) => {
    process.stderr.write(chunk);
  },
};

function runOne(
  entry: ManifestEntry,
  opts: {
    repoRoot: string;
    spawn: typeof defaultSpawn;
    relay: Relay;
  },
): Promise<Row> {
  const [cmd, ...args] = splitCommand(entry.command);
  const cwd = join(opts.repoRoot, entry.cwd);
  const started = Date.now();

  console.log(`▶ ${entry.name}  (${entry.mode})`);

  return new Promise((resolve) => {
    let output = "";
    let launchError: string | undefined;
    let exitStatus: number | null = null;
    let settled = false;

    const finish = () => {
      if (settled) return;
      settled = true;
      const durationSec = Math.round(((Date.now() - started) / 1000) * 10) / 10;
      const result: "pass" | "fail" =
        exitStatus === 0 && launchError === undefined ? "pass" : "fail";
      const row: Row = {
        name: entry.name,
        mode: entry.mode,
        result,
        exitStatus: launchError !== undefined ? null : exitStatus,
        durationSec,
        output,
      };
      if (launchError !== undefined) {
        row.launchError = launchError;
      }
      resolve(row);
    };

    const child = opts.spawn(cmd, args, {
      cwd,
      stdio: ["inherit", "pipe", "pipe"],
    });

    child.stdout?.on("data", (chunk: Buffer | string) => {
      const text = typeof chunk === "string" ? chunk : chunk.toString();
      opts.relay.stdout(text);
      output += text;
    });
    child.stderr?.on("data", (chunk: Buffer | string) => {
      const text = typeof chunk === "string" ? chunk : chunk.toString();
      opts.relay.stderr(text);
      output += text;
    });
    child.on("error", (err: Error) => {
      launchError = err.message;
      exitStatus = null;
      opts.relay.stderr(`launch error (${entry.name}): ${err.message}\n`);
      finish();
    });
    child.on("close", (code: number | null) => {
      if (launchError !== undefined) {
        finish();
        return;
      }
      exitStatus = code;
      finish();
    });
  });
}

export async function runMatrix(
  entries: ManifestEntry[],
  opts: {
    repoRoot: string;
    spawn?: typeof defaultSpawn;
    relay?: Relay;
  },
): Promise<{ rows: Row[]; exitCode: 0 | 1 }> {
  const spawnFn = opts.spawn ?? defaultSpawn;
  const relay = opts.relay ?? defaultRelay;
  const rows: Row[] = [];
  let exitCode: 0 | 1 = 0;

  for (const entry of entries) {
    const row = await runOne(entry, {
      repoRoot: opts.repoRoot,
      spawn: spawnFn,
      relay,
    });
    rows.push(row);
    if (row.result === "fail") {
      if (row.exitStatus === null || entry.mode === "hard") {
        exitCode = 1;
      }
    }
  }

  return { rows, exitCode };
}

function pad(value: string, width: number): string {
  return value.length >= width ? value : value + " ".repeat(width - value.length);
}

export function formatSummary(rows: Row[]): string {
  const headers = ["name", "mode", "result", "exit", "seconds"] as const;
  const cells = rows.map((row) => [
    row.name,
    row.mode,
    row.result,
    row.exitStatus === null ? "-" : String(row.exitStatus),
    row.durationSec.toFixed(1),
  ]);
  const widths = headers.map((header, i) =>
    Math.max(header.length, ...cells.map((cell) => cell[i].length)),
  );
  const lines = [
    headers.map((header, i) => pad(header, widths[i])).join("  "),
    ...cells.map((cell) =>
      cell.map((value, i) => pad(value, widths[i])).join("  "),
    ),
  ];
  for (const row of rows) {
    if (row.launchError !== undefined) {
      lines.push(`launch error (${row.name}): ${row.launchError}`);
    }
  }
  return lines.join("\n");
}
