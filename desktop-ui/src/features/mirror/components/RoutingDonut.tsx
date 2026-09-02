interface SkillRouteStats {
  count: number;
  percentage: number;
  avgConfidence: number;
  topTriggers: string[];
}

export interface RoutingSnapshot {
  distribution: Record<string, SkillRouteStats>;
  totalMessages: number;
  avgRoutingConfidence: number;
  lowConfidenceCount: number;
  capturedAt: string;
}

interface RoutingDonutProps {
  snapshot: RoutingSnapshot | null | undefined;
}

export function RoutingDonut({ snapshot }: RoutingDonutProps) {
  if (!snapshot || Object.keys(snapshot.distribution).length === 0) {
    return (
      <div className="glass-card rounded-xl p-5">
        <h2 className="text-ui font-medium text-fg-secondary mb-3">Skill Routing</h2>
        <p className="text-ui-xs text-fg-secondary">
          No routing data yet. Keep chatting and the Mirror will learn your patterns.
        </p>
      </div>
    );
  }

  const entries = Object.entries(snapshot.distribution).sort(([, a], [, b]) => b.count - a.count);
  const avgConfidencePct = Math.round(snapshot.avgRoutingConfidence * 100);

  return (
    <div className="glass-card rounded-xl p-5">
      <h2 className="text-ui font-medium text-fg-secondary mb-3">Skill Routing</h2>
      <div className="flex flex-col gap-2">
        {entries.map(([skillName, stats]) => (
          <div key={skillName} className="flex items-center gap-3">
            <span
              className="text-ui-xs text-fg-secondary w-32 shrink-0 truncate"
              title={skillName}
            >
              {skillName}
            </span>
            <div className="flex-1 h-1.5 rounded-full bg-control-hover/40 overflow-hidden">
              <div
                className="h-full rounded-full bg-brand/70 transition-all duration-500"
                style={{ width: `${stats.percentage}%` }}
              />
            </div>
            <span className="text-ui-xs text-fg-secondary tabular-nums w-8 text-right shrink-0">
              {Math.round(stats.percentage)}%
            </span>
          </div>
        ))}
      </div>
      <p className="text-ui-xs text-fg-dim mt-3">
        {snapshot.totalMessages} messages · avg confidence {avgConfidencePct}%
      </p>
    </div>
  );
}
