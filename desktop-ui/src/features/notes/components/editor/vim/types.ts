// types.ts — Shared types for the ProseMirror vim adapter

/** CM5-style position used by vim.js */
export interface Pos {
  line: number;
  ch: number;
}

/** Create a position object (replaces CM.Pos constructor) */
export function makePos(line: number, ch: number): Pos {
  return { line, ch };
}

/** Bookmark / position marker that tracks through document changes */
export interface Marker {
  find(): Pos | null;
  clear(): void;
}

/** Search cursor returned by getSearchCursor */
export interface VimSearchCursor {
  find(reverse?: boolean): boolean;
  findNext(): boolean;
  findPrevious(): boolean;
  from(): Pos;
  to(): Pos;
  replace(text: string): void;
  readonly match: RegExpExecArray | null;
}

/** Search overlay for highlighting */
export interface SearchOverlay {
  query: RegExp;
}

/** Scroll info returned by getScrollInfo */
export interface ScrollInfo {
  left: number;
  top: number;
  height: number;
  width: number;
  clientHeight: number;
  clientWidth: number;
}

/** Coordinates returned by charCoords */
export interface CharCoords {
  left: number;
  top: number;
  bottom: number;
}

/** Result from findPosV */
export interface FindPosVResult extends Pos {
  hitSide?: boolean;
}

/** Vim mode as reported by vim.js */
export type VimJsMode = "normal" | "insert" | "visual" | "replace";

/** Mode change event from vim.js */
export interface VimModeChangeEvent {
  mode: VimJsMode;
  subMode?: string; // "linewise", "blockwise", ""
}
