import { useState } from "react";
import { useEffectivenessTrends } from "./hooks";

export function PatternEffectivenessPanel() {
  const [patternId, setPatternId] = useState("");
  const { data, loading, error } = useEffectivenessTrends(patternId);

  return (
    <section className="p-6 space-y-4">
      <header className="flex items-center justify-between">
        <h2 className="text-lg font-medium text-foreground">Pattern Effectiveness</h2>
        <div className="flex gap-2">
          <input
            type="text"
            placeholder="Pattern ID"
            className="bg-surface-base border border-border rounded px-2 py-1 text-sm"
            value={patternId}
            onChange={(e) => setPatternId(e.target.value)}
          />
        </div>
      </header>
      {loading && <div className="text-sm text-muted-foreground">Loading…</div>}
      {error && <div className="text-sm text-destructive">Error: {String(error)}</div>}
      {data && (
        <div className="space-y-3">
          <div className="text-sm text-muted-foreground">{data.patternName || data.patternId}</div>
          <div className="rounded-xl border border-border overflow-hidden">
            <table className="w-full text-[13px]">
              <thead className="text-[11px] uppercase tracking-wider text-muted-foreground bg-accent/30">
                <tr>
                  <th className="py-2 px-3 text-left font-medium">Date</th>
                  <th className="py-2 px-3 text-left font-medium">Score</th>
                </tr>
              </thead>
              <tbody>
                {data.buckets.map((bucket) => (
                  <tr
                    key={bucket.at}
                    className="border-t border-border hover:bg-accent/20 transition-colors"
                  >
                    <td className="py-2 px-3 text-foreground">{bucket.at}</td>
                    <td className="py-2 px-3 text-muted-foreground tabular-nums">
                      {bucket.score.toFixed(2)}
                    </td>
                  </tr>
                ))}
              </tbody>
            </table>
          </div>
        </div>
      )}
    </section>
  );
}
