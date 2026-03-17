import { tagBgColor, tagColor } from "@shared/lib/tagColor";
import { createPortal } from "react-dom";
import type { GraphNode } from "../hooks/useGraphData";

interface GraphNodeTooltipProps {
  node: GraphNode;
  x: number;
  y: number;
}

export function GraphNodeTooltip({ node, x, y }: GraphNodeTooltipProps) {
  return createPortal(
    <div
      className="fixed z-[100] glass-panel rounded-xl px-3 py-2 max-w-[240px] pointer-events-none shadow-lg"
      style={{ left: x + 12, top: y + 12 }}
    >
      {/* Title */}
      <div className="text-sm font-semibold text-foreground truncate">{node.title}</div>

      {/* Body preview */}
      {node.bodyPreview && (
        <div className="text-xs text-muted-foreground mt-1 line-clamp-2">{node.bodyPreview}</div>
      )}

      {/* Tags */}
      {node.tags.length > 0 && (
        <div className="flex flex-wrap gap-1 mt-1.5">
          {node.tags.map((tag) => (
            <span
              key={tag}
              className="px-1.5 py-0.5 rounded-full text-[10px] font-medium"
              style={{
                color: tagColor(tag),
                backgroundColor: tagBgColor(tag),
              }}
            >
              {tag}
            </span>
          ))}
        </div>
      )}

      {/* Link count */}
      <div className="text-[10px] text-dim mt-1.5">
        {node.linkCount} {node.linkCount === 1 ? "link" : "links"}
      </div>
    </div>,
    document.body,
  );
}
