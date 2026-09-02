import { ChevronDown, ChevronUp, Eye, EyeOff } from "lucide-react";
import { useState } from "react";
import type { ClusterInfo } from "../hooks/useGraphElements";

interface GraphLegendProps {
  clusters: ClusterInfo[];
  communities: ClusterInfo[];
  hiddenClusters: Set<string>;
  onToggleCluster: (clusterId: string) => void;
  onShowAll: () => void;
  onHighlight: (clusterId: string | null) => void;
  onCommunityClick?: (communityId: string) => void;
}

function LegendItem({
  cluster,
  isHidden,
  onHighlight,
  onToggleCluster,
  onCommunityClick,
}: {
  cluster: ClusterInfo;
  isHidden: boolean;
  onHighlight: (clusterId: string | null) => void;
  onToggleCluster: (clusterId: string) => void;
  onCommunityClick?: (communityId: string) => void;
}) {
  const isCommunity = cluster.id.startsWith("community:");
  const communityId = isCommunity ? cluster.id.replace("community:", "") : null;

  const handleClick = () => {
    onHighlight(cluster.id);
    if (communityId && onCommunityClick) {
      onCommunityClick(communityId);
    }
  };

  return (
    <div className="flex items-center gap-1 group">
      <button
        type="button"
        onClick={handleClick}
        onDoubleClick={() => onHighlight(null)}
        onMouseEnter={() => onHighlight(cluster.id)}
        onMouseLeave={() => onHighlight(null)}
        className={`flex items-center gap-2 flex-1 min-w-0 text-left px-1 py-0.5 rounded hover:bg-control-hover transition-colors ${
          isHidden ? "opacity-40" : ""
        }`}
      >
        <span
          className="size-2.5 rounded-full shrink-0"
          style={{ backgroundColor: cluster.color }}
        />
        <span className="text-ui-xs text-fg-secondary truncate flex-1 min-w-0">
          {cluster.label}
        </span>
        <span className="text-ui-xs text-fg-dim shrink-0">{cluster.count}</span>
      </button>

      <button
        type="button"
        onClick={() => onToggleCluster(cluster.id)}
        className="size-5 flex items-center justify-center rounded text-fg-dim hover:text-fg-secondary opacity-0 group-hover:opacity-100 transition-opacity"
        aria-label={isHidden ? "Show cluster" : "Hide cluster"}
      >
        {isHidden ? <EyeOff size={10} /> : <Eye size={10} />}
      </button>
    </div>
  );
}

export function GraphLegend({
  clusters,
  communities,
  hiddenClusters,
  onToggleCluster,
  onShowAll,
  onHighlight,
  onCommunityClick,
}: GraphLegendProps) {
  const [collapsed, setCollapsed] = useState(false);

  const totalCount = clusters.length + communities.length;
  if (totalCount === 0) return null;

  const hasHidden = hiddenClusters.size > 0;

  return (
    <div className="absolute bottom-4 left-4 z-10 island px-3 py-2 max-w-[240px]">
      <button
        type="button"
        onClick={() => setCollapsed(!collapsed)}
        className="flex items-center gap-1.5 text-ui-xs font-semibold text-fg-secondary uppercase tracking-wider w-full"
      >
        <span>Legend</span>
        <span className="ml-auto">
          {collapsed ? <ChevronDown size={12} /> : <ChevronUp size={12} />}
        </span>
      </button>

      {!collapsed && (
        <div className="mt-2 space-y-2">
          {hasHidden && (
            <button
              type="button"
              onClick={onShowAll}
              className="flex items-center gap-2 w-full text-left px-1 py-1 rounded text-ui-xs text-brand hover:bg-brand transition-colors"
            >
              <Eye size={10} />
              Show all
            </button>
          )}

          {/* Communities section */}
          {communities.length > 0 && (
            <div>
              <div className="text-ui-xs font-semibold text-fg-dim uppercase tracking-wider px-1 mb-1">
                Communities ({communities.length})
              </div>
              <div className="space-y-0.5">
                {communities.map((c) => (
                  <LegendItem
                    key={c.id}
                    cluster={c}
                    isHidden={hiddenClusters.has(c.id)}
                    onHighlight={onHighlight}
                    onToggleCluster={onToggleCluster}
                    onCommunityClick={onCommunityClick}
                  />
                ))}
              </div>
            </div>
          )}

          {/* Clusters section */}
          {clusters.length > 0 && (
            <div>
              {communities.length > 0 && (
                <div className="text-ui-xs font-semibold text-fg-dim uppercase tracking-wider px-1 mb-1">
                  Clusters ({clusters.length})
                </div>
              )}
              <div className="space-y-0.5">
                {clusters.map((c) => (
                  <LegendItem
                    key={c.id}
                    cluster={c}
                    isHidden={hiddenClusters.has(c.id)}
                    onHighlight={onHighlight}
                    onToggleCluster={onToggleCluster}
                  />
                ))}
              </div>
            </div>
          )}
        </div>
      )}
    </div>
  );
}
