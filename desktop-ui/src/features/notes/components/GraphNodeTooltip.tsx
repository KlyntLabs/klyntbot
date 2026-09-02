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
      className="fixed z-[100] glass-card rounded-xl px-4 py-3 max-w-[260px] pointer-events-none"
      style={{ left: x + 14, top: y + 14 }}
    >
      <div className="text-ui font-semibold text-fg leading-tight">{node.title}</div>

      {node.bodyPreview && (
        <div className="text-ui-sm text-fg-secondary mt-1.5 leading-relaxed line-clamp-2">
          {node.bodyPreview}
        </div>
      )}

      {node.tags.length > 0 && (
        <div className="flex flex-wrap gap-1 mt-2">
          {node.tags.map((tag) => (
            <span
              key={tag}
              className="px-1.5 py-0.5 rounded-full text-ui-xs font-medium"
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

      <div className="text-ui-xs text-fg-dim mt-2">
        {node.linkCount} {node.linkCount === 1 ? "link" : "links"}
      </div>
    </div>,
    document.body,
  );
}
