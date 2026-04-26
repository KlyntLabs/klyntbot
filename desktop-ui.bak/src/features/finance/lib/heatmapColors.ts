export const HEATMAP_COLORS = [
  "rgba(255,255,255,0.03)",
  "rgba(244,63,94,0.12)",
  "rgba(244,63,94,0.25)",
  "rgba(244,63,94,0.4)",
  "rgba(244,63,94,0.6)",
] as const;

export type HeatmapLevel = 0 | 1 | 2 | 3 | 4;

export function computeHeatmapLevels(dailyValues: Map<string, number>): Map<string, HeatmapLevel> {
  const result = new Map<string, HeatmapLevel>();
  const nonZero = [...dailyValues.values()].filter((v) => v > 0).sort((a, b) => a - b);

  if (nonZero.length === 0) {
    for (const [date] of dailyValues) result.set(date, 0);
    return result;
  }

  const q1 = nonZero[Math.floor(nonZero.length * 0.25)] ?? nonZero[0];
  const q2 = nonZero[Math.floor(nonZero.length * 0.5)] ?? q1;
  const q3 = nonZero[Math.floor(nonZero.length * 0.75)] ?? q2;

  for (const [date, value] of dailyValues) {
    if (value <= 0) result.set(date, 0);
    else if (value <= q1) result.set(date, 1);
    else if (value <= q2) result.set(date, 2);
    else if (value <= q3) result.set(date, 3);
    else result.set(date, 4);
  }
  return result;
}
