import { Map as MapIcon } from "lucide-react";
import { useCallback, useEffect, useRef, useState } from "react";
import type { ViewportBounds } from "../hooks/useForceGraph";
import type { ForceLink, ForceNode } from "../hooks/useGraphElements";

// ── Constants ────────────────────────────────────────────────────────────

const CANVAS_WIDTH = 180;
const CANVAS_HEIGHT = 120;
const NODE_RADIUS = 2;
const PADDING = 8;

// ── Props ────────────────────────────────────────────────────────────────

interface GraphMinimapProps {
  nodes: ForceNode[];
  links: ForceLink[];
  viewportBounds: ViewportBounds;
  revealedNodes: Set<string>;
  visible: boolean;
  onToggle: () => void;
  onNavigate: (graphX: number, graphY: number) => void;
}

// ── Helpers ──────────────────────────────────────────────────────────────

function hexToRgba(hex: string, alpha: number): string {
  const r = Number.parseInt(hex.slice(1, 3), 16);
  const g = Number.parseInt(hex.slice(3, 5), 16);
  const b = Number.parseInt(hex.slice(5, 7), 16);
  return `rgba(${r},${g},${b},${alpha})`;
}

function computeGraphBounds(nodes: ForceNode[]): {
  minX: number;
  minY: number;
  maxX: number;
  maxY: number;
} {
  if (nodes.length === 0) {
    return { minX: -100, minY: -100, maxX: 100, maxY: 100 };
  }

  let minX = Number.POSITIVE_INFINITY;
  let minY = Number.POSITIVE_INFINITY;
  let maxX = Number.NEGATIVE_INFINITY;
  let maxY = Number.NEGATIVE_INFINITY;

  for (const node of nodes) {
    const x = node.x ?? 0;
    const y = node.y ?? 0;
    if (x < minX) minX = x;
    if (x > maxX) maxX = x;
    if (y < minY) minY = y;
    if (y > maxY) maxY = y;
  }

  // Add a small buffer so edge nodes aren't clipped
  const bx = (maxX - minX) * 0.1 || 50;
  const by = (maxY - minY) * 0.1 || 50;
  return { minX: minX - bx, minY: minY - by, maxX: maxX + bx, maxY: maxY + by };
}

// ── Component ────────────────────────────────────────────────────────────

