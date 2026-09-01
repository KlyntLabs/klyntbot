import { formatTokens } from "@shared/lib/format";
import { useCostBreakdown } from "./hooks";

const fmtUsd = (v: number) => `$${v.toFixed(v < 0.01 && v > 0 ? 4 : 2)}`;
const fmtNum = (n: number) => n.toLocaleString();

export function CostTrackerPanel() {
  const { data, isLoading } = useCostBreakdown(30);

  if (isLoading) {
    return <div className="p-6 text-muted-foreground">Loading cost data…</div>;
  }
  if (!data || data.rows.length === 0) {
    return (
      <div className="p-6">
        <h2 className="text-lg font-medium mb-2">Cost Tracker</h2>
        <p className="text-muted-foreground text-sm">
          No cost data yet. Costs accumulate from <code>assistantMsg</code> events with{" "}
          <code>token_usage</code> (claude-code / codex / kimi-cli) and from{" "}
          <code>klynt_sessions.total_cost_usd</code>.
        </p>
      </div>
    );
  }

  const max = data.rows.reduce((m, r) => Math.max(m, r.totalCostUsd), 0.0001);

  return (
    <div className="p-6 space-y-6">
      <div>
        <h2 className="text-lg font-medium">Cost Tracker</h2>
        <div className="text-xs text-muted-foreground mt-1">
          Last 30 days · derived from hook events + klynt-cli sessions
        </div>
      </div>

      {/* Summary tiles */}
      <div className="grid grid-cols-2 md:grid-cols-4 gap-3">
        <SummaryTile label="Total cost" value={fmtUsd(data.totalCostUsd)} accent />
        <SummaryTile label="Requests" value={fmtNum(data.totalRequests)} />
        <SummaryTile label="Input tokens" value={formatTokens(data.totalInputTokens)} />
        <SummaryTile label="Output tokens" value={formatTokens(data.totalOutputTokens)} />
        <SummaryTile label="Cached tokens" value={formatTokens(data.totalCachedTokens)} />
        <SummaryTile label="Models seen" value={String(data.modelCount)} />
        <SummaryTile label="Active days" value={String(data.dayCount)} />
        <SummaryTile label="Buckets" value={String(data.bucketCount)} />
      </div>

      {/* Per (date × model) breakdown */}
      <div className="glass-panel rounded-xl overflow-hidden">
        <div className="grid grid-cols-12 gap-2 px-4 py-2 text-[11px] text-muted-foreground uppercase tracking-wider border-b border-border">
          <div className="col-span-2">Date</div>
          <div className="col-span-3">Model</div>
          <div className="col-span-1 text-right">Reqs</div>
          <div className="col-span-1 text-right">In</div>
          <div className="col-span-1 text-right">Out</div>
          <div className="col-span-1 text-right">Cached</div>
          <div className="col-span-3 text-right">Cost</div>
        </div>

        {data.rows.map((r, i) => {
          const widthPct = Math.min((r.totalCostUsd / max) * 100, 100);
          return (
            <div
              key={`${r.date}-${r.model}-${i}`}
              className="relative grid grid-cols-12 gap-2 px-4 py-2 text-sm border-b border-border last:border-0 hover:bg-accent"
            >
              {/* Background bar showing relative cost */}
              <div
                className="absolute inset-y-0 left-0 bg-primary/10 pointer-events-none"
                style={{ width: `${widthPct}%` }}
                aria-hidden="true"
              />
              <div className="col-span-2 relative font-mono text-xs text-muted-foreground">
                {r.date}
              </div>
              <div className="col-span-3 relative">
                <div className="font-medium truncate" title={r.model}>
                  {r.model}
                </div>
                <div className="text-[10px] text-muted-foreground uppercase">{r.source}</div>
              </div>
              <div className="col-span-1 relative text-right text-muted-foreground">
                {fmtNum(r.requests)}
              </div>
              <div className="col-span-1 relative text-right">{formatTokens(r.inputTokens)}</div>
              <div className="col-span-1 relative text-right">{formatTokens(r.outputTokens)}</div>
              <div className="col-span-1 relative text-right text-muted-foreground">
                {r.cachedTokens > 0 ? formatTokens(r.cachedTokens) : "—"}
              </div>
              <div className="col-span-3 relative text-right">
                <div className="font-medium">{fmtUsd(r.totalCostUsd)}</div>
                {r.source === "hooks" && r.totalCostUsd > 0 && (
                  <div className="text-[10px] text-muted-foreground">
                    in {fmtUsd(r.inputCostUsd)} · out {fmtUsd(r.outputCostUsd)}
                  </div>
                )}
              </div>
            </div>
          );
        })}
      </div>

      <div className="text-[11px] text-muted-foreground">
        Pricing applies per-1M-tokens substring match: opus ($15/$75), sonnet ($3/$15), haiku-4
        ($1/$5), gpt-4o ($2.50/$10), gpt-4o-mini ($0.15/$0.60), deepseek ($0.27/$1.10), kimi
        ($0.15/$2.50). Unknown models contribute 0 to derived cost.
      </div>
    </div>
  );
}

function SummaryTile({ label, value, accent }: { label: string; value: string; accent?: boolean }) {
  return (
    <div className="glass-panel rounded-xl p-3">
      <div className="text-[10px] uppercase tracking-wider text-muted-foreground">{label}</div>
      <div className={`text-lg font-medium mt-1 ${accent ? "text-brand" : ""}`}>{value}</div>
    </div>
  );
}
