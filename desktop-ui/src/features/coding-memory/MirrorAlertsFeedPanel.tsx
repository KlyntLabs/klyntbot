import { useState } from "react";
import { useMirrorAlertsFeed } from "./hooks";

export function MirrorAlertsFeedPanel() {
  const [kind, setKind] = useState<string>("");
  const [severity, setSeverity] = useState<string>("");
  const [repo, setRepo] = useState<string>("");
  const { data, isLoading, error } = useMirrorAlertsFeed(
    kind || undefined,
    severity || undefined,
    repo || undefined,
  );

  return (
    <section className="p-6 space-y-4">
      <header className="flex items-center justify-between">
        <h2 className="text-lg font-medium text-foreground">Mirror Alerts</h2>
        <div className="flex gap-2">
          <input
            type="text"
            placeholder="Kind"
            className="bg-surface-base border border-border rounded px-2 py-1 text-sm"
            value={kind}
            onChange={(e) => setKind(e.target.value)}
          />
          <input
            type="text"
            placeholder="Severity"
            className="bg-surface-base border border-border rounded px-2 py-1 text-sm"
            value={severity}
            onChange={(e) => setSeverity(e.target.value)}
          />
          <input
            type="text"
            placeholder="Repo"
            className="bg-surface-base border border-border rounded px-2 py-1 text-sm"
            value={repo}
            onChange={(e) => setRepo(e.target.value)}
          />
        </div>
      </header>
      {isLoading && <div className="text-sm text-muted-foreground">Loading…</div>}
      {error && <div className="text-sm text-destructive">Error: {String(error)}</div>}
      <div className="rounded-xl border border-border overflow-hidden">
        <table className="w-full text-[13px]">
          <thead className="text-[11px] uppercase tracking-wider text-muted-foreground bg-accent/30">
            <tr>
              <th className="py-2 px-3 text-left font-medium">Severity</th>
              <th className="py-2 px-3 text-left font-medium">Kind</th>
              <th className="py-2 px-3 text-left font-medium">Headline</th>
              <th className="py-2 px-3 text-left font-medium">Created</th>
              <th className="py-2 px-3 text-left font-medium">Dismissed</th>
            </tr>
          </thead>
          <tbody>
            {(data ?? []).map((row) => (
              <tr
                key={row.id}
                className="border-t border-border hover:bg-accent/20 transition-colors"
              >
                <td className="py-2 px-3 text-foreground">{row.severity}</td>
                <td className="py-2 px-3 text-muted-foreground">{row.kind}</td>
                <td className="py-2 px-3 text-foreground">{row.headline}</td>
                <td className="py-2 px-3 text-muted-foreground">{row.createdAt}</td>
                <td className="py-2 px-3 text-muted-foreground">{row.dismissed ? "yes" : "no"}</td>
              </tr>
            ))}
          </tbody>
        </table>
      </div>
    </section>
  );
}
