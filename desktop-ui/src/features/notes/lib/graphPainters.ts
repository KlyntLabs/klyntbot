import type { ForceLink, ForceNode } from "../hooks/useGraphElements";
import { hexToRgba } from "./graphUtils";

export interface PaintContext {
  nodeScale: number;
  labelThreshold: number;
  hoveredNodeId: string | null;
  neighborSet: Set<string>;
  highlightedClusterId: string | null;
  highlightedNodeIds: Set<string> | null;
}

/** Compute shared opacity based on hover / cluster / highlight state. */
function resolveOpacity(node: ForceNode, paintCtx: PaintContext): number {
  if (paintCtx.hoveredNodeId) {
    if (node.id === paintCtx.hoveredNodeId) return 1;
    if (paintCtx.neighborSet.has(node.id)) return 0.85;
    return 0.12;
  }
  if (paintCtx.highlightedNodeIds && paintCtx.highlightedNodeIds.size > 0) {
    return paintCtx.highlightedNodeIds.has(node.id) ? 0.95 : 0.12;
  }
  if (paintCtx.highlightedClusterId) {
    return node.clusterId === paintCtx.highlightedClusterId ? 0.9 : 0.12;
  }
  return 0.85;
}

// ── Shared glow halo helper ────────────────────────────────────────────

function paintGlow(
  ctx: CanvasRenderingContext2D,
  x: number,
  y: number,
  innerRadius: number,
  outerRadius: number,
  color: string,
  alpha: number,
  opacity: number,
): void {
  const prevComposite = ctx.globalCompositeOperation;
  ctx.globalCompositeOperation = "screen";
  const gradient = ctx.createRadialGradient(x, y, innerRadius, x, y, outerRadius);
  gradient.addColorStop(0, hexToRgba(color, alpha * opacity));
  gradient.addColorStop(1, hexToRgba(color, 0));
  ctx.fillStyle = gradient;
  ctx.beginPath();
  ctx.arc(x, y, outerRadius, 0, Math.PI * 2);
  ctx.fill();
  ctx.globalCompositeOperation = prevComposite;
}

// ── Note node (circle with glow + label) ────────────────────────────────

function paintNoteNode(
  node: ForceNode,
  ctx: CanvasRenderingContext2D,
  globalScale: number,
  paintCtx: PaintContext,
  x: number,
  y: number,
  opacity: number,
): void {
  const radius = (node.size / 2) * paintCtx.nodeScale;

  paintGlow(ctx, x, y, radius * 0.5, radius * 2.5, node.color, 0.2, opacity);

  // Circle
  ctx.fillStyle = hexToRgba(node.color, opacity);
  ctx.beginPath();
  ctx.arc(x, y, radius, 0, Math.PI * 2);
  ctx.fill();

  ctx.strokeStyle = hexToRgba(node.color, opacity * 0.3);
  ctx.lineWidth = node.id === paintCtx.hoveredNodeId ? 3 : 2;
  ctx.stroke();

  // Expand indicator (small "+" at right edge when tree layer is on)
  if (node.expandable) {
    const indicatorRadius = Math.max(4 / globalScale, 1.5);
    const ix = x + radius + indicatorRadius + 1 / globalScale;
    const iy = y;

    ctx.fillStyle = node.expanded
      ? hexToRgba(node.color, opacity * 0.9)
      : hexToRgba(node.color, opacity * 0.5);
    ctx.beginPath();
    ctx.arc(ix, iy, indicatorRadius, 0, Math.PI * 2);
    ctx.fill();

    // Draw "+" or "-" inside
    const symbolSize = indicatorRadius * 0.7;
    ctx.strokeStyle = `rgba(0,0,0,${opacity * 0.8})`;
    ctx.lineWidth = Math.max(0.8 / globalScale, 0.3);
    ctx.beginPath();
    ctx.moveTo(ix - symbolSize, iy);
    ctx.lineTo(ix + symbolSize, iy);
    if (!node.expanded) {
      ctx.moveTo(ix, iy - symbolSize);
      ctx.lineTo(ix, iy + symbolSize);
    }
    ctx.stroke();
  }

  // Label
  if (globalScale > paintCtx.labelThreshold) {
    const fontSize = Math.max(10 / globalScale, 3);
    ctx.font = `500 ${fontSize}px Inter, system-ui, sans-serif`;
    ctx.textAlign = "left";
    ctx.textBaseline = "middle";
    ctx.fillStyle =
      node.id === paintCtx.hoveredNodeId
        ? `rgba(255,255,255,${opacity})`
        : `rgba(255,255,255,${opacity * 0.7})`;
    const labelX = node.expandable ? x + radius + 12 / globalScale : x + radius + 4 / globalScale;
    ctx.fillText(node.label, labelX, y);
  }
}

