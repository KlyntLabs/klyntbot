import { generateJitteredKeyBetween } from "fractional-indexing-jittered";

export function keyBetween(before: string | null, after: string | null): string {
  return generateJitteredKeyBetween(before, after);
}

/**
 * Compute the fractional key for a move where `toIndex` is the target index
 * in the *original* (pre-removal) list. Returns `null` if the move is a no-op.
 */
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
