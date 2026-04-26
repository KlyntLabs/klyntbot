import { useCallback, useEffect, useMemo, useRef } from "react";
import ForceGraph3D from "react-force-graph-3d";
import { Color, Group, Mesh, MeshStandardMaterial, PointLight, TorusGeometry } from "three";
import { useBrainView } from "../hooks/useBrainView";
import type { ForceLink, ForceNode, GraphElements } from "../hooks/useGraphElements";
import type { GraphSettings } from "../hooks/useGraphSettings";
import { disposePool } from "../lib/geometryPool";

interface GraphBrainViewProps {
  elements: GraphElements;
  settings: GraphSettings;
  highlightedClusterId: string | null;
  activeNoteId: string | null;
  width: number;
  height: number;
  onNodeClick?: (id: string) => void;
  onNodeHover?: (id: string | null, x: number, y: number) => void;
  revealedNodes?: Set<string>;
  isRevealing?: boolean;
}

export function GraphBrainView({
  elements,
  settings,
  highlightedClusterId,
  activeNoteId,
  width,
  height,
  onNodeClick,
  onNodeHover,
  revealedNodes,
  isRevealing,
}: GraphBrainViewProps) {
  const { graphRef, nodeThreeObject, setupPostProcessing, resetIdleTimer } = useBrainView({
    settings,
  });

  // Dispose pooled geometries and materials when the component unmounts
  useEffect(() => {
    return () => disposePool();
  }, []);

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
          `${typeof l.source === "string" ? l.source : (l.source as ForceNode).id}-${typeof l.target === "string" ? l.target : (l.target as ForceNode).id}`,
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
      const sId = typeof link.source === "string" ? link.source : (link.source as ForceNode).id;
      const tId = typeof link.target === "string" ? link.target : (link.target as ForceNode).id;
      if (!adj.has(sId)) adj.set(sId, new Set());
      if (!adj.has(tId)) adj.set(tId, new Set());
      adj.get(sId)?.add(tId);
      adj.get(tId)?.add(sId);
    }
    adjacencyRef.current = adj;
  }, [elements.links]);

  const hoveredIdRef = useRef<string | null>(null);
  const highlightedClusterIdRef = useRef<string | null>(null);
  highlightedClusterIdRef.current = highlightedClusterId;
  const activeNoteIdRef = useRef<string | null>(null);
  activeNoteIdRef.current = activeNoteId;

  const revealedNodesRef = useRef<Set<string> | undefined>(revealedNodes);
  revealedNodesRef.current = revealedNodes;
  const isRevealingRef = useRef<boolean>(isRevealing ?? false);
  isRevealingRef.current = isRevealing ?? false;

  // Build a clusterId lookup from graphData nodes
  const clusterByNodeIdRef = useRef(new Map<string, string>());
  useEffect(() => {
    const map = new Map<string, string>();
    for (const node of elements.nodes) {
      map.set(node.id, node.clusterId);
    }
    clusterByNodeIdRef.current = map;
  }, [elements.nodes]);

  const updateNodeHighlights = useCallback(
    (hoveredId: string | null) => {
      const fg = graphRef.current;
      if (!fg) return;

      const clusterId = highlightedClusterIdRef.current;
      const neighbors = hoveredId
        ? (adjacencyRef.current.get(hoveredId) ?? new Set<string>())
        : null;

      const activeId = activeNoteIdRef.current;

      // Update each node's Three.js material opacity
      const revealed = revealedNodesRef.current;
      const revealing = isRevealingRef.current;

      fg.scene().traverse(
        (obj: { userData?: { nodeId?: string }; material?: MeshStandardMaterial }) => {
          const nodeId = obj.userData?.nodeId;
          if (!nodeId || !obj.material) return;

          const mat = obj.material as MeshStandardMaterial;
          mat.transparent = true;

          // Wave-reveal gating: hide nodes that haven't been revealed yet
          if (revealing && revealed && revealed.size > 0 && !revealed.has(nodeId)) {
            mat.opacity = 0;
            mat.emissiveIntensity = 0;
            return;
          }

          const nodeCluster = clusterByNodeIdRef.current.get(nodeId);
          const baseEmissive = mat.userData?.baseEmissive ?? 0.5;

          // Active node is always fully visible
          if (nodeId === activeId) {
            mat.opacity = 1;
            mat.emissiveIntensity = baseEmissive * 2.5;
            return;
          }

          if (hoveredId) {
            if (nodeId === hoveredId) {
              mat.opacity = 1;
              mat.emissiveIntensity = baseEmissive * 2;
            } else if (neighbors?.has(nodeId)) {
              mat.opacity = 0.9;
              mat.emissiveIntensity = baseEmissive;
            } else {
              mat.opacity = 0.1;
              mat.emissiveIntensity = 0.05;
            }
          } else if (clusterId) {
            if (nodeCluster === clusterId) {
              mat.opacity = 1;
              mat.emissiveIntensity = baseEmissive * 1.5;
            } else {
              mat.opacity = 0.1;
              mat.emissiveIntensity = 0.05;
            }
          } else {
            mat.opacity = 0.9;
            mat.emissiveIntensity = baseEmissive;
          }
        },
      );
    },
    [graphRef],
  );

  // React to legend cluster highlight or active note changes
  // biome-ignore lint/correctness/useExhaustiveDependencies: highlightedClusterId/activeNoteId trigger material update via refs
  useEffect(() => {
    updateNodeHighlights(hoveredIdRef.current);
  }, [highlightedClusterId, activeNoteId, updateNodeHighlights]);

  // React to wave-reveal changes — update scene when new nodes become revealed
  // biome-ignore lint/correctness/useExhaustiveDependencies: revealedNodes/isRevealing trigger material update via refs
  useEffect(() => {
    updateNodeHighlights(hoveredIdRef.current);
  }, [revealedNodes, isRevealing, updateNodeHighlights]);

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

  // Dynamic link color — dim non-neighbor links on hover, dim non-cluster links on cluster highlight
  const linkColor = useCallback(
    (link: { source?: ForceNode | string; target?: ForceNode | string; color?: string }) => {
      const hovId = hoveredIdRef.current;
      const clusterId = highlightedClusterIdRef.current;
      const sId = typeof link.source === "string" ? link.source : (link.source as ForceNode)?.id;
      const tId = typeof link.target === "string" ? link.target : (link.target as ForceNode)?.id;

      if (hovId) {
        const isConnected = sId === hovId || tId === hovId;
        return isConnected ? link.color || "#4B5563" : "rgba(50,50,65,0.3)";
      }
      if (clusterId) {
        const sCluster = sId ? clusterByNodeIdRef.current.get(sId) : null;
        const tCluster = tId ? clusterByNodeIdRef.current.get(tId) : null;
        const isInCluster = sCluster === clusterId || tCluster === clusterId;
        return isInCluster ? link.color || "#4B5563" : "rgba(50,50,65,0.15)";
      }
      return link.color || "#4B5563";
    },
    [],
  );

  // Dynamic link width — thicker for hovered/cluster connections
  const linkWidth = useCallback(
    (link: { source?: ForceNode | string; target?: ForceNode | string }) => {
      const hovId = hoveredIdRef.current;
      const clusterId = highlightedClusterIdRef.current;
      const sId = typeof link.source === "string" ? link.source : (link.source as ForceNode)?.id;
      const tId = typeof link.target === "string" ? link.target : (link.target as ForceNode)?.id;

      if (hovId) {
        return sId === hovId || tId === hovId ? 1.5 : 0.4;
      }
      if (clusterId) {
        const sCluster = sId ? clusterByNodeIdRef.current.get(sId) : null;
        const tCluster = tId ? clusterByNodeIdRef.current.get(tId) : null;
        return sCluster === clusterId || tCluster === clusterId ? 1.2 : 0.3;
      }
      return 0.8;
    },
    [],
  );

  // ── Active node orbiting ring indicator ───────────────────────────
  const ringRef = useRef<Group | null>(null);
  const ringFrameRef = useRef<number>(0);

  // biome-ignore lint/correctness/useExhaustiveDependencies: graphRef.current is stable after mount
  useEffect(() => {
    const fg = graphRef.current;
    if (!fg) return;

    // Clean up previous ring
    if (ringRef.current) {
      fg.scene().remove(ringRef.current);
      ringRef.current = null;
    }

    if (!activeNoteId) {
      cancelAnimationFrame(ringFrameRef.current);
      return;
    }

    const activeNode = graphData.nodes.find((n) => n.id === activeNoteId);
    const baseColor = new Color(activeNode?.color || "#a78bfa");

    const group = new Group();

    // Three rings at different radii and angles
    const rings: { mesh: Mesh; mat: MeshStandardMaterial }[] = [];
    const ringConfigs = [
      { radius: 10, tube: 0.35, emissive: 3, opacity: 0.8, tilt: 0 },
      { radius: 14, tube: 0.25, emissive: 2, opacity: 0.5, tilt: Math.PI / 3 },
      { radius: 18, tube: 0.2, emissive: 1.5, opacity: 0.3, tilt: -Math.PI / 4 },
    ];

    for (const cfg of ringConfigs) {
      const geo = new TorusGeometry(cfg.radius, cfg.tube, 8, 64);
      const mat = new MeshStandardMaterial({
        color: baseColor,
        emissive: baseColor,
        emissiveIntensity: cfg.emissive,
        transparent: true,
        opacity: cfg.opacity,
        depthWrite: false,
      });
      const mesh = new Mesh(geo, mat);
      mesh.rotation.x = cfg.tilt;
      group.add(mesh);
      rings.push({ mesh, mat });
    }

    // PointLight centered on the node — creates a volumetric glow halo
    const light = new PointLight(baseColor, 8, 60, 1.5);
    group.add(light);

    fg.scene().add(group);
    ringRef.current = group;

    // Secondary color for shimmer (complementary hue shift)
    const hsl = { h: 0, s: 0, l: 0 };
    baseColor.getHSL(hsl);
    const accentColor = new Color().setHSL((hsl.h + 0.15) % 1, hsl.s, Math.min(hsl.l + 0.1, 1));

    function animate() {
      if (!ringRef.current) return;
      const node = graphData.nodes.find((n) => n.id === activeNoteIdRef.current);
      if (node?.x != null && node?.y != null) {
        ringRef.current.position.set(node.x, node.y, (node as never as { z: number }).z ?? 0);
      }

      const t = Date.now() / 1000;
      const pulse = (Math.sin(t * 1.5) + 1) / 2;

      // Rotate each ring at different speeds
      rings[0].mesh.rotation.z = t * 0.6;
      rings[0].mesh.rotation.y = Math.sin(t * 0.2) * 0.3;
      rings[1].mesh.rotation.z = -t * 0.4;
      rings[1].mesh.rotation.x = Math.PI / 3 + Math.sin(t * 0.25) * 0.2;
      rings[2].mesh.rotation.y = t * 0.3;
      rings[2].mesh.rotation.x = -Math.PI / 4 + Math.cos(t * 0.15) * 0.15;

      // Pulse opacity and emissive on inner ring
      rings[0].mat.opacity = 0.6 + pulse * 0.4;
      rings[0].mat.emissiveIntensity = 2 + pulse * 2;

      // Shimmer: lerp ring colors between base and accent
      const lerpFactor = (Math.sin(t * 0.8) + 1) / 2;
      const currentColor = baseColor.clone().lerp(accentColor, lerpFactor * 0.4);
      for (const r of rings) {
        r.mat.emissive.copy(currentColor);
      }
      light.color.copy(currentColor);

      // Pulse light intensity
      light.intensity = 5 + pulse * 6;

      ringFrameRef.current = requestAnimationFrame(animate);
    }
    ringFrameRef.current = requestAnimationFrame(animate);

    return () => {
      cancelAnimationFrame(ringFrameRef.current);
      if (ringRef.current) {
        fg.scene().remove(ringRef.current);
        ringRef.current = null;
      }
    };
  }, [activeNoteId, graphData.nodes]);

  // Dispose Three.js renderer and WebGL context on unmount
  useEffect(() => {
    return () => {
      const fg = graphRef.current;
      if (!fg) return;
      try {
        const renderer = fg.renderer();
        if (renderer) {
          renderer.dispose();
          renderer.forceContextLoss();
        }
      } catch {
        // Renderer may already be disposed
      }
    };
  }, [graphRef]);

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
