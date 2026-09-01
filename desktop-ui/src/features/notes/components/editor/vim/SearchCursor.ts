// SearchCursor.ts — Regex search cursor over flat text (for vim.js getSearchCursor)

/**
 * A search cursor that operates over flat document text.
 * vim.js calls getSearchCursor(query, startPos) and then iterates with
 * findNext()/findPrevious(). Positions are absolute character offsets
 * into the flat text — the adapter converts to/from {line, ch}.
 */
export class PMSearchCursor {
  private text: string;
  private query: RegExp;
  private currentFrom = -1;
  private currentTo = -1;
  private currentMatch: RegExpExecArray | null = null;
  private pos: number;

  constructor(text: string, query: RegExp, startPos: number) {
    this.text = text;
    // Ensure global flag for repeated exec
    this.query = new RegExp(
      query.source,
      query.flags.includes("g") ? query.flags : `${query.flags}g`,
    );
    this.pos = startPos;
  }

  findNext(): boolean {
    return this.find(false);
  }

  findPrevious(): boolean {
    return this.find(true);
  }

  find(reverse?: boolean): boolean {
    if (reverse) {
      // Search backward from current position
      const re = new RegExp(this.query.source, this.query.flags);
      let lastMatch: RegExpExecArray | null = null;

      re.lastIndex = 0;
      let m = re.exec(this.text);
      while (m !== null && m.index < this.pos) {
        lastMatch = m;
        if (m.index === re.lastIndex) re.lastIndex++;
        m = re.exec(this.text);
      }

      if (lastMatch) {
        this.currentFrom = lastMatch.index;
        this.currentTo = lastMatch.index + lastMatch[0].length;
        this.currentMatch = lastMatch;
        this.pos = lastMatch.index; // next backward search starts before this
        return true;
      }
      return false;
    }

    // Search forward
    this.query.lastIndex = this.pos;
    const m = this.query.exec(this.text);
    if (m) {
      this.currentFrom = m.index;
      this.currentTo = m.index + m[0].length;
      this.currentMatch = m;
      this.pos = this.currentTo; // next forward search starts after this
      if (m.index === this.query.lastIndex) this.query.lastIndex++;
      return true;
    }
    return false;
  }

  from(): number {
    return this.currentFrom;
  }

  to(): number {
    return this.currentTo;
  }

  get match(): RegExpExecArray | null {
    return this.currentMatch;
  }

  /** Replace current match. Returns replacement info for the adapter to apply. */
  replace(text: string): { from: number; to: number; text: string } {
    return { from: this.currentFrom, to: this.currentTo, text };
  }
}
