import { useCostRollup } from "./hooks";

export function CostTrackerPanel() {
  const { data, isLoading } = useCostRollup();

  return (
    <div className="p-6">
      <h2 className="text-lg font-medium mb-4">Cost Tracker</h2>
      {isLoading && <div className="text-muted-foreground">Loading…</div>}
      {!isLoading && (!data || data.length === 0) && (
        <div className="text-muted-foreground">No cost data yet.</div>
      )}
      <div className="space-y-3">
        {data?.map((rollup) => (
          <div key={rollup.period} className="flex items-center gap-3">
            <div className="w-24 text-sm text-muted-foreground">{rollup.period}</div>
            <div className="flex-1 h-4 bg-accent rounded-full overflow-hidden">
              <div
                className="h-full bg-primary rounded-full"
                style={{ width: `${Math.min((rollup.cost_usd / 1.0) * 100, 100)}%` }}
              />
            </div>
            <div className="w-16 text-sm text-right">${rollup.cost_usd.toFixed(2)}</div>
          </div>
        ))}
      </div>
    </div>
  );
}
