import cytoscape, {
  type Core,
  type ElementDefinition,
  type NodeSingular,
  type Stylesheet,
} from "cytoscape";
import { useCallback, useEffect, useRef } from "react";
import { diffElements } from "../lib/elementDiff";
import { type PositionMap, registerCytoscapePlugins, snapshotPositions } from "../lib/graphUtils";
import { useColaPhysics } from "./useColaPhysics";
import type { GraphSettings } from "./useGraphSettings";
import { useProgressiveReveal } from "./useProgressiveReveal";

registerCytoscapePlugins();

interface UseCytoscapeGraphParams {
  containerRef: React.RefObject<HTMLDivElement | null>;
  elements: ElementDefinition[];
  stylesheet: Stylesheet[];
  settings: GraphSettings;
  /** BFS waves for progressive reveal (from graphBfs.ts) */
  waves: string[][];
  /** Cached positions (null = cache miss, undefined = not yet loaded) */
  cachedPositions: PositionMap | null | undefined;
  /** Whether the cache check has completed */
  cacheReady: boolean;
  /** Callback to save positions after layout completes */
  onSavePositions: (positions: PositionMap) => void;
  onNodeClick?: (id: string) => void;
  onNodeDoubleClick?: (id: string) => void;
  onNodeHover?: (id: string | null, x: number, y: number) => void;
  onNodeContext?: (id: string, x: number, y: number) => void;
}

function buildFcoseOptions(settings: GraphSettings, overrides?: Record<string, unknown>) {
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
    ...overrides,
  };
}

