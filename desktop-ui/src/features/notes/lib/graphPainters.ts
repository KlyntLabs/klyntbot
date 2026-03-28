import type { ForceLink, ForceNode } from "../hooks/useGraphElements";

function hexToRgba(hex: string, alpha: number): string {
  const r = Number.parseInt(hex.slice(1, 3), 16);
  const g = Number.parseInt(hex.slice(3, 5), 16);
  const b = Number.parseInt(hex.slice(5, 7), 16);
  return `rgba(${r},${g},${b},${alpha})`;
}

export interface PaintContext {
  nodeScale: number;
  labelThreshold: number;
  hoveredNodeId: string | null;
  neighborSet: Set<string>;
  highlightedClusterId: string | null;
}

export function paintNode(
  node: ForceNode,
  ctx: CanvasRenderingContext2D,
  globalScale: number,
  paintCtx: PaintContext,
): void {
  const x = node.x ?? 0;
  const y = node.y ?? 0;
  const radius = (node.size / 2) * paintCtx.nodeScale;

  let opacity = 0.85;
  if (paintCtx.hoveredNodeId) {
    if (node.id === paintCtx.hoveredNodeId) {
      opacity = 1;
    } else if (paintCtx.neighborSet.has(node.id)) {
      opacity = 0.85;
    } else {
      opacity = 0.12;
    }
  } else if (paintCtx.highlightedClusterId) {
    opacity = node.clusterId === paintCtx.highlightedClusterId ? 0.9 : 0.12;
  }

  const prevComposite = ctx.globalCompositeOperation;
  ctx.globalCompositeOperation = "screen";
  const glowRadius = radius * 2.5;
  const gradient = ctx.createRadialGradient(x, y, radius * 0.5, x, y, glowRadius);
  gradient.addColorStop(0, hexToRgba(node.color, 0.2 * opacity));
  gradient.addColorStop(1, hexToRgba(node.color, 0));
  ctx.fillStyle = gradient;
  ctx.beginPath();
  ctx.arc(x, y, glowRadius, 0, Math.PI * 2);
  ctx.fill();
  ctx.globalCompositeOperation = prevComposite;

  ctx.fillStyle = hexToRgba(node.color, opacity);
  ctx.beginPath();
  ctx.arc(x, y, radius, 0, Math.PI * 2);
  ctx.fill();

  ctx.strokeStyle = hexToRgba(node.color, opacity * 0.3);
  ctx.lineWidth = node.id === paintCtx.hoveredNodeId ? 3 : 2;
  ctx.stroke();

  if (globalScale > paintCtx.labelThreshold) {
    const fontSize = Math.max(10 / globalScale, 3);
    ctx.font = `500 ${fontSize}px Inter, system-ui, sans-serif`;
    ctx.textAlign = "left";
    ctx.textBaseline = "middle";
    ctx.fillStyle =
      node.id === paintCtx.hoveredNodeId
        ? `rgba(255,255,255,${opacity})`
        : `rgba(255,255,255,${opacity * 0.7})`;
    ctx.fillText(node.label, x + radius + 4 / globalScale, y);
  }
}

export function paintLink(
  link: ForceLink,
  ctx: CanvasRenderingContext2D,
  _globalScale: number,
  paintCtx: PaintContext,
): void {
  const source = link.source as unknown as ForceNode;
  const target = link.target as unknown as ForceNode;
  if (!source.x || !source.y || !target.x || !target.y) return;

  let opacity = 0.35 * link.weight;
  if (paintCtx.hoveredNodeId) {
    const isConnected =
      source.id === paintCtx.hoveredNodeId || target.id === paintCtx.hoveredNodeId;
    opacity = isConnected ? 0.7 * link.weight : 0.05;
  } else if (paintCtx.highlightedClusterId) {
    const isInCluster =
      source.clusterId === paintCtx.highlightedClusterId ||
      target.clusterId === paintCtx.highlightedClusterId;
    opacity = isInCluster ? 0.5 * link.weight : 0.05;
  }

  ctx.strokeStyle = hexToRgba(link.color, Math.min(opacity, 1));
  ctx.lineWidth = Math.max(0.5, link.weight * 0.8);
  ctx.beginPath();
  ctx.moveTo(source.x, source.y);
  ctx.lineTo(target.x, target.y);
  ctx.stroke();
}