// ── Entity node (diamond / rotated square) ──────────────────────────────

function paintEntityNode(
  node: ForceNode,
  ctx: CanvasRenderingContext2D,
  globalScale: number,
  paintCtx: PaintContext,
  x: number,
  y: number,
  opacity: number,
): void {
  // Size 12-24px based on linkCount (mentionCount mapped to linkCount)
  const half = (Math.min(12 + Math.min(node.linkCount, 10) * 1.2, 24) / 2) * paintCtx.nodeScale;

  paintGlow(ctx, x, y, half * 0.3, half * 2.5, node.color, 0.15, opacity);

  // Diamond (rotated 45-degree square)
  ctx.fillStyle = hexToRgba(node.color, opacity);
  ctx.beginPath();
  ctx.moveTo(x, y - half); // top
  ctx.lineTo(x + half, y); // right
  ctx.lineTo(x, y + half); // bottom
  ctx.lineTo(x - half, y); // left
  ctx.closePath();
  ctx.fill();

  ctx.strokeStyle = hexToRgba(node.color, opacity * 0.4);
  ctx.lineWidth = node.id === paintCtx.hoveredNodeId ? 2.5 : 1.5;
  ctx.stroke();

  // Label below
  if (globalScale > paintCtx.labelThreshold) {
    const fontSize = Math.max(9 / globalScale, 2.5);
    ctx.font = `500 ${fontSize}px Inter, system-ui, sans-serif`;
    ctx.textAlign = "center";
    ctx.textBaseline = "top";
    ctx.fillStyle =
      node.id === paintCtx.hoveredNodeId
        ? `rgba(255,255,255,${opacity})`
        : `rgba(255,255,255,${opacity * 0.65})`;
    ctx.fillText(node.label, x, y + half + 3 / globalScale);
  }
}

// ── Tree section node (small circle, parent color at 60% opacity) ───────

function paintTreeSectionNode(
  node: ForceNode,
  ctx: CanvasRenderingContext2D,
  globalScale: number,
  paintCtx: PaintContext,
  x: number,
  y: number,
  opacity: number,
): void {
  // Size 10-16px based on level (stored in size from useGraphElements)
  const radius = (node.size / 2) * paintCtx.nodeScale;

  ctx.fillStyle = hexToRgba(node.color, opacity * 0.6);
  ctx.beginPath();
  ctx.arc(x, y, radius, 0, Math.PI * 2);
  ctx.fill();

  ctx.strokeStyle = hexToRgba(node.color, opacity * 0.2);
  ctx.lineWidth = 1;
  ctx.stroke();

  // Heading title label
  if (globalScale > paintCtx.labelThreshold) {
    const fontSize = Math.max(8 / globalScale, 2.5);
    ctx.font = `400 ${fontSize}px Inter, system-ui, sans-serif`;
    ctx.textAlign = "left";
    ctx.textBaseline = "middle";
    ctx.fillStyle = `rgba(255,255,255,${opacity * 0.55})`;
    ctx.fillText(node.label, x + radius + 3 / globalScale, y);
  }
}

