import cytoscape, { type Core, type ElementDefinition, type Stylesheet } from "cytoscape";
import fcose from "cytoscape-fcose";
import { useCallback, useEffect, useRef } from "react";

cytoscape.use(fcose);

const prefersReducedMotion =
  typeof window !== "undefined"
    ? window.matchMedia("(prefers-reduced-motion: reduce)").matches
    : false;

const FCOSE_OPTIONS = {
  name: "fcose" as const,
  animate: !prefersReducedMotion,
  animationDuration: prefersReducedMotion ? 0 : 800,
  fit: true,
  padding: 40,
  nodeSeparation: 80,
  idealEdgeLength: 120,
  nodeRepulsion: 8000,
  edgeElasticity: 0.45,
  gravity: 0.2,
  gravityRange: 1.5,
  nestingFactor: 0.1,
  numIter: 2500,
  quality: "default" as const,
};

interface UseCytoscapeGraphParams {
  containerRef: React.RefObject<HTMLDivElement | null>;
  elements: ElementDefinition[];
  stylesheet: Stylesheet[];
  onNodeClick?: (id: string) => void;
  onNodeDoubleClick?: (id: string) => void;
  onNodeHover?: (id: string | null, x: number, y: number) => void;
  onNodeContext?: (id: string, x: number, y: number) => void;
}

/** Compute a stable fingerprint of element IDs to detect real data changes */
function elementsFingerprint(elements: ElementDefinition[]): string {
  return elements.map((e) => e.data?.id || "").sort().join(",");
}

export function useCytoscapeGraph({
  containerRef,
  elements,
  stylesheet,
  onNodeClick,
  onNodeDoubleClick,
  onNodeHover,
  onNodeContext,
}: UseCytoscapeGraphParams): { cy: React.MutableRefObject<Core | null>; runLayout: () => void } {
  const cyRef = useRef<Core | null>(null);
  const prevFingerprint = useRef("");

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

    // Make compound parents non-interactive (can't grab/select them)
    cy.on("grab", "node:parent", (evt) => {
      evt.target.ungrabify();
    });

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

    // ── Zoom-adaptive labels ──
    cy.on("zoom", () => {
      const zoom = cy.zoom();
      const childless = cy.nodes(":childless");
      if (zoom < 0.5) {
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
    cy.layout(FCOSE_OPTIONS).run();

    return () => {
      document.removeEventListener("keydown", handleKeyDown);
      cy.destroy();
      cyRef.current = null;
    };
    // eslint-disable-next-line react-hooks/exhaustive-deps -- mount/unmount only
  }, [containerRef, stylesheet]);

  // ── Update elements only when data actually changes (not on hover re-renders) ──
  useEffect(() => {
    const cy = cyRef.current;
    if (!cy) return;

    const newFingerprint = elementsFingerprint(elements);
    if (newFingerprint === prevFingerprint.current) return;
    prevFingerprint.current = newFingerprint;

    cy.json({ elements });
    cy.layout(FCOSE_OPTIONS).run();
  }, [elements]);

  const runLayout = useCallback(() => {
    cyRef.current?.layout(FCOSE_OPTIONS).run();
  }, []);

  return { cy: cyRef, runLayout };
}
