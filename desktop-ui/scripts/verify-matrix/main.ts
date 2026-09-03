import { join } from "node:path";
import {
  formatSummary,
  loadManifest,
  ManifestError,
  runMatrix,
  selectChecks,
  SelectionError,
  type ManifestEntry,
} from "./matrix.ts";

function defaultManifestPath(): string {
  return join(
    import.meta.dir,
    "../../../scripts/verify-frontend.manifest.json",
  );
}

function parseArgs(argv: string[]): {
  list: boolean;
  profile?: string;
  manifest?: string;
  names: string[];
} {
  const names: string[] = [];
  let list = false;
  let profile: string | undefined;
  let manifest: string | undefined;

  for (let i = 0; i < argv.length; i++) {
    const arg = argv[i];
    if (arg === "--list") {
      list = true;
      continue;
    }
    if (arg === "--profile") {
      profile = argv[++i];
      continue;
    }
    if (arg === "--manifest") {
      manifest = argv[++i];
      continue;
    }
    names.push(arg);
  }

  return { list, profile, manifest, names };
}

function formatList(entries: ManifestEntry[]): string {
  return entries
    .map((entry) =>
      [
        entry.name,
        entry.mode,
        entry.profiles.join(","),
        entry.cwd,
        entry.command,
      ].join("  "),
    )
    .join("\n");
}

async function main(): Promise<void> {
  const opts = parseArgs(process.argv.slice(2));
  const manifestPath = opts.manifest ?? defaultManifestPath();

  try {
    const entries = loadManifest(manifestPath);

    if (opts.list) {
      console.log(formatList(entries));
      process.exit(0);
    }

    const selected = selectChecks(
      entries,
      opts.names.length > 0
        ? { names: opts.names }
        : opts.profile !== undefined
          ? { profile: opts.profile }
          : {},
    );

    const { rows, exitCode } = await runMatrix(selected, {
      repoRoot: process.cwd(),
    });
    console.log(formatSummary(rows));
    process.exit(exitCode);
  } catch (err) {
    if (err instanceof ManifestError || err instanceof SelectionError) {
      console.error(err.message);
      process.exit(1);
    }
    throw err;
  }
}

await main();
