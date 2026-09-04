import { appendFileSync } from "node:fs";
import type { LatestRun, RowDelta, Subcode } from "./contract.ts";

export const ADVISORY =
  "This result is an advisory WebKit proxy, not native rendering evidence.";

function outcomeHeading(run: LatestRun, subcode?: Subcode): string {
  const code = subcode ?? run.subcode;
  if (run.outcome === "COULD_NOT_MEASURE" && code) {
    return `COULD_NOT_MEASURE / ${code}`;
  }
  return run.outcome;
}

export function renderSummary(
  run: LatestRun,
  comparison?: RowDelta[],
  subcode?: Subcode,
): string {
  const lines: string[] = [];
  lines.push(`# ${outcomeHeading(run, subcode)}`);
  lines.push("");
  lines.push(`Identity: \`${run.identity.environmentKey}\``);
  lines.push("");

  const deltas = comparison ?? run.comparison ?? [];
  if (deltas.length > 0) {
    lines.push(
      "| row | metric | baseline | current | delta | margin | exceeded |",
    );
    lines.push("| --- | --- | ---: | ---: | ---: | ---: | --- |");
    for (const delta of deltas) {
      lines.push(
        `| ${delta.row} | ${delta.metric} | ${delta.baseline} | ${delta.current} | ${delta.delta} | ${delta.margin} | ${delta.exceeded} |`,
      );
    }
    lines.push("");
  }

  lines.push(ADVISORY);
  lines.push("");
  return lines.join("\n");
}

export function writeStepSummary(
  markdown: string,
  env: NodeJS.ProcessEnv,
): void {
  const path = env.GITHUB_STEP_SUMMARY;
  if (!path) {
    return;
  }
  appendFileSync(path, markdown, "utf8");
}