// ── Tree text node (tiny dot, no label) ─────────────────────────────────

function paintTreeTextNode(
  node: ForceNode,
  ctx: CanvasRenderingContext2D,
  _globalScale: number,
  paintCtx: PaintContext,
  x: number,
  y: number,
  opacity: number,
): void {
  const radius = 3 * paintCtx.nodeScale;

  ctx.fillStyle = hexToRgba(node.color, opacity * 0.3);
  ctx.beginPath();
  ctx.arc(x, y, radius, 0, Math.PI * 2);
  ctx.fill();
}

// ── Community label node (rounded rect with name + count) ───────────────

function paintCommunityLabelNode(
  node: ForceNode,
  ctx: CanvasRenderingContext2D,
  globalScale: number,
  _paintCtx: PaintContext,
  x: number,
  y: number,
  opacity: number,
): void {
  const fontSize = Math.max(9 / globalScale, 3);
  ctx.font = `600 ${fontSize}px Inter, system-ui, sans-serif`;
  const text = node.label;
  const metrics = ctx.measureText(text);
  const padX = 6 / globalScale;
  const padY = 3 / globalScale;
  const rectW = metrics.width + padX * 2;
  const rectH = fontSize + padY * 2;
  const cornerRadius = 3 / globalScale;

  // Rounded rect background
  ctx.fillStyle = hexToRgba(node.color, 0.2 * opacity);
  ctx.beginPath();
  ctx.roundRect(x - rectW / 2, y - rectH / 2, rectW, rectH, cornerRadius);
  ctx.fill();

  ctx.strokeStyle = hexToRgba(node.color, 0.3 * opacity);
  ctx.lineWidth = 0.5;
  ctx.stroke();

  // Text
  ctx.textAlign = "center";
  ctx.textBaseline = "middle";
  ctx.fillStyle = hexToRgba(node.color, 0.9 * opacity);
  ctx.fillText(text, x, y);
}

// ── Main paintNode dispatcher ───────────────────────────────────────────

export function paintNode(
  node: ForceNode,
  ctx: CanvasRenderingContext2D,
  globalScale: number,
  paintCtx: PaintContext,
): void {
  const x = node.x ?? 0;
  const y = node.y ?? 0;
  const opacity = resolveOpacity(node, paintCtx);

  switch (node.nodeType) {
    case "entity":
      paintEntityNode(node, ctx, globalScale, paintCtx, x, y, opacity);
      break;
    case "tree_section":
      paintTreeSectionNode(node, ctx, globalScale, paintCtx, x, y, opacity);
      break;
    case "tree_text":
      paintTreeTextNode(node, ctx, globalScale, paintCtx, x, y, opacity);
      break;
    case "community_label":
      paintCommunityLabelNode(node, ctx, globalScale, paintCtx, x, y, opacity);
      break;
    default:
      paintNoteNode(node, ctx, globalScale, paintCtx, x, y, opacity);
      break;
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

  let opacity = 0.55 * link.weight;
  if (paintCtx.hoveredNodeId) {
    const isConnected =
      source.id === paintCtx.hoveredNodeId || target.id === paintCtx.hoveredNodeId;
    opacity = isConnected ? 0.85 * link.weight : 0.1;
  } else if (paintCtx.highlightedClusterId) {
    const isInCluster =
      source.clusterId === paintCtx.highlightedClusterId ||
      target.clusterId === paintCtx.highlightedClusterId;
    opacity = isInCluster ? 0.65 * link.weight : 0.1;
  }

  ctx.strokeStyle = hexToRgba(link.color, Math.min(opacity, 1));
  ctx.lineWidth = Math.max(0.8, link.weight * 1.2);
  ctx.beginPath();
  ctx.moveTo(source.x, source.y);
  ctx.lineTo(target.x, target.y);
  ctx.stroke();
}
