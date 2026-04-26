import { useMemoryBrowser } from "./hooks";

export function MemoryBrowserPanel() {
  const { data, isLoading } = useMemoryBrowser();

  return (
    <div className="p-6">
      <h2 className="text-lg font-medium mb-4">Memory Browser</h2>
      {isLoading && <div className="text-muted-foreground">Loading…</div>}
      {!isLoading && (!data || data.length === 0) && (
        <div className="text-muted-foreground">No memories yet.</div>
      )}
      <div className="space-y-2">
        {data?.map((row) => (
          <div key={row.id} className="glass-surface rounded-xl p-3 text-sm">
            <div className="font-medium">
              {row.subject} → {row.predicate}
            </div>
            <div className="text-muted-foreground">{row.object}</div>
          </div>
        ))}
      </div>
    </div>
  );
}
