interface SkillDistribution {
  skillName: string;
  count: number;
  percentage: number;
}

export interface RoutingSnapshot {
  skillDistributions: SkillDistribution[];
  totalMessages: number;
  avgConfidence: number;
  capturedAt: string;
}

interface RoutingDonutProps {
  snapshot: RoutingSnapshot | null | undefined;
}

export function RoutingDonut({ snapshot }: RoutingDonutProps) {
  if (!snapshot || snapshot.skillDistributions.length === 0) {
    return (
      <div className="glass-card rounded-xl p-5">
        <h2 className="text-[13px] font-medium text-muted-foreground mb-3">Skill Routing</h2>
        <p className="text-[11px] text-muted-foreground">
          No routing data yet. Keep chatting and the Mirror will learn your patterns.
        </p>
      </div>
    );
  }

  const avgConfidencePct = Math.round(snapshot.avgConfidence * 100);

  return (
    <div className="glass-card rounded-xl p-5">
      <h2 className="text-[13px] font-medium text-muted-foreground mb-3">Skill Routing</h2>
      <div className="flex flex-col gap-2">
        {snapshot.skillDistributions.map((skill) => (
          <div key={skill.skillName} className="flex items-center gap-3">
            <span
              className="text-[11px] text-muted-foreground w-32 shrink-0 truncate"
              title={skill.skillName}
            >
              {skill.skillName}
            </span>
            <div className="flex-1 h-1.5 rounded-full bg-accent/40 overflow-hidden">
              <div
                className="h-full rounded-full bg-brand/70 transition-all duration-500"
                style={{ width: `${skill.percentage}%` }}
              />
            </div>
            <span className="text-[11px] text-muted-foreground tabular-nums w-8 text-right shrink-0">
              {skill.percentage}%
            </span>
          </div>
        ))}
      </div>
      <p className="text-2xs text-dim mt-3">
        {snapshot.totalMessages} messages · avg confidence {avgConfidencePct}%
      </p>
    </div>
  );
}
