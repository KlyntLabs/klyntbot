interface PatternCardProps {
  name: string;
  description: string;
  domain: string;
  confidence: number;
  signalCount: number;
}

export function PatternCard({
  name,
  description,
  domain,
  confidence,
  signalCount,
}: PatternCardProps) {
  const pct = Math.round(confidence * 100);

  return (
    <div className="glass-card rounded-xl p-4">
      <div className="flex items-start justify-between mb-2">
        <h3 className="text-xs font-medium text-foreground">{name}</h3>
        <span className="text-[9px] px-1.5 py-0.5 rounded-full bg-accent/30 text-dim shrink-0">
          {domain}
        </span>
      </div>
      <p className="text-[11px] text-muted-foreground leading-relaxed mb-3">{description}</p>
      <div className="flex items-center gap-4">
        <div className="flex items-center gap-1.5">
          <div className="h-1 flex-1 rounded-full bg-accent overflow-hidden w-16">
            <div
              className="h-full rounded-full bg-primary transition-all"
              style={{ width: `${pct}%` }}
            />
          </div>
          <span className="text-2xs tabular-nums text-dim">{pct}%</span>
        </div>
        <span className="text-2xs text-dim tabular-nums">{signalCount} signals</span>
      </div>
    </div>
  );
}
