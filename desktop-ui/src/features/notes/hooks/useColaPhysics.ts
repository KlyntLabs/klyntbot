import type { Core, Layouts, NodeSingular } from "cytoscape";
import { useCallback, useEffect, useRef } from "react";
import { snapshotPositions, type PositionMap } from "../lib/graphUtils";
import type { GraphSettings } from "./useGraphSettings";

const SETTLE_DURATION_MS = 300;
const IDLE_TIMEOUT_MS = 30_000;
const HUB_CONNECTION_CAP = 8;

interface UseColaPhysicsParams {
  cy: React.MutableRefObject<Core | null>;
  settings: GraphSettings;
  onPositionsChanged: (positions: PositionMap) => void;
}

function getNeighborhood(cy: Core, nodeId: string, hops: number): Set<string> {
  const visited = new Set<string>();
  let frontier = [nodeId];
  visited.add(nodeId);

  for (let i = 0; i < hops; i++) {
    const nextFrontier: string[] = [];
    for (const id of frontier) {
      const node = cy.getElementById(id);
      if (node.empty()) continue;
      node.neighborhood("node:childless").forEach((n) => {
        if (!visited.has(n.id())) {
          visited.add(n.id());
          nextFrontier.push(n.id());
        }
      });
    }
    frontier = nextFrontier;
  }

  return visited;
}

/**
 * Cola physics hook. Provides:
 * - Auto-activate on drag (scoped to N-hop neighborhood)
 * - Live Physics mode (continuous simulation on visible nodes)
 */
