import type { Core } from "cytoscape";
import cytoscape from "cytoscape";
import fcose from "cytoscape-fcose";
import cola from "cytoscape-cola";

// Register Cytoscape plugins once. Guard with a flag to prevent
// "Plugin already registered" errors during HMR/test re-runs.
let pluginsRegistered = false;
export function registerCytoscapePlugins() {
  if (pluginsRegistered) return;
  cytoscape.use(fcose);
  cytoscape.use(cola);
  pluginsRegistered = true;
}

export interface PositionEntry {
  x: number;
  y: number;
}

export type PositionMap = Record<string, PositionEntry>;

/** Snapshot all leaf node positions from a Cytoscape instance. */
export function snapshotPositions(cy: Core): PositionMap {
  const positions: PositionMap = {};
  cy.nodes(":childless").forEach((node) => {
    const pos = node.position();
    positions[node.id()] = { x: pos.x, y: pos.y };
  });
  return positions;
}
