import type {
  Baseline,
  Identity,
  LatestRun,
  MetricStat,
  RowDelta,
  RowResult,
} from "./contract.ts";

export function identityMatches(a: Identity, b: Identity): boolean {
  return (
    a.schemaVersion === b.schemaVersion &&
    a.measurementContractVersion === b.measurementContractVersion &&
    a.environmentKey === b.environmentKey
  );
}

function deltaFor(
  row: string,
  metric: string,
  current: number,
  stat: MetricStat,
): RowDelta {
  const delta = current - stat.median;
  return {
    row,
    metric,
    baseline: stat.median,
    current,
    delta,
    margin: stat.margin,
    exceeded: current > stat.median + stat.margin,
  };
}

function compareRow(result: RowResult, baseline: Baseline["rows"][string]): RowDelta[] {
  return [
    deltaFor(result.row, "raf.p95", result.raf.p95Ms, baseline.raf.p95),
    deltaFor(
      result.row,
      "screenshot.p50",
      result.screenshot.p50Ms,
      baseline.screenshot.p50,
    ),
  ];
}

export function compareRows(
  latest: LatestRun,
  baseline: Baseline,
): { outcome: "HEALTHY" | "DEGRADED"; rows: RowDelta[] } {
  const rows: RowDelta[] = [];
  for (const result of latest.rows) {
    const stats = baseline.rows[result.row];
    if (!stats) {
      continue;
    }
    rows.push(...compareRow(result, stats));
  }
  const outcome = rows.some((delta) => delta.exceeded) ? "DEGRADED" : "HEALTHY";
  return { outcome, rows };
}
