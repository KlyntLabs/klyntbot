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
    <div className="island p-4">
      <div className="flex items-start justify-between mb-2">
        <h3 className="text-ui-sm font-medium text-fg">{name}</h3>
        <span className="text-[9px] px-1.5 py-0.5 rounded-full bg-control-hover/30 text-fg-dim shrink-0">
          {domain}
        </span>
      </div>
      <p className="text-ui-xs text-fg-secondary leading-relaxed mb-3">{description}</p>
      <div className="flex items-center gap-4">
        <div className="flex items-center gap-1.5">
          <div className="h-1 flex-1 rounded-full bg-control-hover overflow-hidden w-16">
            <div
              className="h-full rounded-full bg-brand transition-all"
              style={{ width: `${pct}%` }}
            />
          </div>
          <span className="text-ui-xs tabular-nums text-fg-dim">{pct}%</span>
        </div>
        <span className="text-ui-xs text-fg-dim tabular-nums">{signalCount} signals</span>
      </div>
    </div>
  );
}
