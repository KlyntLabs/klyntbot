import { createHash } from "node:crypto";
import type {
  Diagnostics,
  KeyedDims,
  OsFacts,
  RowEnvironment,
  RowResult,
} from "./contract.ts";

export class IdentityMismatch extends Error {
  constructor(message = "row environments disagree") {
    super(message);
    this.name = "IdentityMismatch";
  }
}

function environmentsEqual(a: RowEnvironment, b: RowEnvironment): boolean {
  return (
    a.webkitVersion === b.webkitVersion &&
    a.headless === b.headless &&
    a.dpr === b.dpr &&
    a.userAgent === b.userAgent &&
    a.viewport.width === b.viewport.width &&
    a.viewport.height === b.viewport.height
  );
}

function agreeEnvironment(rows: RowResult[]): RowEnvironment {
  if (rows.length === 0) {
    throw new IdentityMismatch("no row environments to agree on");
  }
  const first = rows[0].environment;
  for (let i = 1; i < rows.length; i++) {
    if (!environmentsEqual(first, rows[i].environment)) {
      throw new IdentityMismatch();
    }
  }
  return first;
}

function runnerClass(env: NodeJS.ProcessEnv, hostname: string): string {
  if (env.CI && env.ImageOS) {
    return `github:${env.ImageOS}`;
  }
  const digest = createHash("sha256").update(hostname).digest("hex");
  return `local:${digest}`;
}

function osMajor(release: string): string {
  return release.split(".")[0] ?? release;
}

function canonicalKeyedJson(keyed: KeyedDims): string {
  return JSON.stringify({
    runnerClass: keyed.runnerClass,
    platform: keyed.platform,
    arch: keyed.arch,
    osMajor: keyed.osMajor,
    webkitVersion: keyed.webkitVersion,
    headless: keyed.headless,
    viewport: {
      width: keyed.viewport.width,
      height: keyed.viewport.height,
    },
    dpr: keyed.dpr,
  });
}

export function describeEnvironment(input: {
  env: NodeJS.ProcessEnv;
  os: OsFacts;
  rows: RowResult[];
}): { key: string; keyed: KeyedDims; diagnostic: Diagnostics } {
  const browser = agreeEnvironment(input.rows);
  const keyed: KeyedDims = {
    runnerClass: runnerClass(input.env, input.os.hostname),
    platform: input.os.platform,
    arch: input.os.arch,
    osMajor: osMajor(input.os.release),
    webkitVersion: browser.webkitVersion,
    headless: browser.headless,
    viewport: {
      width: browser.viewport.width,
      height: browser.viewport.height,
    },
    dpr: browser.dpr,
  };
  const diagnostic: Diagnostics = {
    cpuModel: input.os.cpus[0]?.model ?? "",
    osRelease: input.os.release,
    userAgent: browser.userAgent,
  };
  const key = createHash("sha256")
    .update(canonicalKeyedJson(keyed))
    .digest("hex")
    .slice(0, 16);
  return { key, keyed, diagnostic };
}
