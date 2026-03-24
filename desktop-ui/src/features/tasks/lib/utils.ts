export { cn } from "@shared/lib/utils";

/**
 * LexoRank — minimal implementation for ordering issues.
 * Generates ranks between two existing ranks (or at start/end).
 */
export class LexoRank {
  private static readonly BUCKET = "0";
  private static readonly MIN_CHAR = "a";
  private static readonly MAX_CHAR = "z";

  /**
   * Returns a rank string that sorts between `prev` and `next`.
   * Pass undefined/null for open-ended ranges.
   */
  static between(prev: string | null | undefined, next: string | null | undefined): string {
    const prevValue = prev ? LexoRank.extractValue(prev) : LexoRank.MIN_CHAR;
    const nextValue = next ? LexoRank.extractValue(next) : LexoRank.MAX_CHAR + LexoRank.MAX_CHAR;

    const mid = LexoRank.midString(prevValue, nextValue);
    return `${LexoRank.BUCKET}|${mid}:`;
  }

  /** Returns a rank at the very beginning. */
  static initial(): string {
    return `${LexoRank.BUCKET}|hzzzzz:`;
  }

  private static extractValue(rank: string): string {
    const match = rank.match(/\|(.+):/);
    return match ? match[1] : rank;
  }

  private static midString(prev: string, next: string): string {
    let p = 0;
    let n = 0;

    const maxLen = Math.max(prev.length, next.length);
    const result: string[] = [];

    for (let i = 0; i < maxLen || p !== n; i++) {
      const pc = i < prev.length ? prev.charCodeAt(i) : 97; // 'a'
      const nc = i < next.length ? next.charCodeAt(i) : 122; // 'z'

      const mid = Math.floor((pc + nc) / 2);
      result.push(String.fromCharCode(mid));

      if (mid === pc) {
        p = pc;
        n = nc;
        continue;
      }
      break;
    }

    return result.join("");
  }
}
