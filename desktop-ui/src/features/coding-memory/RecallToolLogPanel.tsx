import { useState } from "react";
import { useRecallLog } from "./hooks";

const LAYERS = [
  "index",
  "timeline",
  "fetch",
  "dead_end",
  "facts_as_of",
  "change_history",
  "decision_points",
  "session_start_inject",
  "user_prompt_inject",
] as const;

export function RecallToolLogPanel() {
  const [layer, setLayer] = useState<string | undefined>(undefined);
  const [page, setPage] = useState(0);
  const limit = 50;
  const { data, isLoading, error } = useRecallLog(layer, limit, page * limit);

  return (
    <section className="p-6 space-y-4">
      <header className="flex items-center justify-between">
        <h1 className="text-2xl font-semibold text-foreground">Recall Tool Log</h1>
        <select
          aria-label="Filter by layer"
          className="bg-surface-base border border-border rounded px-2 py-1 text-sm"
          value={layer ?? ""}
          onChange={(e) => {
            setLayer(e.target.value || undefined);
            setPage(0);
          }}
        >
          <option value="">all layers</option>
          {LAYERS.map((l) => (
            <option key={l} value={l}>
              {l}
            </option>
          ))}
        </select>
      </header>
      {isLoading && <div className="text-muted-foreground">Loading…</div>}
      {error && <div className="text-error">Error: {String(error)}</div>}
      <ul className="divide-y divide-border">
        {(data ?? []).map((row) => (
          <li key={row.id} className="py-3 flex flex-col gap-1">
            <div className="flex items-center justify-between text-sm">
              <span className="text-foreground font-mono">{row.layer}</span>
              <span className="text-muted-foreground">
                {new Date(row.occurredAt).toLocaleString()}
              </span>
            </div>
            <div className="text-sm text-foreground truncate">{row.query || "(no query)"}</div>
            <div className="text-xs text-muted-foreground flex gap-3">
              <span>cov={row.coverageScore?.toFixed(2) ?? "—"}</span>
              <span>{row.latencyMs}ms</span>
              {row.skillUsed && <span>skill={row.skillUsed}</span>}
              <span>{row.resultIds.length} ids</span>
            </div>
          </li>
        ))}
      </ul>
      <div className="flex justify-between">
        <button
          type="button"
          disabled={page === 0}
          onClick={() => setPage((p) => Math.max(0, p - 1))}
          className="text-sm text-foreground disabled:text-muted-foreground"
        >
          Prev
        </button>
        <button
          type="button"
          disabled={(data?.length ?? 0) < limit}
          onClick={() => setPage((p) => p + 1)}
          className="text-sm text-foreground disabled:text-muted-foreground"
        >
          Next
        </button>
      </div>
    </section>
  );
}
