import type { DiffResult } from "@shared/types";

export function DiffViewer({ diff }: { diff: DiffResult }) {
  return (
    <div className="space-y-4">
      {diff.frontmatterChanges.length > 0 && (
        <section>
          <h3 className="text-sm font-medium text-foreground mb-2">Frontmatter</h3>
          <table className="w-full text-xs border border-border">
            <thead>
              <tr className="text-muted-foreground">
                <th className="text-left p-1">Field</th>
                <th className="text-left p-1">Before</th>
                <th className="text-left p-1">After</th>
              </tr>
            </thead>
            <tbody>
              {diff.frontmatterChanges.map((c) => (
                <tr key={c.field}>
                  <td className="p-1 font-mono">{c.field}</td>
                  <td className="p-1 text-red-400">{JSON.stringify(c.before)}</td>
                  <td className="p-1 text-green-400">{JSON.stringify(c.after)}</td>
                </tr>
              ))}
            </tbody>
          </table>
        </section>
      )}
      {(diff.bootstrapsAdded.length > 0 || diff.bootstrapsRemoved.length > 0) && (
        <section>
          <h3 className="text-sm font-medium text-foreground mb-2">Bootstraps</h3>
          {diff.bootstrapsAdded.map((b) => (
            <div key={b} className="text-green-400 text-xs">
              + {b}
            </div>
          ))}
          {diff.bootstrapsRemoved.map((b) => (
            <div key={b} className="text-red-400 text-xs">
              - {b}
            </div>
          ))}
        </section>
      )}
      <section>
        <h3 className="text-sm font-medium text-foreground mb-2">Body</h3>
        <pre className="text-xs font-mono bg-surface-base p-2 overflow-x-auto max-h-96">
          {diff.bodyLines.map((l, i) => (
            <span
              // biome-ignore lint/suspicious/noArrayIndexKey: diff lines are positional
              key={i}
              className={
                l.tag === "insert"
                  ? "text-green-400 block"
                  : l.tag === "delete"
                    ? "text-red-400 block"
                    : "text-muted-foreground block"
              }
            >
              {l.tag === "insert" ? "+" : l.tag === "delete" ? "-" : " "} {l.text}
            </span>
          ))}
        </pre>
      </section>
    </div>
  );
}
