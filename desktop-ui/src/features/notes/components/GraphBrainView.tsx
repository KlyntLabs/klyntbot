import { useCallback, useEffect, useMemo, useRef } from "react";
import ForceGraph3D from "react-force-graph-3d";
import type { MeshStandardMaterial } from "three";
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
    const nodeIds = elements.nodes
      .map((n) => n.id)
      .sort()
      .join(",");
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

  // ── Hover highlight: dim non-neighbors in 3D ──────────────────────

  // Precompute adjacency for neighbor lookup
  const adjacencyRef = useRef(new Map<string, Set<string>>());
  useEffect(() => {
    const adj = new Map<string, Set<string>>();
    for (const link of elements.links) {
      const sId =
        typeof link.source === "string" ? link.source : (link.source as never as ForceNode).id;
      const tId =
        typeof link.target === "string" ? link.target : (link.target as never as ForceNode).id;
      if (!adj.has(sId)) adj.set(sId, new Set());
      if (!adj.has(tId)) adj.set(tId, new Set());
      adj.get(sId)?.add(tId);
      adj.get(tId)?.add(sId);
    }
    adjacencyRef.current = adj;
  }, [elements.links]);

  const hoveredIdRef = useRef<string | null>(null);

  const updateNodeHighlights = useCallback(
    (hoveredId: string | null) => {
      const fg = graphRef.current;
      if (!fg) return;

      const neighbors = hoveredId ? adjacencyRef.current.get(hoveredId) ?? new Set<string>() : null;

      // Update each node's Three.js material opacity
      fg.scene().traverse((obj: { userData?: { nodeId?: string }; material?: MeshStandardMaterial }) => {
        const nodeId = obj.userData?.nodeId;
        if (!nodeId || !obj.material) return;

        const mat = obj.material as MeshStandardMaterial;
        if (!hoveredId) {
          // No hover — full brightness
          mat.opacity = 0.9;
          mat.emissiveIntensity = mat.userData?.baseEmissive ?? 0.5;
        } else if (nodeId === hoveredId) {
          // Hovered node — extra bright
          mat.opacity = 1;
          mat.emissiveIntensity = (mat.userData?.baseEmissive ?? 0.5) * 2;
        } else if (neighbors?.has(nodeId)) {
          // Neighbor — normal brightness
          mat.opacity = 0.9;
          mat.emissiveIntensity = mat.userData?.baseEmissive ?? 0.5;
        } else {
          // Non-neighbor — dimmed
          mat.opacity = 0.1;
          mat.emissiveIntensity = 0.05;
        }
      });
    },
    [graphRef],
  );

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
      const id = node?.id ? String(node.id) : null;
      hoveredIdRef.current = id;
      updateNodeHighlights(id);
      onNodeHover?.(id, 0, 0);
    },
    [onNodeHover, updateNodeHighlights],
  );

  const handleNodeDrag = useCallback(() => {
    resetIdleTimer();
  }, [resetIdleTimer]);

  // Dynamic link color — dim non-neighbor links on hover
  const linkColor = useCallback(
    (link: { source?: ForceNode | string; target?: ForceNode | string; color?: string }) => {
      const hovId = hoveredIdRef.current;
      if (!hovId) return link.color || "#4B5563";

      const sId =
        typeof link.source === "string" ? link.source : (link.source as ForceNode)?.id;
      const tId =
        typeof link.target === "string" ? link.target : (link.target as ForceNode)?.id;
      const isConnected = sId === hovId || tId === hovId;
      return isConnected ? link.color || "#4B5563" : "rgba(50,50,65,0.3)";
    },
    [],
  );

  // Dynamic link width — thicker for hovered connections
  const linkWidth = useCallback(
    (link: { source?: ForceNode | string; target?: ForceNode | string }) => {
      const hovId = hoveredIdRef.current;
      if (!hovId) return 0.8;

      const sId =
        typeof link.source === "string" ? link.source : (link.source as ForceNode)?.id;
      const tId =
        typeof link.target === "string" ? link.target : (link.target as ForceNode)?.id;
      return sId === hovId || tId === hovId ? 1.5 : 0.4;
    },
    [],
  );

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
      linkOpacity={0.55}
      linkWidth={linkWidth as never}
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
