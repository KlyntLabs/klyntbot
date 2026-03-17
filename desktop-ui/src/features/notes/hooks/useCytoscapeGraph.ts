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
  idealEdgeLength: 100,
  nodeRepulsion: 6000,
  edgeElasticity: 0.45,
  gravity: 0.25,
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
    });

    cyRef.current = cy;

    cy.on("tap", "node:childless", (evt) => onNodeClick?.(evt.target.id()));
    cy.on("dbltap", "node:childless", (evt) => onNodeDoubleClick?.(evt.target.id()));
    cy.on("tap", "node:parent", (evt) => {
      cy.animate({ fit: { eles: evt.target.children(), padding: 50 }, duration: 300 });
    });

    cy.on("mouseover", "node:childless", (evt) => {
      const node = evt.target;
      const pos = node.renderedPosition();
      onNodeHover?.(node.id(), pos.x, pos.y);
      const neighborhood = node.neighborhood().add(node);
      cy.elements().not(neighborhood).addClass("dimmed");
      neighborhood.connectedEdges().addClass("highlighted");
    });

    cy.on("mouseout", "node:childless", () => {
      onNodeHover?.(null, 0, 0);
      cy.elements().removeClass("dimmed").removeClass("highlighted");
    });

    cy.on("cxttap", "node:childless", (evt) => {
      const node = evt.target;
      const pos = node.renderedPosition();
      onNodeContext?.(node.id(), pos.x, pos.y);
    });

    cy.on("zoom", () => {
      const zoom = cy.zoom();
      const childless = cy.nodes(":childless");
      if (zoom < 0.5) {
        childless.addClass("hide-label");
      } else {
        childless.removeClass("hide-label");
      }
    });

    // Keyboard shortcuts
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

    cy.layout(FCOSE_OPTIONS).run();

    return () => {
      document.removeEventListener("keydown", handleKeyDown);
      cy.destroy();
      cyRef.current = null;
    };
  }, [containerRef, stylesheet]);

  useEffect(() => {
    const cy = cyRef.current;
    if (!cy) return;
    cy.json({ elements });
    cy.layout(FCOSE_OPTIONS).run();
  }, [elements]);

  const runLayout = useCallback(() => {
    cyRef.current?.layout(FCOSE_OPTIONS).run();
  }, []);

  return { cy: cyRef, runLayout };
}