export function useCytoscapeGraph({
  containerRef,
  elements,
  stylesheet,
  settings,
  waves,
  cachedPositions,
  cacheReady,
  onSavePositions,
  onNodeClick,
  onNodeDoubleClick,
  onNodeHover,
  onNodeContext,
}: UseCytoscapeGraphParams): { cy: React.MutableRefObject<Core | null>; runLayout: () => void } {
  const cyRef = useRef<Core | null>(null);
  const prevElementsRef = useRef<ElementDefinition[]>([]);
  const settingsRef = useRef(settings);
  settingsRef.current = settings;
  const initialLoadDoneRef = useRef(false);

  // Sub-hooks
  const { revealWithPositions, cancelReveal } = useProgressiveReveal();
  const { startDragCola, stopDragCola, startLivePhysics, stopLivePhysics } = useColaPhysics({
    cy: cyRef,
    settings,
    onPositionsChanged: onSavePositions,
  });

  // ── Create Cytoscape instance on mount ──
  // biome-ignore lint/correctness/useExhaustiveDependencies: mount/unmount only — callbacks accessed via ref or stable
  useEffect(() => {
    if (!containerRef.current) return;

    const cy = cytoscape({
      container: containerRef.current,
      elements: [], // Start empty — progressive reveal will add elements
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
    initialLoadDoneRef.current = false;

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

    // ── Drag → Cola physics ──
    cy.on("grab", "node:childless", (evt) => {
      startDragCola(evt.target.id());
    });

    cy.on("free", "node:childless", () => {
      stopDragCola();
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
          cy.animate({ fit: { eles: cy.elements(), padding: 40 }, duration: 300 });
          break;
        case "Escape":
          cy.elements(":selected").unselect();
          break;
      }
    };
    document.addEventListener("keydown", handleKeyDown);

    return () => {
      cancelReveal();
      document.removeEventListener("keydown", handleKeyDown);
      cy.destroy();
      cyRef.current = null;
    };
  }, [containerRef, stylesheet]);

  // ── Initial load: progressive reveal or fCoSE ──
  useEffect(() => {
    const cy = cyRef.current;
    if (!cy || elements.length === 0 || initialLoadDoneRef.current || !cacheReady) return;

    initialLoadDoneRef.current = true;
    prevElementsRef.current = elements;

    if (cachedPositions && Object.keys(cachedPositions).length > 0) {
      // Cache HIT → progressive reveal with cached positions
      revealWithPositions(cy, waves, elements, cachedPositions, {
        waveDelay: 80,
        maxWaves: 5,
        instant: settingsRef.current.instantLoad,
      });
    } else if (settingsRef.current.instantLoad) {
      // Cache MISS + instant mode → add all, run fCoSE once
      cy.add(elements);
      cy.nodes(":parent").ungrabify().unselectify();
      const layout = cy.layout(buildFcoseOptions(settingsRef.current));
      layout.on("layoutstop", () => {
        onSavePositions(snapshotPositions(cy));
      });
      layout.run();
    } else {
      // Cache MISS → progressive fCoSE: add nodes wave by wave,
      // pinning earlier waves with fixedNodeConstraint.
      const elementById = new Map<string, ElementDefinition>();
      for (const el of elements) {
        if (el.data?.id) elementById.set(el.data.id, el);
      }
      const allEdges = elements.filter((el) => el.group === "edges");
      const revealedNodes = new Set<string>();
      const maxAnimatedWaves = Math.min(waves.length, 5);

      const revealWaveFcose = (waveIndex: number) => {
        if (waveIndex >= waves.length) {
          onSavePositions(snapshotPositions(cy));
          return;
        }

        const nodeIds =
          waveIndex >= maxAnimatedWaves ? waves.slice(waveIndex).flat() : waves[waveIndex];

        const batch: ElementDefinition[] = [];

        // Add compound parents first
        for (const id of nodeIds) {
          const el = elementById.get(id);
          if (!el) continue;
          const parentId = el.data?.parent as string | undefined;
          if (parentId && !revealedNodes.has(parentId)) {
            const parentEl = elementById.get(parentId);
            if (parentEl) {
              batch.push(parentEl);
              revealedNodes.add(parentId);
            }
          }
        }

        // Add nodes
        for (const id of nodeIds) {
          const el = elementById.get(id);
          if (!el || el.group === "edges") continue;
          batch.push(el);
          revealedNodes.add(id);
        }

        // Add edges where both endpoints visible
        for (const el of allEdges) {
          const src = el.data?.source as string;
          const tgt = el.data?.target as string;
          const edgeId = el.data?.id as string;
          if (
            revealedNodes.has(src) &&
            revealedNodes.has(tgt) &&
            !cy.getElementById(edgeId).nonempty()
          ) {
            batch.push(el);
          }
        }

        if (batch.length > 0) cy.add(batch);
        cy.nodes(":parent").ungrabify().unselectify();

        // Build fixedNodeConstraint for all previously placed nodes
        const fixedConstraints: { nodeId: string; position: { x: number; y: number } }[] = [];
        cy.nodes(":childless").forEach((n) => {
          if (!nodeIds.includes(n.id())) {
            const pos = n.position();
            fixedConstraints.push({ nodeId: n.id(), position: { x: pos.x, y: pos.y } });
          }
        });

        const numIter = waveIndex <= 2 ? 2500 : waveIndex <= 4 ? 1500 : 1000;

        const layout = cy.layout(
          buildFcoseOptions(settingsRef.current, {
            fit: true,
            animate: true,
            animationDuration: 400,
            randomize: false,
            quality: "proof",
            numIter,
            fixedNodeConstraint: fixedConstraints.length > 0 ? fixedConstraints : undefined,
          }),
        );

        layout.on("layoutstop", () => {
          const isFinal = waveIndex >= maxAnimatedWaves || waveIndex >= waves.length - 1;
          if (isFinal) {
            onSavePositions(snapshotPositions(cy));
          } else {
            setTimeout(() => revealWaveFcose(waveIndex + 1), 150);
          }
        });

        layout.run();
      };

      revealWaveFcose(0);
    }
  }, [elements, cachedPositions, cacheReady, waves, revealWithPositions, onSavePositions]);

  // ── Incremental updates: element diffing ──
  useEffect(() => {
    const cy = cyRef.current;
    if (!cy || !initialLoadDoneRef.current) return;

    // Capture old elements BEFORE updating the ref (fixes stale ref bug)
    const prevElements = prevElementsRef.current;
    const diff = diffElements(prevElements, elements);
    if (!diff.hasChanges) return;

    prevElementsRef.current = elements;

    // Remove first (prevents dangling edges)
    for (const id of diff.removedEdgeIds) {
      cy.getElementById(id).remove();
    }
    for (const id of diff.removedNodeIds) {
      cy.getElementById(id).remove();
    }

    // Add compound parents before their children
    const parentNodes = diff.addedNodes.filter((el) => !el.data?.parent && el.data?.type);
    const childNodes = diff.addedNodes.filter((el) => el.data?.parent || !el.data?.type);

    if (parentNodes.length > 0) cy.add(parentNodes);

    if (childNodes.length > 0) {
      // Place new nodes near their neighbors if possible
      for (const el of childNodes) {
        const id = el.data?.id;
        if (!id) continue;
        const connectedEdges = elements.filter(
          (e) => e.group === "edges" && (e.data?.source === id || e.data?.target === id),
        );
        const neighborIds = connectedEdges
          .map((e) => (e.data?.source === id ? e.data?.target : e.data?.source))
          .filter((nid): nid is string => !!nid && cy.getElementById(nid as string).nonempty());

        if (neighborIds.length > 0) {
          let avgX = 0;
          let avgY = 0;
          for (const nid of neighborIds) {
            const pos = cy.getElementById(nid).position();
            avgX += pos.x;
            avgY += pos.y;
          }
          avgX /= neighborIds.length;
          avgY /= neighborIds.length;
          const angle = Math.random() * Math.PI * 2;
          const offset = 120 + Math.random() * 60;
          el.position = {
            x: avgX + Math.cos(angle) * offset,
            y: avgY + Math.sin(angle) * offset,
          };
        }
      }

      cy.add(childNodes);
    }

    if (diff.addedEdges.length > 0) {
      cy.add(diff.addedEdges);
    }

    cy.nodes(":parent").ungrabify().unselectify();

    // Run scoped fCoSE for new nodes only (existing stay pinned)
    if (childNodes.length > 0) {
      // Use prevElements (captured before ref update) to identify existing nodes
      const existingNodeIds = new Set(
        prevElements
          .filter((el) => el.group !== "edges")
          .map((el) => el.data?.id)
          .filter(Boolean) as string[],
      );
      const fixedConstraints = cy
        .nodes(":childless")
        .filter(
          (n) => existingNodeIds.has(n.id()) || !childNodes.some((el) => el.data?.id === n.id()),
        )
        .map((n) => {
          const node = n as NodeSingular;
          const pos = node.position();
          return { nodeId: node.id(), position: { x: pos.x, y: pos.y } };
        });

      if (fixedConstraints.length > 0) {
        const layout = cy.layout(
          buildFcoseOptions(settingsRef.current, {
            fit: false,
            animate: true,
            animationDuration: 400,
            randomize: false,
            quality: "proof",
            fixedNodeConstraint: fixedConstraints,
          }),
        );
        layout.on("layoutstop", () => {
          onSavePositions(snapshotPositions(cy));
        });
        layout.run();
      }
    } else if (diff.removedNodeIds.length > 0) {
      onSavePositions(snapshotPositions(cy));
    }
  }, [elements, onSavePositions]);

  // ── Re-layout when physics settings change ──
  // biome-ignore lint/correctness/useExhaustiveDependencies: intentionally specific deps to avoid re-layout on non-physics changes
  useEffect(() => {
    const cy = cyRef.current;
    if (!cy || cy.elements().length === 0 || !initialLoadDoneRef.current) return;
    const layout = cy.layout(buildFcoseOptions(settings));
    layout.on("layoutstop", () => {
      onSavePositions(snapshotPositions(cy));
    });
    layout.run();
  }, [settings.linkDistance, settings.repulsion, settings.centerForce, onSavePositions]);

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

  // ── Live Physics toggle ──
  useEffect(() => {
    if (settings.livePhysics) {
      startLivePhysics();
    } else {
      stopLivePhysics();
    }
  }, [settings.livePhysics, startLivePhysics, stopLivePhysics]);

  const runLayout = useCallback(() => {
    const cy = cyRef.current;
    if (!cy) return;
    const layout = cy.layout(buildFcoseOptions(settingsRef.current));
    layout.on("layoutstop", () => {
      onSavePositions(snapshotPositions(cy));
    });
    layout.run();
  }, [onSavePositions]);

  return { cy: cyRef, runLayout };
}
