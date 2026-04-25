import { useSensitivityInspector } from "./hooks";

export function SensitivityInspectorPanel() {
  const { data, isLoading } = useSensitivityInspector();

  return (
    <div className="p-6">
      <h2 className="text-lg font-medium mb-4">Sensitivity Inspector</h2>
      {isLoading && <div className="text-muted-foreground">Loading…</div>}
      {!isLoading && (!data || data.length === 0) && (
        <div className="text-muted-foreground">No facts to inspect.</div>
      )}
      <div className="space-y-2">
        {data?.map((fact) => (
          <div
            key={fact.id}
            className="glass-surface rounded-xl p-3 text-sm flex items-center justify-between"
          >
            <div>
              <div className="font-medium">
                {fact.subject} → {fact.predicate}
              </div>
              <div className="text-muted-foreground">{fact.object}</div>
            </div>
            <div className="px-2 py-1 rounded bg-accent text-xs uppercase">{fact.sensitivity}</div>
          </div>
        ))}
      </div>
    </div>
  );
}
