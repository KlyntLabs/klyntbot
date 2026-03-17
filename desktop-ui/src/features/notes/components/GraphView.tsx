import { useClickOutside } from "@shared/hooks/useClickOutside";
import type { Note, Notebook } from "@shared/types";
import { Maximize2, Minus, Plus, RotateCcw, Settings2 } from "lucide-react";
import { useCallback, useRef, useState } from "react";
import { type ClusterMode, useCytoscapeElements } from "../hooks/useCytoscapeElements";
import { useCytoscapeGraph } from "../hooks/useCytoscapeGraph";
import { useCytoscapeTheme } from "../hooks/useCytoscapeTheme";
import type { SmartView } from "../hooks/useGraphData";
import { useGraphData } from "../hooks/useGraphData";
import { useGraphSettings } from "../hooks/useGraphSettings";
import { GraphLegend } from "./GraphLegend";
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
  const [clusterMode] = useState<ClusterMode>("notebook");
  const [tooltip, setTooltip] = useState<{ nodeId: string; x: number; y: number } | null>(null);
  const [hiddenClusters, setHiddenClusters] = useState<Set<string>>(new Set());

  // Settings popover
  const [settingsOpen, setSettingsOpen] = useState(false);
  const settingsRef = useRef<HTMLDivElement>(null);
  useClickOutside(settingsRef, () => setSettingsOpen(false), settingsOpen);

  const { settings, setSettings, resetSettings, defaults } = useGraphSettings();

  // Data pipeline
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

  const { elements: allElements, clusters } = useCytoscapeElements({
    nodes: filteredNodes,
    links: filteredLinks,
    notebooks,
    clusterMode,
    activeNoteId,
  });

  // Apply cluster filtering — hide elements belonging to hidden clusters
  const elements =
    hiddenClusters.size > 0
      ? allElements.filter((el) => {
          if (el.group === "edges") return true; // edges filtered by Cytoscape automatically
          const parent = el.data?.parent as string | undefined;
          if (parent && hiddenClusters.has(parent)) return false;
          // Compound parent nodes
          if (!parent && el.data?.type && hiddenClusters.has(el.data.id as string)) return false;
          return true;
        })
      : allElements;

  const { stylesheet } = useCytoscapeTheme();
  const nodeMap = new Map(filteredNodes.map((n) => [n.id, n]));

  const { cy, runLayout } = useCytoscapeGraph({
    containerRef,
    elements,
    stylesheet,
    settings,
    onNodeClick: onSelectNote,
    onNodeDoubleClick: onOpenInEditor,
    onNodeHover: useCallback((id: string | null, x: number, y: number) => {
      if (id) {
        setTooltip({ nodeId: id, x, y });
      } else {
        setTooltip(null);
      }
    }, []),
  });

  // Legend: highlight cluster
  const handleLegendHighlight = useCallback(
    (clusterId: string | null) => {
      const cyInstance = cy.current;
      if (!cyInstance) return;

      if (!clusterId) {
        cyInstance.elements().removeClass("dimmed");
        return;
      }

      const parent = cyInstance.getElementById(clusterId);
      if (parent.nonempty()) {
        const children = parent.children();
        const edges = children.connectedEdges();
        cyInstance.elements().addClass("dimmed");
        children.removeClass("dimmed");
        edges.removeClass("dimmed");
        parent.removeClass("dimmed");
      }
    },
    [cy],
  );

  // Legend: toggle cluster visibility
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

  // Zoom controls
  const zoomIn = () =>
    cy.current?.zoom({
      level: (cy.current.zoom() || 1) * 1.3,
      renderedPosition: {
        x: (containerRef.current?.clientWidth || 0) / 2,
        y: (containerRef.current?.clientHeight || 0) / 2,
      },
    });
  const zoomOut = () =>
    cy.current?.zoom({
      level: (cy.current.zoom() || 1) / 1.3,
      renderedPosition: {
        x: (containerRef.current?.clientWidth || 0) / 2,
        y: (containerRef.current?.clientHeight || 0) / 2,
      },
    });
  const fitScreen = () => cy.current?.animate({ fit: { padding: 40 }, duration: 300 });

  // Empty state
  if (notes.length === 0) {
    return (
      <div className="flex-1 flex flex-col items-center justify-center gap-2 text-muted">
        <p className="text-sm font-medium">Your knowledge graph will appear here</p>
        <p className="text-xs text-dim">Create your first note to get started</p>
      </div>
    );
  }

  return (
    <div className="flex-1 flex flex-col min-h-0">
      <GraphToolbar
        view={smartView}
        onViewChange={setSmartView}
        hopRadius={hopRadius}
        onHopRadiusChange={setHopRadius}
        searchQuery={searchQuery}
        onSearchChange={setSearchQuery}
      />

      <div
        className="flex-1 relative min-h-0 bg-background"
        style={{
          backgroundImage: "radial-gradient(circle, var(--border) 0.5px, transparent 0.5px)",
          backgroundSize: "20px 20px",
        }}
      >
        <div
          ref={containerRef}
          style={{ position: "absolute", inset: 0, width: "100%", height: "100%" }}
        />

        {/* Legend with filter */}
        <GraphLegend
          clusters={clusters}
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
              className={`w-7 h-7 glass-button flex items-center justify-center transition-colors ${
                settingsOpen ? "text-brand" : "text-secondary hover:text-primary"
              }`}
              aria-label="Graph settings"
            >
              <Settings2 size={14} />
            </button>
            {settingsOpen && (
              <div className="absolute bottom-9 right-0 glass-dropdown p-3">
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
            onClick={zoomIn}
            className="w-7 h-7 glass-button flex items-center justify-center text-secondary hover:text-primary"
            aria-label="Zoom in"
          >
            <Plus size={14} />
          </button>
          <button
            type="button"
            onClick={zoomOut}
            className="w-7 h-7 glass-button flex items-center justify-center text-secondary hover:text-primary"
            aria-label="Zoom out"
          >
            <Minus size={14} />
          </button>
          <button
            type="button"
            onClick={fitScreen}
            className="w-7 h-7 glass-button flex items-center justify-center text-secondary hover:text-primary"
            aria-label="Fit to screen"
          >
            <Maximize2 size={14} />
          </button>
          <button
            type="button"
            onClick={runLayout}
            className="w-7 h-7 glass-button flex items-center justify-center text-secondary hover:text-primary"
            aria-label="Re-layout"
          >
            <RotateCcw size={14} />
          </button>
        </div>

        {/* Tooltip */}
        {tooltip && nodeMap.has(tooltip.nodeId) && (
          <GraphNodeTooltip node={nodeMap.get(tooltip.nodeId)!} x={tooltip.x} y={tooltip.y} />
        )}
      </div>
    </div>
  );
}
