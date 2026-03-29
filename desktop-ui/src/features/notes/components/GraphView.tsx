import { useClickOutside } from "@shared/hooks/useClickOutside";
import type { Note, Notebook } from "@shared/types";
import { Maximize2, Minus, Plus, RotateCcw, Settings2 } from "lucide-react";
import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import ForceGraph2D from "react-force-graph-2d";
import { useFabricGraph } from "../hooks/useFabricGraph";
import type { DragCommunityChange, ViewportBounds } from "../hooks/useForceGraph";
import { useForceGraph } from "../hooks/useForceGraph";
import type { GraphNode, SmartView } from "../hooks/useGraphData";
import { useGraphData } from "../hooks/useGraphData";
import type { FabricData, ForceNode } from "../hooks/useGraphElements";
import { useGraphElements } from "../hooks/useGraphElements";
import { useGraphPositionCache } from "../hooks/useGraphPositionCache";
import { useGraphSettings } from "../hooks/useGraphSettings";
import { useWaveReveal } from "../hooks/useWaveReveal";
import { selectHub } from "../lib/graphBfs";
import { resolveLinkEndpointId } from "../lib/graphUtils";
import { BatchActionBar } from "./FabricPulseBadge";
import { GraphBrainView } from "./GraphBrainView";
import type { ContextMenuAction } from "./GraphContextMenu";
import { GraphContextMenu } from "./GraphContextMenu";
import { GraphLegend } from "./GraphLegend";
import { GraphMinimap } from "./GraphMinimap";
import { GraphNodeTooltip } from "./GraphNodeTooltip";
import { GraphSettingsPopover } from "./GraphSettingsPopover";
import { GraphToolbar } from "./GraphToolbar";
import { QuickBridgePopover } from "./QuickBridgePopover";

