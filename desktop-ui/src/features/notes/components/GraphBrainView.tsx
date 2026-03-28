import { useCallback, useEffect, useMemo, useRef } from "react";
import ForceGraph3D from "react-force-graph-3d";
import { useBrainView } from "../hooks/useBrainView";
import type { ForceLink, ForceNode, GraphElements } from "../hooks/useGraphElements";
import type { GraphSettings } from "../hooks/useGraphSettings";

interface GraphBrainViewProps {
  elements: GraphElements;
  settings: GraphSettings;
  width: number;
  height: number;
  onNodeClick?: (id: string) => void;
  onNodeHover?: (id: string | null, x: number, y: number) => void;
}

export function GraphBrainView({
  elements,
  settings,
  width,
  height,
  onNodeClick,
  onNodeHover,
}: GraphBrainViewProps) {
  const { graphRef, nodeThreeObject, setupPostProcessing, resetIdleTimer } = useBrainView({
    settings,
  });

  // Set up bloom post-processing once the graph mounts
  const postProcessingDone = useRef(false);
  useEffect(() => {
    if (postProcessingDone.current) return;
    const timer = setTimeout(() => {
      setupPostProcessing();
      postProcessingDone.current = true;
    }, 100);
    return () => clearTimeout(timer);
  }, [setupPostProcessing]);

  // Start idle rotation timer once on mount
  const idleTimerStarted = useRef(false);
  useEffect(() => {
    if (idleTimerStarted.current) return;
    idleTimerStarted.current = true;
    resetIdleTimer();
  }, [resetIdleTimer]);

  // Stable graphData — same pattern as 2D to prevent simulation restarts
  const graphDataRef = useRef<{ nodes: ForceNode[]; links: ForceLink[] }>({ nodes: [], links: [] });
  const prevFingerprintRef = useRef("");

  const graphData = useMemo(() => {
    const nodeIds = elements.nodes.map((n) => n.id).sort().join(",");
    const linkIds = elements.links
      .map(
        (l) =>
          `${typeof l.source === "string" ? l.source : (l.source as never as ForceNode).id}-${typeof l.target === "string" ? l.target : (l.target as never as ForceNode).id}`,
      )
      .sort()
      .join(",");
    const fingerprint = `${nodeIds}|${linkIds}`;

    if (fingerprint === prevFingerprintRef.current) {
      return graphDataRef.current;
    }
    prevFingerprintRef.current = fingerprint;

    const nodes = elements.nodes.map((node) => {
      const existing = graphDataRef.current.nodes.find((n) => n.id === node.id);
      if (existing) {
        Object.assign(existing, {
          label: node.label,
          color: node.color,
          size: node.size,
          linkCount: node.linkCount,
          clusterId: node.clusterId,
        });
        return existing;
      }
      return { ...node };
    });

    const data = { nodes, links: [...elements.links] };
    graphDataRef.current = data;
    return data;
  }, [elements]);

  // Stable callbacks
  const handleNodeClick = useCallback(
    (node: { id?: string }) => {
      if (node.id && onNodeClick) onNodeClick(String(node.id));
      resetIdleTimer();
    },
    [onNodeClick, resetIdleTimer],
  );

  const handleNodeHover = useCallback(
    (node: { id?: string } | null) => {
      if (onNodeHover) {
        onNodeHover(node?.id ? String(node.id) : null, 0, 0);
      }
    },
    [onNodeHover],
  );

  const handleNodeDrag = useCallback(() => {
    resetIdleTimer();
  }, [resetIdleTimer]);

  const linkColor = useCallback((link: { color?: string }) => link.color || "#4B5563", []);

  return (
    <ForceGraph3D
      ref={graphRef as never}
      graphData={graphData as never}
      width={width}
      height={height}
      backgroundColor="rgba(0,0,0,0)"
      nodeThreeObject={nodeThreeObject as never}
      nodeThreeObjectExtend={false}
      linkColor={linkColor as never}
      linkOpacity={0.35}
      linkWidth={0.5}
      linkDirectionalParticles={settings.showArrows ? 2 : 0}
      linkDirectionalParticleSpeed={0.005}
      linkDirectionalParticleWidth={0.8}
      showNavInfo={false}
      onNodeClick={handleNodeClick as never}
      onNodeHover={handleNodeHover as never}
      onNodeDrag={handleNodeDrag as never}
      cooldownTicks={100}
      enableNodeDrag={true}
      enableNavigationControls={true}
    />
  );
}