export function GraphMinimap({
  nodes,
  links,
  viewportBounds,
  revealedNodes,
  visible,
  onToggle,
  onNavigate,
}: GraphMinimapProps) {
  const canvasRef = useRef<HTMLCanvasElement>(null);
  const graphBoundsRef = useRef({ minX: -100, minY: -100, maxX: 100, maxY: 100 });
  const [, setRenderTick] = useState(0);

  // Recompute graph bounds when nodes change positions
  useEffect(() => {
    graphBoundsRef.current = computeGraphBounds(nodes);
    setRenderTick((t) => t + 1);
  }, [nodes]);

  // Paint the minimap canvas
  useEffect(() => {
    const canvas = canvasRef.current;
    if (!canvas || !visible) return;

    const ctx = canvas.getContext("2d");
    if (!ctx) return;

    const dpr = window.devicePixelRatio || 1;
    canvas.width = CANVAS_WIDTH * dpr;
    canvas.height = CANVAS_HEIGHT * dpr;
    ctx.scale(dpr, dpr);

    // Clear
    ctx.clearRect(0, 0, CANVAS_WIDTH, CANVAS_HEIGHT);

    const bounds = graphBoundsRef.current;
    const graphW = bounds.maxX - bounds.minX || 1;
    const graphH = bounds.maxY - bounds.minY || 1;

    // Scale to fit canvas with padding
    const drawW = CANVAS_WIDTH - PADDING * 2;
    const drawH = CANVAS_HEIGHT - PADDING * 2;
    const scale = Math.min(drawW / graphW, drawH / graphH);

    const toCanvasX = (gx: number) => PADDING + (gx - bounds.minX) * scale;
    const toCanvasY = (gy: number) => PADDING + (gy - bounds.minY) * scale;

    // Draw links
    ctx.lineWidth = 0.5;
    for (const link of links) {
      const source = link.source as unknown as ForceNode;
      const target = link.target as unknown as ForceNode;
      const sx = source.x ?? 0;
      const sy = source.y ?? 0;
      const tx = target.x ?? 0;
      const ty = target.y ?? 0;

      const isRevealed =
        revealedNodes.size === 0 || (revealedNodes.has(source.id) && revealedNodes.has(target.id));

      ctx.strokeStyle = isRevealed ? hexToRgba(link.color, 0.15) : "rgba(255,255,255,0.03)";
      ctx.beginPath();
      ctx.moveTo(toCanvasX(sx), toCanvasY(sy));
      ctx.lineTo(toCanvasX(tx), toCanvasY(ty));
      ctx.stroke();
    }

    // Draw nodes
    for (const node of nodes) {
      const x = node.x ?? 0;
      const y = node.y ?? 0;
      const isRevealed = revealedNodes.size === 0 || revealedNodes.has(node.id);

      ctx.fillStyle = isRevealed ? hexToRgba(node.color, 0.7) : "rgba(255,255,255,0.08)";
      ctx.beginPath();
      ctx.arc(toCanvasX(x), toCanvasY(y), NODE_RADIUS, 0, Math.PI * 2);
      ctx.fill();
    }

    // Draw viewport rectangle
    const vx = toCanvasX(viewportBounds.x);
    const vy = toCanvasY(viewportBounds.y);
    const vw = viewportBounds.width * scale;
    const vh = viewportBounds.height * scale;

    ctx.strokeStyle = "rgba(255,255,255,0.5)";
    ctx.lineWidth = 1;
    ctx.setLineDash([3, 2]);
    ctx.strokeRect(vx, vy, vw, vh);
    ctx.setLineDash([]);

    // Faint fill for the viewport area
    ctx.fillStyle = "rgba(255,255,255,0.04)";
    ctx.fillRect(vx, vy, vw, vh);
  }, [nodes, links, viewportBounds, revealedNodes, visible]);

  // Click to navigate
  const handleCanvasClick = useCallback(
    (e: React.MouseEvent<HTMLCanvasElement>) => {
      const canvas = canvasRef.current;
      if (!canvas) return;

      const rect = canvas.getBoundingClientRect();
      const mx = e.clientX - rect.left;
      const my = e.clientY - rect.top;

      const bounds = graphBoundsRef.current;
      const graphW = bounds.maxX - bounds.minX || 1;
      const graphH = bounds.maxY - bounds.minY || 1;
      const drawW = CANVAS_WIDTH - PADDING * 2;
      const drawH = CANVAS_HEIGHT - PADDING * 2;
      const scale = Math.min(drawW / graphW, drawH / graphH);

      const graphX = (mx - PADDING) / scale + bounds.minX;
      const graphY = (my - PADDING) / scale + bounds.minY;

      onNavigate(graphX, graphY);
    },
    [onNavigate],
  );

  return (
    <div className="absolute top-4 right-4 z-10 flex flex-col items-end gap-1">
      <button
        type="button"
        onClick={onToggle}
        className={`size-7 glass-button flex items-center justify-center transition-colors ${
          visible ? "text-brand" : "text-muted-foreground hover:text-foreground"
        }`}
        aria-label={visible ? "Hide minimap" : "Show minimap"}
      >
        <MapIcon size={14} />
      </button>

      {visible && nodes.length > 0 && (
        <div className="glass-panel rounded-lg overflow-hidden shadow-lg">
          <canvas
            ref={canvasRef}
            width={CANVAS_WIDTH}
            height={CANVAS_HEIGHT}
            style={{ width: CANVAS_WIDTH, height: CANVAS_HEIGHT, cursor: "crosshair" }}
            onClick={handleCanvasClick}
          />
        </div>
      )}
    </div>
  );
}