export function useColaPhysics({
  cy: cyRef,
  settings,
  onPositionsChanged,
}: UseColaPhysicsParams) {
  const activeLayoutRef = useRef<Layouts | null>(null);
  const settingsRef = useRef(settings);
  settingsRef.current = settings;
  const idleTimerRef = useRef<number | null>(null);
  const livePhysicsActiveRef = useRef(false);

  const stopActiveLayout = useCallback(() => {
    if (activeLayoutRef.current) {
      activeLayoutRef.current.stop();
      activeLayoutRef.current = null;
    }
    const cy = cyRef.current;
    if (cy) {
      cy.nodes().unlock();
      cy.nodes(":childless").removeClass("cola-dragging cola-neighbor");
    }
  }, [cyRef]);

  /**
   * Run scoped Cola on drag: lock all nodes except the neighborhood,
   * start Cola with infinite: true, stop after release + settle.
   */
  const startDragCola = useCallback(
    (draggedNodeId: string) => {
      const cy = cyRef.current;
      if (!cy) return;

      stopActiveLayout();

      const totalNodes = cy.nodes(":childless").length;
      const hops = totalNodes >= 800 ? 1 : 2;
      let scope = getNeighborhood(cy, draggedNodeId, hops);

      // Cap hub connections
      if (scope.size > HUB_CONNECTION_CAP + 1) {
        const draggedNode = cy.getElementById(draggedNodeId);
        const neighbors = draggedNode
          .neighborhood("node:childless")
          .sort((a, b) => {
            const wA = (a as NodeSingular).connectedEdges().reduce((sum: number, e) => sum + ((e.data("weight") as number) || 1), 0);
            const wB = (b as NodeSingular).connectedEdges().reduce((sum: number, e) => sum + ((e.data("weight") as number) || 1), 0);
            return wB - wA;
          })
          .toArray()
          .slice(0, HUB_CONNECTION_CAP) as NodeSingular[];

        scope = new Set([draggedNodeId]);
        neighbors.forEach((n) => scope.add(n.id()));
      }

      // Lock all nodes outside scope
      cy.nodes(":childless").forEach((node) => {
        if (!scope.has(node.id())) {
          node.lock();
        }
      });

      // Visual feedback
      cy.getElementById(draggedNodeId).addClass("cola-dragging");
      scope.forEach((id) => {
        if (id !== draggedNodeId) {
          cy.getElementById(id).addClass("cola-neighbor");
        }
      });

      // Start Cola
      const s = settingsRef.current;
      // biome-ignore lint/suspicious/noExplicitAny: cytoscape-cola options not typed
      const layout = cy.layout({
        name: "cola",
        infinite: true,
        fit: false,
        animate: true,
        handleDisconnected: false,
        edgeLength: s.linkDistance,
        nodeSpacing: Math.max(5, Math.round(s.repulsion / 1000)),
        unconstrainedIterations: 50,
        userConstraintIterations: 100,
      } as any);

      activeLayoutRef.current = layout;
      layout.run();
    },
    [cyRef, stopActiveLayout],
  );

  const stopDragCola = useCallback(() => {
    // Let Cola settle briefly, then stop
    setTimeout(() => {
      stopActiveLayout();
      const cy = cyRef.current;
      if (cy) {
        onPositionsChanged(snapshotPositions(cy));
      }
    }, SETTLE_DURATION_MS);
  }, [cyRef, stopActiveLayout, onPositionsChanged]);

  /**
   * Toggle Live Physics mode: continuous Cola on viewport-visible nodes.
   */
  const startLivePhysics = useCallback(() => {
    const cy = cyRef.current;
    if (!cy) return;

    stopActiveLayout();
    livePhysicsActiveRef.current = true;

    // Lock off-screen nodes
    const extent = cy.extent();
    const buffer = 200; // px buffer around viewport
    cy.nodes(":childless").forEach((node) => {
      const pos = node.position();
      if (
        pos.x < extent.x1 - buffer ||
        pos.x > extent.x2 + buffer ||
        pos.y < extent.y1 - buffer ||
        pos.y > extent.y2 + buffer
      ) {
        node.lock();
      }
    });

    const s = settingsRef.current;
    // biome-ignore lint/suspicious/noExplicitAny: cytoscape-cola options not typed
    const layout = cy.layout({
      name: "cola",
      infinite: true,
      fit: false,
      animate: true,
      handleDisconnected: false,
      edgeLength: s.linkDistance,
      nodeSpacing: Math.max(5, Math.round(s.repulsion / 1000)),
    } as any);

    activeLayoutRef.current = layout;
    layout.run();

    // Update lock set on viewport pan/zoom (debounced)
    let viewportTimer: number | null = null;
    const updateLockSet = () => {
      if (viewportTimer !== null) clearTimeout(viewportTimer);
      viewportTimer = window.setTimeout(() => {
        const ext = cy.extent();
        const buf = 200;
        cy.nodes(":childless").forEach((node) => {
          const p = node.position();
          const offScreen =
            p.x < ext.x1 - buf || p.x > ext.x2 + buf ||
            p.y < ext.y1 - buf || p.y > ext.y2 + buf;
          if (offScreen && !node.locked()) node.lock();
          else if (!offScreen && node.locked()) node.unlock();
        });
      }, 100);
    };
    cy.on("viewport", updateLockSet);

    // Auto-pause after idle
    const resetIdle = () => {
      if (idleTimerRef.current !== null) clearTimeout(idleTimerRef.current);
      idleTimerRef.current = window.setTimeout(() => {
        if (livePhysicsActiveRef.current) {
          stopActiveLayout();
          livePhysicsActiveRef.current = false;
          const c = cyRef.current;
          if (c) onPositionsChanged(snapshotPositions(c));
        }
      }, IDLE_TIMEOUT_MS);
    };

    cy.on("mousemove", resetIdle);
    resetIdle();
  }, [cyRef, stopActiveLayout, onPositionsChanged]);

  const stopLivePhysics = useCallback(() => {
    livePhysicsActiveRef.current = false;
    stopActiveLayout();
    const cy = cyRef.current;
    if (cy) {
      onPositionsChanged(snapshotPositions(cy));
    }
    if (idleTimerRef.current !== null) {
      clearTimeout(idleTimerRef.current);
      idleTimerRef.current = null;
    }
  }, [cyRef, stopActiveLayout, onPositionsChanged]);

  // Cleanup on unmount
  useEffect(() => {
    return () => {
      stopActiveLayout();
      if (idleTimerRef.current !== null) clearTimeout(idleTimerRef.current);
    };
  }, [stopActiveLayout]);

  return {
    startDragCola,
    stopDragCola,
    startLivePhysics,
    stopLivePhysics,
    isLivePhysicsActive: livePhysicsActiveRef,
  };
}