/** Extract first 2-3 markdown headings from a body preview string. */
function extractHeadings(bodyPreview: string): string[] {
  if (!bodyPreview) return [];
  const headings: string[] = [];
  for (const line of bodyPreview.split("\n")) {
    const match = line.match(/^#{1,3}\s+(.+)/);
    if (match) {
      headings.push(match[1].trim());
      if (headings.length >= 3) break;
    }
  }
  return headings;
}

interface GraphViewProps {
  notes: Note[];
  notebooks: Notebook[];
  activeNoteId: string | null;
  onSelectNote: (id: string) => void;
  onOpenInEditor?: (id: string) => void;
}

const LAYER_SETTINGS_KEY: Record<"entities" | "tree", "layerEntities" | "layerTree"> = {
  entities: "layerEntities",
  tree: "layerTree",
};

export function GraphView({
  notes,
  notebooks,
  activeNoteId,
  onSelectNote,
  onOpenInEditor,
}: GraphViewProps) {
  const containerRef = useRef<HTMLDivElement>(null);

  const [smartView, setSmartView] = useState<SmartView>("full");
  const [hopRadius, setHopRadius] = useState(2);
  const [searchQuery, setSearchQuery] = useState("");
  const [hiddenClusters, setHiddenClusters] = useState<Set<string>>(new Set());
  const [highlightedClusterId, setHighlightedClusterId] = useState<string | null>(null);
  const [minimapVisible, setMinimapVisible] = useState(true);
  const [viewportBounds, setViewportBounds] = useState<ViewportBounds>({
    x: 0,
    y: 0,
    width: 100,
    height: 100,
  });

  // ── Context menu state ──────────────────────────────────────────────
  const [contextMenu, setContextMenu] = useState<{
    node: ForceNode;
    position: { x: number; y: number };
  } | null>(null);

  // ── Quick bridge state ────────────────────────────────────────────
  const [bridgeState, setBridgeState] = useState<{
    sourceName: string;
    targetName: string;
  } | null>(null);

  // ── Drag-to-merge toast ───────────────────────────────────────────
  const [dragMergeToast, setDragMergeToast] = useState<DragCommunityChange | null>(null);
  const dragMergeTimerRef = useRef<ReturnType<typeof setTimeout> | null>(null);

  // Clean up pending drag-merge timer on unmount
  useEffect(() => {
    return () => {
      if (dragMergeTimerRef.current) clearTimeout(dragMergeTimerRef.current);
    };
  }, []);

  // ── Selected fabric node (entity/tree/community — not a note) ─────
  const [selectedFabricNode, setSelectedFabricNode] = useState<ForceNode | null>(null);

  // ── Multi-select state ────────────────────────────────────────────
  const [selectedNodeIds, setSelectedNodeIds] = useState<Set<string>>(new Set());

  // Settings popover
  const [settingsOpen, setSettingsOpen] = useState(false);
  const settingsRef = useRef<HTMLDivElement>(null);
  useClickOutside(settingsRef, () => setSettingsOpen(false), settingsOpen);

  const { settings, setSettings, resetSettings, defaults } = useGraphSettings();

  // ── Fabric graph ──────────────────────────────────────────────────
  const anyLayerEnabled = settings.layerEntities || settings.layerTree;
  const fabricEnabled = anyLayerEnabled || settings.clusteringMode === "semantic";
  const fabric = useFabricGraph(fabricEnabled);

  const handleLayerToggle = useCallback(
    (layer: "entities" | "tree") => {
      const settingsKey = LAYER_SETTINGS_KEY[layer];
      const currentlyEnabled = settings[settingsKey];
      setSettings({ [settingsKey]: !currentlyEnabled });

      // When enabling a layer, fetch its data
      if (!currentlyEnabled) {
        if (layer === "entities") {
          fabric.expandLayer("entities");
        } else if (layer === "tree") {
          const noteIds = notes.map((n) => n.id);
          fabric.expandLayer("tree", noteIds);
        }
      }
    },
    [settings, setSettings, fabric, notes],
  );

  // Auto-fetch layer data on mount when layers are already enabled (from saved settings)
  const initialFetchDone = useRef(false);
  useEffect(() => {
    if (initialFetchDone.current || !fabric.base) return;
    initialFetchDone.current = true;
    if (settings.layerEntities) fabric.expandLayer("entities");
    if (settings.layerTree && notes.length > 0) {
      fabric.expandLayer(
        "tree",
        notes.map((n) => n.id),
      );
    }
  }, [fabric.base, settings.layerEntities, settings.layerTree, notes, fabric]);

  const fabricData: FabricData | undefined = fabricEnabled
    ? {
        communities: fabric.base?.communities ?? [],
        entities: fabric.entities,
        entityEdges: fabric.entityEdges,
        expandedTrees: fabric.expandedTrees,
        layerEntities: settings.layerEntities,
        layerTree: settings.layerTree,
      }
    : undefined;

  // ── Data pipeline ──────────────────────────────────────────────────

  const { nodes: rawNodes, links: rawLinks } = useGraphData(
    smartView,
    notes,
    activeNoteId,
    hopRadius,
  );

  // Inject project nodes from fabric base (projects are community members but not in the note list)
  const noteIds = new Set(rawNodes.map((n) => n.id));
  const projectNodes: GraphNode[] = (fabric.base?.notes ?? [])
    .filter((fn) => fn.tags.includes("project") && !noteIds.has(fn.id))
    .map((fn) => ({
      id: fn.id,
      title: fn.title,
      linkCount: 0,
      primaryTag: "project",
      notebookId: null,
      bodyPreview: "",
      tags: ["project"],
    }));
  const allRawNodes = projectNodes.length > 0 ? [...rawNodes, ...projectNodes] : rawNodes;

  // Orphan filter
  let filteredNodes = allRawNodes;
  if (!settings.showOrphans) {
    filteredNodes = filteredNodes.filter((n) => n.linkCount > 0 || n.tags.includes("project"));
  }

  // Search highlights matching nodes (dims others) instead of removing them
  const searchMatchIds = (() => {
    if (!searchQuery) return null;
    const q = searchQuery.toLowerCase();
    const matches = new Set<string>();
    for (const n of filteredNodes) {
      if (n.title.toLowerCase().includes(q)) matches.add(n.id);
    }
    return matches.size > 0 ? matches : null;
  })();

  // Build force-graph elements with all nodes (search just highlights)
  const allElements = useGraphElements({
    nodes: filteredNodes,
    links: rawLinks,
    notebooks,
    clusteringMode: settings.clusteringMode,
    fabricData,
  });

  // Apply cluster filtering -- hide nodes belonging to hidden clusters
  const elements = useMemo(() => {
    if (hiddenClusters.size === 0) return allElements;

    const visibleNodes = allElements.nodes.filter((n) => !hiddenClusters.has(n.clusterId));
    const visibleIds = new Set(visibleNodes.map((n) => n.id));

    return {
      ...allElements,
      nodes: visibleNodes,
      links: allElements.links.filter((l) => {
        return (
          visibleIds.has(resolveLinkEndpointId(l.source)) &&
          visibleIds.has(resolveLinkEndpointId(l.target))
        );
      }),
    };
  }, [allElements, hiddenClusters]);

  // ── Position cache ─────────────────────────────────────────────────

  const { loadPositions, savePositions, clearPositions } = useGraphPositionCache(
    smartView,
    allElements.fingerprint,
  );
  const [cachedPositions, setCachedPositions] = useState<
    Record<string, { x: number; y: number }> | null | undefined
  >(undefined);
  const [cacheReady, setCacheReady] = useState(false);

  // Load positions on mount or fingerprint change
  useEffect(() => {
    setCacheReady(false);
    setCachedPositions(undefined);
    loadPositions().then((pos) => {
      setCachedPositions(pos);
      setCacheReady(true);
    });
  }, [loadPositions]);

  // ── Wave reveal ────────────────────────────────────────────────────

  const waveReveal = useWaveReveal(settings.revealSpeed);

  // Trigger reveal on initial load when cache + elements are ready
  const hasRevealedRef = useRef(false);
  useEffect(() => {
    if (!cacheReady || elements.nodes.length === 0 || hasRevealedRef.current) return;
    hasRevealedRef.current = true;

    const hubId = selectHub(
      elements.nodes.map((n) => ({ id: n.id, linkCount: n.linkCount, title: n.label })),
      activeNoteId,
    );
    waveReveal.revealWave(hubId, elements, cachedPositions);
  }, [cacheReady, elements, activeNoteId, cachedPositions, waveReveal]);

  // Reset reveal flag when the actual graph structure changes (nodes/links added/removed).
  // smartView is intentionally excluded — view-only switches (Full→By Tag) keep the same
  // nodes visible; re-animating the wave reveal causes nodes to disappear because the
  // simulation has already cooled down and the canvas won't repaint mid-wave.
  // biome-ignore lint/correctness/useExhaustiveDependencies: fingerprint triggers reset, not read inside
  useEffect(() => {
    hasRevealedRef.current = false;
  }, [allElements.fingerprint]);

  // ── Force graph ────────────────────────────────────────────────────

  // ── Tree expansion handlers ────────────────────────────────────────

  const handleTreeExpand = useCallback(
    (noteId: string) => {
      fabric.expandLayer("tree", [noteId]);
    },
    [fabric],
  );

  const handleTreeCollapse = useCallback(
    (noteId: string) => {
      fabric.collapseTree(noteId);
    },
    [fabric],
  );

  // ── Node double-click handler (type-aware in useForceGraph) ───────

  const handleNodeDoubleClick = useCallback(
    (nodeId: string) => {
      onOpenInEditor?.(nodeId);
    },
    [onOpenInEditor],
  );

  // ── Right-click context menu handler ──────────────────────────────

  const handleNodeRightClick = useCallback(
    (node: ForceNode, screenPos: { x: number; y: number }) => {
      setContextMenu({ node, position: screenPos });
    },
    [],
  );

  // ── Drag-to-merge handler ─────────────────────────────────────────

  const handleDragCommunityChange = useCallback(
    (change: DragCommunityChange) => {
      // Clear any existing timer
      if (dragMergeTimerRef.current) clearTimeout(dragMergeTimerRef.current);

      setDragMergeToast(change);

      // Auto-dismiss after 5 seconds and trigger merge
      dragMergeTimerRef.current = setTimeout(() => {
        fabric.performAction("suggest_merge", {
          noteId: change.nodeId,
          targetCommunityId: change.targetCommunityId,
        });
        setDragMergeToast(null);
      }, 5000);
    },
    [fabric],
  );

  const handleUndoDragMerge = useCallback(() => {
    if (dragMergeTimerRef.current) {
      clearTimeout(dragMergeTimerRef.current);
      dragMergeTimerRef.current = null;
    }
    setDragMergeToast(null);
  }, []);

  // ── Multi-select batch actions ────────────────────────────────────

  const handleExpandAllTrees = useCallback(() => {
    const noteIds = [...selectedNodeIds].filter((id) => {
      const node = elements.nodes.find((n) => n.id === id);
      return node?.nodeType === "note" && node?.expandable;
    });
    if (noteIds.length > 0) fabric.expandLayer("tree", noteIds);
  }, [selectedNodeIds, elements.nodes, fabric]);

  const handleCreateBridgeFromSelection = useCallback(() => {
    const ids = [...selectedNodeIds];
    if (ids.length < 2) return;
    const first = elements.nodes.find((n) => n.id === ids[0]);
    const second = elements.nodes.find((n) => n.id === ids[1]);
    if (first && second) {
      setBridgeState({ sourceName: first.label, targetName: second.label });
    }
  }, [selectedNodeIds, elements.nodes]);

  const handleCompareInChat = useCallback(() => {
    // Select first node as active note for chat context
    const firstId = [...selectedNodeIds][0];
    if (firstId) onSelectNote(firstId);
  }, [selectedNodeIds, onSelectNote]);

  const forceGraph = useForceGraph({
    elements,
    settings,
    renderMode: settings.renderMode,
    activeNoteId,
    highlightedClusterId,
    searchMatchIds,
    revealedNodes: waveReveal.revealedNodes,
    cachedPositions,
    onNodeClick: (nodeId: string) => {
      const node = elements.nodes.find((n) => n.id === nodeId);
      if (!node || node.nodeType === "note") {
        setSelectedFabricNode(null);
        onSelectNote(nodeId);
      } else {
        setSelectedFabricNode(node);
      }
    },
    onNodeDoubleClick: handleNodeDoubleClick,
    onNodeRightClick: handleNodeRightClick,
    onSavePositions: savePositions,
    onTreeExpand: handleTreeExpand,
    onTreeCollapse: handleTreeCollapse,
    onDragCommunityChange: handleDragCommunityChange,
  });

  // ── Context menu action handler ─────────────────────────────────────

  const handleContextMenuAction = useCallback(
    (action: ContextMenuAction, node: ForceNode) => {
      switch (action) {
        case "open_in_editor":
        case "open_at_heading":
          onOpenInEditor?.(node.nodeType === "tree_section" ? node.id.split(":")[1] : node.id);
          break;
        case "expand_tree":
          fabric.expandLayer("tree", [node.id]);
          break;
        case "find_related":
          onSelectNote(node.id);
          break;
        case "quick_bridge": {
          const selectedLabel =
            selectedNodeIds.size === 1
              ? (elements.nodes.find((n) => n.id === [...selectedNodeIds][0])?.label ?? "")
              : "";
          setBridgeState({
            sourceName: node.label,
            targetName: selectedLabel,
          });
          break;
        }
        case "focus_community": {
          const fg = forceGraph.graphRef.current;
          fg?.zoomToFit(400, 40, ((n: ForceNode) => n.clusterId === node.clusterId) as never);
          break;
        }
        case "pin_to_focus":
          for (const n of forceGraph.graphData.nodes) {
            if (n.clusterId === node.clusterId && n.x != null && n.y != null) {
              n.fx = n.x;
              n.fy = n.y;
            }
          }
          break;
        case "view_members":
          setHighlightedClusterId(node.clusterId);
          break;
        case "find_across_notes":
        case "open_references":
          onSelectNote(node.id);
          break;
        case "hide_from_graph":
          setHiddenClusters((prev) => {
            const next = new Set(prev);
            next.add(node.clusterId);
            return next;
          });
          break;
        case "create_flashcard":
          onSelectNote(node.id.split(":")[1] ?? node.id);
          break;
      }
    },
    [onOpenInEditor, onSelectNote, fabric, selectedNodeIds, elements.nodes, forceGraph],
  );

  // ── Viewport bounds update (for minimap) ───────────────────────────

  useEffect(() => {
    if (!minimapVisible || !settings.showMinimap) return;
    const interval = setInterval(() => {
      setViewportBounds(forceGraph.getViewportBounds());
    }, 500);
    return () => clearInterval(interval);
  }, [minimapVisible, settings.showMinimap, forceGraph.getViewportBounds]);

  // ── Legend interactions ────────────────────────────────────────────

  const handleLegendHighlight = useCallback((clusterId: string | null) => {
    setHighlightedClusterId(clusterId);
  }, []);

  const handleToggleCluster = useCallback((clusterId: string) => {
    setHiddenClusters((prev) => {
      const next = new Set(prev);
      if (next.has(clusterId)) {
        next.delete(clusterId);
      } else {
        next.add(clusterId);
      }
      return next;
    });
  }, []);

  const handleShowAll = useCallback(() => {
    setHiddenClusters(new Set());
  }, []);

  // ── Minimap navigation ─────────────────────────────────────────────

  const handleMinimapNavigate = useCallback(
    (graphX: number, graphY: number) => {
      const fg = forceGraph.graphRef.current;
      if (!fg) return;
      fg.centerAt(graphX, graphY, 300);
    },
    [forceGraph.graphRef],
  );

  // ── Multi-select via Cmd/Ctrl + Click on canvas ────────────────────

  useEffect(() => {
    const container = containerRef.current;
    if (!container) return;

    const handleCanvasClick = (e: MouseEvent) => {
      if (!(e.metaKey || e.ctrlKey)) return;

      // Find which node was clicked using the force graph
      const fg = forceGraph.graphRef.current;
      if (!fg) return;

      const graphCoords = fg.screen2GraphCoords(e.offsetX, e.offsetY);
      // Find closest node within hit radius
      let closestNode: ForceNode | null = null;
      let closestDist = Number.POSITIVE_INFINITY;
      for (const node of forceGraph.graphData.nodes) {
        if (node.x == null || node.y == null) continue;
        const dx = node.x - graphCoords.x;
        const dy = node.y - graphCoords.y;
        const dist = Math.sqrt(dx * dx + dy * dy);
        const hitRadius = (node.size / 2) * settings.nodeScale + 4;
        if (dist < hitRadius && dist < closestDist) {
          closestDist = dist;
          closestNode = node;
        }
      }

      if (closestNode) {
        e.preventDefault();
        e.stopPropagation();
        setSelectedNodeIds((prev) => {
          const next = new Set(prev);
          if (next.has(closestNode.id)) {
            next.delete(closestNode.id);
          } else {
            next.add(closestNode.id);
          }
          return next;
        });
      }
    };

    container.addEventListener("click", handleCanvasClick, true);
    return () => container.removeEventListener("click", handleCanvasClick, true);
  }, [forceGraph, settings.nodeScale]);

  // ── Keyboard shortcuts ─────────────────────────────────────────────

  useEffect(() => {
    const handleKeyDown = (e: KeyboardEvent) => {
      // Ignore when typing in an input or textarea
      const tag = (e.target as HTMLElement)?.tagName;
      if (tag === "INPUT" || tag === "TEXTAREA" || tag === "SELECT") return;

      switch (e.key) {
        case "1":
          handleLayerToggle("entities");
          break;
        case "2":
          handleLayerToggle("tree");
          break;
        case "s":
        case "S":
          // Apply semantic preset
          if (!settings.layerEntities) handleLayerToggle("entities");
          setSettings({ clusteringMode: "semantic" });
          break;
        case "f":
        case "F":
          forceGraph.fitToScreen();
          break;
        case " ": {
          // Spacebar: toggle tree expansion on selected note
          if (activeNoteId && settings.layerTree) {
            e.preventDefault();
            const selectedNode = forceGraph.graphData.nodes.find((n) => n.id === activeNoteId);
            if (selectedNode?.expandable) {
              if (selectedNode.expanded) {
                handleTreeCollapse(activeNoteId);
              } else {
                handleTreeExpand(activeNoteId);
              }
            }
          }
          break;
        }
        case "Escape":
          setContextMenu(null);
          setSelectedNodeIds(new Set());
          setSelectedFabricNode(null);
          onSelectNote("");
          break;
      }
    };
    window.addEventListener("keydown", handleKeyDown);
    return () => window.removeEventListener("keydown", handleKeyDown);
  }, [
    handleLayerToggle,
    settings,
    setSettings,
    forceGraph,
    onSelectNote,
    activeNoteId,
    handleTreeExpand,
    handleTreeCollapse,
  ]);

  // ── Re-layout ──────────────────────────────────────────────────────

  const handleRelayout = useCallback(() => {
    clearPositions();
    forceGraph.configureForces();
    // Unpin all nodes
    for (const node of forceGraph.graphData.nodes) {
      node.fx = undefined;
      node.fy = undefined;
    }
    const fg = forceGraph.graphRef.current;
    if (fg) fg.d3ReheatSimulation();
  }, [clearPositions, forceGraph]);

  // ── Tooltip ────────────────────────────────────────────────────────
  // Compute tooltip element outside the JSX tree for clarity.
  const tooltipNodeMap = new Map(filteredNodes.map((n) => [n.id, n]));
  const tooltipElement = (() => {
    const hoveredId = forceGraph.hoveredNodeId;
    if (!hoveredId) return null;

    const fg = forceGraph.graphRef.current;
    const forceNode = forceGraph.graphData.nodes.find((n) => n.id === hoveredId);
    if (!fg || !forceNode || forceNode.x == null || forceNode.y == null) return null;
    const screenPos = fg.graph2ScreenCoords(forceNode.x, forceNode.y);

    // For note nodes, show the rich tooltip with heading breadcrumbs
    const tooltipNode = tooltipNodeMap.get(hoveredId);
    if (tooltipNode) {
      const headings = extractHeadings(tooltipNode.bodyPreview);
      const augmented =
        headings.length > 0 ? { ...tooltipNode, bodyPreview: headings.join(" > ") } : tooltipNode;
      return <GraphNodeTooltip node={augmented} x={screenPos.x} y={screenPos.y} />;
    }

    // For non-note nodes (except tree_text), build a synthetic tooltip
    if (forceNode.nodeType !== "tree_text") {
      return (
        <GraphNodeTooltip
          node={{
            id: forceNode.id,
            title: forceNode.label,
            bodyPreview: forceNode.bodyPreview,
            linkCount: forceNode.linkCount,
            tags: forceNode.tags,
            primaryTag: forceNode.tags[0] ?? null,
            notebookId: forceNode.notebookId,
          }}
          x={screenPos.x}
          y={screenPos.y}
        />
      );
    }

    return null;
  })();

  // ── Empty state ────────────────────────────────────────────────────

  if (notes.length === 0) {
    return (
      <div className="flex-1 flex flex-col items-center justify-center gap-2 text-muted-foreground">
        <p className="text-sm font-medium">Your knowledge graph will appear here</p>
        <p className="text-xs text-dim">Create your first note to get started</p>
      </div>
    );
  }

  // ── Render ─────────────────────────────────────────────────────────

  return (
    <div className="flex-1 flex flex-col min-h-0">
      <GraphToolbar
        view={smartView}
        onViewChange={setSmartView}
        hopRadius={hopRadius}
        onHopRadiusChange={setHopRadius}
        searchQuery={searchQuery}
        onSearchChange={setSearchQuery}
        clusteringMode={settings.clusteringMode}
        onClusteringModeChange={(mode) => setSettings({ clusteringMode: mode })}
        renderMode={settings.renderMode}
        onRenderModeChange={(mode) => setSettings({ renderMode: mode })}
        layerState={{
          entities: settings.layerEntities,
          tree: settings.layerTree,
        }}
        onLayerToggle={handleLayerToggle}
      />

      <div
        className="flex-1 relative min-h-0 bg-background"
        style={{
          backgroundImage: "radial-gradient(circle, var(--border) 0.5px, transparent 0.5px)",
          backgroundSize: "20px 20px",
        }}
      >
        {/* 2D Force Graph */}
        {settings.renderMode === "2d" && (
          <div
            ref={containerRef}
            style={{ position: "absolute", inset: 0, width: "100%", height: "100%" }}
          >
            {cacheReady && (
              <ForceGraph2D
                // react-force-graph-2d generics don't align with our custom ForceNode/ForceLink
                // types, but the runtime behavior is correct. Use `as never` to bypass deep
                // generic variance mismatch.
                ref={forceGraph.graphRef as never}
                graphData={forceGraph.graphData as never}
                width={containerRef.current?.clientWidth}
                height={containerRef.current?.clientHeight}
                nodeCanvasObject={forceGraph.nodeCanvasObject as never}
                nodeCanvasObjectMode={forceGraph.nodeCanvasObjectMode as never}
                linkCanvasObject={forceGraph.linkCanvasObject as never}
                linkCanvasObjectMode={forceGraph.linkCanvasObjectMode as never}
                nodePointerAreaPaint={forceGraph.nodePointerAreaPaint as never}
                onNodeClick={forceGraph.onNodeClick as never}
                onNodeRightClick={forceGraph.onNodeRightClick as never}
                onNodeHover={forceGraph.onNodeHover as never}
                onNodeDrag={forceGraph.onNodeDrag as never}
                onNodeDragEnd={forceGraph.onNodeDragEnd as never}
                onBackgroundClick={() => {
                  forceGraph.onBackgroundClick();
                  setSelectedFabricNode(null);
                }}
                onEngineStop={forceGraph.onEngineStop}
                onRenderFramePost={forceGraph.onRenderFramePost as never}
                cooldownTicks={settings.livePhysics ? Infinity : 100}
                enableNodeDrag={true}
                enableZoomInteraction={true}
                enablePanInteraction={true}
                autoPauseRedraw={false}
              />
            )}
          </div>
        )}

        {/* 3D Brain View */}
        {settings.renderMode === "3d" && (
          <div
            ref={containerRef}
            style={{ position: "absolute", inset: 0, width: "100%", height: "100%" }}
          >
            <GraphBrainView
              elements={elements}
              settings={settings}
              highlightedClusterId={highlightedClusterId}
              activeNoteId={activeNoteId}
              width={containerRef.current?.clientWidth ?? 800}
              height={containerRef.current?.clientHeight ?? 600}
              onNodeClick={(nodeId: string) => {
                const node = elements.nodes.find((n) => n.id === nodeId);
                if (!node || node.nodeType === "note") {
                  setSelectedFabricNode(null);
                  onSelectNote(nodeId);
                } else {
                  setSelectedFabricNode(node);
                }
              }}
            />
          </div>
        )}

        {/* Loading overlay while cache check completes */}
        {!cacheReady && (
          <div className="absolute inset-0 flex items-center justify-center text-muted-foreground text-sm z-20">
            Loading graph...
          </div>
        )}

        {/* Minimap (top-right) */}
        {settings.showMinimap && settings.renderMode === "2d" && (
          <GraphMinimap
            nodes={forceGraph.graphData.nodes}
            links={forceGraph.graphData.links}
            viewportBounds={viewportBounds}
            revealedNodes={waveReveal.revealedNodes}
            visible={minimapVisible}
            onToggle={() => setMinimapVisible((v) => !v)}
            onNavigate={handleMinimapNavigate}
          />
        )}

        {/* Legend with filter (bottom-left) */}
        <GraphLegend
          clusters={allElements.clusters}
          communities={allElements.communities}
          hiddenClusters={hiddenClusters}
          onToggleCluster={handleToggleCluster}
          onShowAll={handleShowAll}
          onHighlight={handleLegendHighlight}
        />

        {/* Controls (bottom-right) */}
        <div className="absolute bottom-4 right-4 z-10 flex flex-col gap-1">
          {/* Settings popover */}
          <div className="relative" ref={settingsRef}>
            <button
              type="button"
              onClick={() => setSettingsOpen(!settingsOpen)}
              className={`size-7 glass-button flex items-center justify-center transition-colors ${
                settingsOpen ? "text-brand" : "text-muted-foreground hover:text-foreground"
              }`}
              aria-label="Graph settings"
            >
              <Settings2 size={14} />
            </button>
            {settingsOpen && (
              <div className="absolute bottom-9 right-0 glass-card p-3">
                <GraphSettingsPopover
                  settings={settings}
                  defaults={defaults}
                  onChange={setSettings}
                  onReset={resetSettings}
                />
              </div>
            )}
          </div>

          <button
            type="button"
            onClick={forceGraph.zoomIn}
            className="size-7 glass-button flex items-center justify-center text-muted-foreground hover:text-foreground"
            aria-label="Zoom in"
          >
            <Plus size={14} />
          </button>
          <button
            type="button"
            onClick={forceGraph.zoomOut}
            className="size-7 glass-button flex items-center justify-center text-muted-foreground hover:text-foreground"
            aria-label="Zoom out"
          >
            <Minus size={14} />
          </button>
          <button
            type="button"
            onClick={forceGraph.fitToScreen}
            className="size-7 glass-button flex items-center justify-center text-muted-foreground hover:text-foreground"
            aria-label="Fit to screen"
          >
            <Maximize2 size={14} />
          </button>
          <button
            type="button"
            onClick={handleRelayout}
            className="size-7 glass-button flex items-center justify-center text-muted-foreground hover:text-foreground"
            aria-label="Re-layout"
          >
            <RotateCcw size={14} />
          </button>
        </div>

        {/* Tooltip */}
        {tooltipElement}

        {/* Context menu */}
        {contextMenu && (
          <GraphContextMenu
            node={contextMenu.node}
            position={contextMenu.position}
            onAction={handleContextMenuAction}
            onClose={() => setContextMenu(null)}
          />
        )}

        {/* Quick bridge popover */}
        {bridgeState && (
          <QuickBridgePopover
            sourceName={bridgeState.sourceName}
            targetName={bridgeState.targetName}
            onClose={() => setBridgeState(null)}
            onCreateNote={(title, content) => {
              fabric.performAction("create_bridge_note", { title, content });
            }}
          />
        )}

        {/* Drag-to-merge toast */}
        {dragMergeToast && (
          <div
            className="absolute top-4 left-1/2 -translate-x-1/2 z-30
              glass-panel rounded-lg px-4 py-2.5 flex items-center gap-3 shadow-xl"
          >
            <span className="text-xs text-foreground">
              Moved <span className="font-medium">{dragMergeToast.nodeLabel}</span> to{" "}
              <span className="font-medium">{dragMergeToast.targetCommunityName}</span>.
            </span>
            <button
              type="button"
              onClick={handleUndoDragMerge}
              className="text-xs font-medium text-brand hover:text-brand/80 transition-colors"
            >
              Undo
            </button>
          </div>
        )}

        {/* Multi-select batch action bar */}
        <BatchActionBar
          selectedCount={selectedNodeIds.size}
          onExpandAllTrees={handleExpandAllTrees}
          onCreateBridgeNote={handleCreateBridgeFromSelection}
          onCompareInChat={handleCompareInChat}
        />
      </div>

      {/* Fabric node detail panel (entity/tree/community) */}
      {selectedFabricNode && (
        <FabricNodePanel node={selectedFabricNode} onClose={() => setSelectedFabricNode(null)} />
      )}
    </div>
  );
}

function FabricNodePanel({ node, onClose }: { node: ForceNode; onClose: () => void }) {
  const typeLabel =
    node.nodeType === "entity" ? "Entity" : node.nodeType === "tree_section" ? "Section" : "Node";

  const typeIcon = node.nodeType === "entity" ? "diamond" : "section";

  return (
    <div className="absolute right-0 top-12 bottom-0 w-[280px] z-20 glass-panel border-l border-border/30 overflow-y-auto">
      <div className="p-4">
        <div className="flex items-center justify-between mb-3">
          <span
            className="inline-flex items-center gap-1.5 rounded-full px-2 py-0.5 text-2xs font-medium"
            style={{ backgroundColor: `${node.color}20`, color: node.color }}
          >
            {typeIcon === "diamond" && "◆ "}
            {typeIcon === "section" && "◎ "}
            {typeLabel}
          </span>
          <button
            type="button"
            onClick={onClose}
            className="text-muted-foreground hover:text-foreground text-xs"
          >
            ✕
          </button>
        </div>

        <h3 className="text-sm font-semibold text-foreground mb-2">{node.label}</h3>

        {node.bodyPreview && (
          <p className="text-xs text-muted-foreground leading-relaxed mb-3">{node.bodyPreview}</p>
        )}

        <div className="space-y-1.5 text-2xs text-muted-foreground">
          {node.nodeType === "entity" && (
            <>
              <div>
                Connections: <span className="text-foreground">{node.linkCount}</span>
              </div>
              <div>
                Type: <span className="text-foreground">{node.tags[0] || "concept"}</span>
              </div>
            </>
          )}
          {node.nodeType === "tree_section" && (
            <div>
              Level: <span className="text-foreground">{node.size > 12 ? "Section" : "Text"}</span>
            </div>
          )}
        </div>
      </div>
    </div>
  );
}
