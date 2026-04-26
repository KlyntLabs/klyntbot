import type { CommunityCardData } from "@shared/types/tree-path";

interface Props {
  community: CommunityCardData;
}

export function CommunityCard({ community }: Props) {
  const trend =
    community.stabilityTrend >= 0
      ? `+${community.stabilityTrend.toFixed(2)}`
      : community.stabilityTrend.toFixed(2);
  const trendColor = community.stabilityTrend >= 0 ? "text-success" : "text-destructive";

  return (
    <div className="mt-2 rounded-lg border border-border/50 bg-surface-raised/30 p-2.5 text-xs">
      <div className="mb-1 flex items-center justify-between">
        <span className="font-medium text-foreground">{community.name}</span>
        <span className="text-muted">
          {community.sourceNoteCount} notebook
          {community.sourceNoteCount !== 1 ? "s" : ""}
          <span className={`ml-1.5 ${trendColor}`}>{trend}</span>
        </span>
      </div>
      <div className="space-y-0.5 text-muted">
        {community.representativePaths.slice(0, 3).map((path) => (
          <div key={path} className="truncate">
            {path}
          </div>
        ))}
      </div>
    </div>
  );
}
