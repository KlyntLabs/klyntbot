import { generateJitteredKeyBetween } from "fractional-indexing-jittered";

export function keyBetween(before: string | null, after: string | null): string {
  return generateJitteredKeyBetween(before, after);
}

/**
 * Compute the fractional key for a move where `toIndex` is the target index
 * in the *original* (pre-removal) list. Returns `null` if the move is a no-op.
 */
/**
 * Given the current visible order and a drag result (active → over),
 * compute the `{ beforeId, afterId }` anchors for `db_reorder_entity`.
 * Returns null if the drop is a no-op (same item or unknown id).
 */
export function computeReorderAnchors(
  orderedIds: string[],
  activeId: string,
  overId: string,
): { beforeId?: string; afterId?: string } | null {
  if (activeId === overId) return null;
  const from = orderedIds.indexOf(activeId);
  const to = orderedIds.indexOf(overId);
  if (from < 0 || to < 0) return null;
  const movingForward = from < to;
  return movingForward
    ? { beforeId: orderedIds[to], afterId: orderedIds[to + 1] }
    : { beforeId: orderedIds[to - 1], afterId: orderedIds[to] };
}

export function keyForMove(
  ordered: Array<{ position: string }>,
  fromIndex: number,
  toIndex: number,
): string | null {
  if (fromIndex === toIndex) return null;
  const [beforeIdx, afterIdx] =
    toIndex < fromIndex ? [toIndex - 1, toIndex] : [toIndex, toIndex + 1];
  const before = ordered[beforeIdx]?.position ?? null;
  const after = ordered[afterIdx]?.position ?? null;
  return keyBetween(before, after);
}
