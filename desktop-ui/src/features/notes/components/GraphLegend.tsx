import { ChevronDown, ChevronUp } from "lucide-react";
import { useState } from "react";
import type { ClusterInfo } from "../hooks/useCytoscapeElements";

interface GraphLegendProps {
  clusters: ClusterInfo[];
  onHighlight: (clusterId: string | null) => void;
}

export function GraphLegend({ clusters, onHighlight }: GraphLegendProps) {
  const [collapsed, setCollapsed] = useState(false);

  if (clusters.length === 0) return null;

  return (
    <div className="absolute bottom-4 left-4 z-10 glass-card px-3 py-2 max-w-[220px]">
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
        <div className="mt-2 space-y-1">
          {clusters.map((cluster) => (
            <button
              key={cluster.id}
              type="button"
              onClick={() => onHighlight(cluster.id)}
              onDoubleClick={() => onHighlight(null)}
              className="flex items-center gap-2 w-full text-left px-1 py-0.5 rounded hover:bg-surface-base transition-colors"
            >
              <span
                className="w-2.5 h-2.5 rounded-full shrink-0"
                style={{ backgroundColor: cluster.color }}
              />
              <span className="text-[11px] text-secondary truncate flex-1">{cluster.label}</span>
              <span className="text-[10px] text-dim">{cluster.count}</span>
            </button>
          ))}
        </div>
      )}
    </div>
  );
}
