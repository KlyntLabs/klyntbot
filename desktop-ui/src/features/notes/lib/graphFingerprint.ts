/**
 * Compute a deterministic fingerprint from graph structure.
 * Used as cache key for position persistence — changes when nodes/edges change.
 * Uses null byte separator for edge pairs to avoid conflicts with node IDs containing colons.
 */
export function computeFingerprint(nodeIds: string[], edgePairs: [string, string][]): string {
  const sortedNodes = [...nodeIds].sort().join(",");
  const sortedEdges = edgePairs
    .map(([a, b]) => [a, b].sort().join("\x00"))
    .sort()
    .join(",");
  const raw = `${sortedNodes}|${sortedEdges}`;
  // Simple djb2 hash — fast and sufficient for cache key comparison.
  // 32-bit — collisions acceptable (worst case: stale layout, re-computed on next load).
  let hash = 5381;
  for (let i = 0; i < raw.length; i++) {
    hash = ((hash << 5) + hash + raw.charCodeAt(i)) >>> 0;
  }
  return hash.toString(36);
}
