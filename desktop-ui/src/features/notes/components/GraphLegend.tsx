import { ChevronDown, ChevronUp, Eye, EyeOff } from "lucide-react";
import { useState } from "react";
import type { ClusterInfo } from "../hooks/useCytoscapeElements";

interface GraphLegendProps {
  clusters: ClusterInfo[];
  hiddenClusters: Set<string>;
  onToggleCluster: (clusterId: string) => void;
  onShowAll: () => void;
  onHighlight: (clusterId: string | null) => void;
}

export function GraphLegend({
  clusters,
  hiddenClusters,
  onToggleCluster,
  onShowAll,
  onHighlight,
}: GraphLegendProps) {
  const [collapsed, setCollapsed] = useState(false);

  if (clusters.length === 0) return null;

  const hasHidden = hiddenClusters.size > 0;

  return (
    <div className="absolute bottom-4 left-4 z-10 glass-card px-3 py-2 max-w-[240px]">
      <button
        type="button"
        onClick={() => setCollapsed(!collapsed)}
        className="flex items-center gap-1.5 text-[10px] font-semibold text-muted uppercase tracking-wider w-full"
      >
        <span>Clusters</span>
        <span className="text-dim">({clusters.length})</span>
        <span className="ml-auto">
          {collapsed ? <ChevronDown size={12} /> : <ChevronUp size={12} />}
        </span>
      </button>

      {!collapsed && (
        <div className="mt-2 space-y-0.5">
          {hasHidden && (
            <button
              type="button"
              onClick={onShowAll}
              className="flex items-center gap-2 w-full text-left px-1 py-1 rounded text-[10px] text-brand hover:bg-surface-base transition-colors mb-1"
            >
              <Eye size={10} />
              Show all clusters
            </button>
          )}

          {clusters.map((cluster) => {
            const isHidden = hiddenClusters.has(cluster.id);
            return (
              <div key={cluster.id} className="flex items-center gap-1 group">
                <button
                  type="button"
                  onClick={() => onHighlight(cluster.id)}
                  onDoubleClick={() => onHighlight(null)}
                  className={`flex items-center gap-2 flex-1 text-left px-1 py-0.5 rounded hover:bg-surface-base transition-colors ${
                    isHidden ? "opacity-40" : ""
                  }`}
                >
                  <span
                    className="w-2.5 h-2.5 rounded-full shrink-0"
                    style={{ backgroundColor: cluster.color }}
                  />
                  <span className="text-[11px] text-secondary truncate flex-1">
                    {cluster.label}
                  </span>
                  <span className="text-[10px] text-dim">{cluster.count}</span>
                </button>

                <button
                  type="button"
                  onClick={() => onToggleCluster(cluster.id)}
                  className="w-5 h-5 flex items-center justify-center rounded text-dim hover:text-secondary opacity-0 group-hover:opacity-100 transition-opacity"
                  aria-label={isHidden ? "Show cluster" : "Hide cluster"}
                >
                  {isHidden ? <EyeOff size={10} /> : <Eye size={10} />}
                </button>
              </div>
            );
          })}
        </div>
      )}
    </div>
  );
}
