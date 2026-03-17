import cytoscape, { type Core, type ElementDefinition, type Stylesheet } from "cytoscape";
import fcose from "cytoscape-fcose";
import { useCallback, useEffect, useRef } from "react";
import type { GraphSettings } from "./useGraphSettings";

cytoscape.use(fcose);

interface UseCytoscapeGraphParams {
  containerRef: React.RefObject<HTMLDivElement | null>;
  elements: ElementDefinition[];
  stylesheet: Stylesheet[];
  settings: GraphSettings;
  onNodeClick?: (id: string) => void;
  onNodeDoubleClick?: (id: string) => void;
  onNodeHover?: (id: string | null, x: number, y: number) => void;
  onNodeContext?: (id: string, x: number, y: number) => void;
}

function elementsFingerprint(elements: ElementDefinition[]): string {
  return elements
    .map((e) => e.data?.id || "")
    .sort()
    .join(",");
}

function buildLayoutOptions(settings: GraphSettings) {
  return {
    name: "fcose" as const,
    animate: true,
    animationDuration: 600,
    fit: true,
    padding: 40,
    nodeSeparation: 75,
    idealEdgeLength: settings.linkDistance,
    nodeRepulsion: settings.repulsion,
    edgeElasticity: 0.45,
    gravity: settings.centerForce,
    gravityRange: 1.5,
    nestingFactor: 0.1,
    numIter: 2500,
    quality: "default" as const,
  };
}

export function useCytoscapeGraph({
  containerRef,
  elements,
  stylesheet,
  settings,
  onNodeClick,
  onNodeDoubleClick,
  onNodeHover,
  onNodeContext,
}: UseCytoscapeGraphParams): { cy: React.MutableRefObject<Core | null>; runLayout: () => void } {
  const cyRef = useRef<Core | null>(null);
  const prevFingerprint = useRef("");
  const settingsRef = useRef(settings);
  settingsRef.current = settings;

  // ── Create instance on mount ──
  useEffect(() => {
    if (!containerRef.current) return;

    const cy = cytoscape({
      container: containerRef.current,
      elements,
      style: stylesheet,
      layout: { name: "preset" },
      minZoom: 0.1,
      maxZoom: 5,
      wheelSensitivity: 0.3,
      boxSelectionEnabled: true,
      selectionType: "single",
      autoungrabify: false,
    });

    cyRef.current = cy;
    prevFingerprint.current = elementsFingerprint(elements);

    // Make compound parents fully non-interactive
    cy.nodes(":parent").ungrabify();
    cy.nodes(":parent").unselectify();

    // ── Node events ──
    cy.on("tap", "node:childless", (evt) => onNodeClick?.(evt.target.id()));
    cy.on("dbltap", "node:childless", (evt) => onNodeDoubleClick?.(evt.target.id()));

    cy.on("mouseover", "node:childless", (evt) => {
      const node = evt.target;
      const pos = node.renderedPosition();
      onNodeHover?.(node.id(), pos.x, pos.y);
      node.addClass("hovered");
      const neighborhood = node.neighborhood().add(node);
      cy.elements().not(neighborhood).addClass("dimmed");
      neighborhood.connectedEdges().addClass("highlighted");
    });

    cy.on("mouseout", "node:childless", () => {
      onNodeHover?.(null, 0, 0);
      cy.elements().removeClass("dimmed").removeClass("highlighted").removeClass("hovered");
    });

    cy.on("cxttap", "node:childless", (evt) => {
      const node = evt.target;
      const pos = node.renderedPosition();
      onNodeContext?.(node.id(), pos.x, pos.y);
    });

    // ── Drag → re-heat simulation for spring-physics feel ──
    cy.on("drag", "node:childless", () => {
      // After drag ends, run a quick partial layout on neighbors
      // to make connected nodes follow slightly
    });

    cy.on("free", "node:childless", (evt) => {
      const node = evt.target;
      // After releasing a dragged node, run a gentle re-layout
      // that keeps the dragged node fixed and lets neighbors settle
      const pos = node.position();
      const opts = buildLayoutOptions(settingsRef.current);
      cy.layout({
        ...opts,
        animate: true,
        animationDuration: 400,
        fit: false,
        fixedNodeConstraint: [{ nodeId: node.id(), position: { x: pos.x, y: pos.y } }],
      } as Record<string, unknown>).run();
    });

    // ── Zoom-adaptive labels ──
    cy.on("zoom", () => {
      const zoom = cy.zoom();
      const threshold = settingsRef.current.labelThreshold;
      const childless = cy.nodes(":childless");
      if (zoom < threshold) {
        childless.addClass("hide-label");
      } else {
        childless.removeClass("hide-label");
      }
    });

    // ── Keyboard shortcuts ──
    const handleKeyDown = (e: KeyboardEvent) => {
      if (
        document.activeElement?.tagName === "INPUT" ||
        document.activeElement?.tagName === "TEXTAREA"
      )
        return;
      switch (e.key) {
        case "+":
        case "=":
          cy.zoom({
            level: cy.zoom() * 1.2,
            renderedPosition: { x: cy.width() / 2, y: cy.height() / 2 },
          });
          break;
        case "-":
          cy.zoom({
            level: cy.zoom() / 1.2,
            renderedPosition: { x: cy.width() / 2, y: cy.height() / 2 },
          });
          break;
        case "f":
          cy.animate({ fit: { padding: 40 }, duration: 300 });
          break;
        case "Escape":
          cy.elements(":selected").unselect();
          break;
      }
    };
    document.addEventListener("keydown", handleKeyDown);

    // Run initial layout
    cy.layout(buildLayoutOptions(settings)).run();

    return () => {
      document.removeEventListener("keydown", handleKeyDown);
      cy.destroy();
      cyRef.current = null;
    };
    // eslint-disable-next-line react-hooks/exhaustive-deps -- mount/unmount only
  }, [containerRef, stylesheet]);

  // ── Update elements only when data actually changes ──
  useEffect(() => {
    const cy = cyRef.current;
    if (!cy) return;

    const newFingerprint = elementsFingerprint(elements);
    if (newFingerprint === prevFingerprint.current) return;
    prevFingerprint.current = newFingerprint;

    cy.json({ elements });
    cy.nodes(":parent").ungrabify();
    cy.nodes(":parent").unselectify();
    cy.layout(buildLayoutOptions(settingsRef.current)).run();
  }, [elements]);

  // ── Re-layout when physics settings change ──
  useEffect(() => {
    const cy = cyRef.current;
    if (!cy || cy.elements().length === 0) return;
    cy.layout(buildLayoutOptions(settings)).run();
  }, [settings.linkDistance, settings.repulsion, settings.centerForce]);

  // ── Update node sizes when nodeScale changes ──
  useEffect(() => {
    const cy = cyRef.current;
    if (!cy) return;
    cy.nodes(":childless").forEach((node) => {
      const baseSize = node.data("size") as number;
      if (baseSize) {
        const scaled = baseSize * settings.nodeScale;
        node.style({ width: scaled, height: scaled });
      }
    });
  }, [settings.nodeScale]);

  // ── Update arrow visibility ──
  useEffect(() => {
    const cy = cyRef.current;
    if (!cy) return;
    cy.edges().style({
      "target-arrow-shape": settings.showArrows ? "triangle" : "none",
    });
  }, [settings.showArrows]);

  const runLayout = useCallback(() => {
    cyRef.current?.layout(buildLayoutOptions(settingsRef.current)).run();
  }, []);

  return { cy: cyRef, runLayout };
}
