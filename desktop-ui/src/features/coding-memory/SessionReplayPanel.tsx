import type { SessionReplayEntry } from "@shared/types";
import { ChevronLeft, ChevronRight, X } from "lucide-react";
import { useState } from "react";
import { useSessionRecallOverlay, useSessionReplay } from "./hooks";

function SessionRecallOverlay({ sessionId }: { sessionId: string }) {
  const { data: overlay } = useSessionRecallOverlay(sessionId);
  if (!overlay || overlay.length === 0) return null;
  return (
    <div className="space-y-2">
      <h4 className="text-xs font-semibold text-foreground uppercase tracking-wider">
        Recall on this session
      </h4>
      {overlay.map((row) => (
        <div key={row.id} className="text-xs text-foreground border-l-2 border-brand pl-2">
          <div className="font-mono">{row.layer}</div>
          <div className="text-muted-foreground truncate">{row.query}</div>
        </div>
      ))}
    </div>
  );
}

export function SessionReplayPanel() {
  const [offset, setOffset] = useState(0);
  const rows = useSessionReplay(undefined, 500, offset);
  const [selected, setSelected] = useState<SessionReplayEntry | null>(null);

  if (!rows.data) {
    return <div className="p-6 text-sm text-muted-foreground">Loading…</div>;
  }

  return (
    <section className="flex h-full">
      <div className="flex-1 overflow-auto p-6">
        <header className="mb-4 flex items-center justify-between">
          <h2 className="text-lg font-medium text-foreground">Session Replay</h2>
          <div className="flex items-center gap-2">
            <button
              type="button"
              onClick={() => setOffset(Math.max(0, offset - 500))}
              disabled={offset === 0}
              className="flex items-center gap-1 rounded-lg border border-border px-2 py-1 text-xs text-muted-foreground hover:text-foreground hover:bg-accent transition-colors disabled:opacity-50"
            >
              <ChevronLeft className="size-3" />
              Prev
            </button>
            <button
              type="button"
              onClick={() => setOffset(offset + 500)}
              disabled={rows.data.length < 500}
              className="flex items-center gap-1 rounded-lg border border-border px-2 py-1 text-xs text-muted-foreground hover:text-foreground hover:bg-accent transition-colors disabled:opacity-50"
            >
              Next
              <ChevronRight className="size-3" />
            </button>
          </div>
        </header>
        <ul className="divide-y divide-border font-mono text-xs">
          {rows.data.map((r) => (
            <li key={r.id}>
              <button
                type="button"
                onClick={() => setSelected(r)}
                className="flex w-full items-baseline gap-4 py-1.5 text-left hover:bg-accent/20 transition-colors px-2 rounded-lg"
              >
                <span className="w-40 shrink-0 text-muted-foreground">{r.occurredAt}</span>
                <span className="w-24 shrink-0 text-brand">{r.kind}</span>
                <span className="w-24 shrink-0 text-muted-foreground">{r.source}</span>
                <span className="truncate text-foreground">{r.sessionId}</span>
              </button>
            </li>
          ))}
        </ul>
      </div>
      {selected && (
        <aside className="w-1/3 border-l border-border p-6 overflow-auto space-y-4">
          <header className="mb-3 flex items-center justify-between">
            <h3 className="text-sm font-semibold text-foreground">Event detail</h3>
            <button
              type="button"
              onClick={() => setSelected(null)}
              className="text-xs text-muted-foreground hover:text-foreground transition-colors"
            >
              <X className="size-3.5" />
            </button>
          </header>
          <pre className="whitespace-pre-wrap break-words rounded-lg bg-accent/30 p-3 text-xs text-foreground">
            {JSON.stringify(JSON.parse(selected.payload), null, 2)}
          </pre>
          <SessionRecallOverlay sessionId={selected.sessionId} />
        </aside>
      )}
    </section>
  );
}
