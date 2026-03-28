import { useClickOutside } from "@shared/hooks/useClickOutside";
import type { Note, Notebook } from "@shared/types";
import { Maximize2, Minus, Plus, RotateCcw, Settings2 } from "lucide-react";
import { useCallback, useEffect, useRef, useState } from "react";
import ForceGraph2D from "react-force-graph-2d";
import type { ViewportBounds } from "../hooks/useForceGraph";
import { useForceGraph } from "../hooks/useForceGraph";
import type { SmartView } from "../hooks/useGraphData";
import { useGraphData } from "../hooks/useGraphData";
import { useGraphElements } from "../hooks/useGraphElements";
import { useGraphPositionCache } from "../hooks/useGraphPositionCache";
import { useGraphSettings } from "../hooks/useGraphSettings";
import { useWaveReveal } from "../hooks/useWaveReveal";
import { selectHub } from "../lib/graphBfs";
import { GraphBrainView } from "./GraphBrainView";
import { GraphLegend } from "./GraphLegend";
import { GraphMinimap } from "./GraphMinimap";
import { GraphNodeTooltip } from "./GraphNodeTooltip";
import { GraphSettingsPopover } from "./GraphSettingsPopover";
import { GraphToolbar } from "./GraphToolbar";

interface GraphViewProps {
  notes: Note[];
  notebooks: Notebook[];
  activeNoteId: string | null;
  onSelectNote: (id: string) => void;
  onOpenInEditor?: (id: string) => void;
}

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

  // Settings popover
  const [settingsOpen, setSettingsOpen] = useState(false);
  const settingsRef = useRef<HTMLDivElement>(null);
  useClickOutside(settingsRef, () => setSettingsOpen(false), settingsOpen);

  const { settings, setSettings, resetSettings, defaults } = useGraphSettings();

  // ── Data pipeline ──────────────────────────────────────────────────

  const { nodes: rawNodes, links: rawLinks } = useGraphData(
    smartView,
    notes,
    activeNoteId,
    hopRadius,
  );

  // Search filter
  let filteredNodes = searchQuery
    ? rawNodes.filter((n) => n.title.toLowerCase().includes(searchQuery.toLowerCase()))
    : rawNodes;

  // Orphan filter
  if (!settings.showOrphans) {
    filteredNodes = filteredNodes.filter((n) => n.linkCount > 0);
  }

  const filteredNodeIds = new Set(filteredNodes.map((n) => n.id));
  const filteredLinks = rawLinks.filter((l) => {
    const sId = typeof l.source === "string" ? l.source : l.source.id;
    const tId = typeof l.target === "string" ? l.target : l.target.id;
    return filteredNodeIds.has(sId) && filteredNodeIds.has(tId);
  });

  // Build force-graph elements from filtered data
  const allElements = useGraphElements({
    nodes: filteredNodes,
    links: filteredLinks,
    notebooks,
    clusteringMode: settings.clusteringMode,
    activeNoteId,
  });

  // Apply cluster filtering -- hide nodes belonging to hidden clusters
  const elements =
    hiddenClusters.size > 0
      ? {
          ...allElements,
          nodes: allElements.nodes.filter((n) => !hiddenClusters.has(n.clusterId)),
          links: allElements.links.filter((l) => {
            const sId = typeof l.source === "string" ? l.source : l.source;
            const tId = typeof l.target === "string" ? l.target : l.target;
            // Re-check that both endpoints are still present after node filtering
            const visibleIds = new Set(
              allElements.nodes.filter((n) => !hiddenClusters.has(n.clusterId)).map((n) => n.id),
            );
            return visibleIds.has(sId) && visibleIds.has(tId);
          }),
        }
      : allElements;

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

  const handleSavePositions = useCallback(
    (positions: Record<string, { x: number; y: number }>) => {
      savePositions(positions);
    },
    [savePositions],
  );

  const forceGraph = useForceGraph({
    elements,
    settings,
    renderMode: settings.renderMode,
    activeNoteId,
    highlightedClusterId,
    revealedNodes: waveReveal.revealedNodes,
    cachedPositions,
    onNodeClick: onSelectNote,
    onNodeDoubleClick: onOpenInEditor,
    onSavePositions: handleSavePositions,
  });

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

  // ── Tooltip node map ───────────────────────────────────────────────
  // GraphNodeTooltip expects GraphNode (title, bodyPreview, tags, linkCount).
  // Build a lookup from the original filtered GraphNodes.
  const tooltipNodeMap = new Map(filteredNodes.map((n) => [n.id, n]));

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
                onNodeHover={forceGraph.onNodeHover as never}
                onNodeDragEnd={forceGraph.onNodeDragEnd as never}
                onBackgroundClick={forceGraph.onBackgroundClick}
                onEngineStop={forceGraph.onEngineStop}
                cooldownTicks={settings.livePhysics ? Infinity : 100}
                enableNodeDrag={true}
                enableZoomInteraction={true}
                enablePanInteraction={true}
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
              width={containerRef.current?.clientWidth ?? 800}
              height={containerRef.current?.clientHeight ?? 600}
              onNodeClick={onSelectNote}
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
        {(() => {
          const hoveredId = forceGraph.hoveredNodeId;
          if (!hoveredId) return null;
          const tooltipNode = tooltipNodeMap.get(hoveredId);
          if (!tooltipNode) return null;

          // Get screen position of hovered node from force graph
          const fg = forceGraph.graphRef.current;
          const forceNode = forceGraph.graphData.nodes.find((n) => n.id === hoveredId);
          if (!fg || !forceNode || forceNode.x == null || forceNode.y == null) return null;
          const screenPos = fg.graph2ScreenCoords(forceNode.x, forceNode.y);

          return <GraphNodeTooltip node={tooltipNode} x={screenPos.x} y={screenPos.y} />;
        })()}
      </div>
    </div>
  );
}
