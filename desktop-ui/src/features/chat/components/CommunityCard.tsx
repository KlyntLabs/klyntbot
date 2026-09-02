import type { CommunityCardData } from "@shared/types/tree-path";

interface Props {
  community: CommunityCardData;
}

export function CommunityCard({ community }: Props) {
  const trend =
    community.stabilityTrend >= 0
      ? `+${community.stabilityTrend.toFixed(2)}`
      : community.stabilityTrend.toFixed(2);
  const trendColor = community.stabilityTrend >= 0 ? "text-status-success" : "text-status-danger";

  return (
    <div className="mt-2 rounded-lg border border-separator/50 bg-glass-subtle/30 p-2.5 text-ui-sm">
      <div className="mb-1 flex items-center justify-between">
        <span className="font-medium text-fg">{community.name}</span>
        <span className="text-fg-secondary">
          {community.sourceNoteCount} notebook
          {community.sourceNoteCount !== 1 ? "s" : ""}
          <span className={`ml-1.5 ${trendColor}`}>{trend}</span>
        </span>
      </div>
      <div className="space-y-0.5 text-fg-secondary">
        {community.representativePaths.slice(0, 3).map((path) => (
          <div key={path} className="truncate">
            {path}
          </div>
        ))}
      </div>
    </div>
  );
}
