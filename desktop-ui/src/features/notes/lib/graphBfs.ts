interface HubCandidate {
  id: string;
  linkCount: number;
  title: string;
}

/**
 * Select the hub node (center of the graph).
 * Priority: activeNoteId > most-connected > alphabetical fallback.
 */
export function selectHub(
  nodes: HubCandidate[],
  activeNoteId: string | null,
): string {
  if (activeNoteId && nodes.some((n) => n.id === activeNoteId)) {
    return activeNoteId;
  }
  const sorted = [...nodes].sort((a, b) => {
    if (b.linkCount !== a.linkCount) return b.linkCount - a.linkCount;
    return a.title.localeCompare(b.title);
  });
  return sorted[0]?.id ?? "";
}

/**
 * Compute BFS waves from a hub node outward.
 * Returns array of waves, where each wave is a list of node IDs.
 * Nodes unreachable from the hub (orphans + disconnected components)
 * are placed in the final wave.
 */
export function computeBfsWaves(
  hubId: string,
  adjacency: Map<string, Set<string>>,
  allNodeIds: Set<string>,
): string[][] {
  const waves: string[][] = [];
  const visited = new Set<string>();

  let currentWave = [hubId];
  visited.add(hubId);

  while (currentWave.length > 0) {
    waves.push(currentWave);
    const nextWave: string[] = [];
    for (const nodeId of currentWave) {
      const neighbors = adjacency.get(nodeId);
      if (!neighbors) continue;
      for (const neighbor of neighbors) {
        if (!visited.has(neighbor) && allNodeIds.has(neighbor)) {
          visited.add(neighbor);
          nextWave.push(neighbor);
        }
      }
    }
    currentWave = nextWave;
  }

  const remaining: string[] = [];
  for (const id of allNodeIds) {
    if (!visited.has(id)) {
      remaining.push(id);
    }
  }
  if (remaining.length > 0) {
    waves.push(remaining);
  }

  return waves;
}
