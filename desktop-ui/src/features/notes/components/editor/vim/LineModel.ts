// LineModel.ts — Bidirectional mapping between ProseMirror node tree and vim line/ch coordinates
import type { Node as PMNode } from "@tiptap/pm/model";
import type { Pos } from "./types";
import { makePos } from "./types";

interface LineEntry {
  /** Text content of this line */
  text: string;
  /** ProseMirror position at start of text content */
  pmStart: number;
}

/**
 * Maps a ProseMirror document's block structure to a flat array of lines,
 * as vim.js expects. Each block-level textblock = one line, except code blocks
 * where each \n-delimited segment = one line.
 *
 * Immutable — create a new LineModel when the doc changes.
 * ProseMirror docs are immutable, so cache by doc identity.
 */
export class LineModel {
  private lines: LineEntry[] = [];

  constructor(doc: PMNode) {
    this.build(doc);
  }

  private build(doc: PMNode): void {
    doc.descendants((node, pos) => {
      if (!node.isTextblock) return true;

      const textStart = pos + 1; // skip the opening tag
      const text = node.textContent;

      if (node.type.spec.code && text.includes("\n")) {
        // Code block: split on newlines
        const segments = text.split("\n");
        let offset = 0;
        for (const seg of segments) {
          this.lines.push({ text: seg, pmStart: textStart + offset });
          offset += seg.length + 1; // +1 for the \n
        }
      } else {
        this.lines.push({ text, pmStart: textStart });
      }

      return false; // don't descend into inline content
    });

    // Ensure at least one line
    if (this.lines.length === 0) {
      this.lines.push({ text: "", pmStart: 1 });
    }
  }

  lineCount(): number {
    return this.lines.length;
  }

  firstLine(): number {
    return 0;
  }

  lastLine(): number {
    return this.lines.length - 1;
  }

  getLine(row: number): string {
    if (row < 0 || row >= this.lines.length) return "";
    return this.lines[row].text;
  }

  /** Convert {line, ch} to absolute ProseMirror position */
  toPMPos(line: number, ch: number): number {
    const row = Math.max(0, Math.min(line, this.lines.length - 1));
    const entry = this.lines[row];
    const clampedCh = Math.max(0, Math.min(ch, entry.text.length));
    return entry.pmStart + clampedCh;
  }

  /** Convert absolute ProseMirror position to {line, ch} */
  fromPMPos(pmPos: number): Pos {
    // Binary search for the line containing pmPos
    let lo = 0;
    let hi = this.lines.length - 1;

    while (lo < hi) {
      const mid = (lo + hi + 1) >> 1;
      if (this.lines[mid].pmStart <= pmPos) {
        lo = mid;
      } else {
        hi = mid - 1;
      }
    }

    const entry = this.lines[lo];
    const ch = Math.max(0, Math.min(pmPos - entry.pmStart, entry.text.length));
    return makePos(lo, ch);
  }

  /** Clamp a Pos to valid document bounds */
  clipPos(pos: Pos): Pos {
    const line = Math.max(0, Math.min(pos.line, this.lines.length - 1));
    const entry = this.lines[line];
    const ch = Math.max(0, Math.min(pos.ch, entry.text.length));
    return makePos(line, ch);
  }

  /**
   * Get text between two positions (used by getRange).
   * Returns the text content between start and end, joining with \n across lines.
   */
  getRange(start: Pos, end: Pos): string {
    const sLine = Math.max(0, start.line);
    const eLine = Math.min(end.line, this.lines.length - 1);

    if (sLine === eLine) {
      return this.lines[sLine].text.slice(start.ch, end.ch);
    }

    const parts: string[] = [];
    parts.push(this.lines[sLine].text.slice(start.ch));
    for (let i = sLine + 1; i < eLine; i++) {
      parts.push(this.lines[i].text);
    }
    parts.push(this.lines[eLine].text.slice(0, end.ch));
    return parts.join("\n");
  }

  /** Convert an absolute character offset to {line, ch} (used by posFromIndex) */
  fromIndex(offset: number): Pos {
    let remaining = offset;
    for (let i = 0; i < this.lines.length; i++) {
      const lineLen = this.lines[i].text.length;
      if (remaining <= lineLen) {
        return makePos(i, remaining);
      }
      remaining -= lineLen + 1; // +1 for newline
    }
    // Past end of document
    const last = this.lines.length - 1;
    return makePos(last, this.lines[last].text.length);
  }

  /** Convert {line, ch} to absolute character offset (used by indexFromPos) */
  toIndex(pos: Pos): number {
    let idx = 0;
    const line = Math.min(pos.line, this.lines.length - 1);
    for (let i = 0; i < line; i++) {
      idx += this.lines[i].text.length + 1;
    }
    idx += Math.min(pos.ch, this.lines[line].text.length);
    return idx;
  }
}

// ── Caching ──────────────────────────────────────────────────────────────

let cachedDoc: PMNode | null = null;
let cachedModel: LineModel | null = null;

/** Get or create a LineModel for the given doc, caching by doc identity */
export function getLineModel(doc: PMNode): LineModel {
  if (cachedDoc === doc && cachedModel) return cachedModel;
  cachedModel = new LineModel(doc);
  cachedDoc = doc;
  return cachedModel;
}
