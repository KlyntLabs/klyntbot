// ProseMirrorAdapter.ts — CM5-compatible adapter for ProseMirror
//
// vim.js receives this class via initVim(CM) and calls instance methods
// on it. Each method translates the CM5 API to ProseMirror equivalents.

import { splitBlock } from "@tiptap/pm/commands";
import { redo, undo } from "@tiptap/pm/history";
import type { Transaction } from "@tiptap/pm/state";
import { TextSelection } from "@tiptap/pm/state";
import type { EditorView } from "@tiptap/pm/view";
import { getLineModel, type LineModel } from "./LineModel";
import { PMSearchCursor } from "./SearchCursor";
import type {
  CharCoords,
  FindPosVResult,
  Marker,
  Pos,
  ScrollInfo,
  SearchOverlay,
  VimSearchCursor,
} from "./types";
import { makePos } from "./types";

// ── Static helpers (attached to class, used by vim.js as CM.xxx) ──────

/** Simple string stream for ex-command parsing (replaces @codemirror/language StringStream) */
export class StringStream {
  string: string;
  pos: number;
  start: number;

  constructor(str: string) {
    this.string = str;
    this.pos = 0;
    this.start = 0;
  }

  eol(): boolean {
    return this.pos >= this.string.length;
  }

  sol(): boolean {
    return this.pos === 0;
  }

  peek(): string | undefined {
    return this.string.charAt(this.pos) || undefined;
  }

  next(): string | undefined {
    if (this.pos < this.string.length) return this.string.charAt(this.pos++);
    return undefined;
  }

  eat(match: string | RegExp | ((ch: string) => boolean)): string | undefined {
    const ch = this.string.charAt(this.pos);
    let ok = false;
    if (typeof match === "string") ok = ch === match;
    else if (match instanceof RegExp) ok = match.test(ch);
    else ok = ch !== "" && match(ch);
    if (ok) {
      this.pos++;
      return ch;
    }
    return undefined;
  }

  eatWhile(match: string | RegExp | ((ch: string) => boolean)): boolean {
    const start = this.pos;
    while (this.eat(match)) {}
    return this.pos > start;
  }

  eatSpace(): boolean {
    const start = this.pos;
    while (/\s/.test(this.string.charAt(this.pos))) this.pos++;
    return this.pos > start;
  }

  skipToEnd(): void {
    this.pos = this.string.length;
  }

  skipTo(ch: string): boolean {
    const found = this.string.indexOf(ch, this.pos);
    if (found > -1) {
      this.pos = found;
      return true;
    }
    return false;
  }

  match(
    pattern: string | RegExp,
    consume?: boolean,
    caseInsensitive?: boolean,
  ): boolean | RegExpMatchArray | null {
    if (typeof pattern === "string") {
      const cased = caseInsensitive
        ? this.string.slice(this.pos, this.pos + pattern.length).toLowerCase() ===
          pattern.toLowerCase()
        : this.string.slice(this.pos, this.pos + pattern.length) === pattern;
      if (cased) {
        if (consume !== false) this.pos += pattern.length;
        return true;
      }
      return null;
    }
    const m = this.string.slice(this.pos).match(pattern);
    if (m && m.index !== undefined && m.index > 0) return null;
    if (m && consume !== false) this.pos += m[0].length;
    return m;
  }

  backUp(n: number): void {
    this.pos -= n;
  }

  column(): number {
    return this.start;
  }

  indentation(): number {
    const m = this.string.match(/^\s*/);
    return m ? m[0].length : 0;
  }

  current(): string {
    return this.string.slice(this.start, this.pos);
  }
}

// ── ProseMirrorAdapter class ─────────────────────────────────────────

export class ProseMirrorAdapter {
  view: EditorView;
  state: Record<string, unknown>; // vim.js reads/writes cm.state.vim, cm.state.overwrite, etc.

  // biome-ignore lint/suspicious/noExplicitAny: vim.js reads/writes arbitrary properties on curOp
  curOp: Record<string, any> | null = null;

  private _listeners: Map<string, Set<Function>> = new Map();
  private _lastChange: { from: Pos; to: Pos; text: string[] } | null = null;
  private _inOperation = false;
  private _pendingCursorActivity = false;
  private _searchOverlay: SearchOverlay | null = null;
  private _markers: Set<{
    pmPos: number;
    insertLeft: boolean;
    cleared: boolean;
  }> = new Set();

  // ── Static properties (vim.js accesses these as CM.xxx) ──────────

  static Pos = makePos;
  static StringStream = StringStream;
  static commands: Record<string, (cm: ProseMirrorAdapter) => void> = {
    undo: (cm) => undo(cm.view.state, cm.view.dispatch),
    redo: (cm) => redo(cm.view.state, cm.view.dispatch),
    newlineAndIndent: (cm) => splitBlock(cm.view.state, cm.view.dispatch),
  };
  static isWordChar = (ch: string): boolean => /[\w\p{Alphabetic}\p{Number}_]/u.test(ch);
  static isMac = typeof navigator !== "undefined" && /Mac/.test(navigator.platform);

  static e_preventDefault(e: Event): void {
    e.preventDefault();
  }
  static e_stop(e: Event): void {
    e.preventDefault();
    e.stopPropagation();
  }

  static signal(emitter: unknown, type: string, ...args: unknown[]): void {
    if (emitter && typeof emitter === "object" && "_listeners" in emitter) {
      const adapter = emitter as unknown as ProseMirrorAdapter;
      const listeners = adapter._listeners.get(type);
      if (listeners) {
        for (const fn of listeners) fn(...args);
      }
    }
  }

  static on(emitter: unknown, type: string, fn: Function): void {
    if (emitter && typeof emitter === "object" && "on" in emitter) {
      (emitter as ProseMirrorAdapter).on(type, fn);
    }
  }

  static off(emitter: unknown, type: string, fn: Function): void {
    if (emitter && typeof emitter === "object" && "off" in emitter) {
      (emitter as ProseMirrorAdapter).off(type, fn);
    }
  }

  static lookupKey(_key: string, _map: unknown, handle: (binding: string) => void): string {
    // vim.js uses this for fallthrough handling; we handle keys directly
    handle("handled");
    return "handled";
  }

  static findEnclosingTag(): undefined {
    return undefined;
  }

  static findMatchingTag(): null {
    return null;
  }

  constructor(view: EditorView) {
    this.view = view;
    this.state = {};
  }

  // ── Line model (lazily built, cached per doc identity) ────────────

  private get model(): LineModel {
    return getLineModel(this.view.state.doc);
  }

  // ── Event system ──────────────────────────────────────────────────

  on(type: string, fn: Function): void {
    if (!this._listeners.has(type)) this._listeners.set(type, new Set());
    this._listeners.get(type)!.add(fn);
  }

  off(type: string, fn: Function): void {
    this._listeners.get(type)?.delete(fn);
  }

  signal(type: string, ...args: unknown[]): void {
    const listeners = this._listeners.get(type);
    if (listeners) {
      for (const fn of listeners) fn(...args);
    }
  }

  // ── Document content ──────────────────────────────────────────────

  getLine(row: number): string {
    return this.model.getLine(row);
  }

  lineCount(): number {
    return this.model.lineCount();
  }

  firstLine(): number {
    return 0;
  }

  lastLine(): number {
    return this.model.lastLine();
  }

  getValue(): string {
    const lines: string[] = [];
    for (let i = 0; i < this.model.lineCount(); i++) {
      lines.push(this.model.getLine(i));
    }
    return lines.join("\n");
  }

  getRange(s: Pos, e: Pos): string {
    return this.model.getRange(s, e);
  }

  // ── Cursor / Selection ────────────────────────────────────────────

  getCursor(type?: "head" | "anchor" | "start" | "end"): Pos {
    const sel = this.view.state.selection;
    let pmPos: number;
    switch (type) {
      case "anchor":
        pmPos = sel.anchor;
        break;
      case "start":
        pmPos = sel.from;
        break;
      case "end":
        pmPos = sel.to;
        break;
      default:
        pmPos = sel.head;
    }
    return this.model.fromPMPos(pmPos);
  }

  setCursor(lineOrPos: number | Pos, ch?: number): void {
    const pos = typeof lineOrPos === "number" ? makePos(lineOrPos, ch ?? 0) : lineOrPos;
    const pmPos = this.model.toPMPos(pos.line, pos.ch);
    const tr = this.view.state.tr.setSelection(TextSelection.create(this.view.state.doc, pmPos));
    this.view.dispatch(tr);
  }

  listSelections(): { anchor: Pos; head: Pos }[] {
    const sel = this.view.state.selection;
    return [
      {
        anchor: this.model.fromPMPos(sel.anchor),
        head: this.model.fromPMPos(sel.head),
      },
    ];
  }

  setSelection(anchor: Pos, head: Pos): void {
    const pmAnchor = this.model.toPMPos(anchor.line, anchor.ch);
    const pmHead = this.model.toPMPos(head.line, head.ch);
    const tr = this.view.state.tr.setSelection(
      TextSelection.create(this.view.state.doc, pmAnchor, pmHead),
    );
    this.view.dispatch(tr);
  }

  setSelections(ranges: { anchor: Pos; head: Pos }[], _primIndex?: number): void {
    // ProseMirror supports only single selection — use the first range
    if (ranges.length > 0) {
      this.setSelection(ranges[0].anchor, ranges[0].head);
    }
  }

  somethingSelected(): boolean {
    return !this.view.state.selection.empty;
  }

  getSelection(): string {
    const { from, to } = this.view.state.selection;
    if (from === to) return "";
    return this.view.state.doc.textBetween(from, to);
  }

  getSelections(): string[] {
    return [this.getSelection()];
  }

  replaceRange(text: string, s: Pos, e?: Pos): void {
    const from = this.model.toPMPos(s.line, s.ch);
    const to = e ? this.model.toPMPos(e.line, e.ch) : from;
    const doc = this.view.state.doc;
    const $from = doc.resolve(from);
    const $to = doc.resolve(to);

    // Linewise deletion: when range spans block boundaries and replacement is empty,
    // expand to cover entire block nodes to avoid merging paragraphs.
    // Two patterns from vim.js:
    //   Non-last line dd: replaceRange("", {line:N, ch:0}, {line:N+1, ch:0}) — s.ch===0
    //   Last line dd:     replaceRange("", {line:N-1, ch:end}, {line:N+1, ch:0}) — s.ch>0
    if (text === "" && $from.parent !== $to.parent) {
      let blockFrom: number;
      let blockTo: number;

      if (s.ch === 0) {
        // Deleting from start of a line — remove the starting block(s)
        blockFrom = $from.before($from.depth);
        // e.ch===0 means "up to but not including this block" — UNLESS e.line
        // is past the document end (clamped), which means "delete through the end"
        const eWithinBounds = e && e.line < this.model.lineCount();
        blockTo = e && e.ch === 0 && eWithinBounds ? $to.before($to.depth) : $to.after($to.depth);
      } else {
        // Deleting from end/middle of a line (last-line dd) — keep starting block,
        // remove the ending block(s)
        blockFrom = $from.after($from.depth);
        blockTo = $to.after($to.depth);
      }

      const tr = this.view.state.tr.delete(blockFrom, blockTo);
      this._dispatchChange(tr, s, e ?? s, text);
      return;
    }

    const tr = this.view.state.tr.insertText(text, from, to);
    this._dispatchChange(tr, s, e ?? s, text);
  }

  replaceSelection(text: string): void {
    const { from, to } = this.view.state.selection;
    const tr = this.view.state.tr.insertText(text, from, to);
    this.view.dispatch(tr);
  }

  replaceSelections(replacements: string[]): void {
    // Single selection only — use first replacement
    if (replacements.length > 0) {
      this.replaceSelection(replacements[0]);
    }
  }

  clipPos(pos: Pos): Pos {
    return this.model.clipPos(pos);
  }

  indexFromPos(pos: Pos): number {
    return this.model.toIndex(pos);
  }

  posFromIndex(offset: number): Pos {
    return this.model.fromIndex(offset);
  }

  // ── Coordinate / scroll methods ───────────────────────────────────

  charCoords(pos: Pos, _mode?: string): CharCoords {
    const pmPos = this.model.toPMPos(pos.line, pos.ch);
    const coords = this.view.coordsAtPos(pmPos);
    return { left: coords.left, top: coords.top, bottom: coords.bottom };
  }

  coordsChar(coords: { left: number; top: number }, _mode?: string): Pos {
    const result = this.view.posAtCoords(coords);
    if (!result) return makePos(0, 0);
    return this.model.fromPMPos(result.pos);
  }

  getScrollInfo(): ScrollInfo {
    const dom = this.view.dom;
    return {
      left: dom.scrollLeft,
      top: dom.scrollTop,
      height: dom.scrollHeight,
      width: dom.scrollWidth,
      clientHeight: dom.clientHeight,
      clientWidth: dom.clientWidth,
    };
  }

  scrollTo(_x?: number | null, y?: number | null): void {
    if (y != null) {
      this.view.dom.scrollTop = y;
    }
  }

  scrollIntoView(_pos?: Pos, _margin?: number): void {
    this.view.dispatch(this.view.state.tr.scrollIntoView());
  }

  defaultTextHeight(): number {
    // Estimate line height from first character coordinates
    try {
      const coords = this.view.coordsAtPos(1);
      return coords.bottom - coords.top || 20;
    } catch {
      return 20;
    }
  }

  findPosV(
    start: Pos,
    amount: number,
    unit: "page" | "line",
    _goalColumn?: number,
  ): FindPosVResult {
    const pmPos = this.model.toPMPos(start.line, start.ch);
    try {
      const coords = this.view.coordsAtPos(pmPos);
      const lineHeight = this.defaultTextHeight();

      let targetY: number;
      if (unit === "page") {
        const pageHeight = this.view.dom.clientHeight;
        targetY = coords.top + amount * pageHeight;
      } else {
        targetY = coords.top + amount * lineHeight;
      }

      const result = this.view.posAtCoords({
        left: coords.left,
        top: targetY + lineHeight / 2,
      });

      if (!result) {
        const hitSide = amount > 0;
        const edgePos = hitSide
          ? this.model.fromPMPos(this.view.state.doc.content.size)
          : makePos(0, 0);
        return { ...edgePos, hitSide };
      }

      return this.model.fromPMPos(result.pos);
    } catch {
      return { ...start, hitSide: true };
    }
  }

  // ── Editing commands ──────────────────────────────────────────────

  execCommand(name: string): void {
    const cmd = ProseMirrorAdapter.commands[name];
    if (cmd) cmd(this);
  }

  indentLine(line: number, more?: boolean): void {
    const pmPos = this.model.toPMPos(line, 0);
    const lineText = this.model.getLine(line);
    if (more) {
      const tr = this.view.state.tr.insertText("  ", pmPos, pmPos);
      this.view.dispatch(tr);
    } else {
      // Remove up to 2 leading spaces
      const stripped = lineText.replace(/^ {1,2}/, "");
      const removed = lineText.length - stripped.length;
      if (removed > 0) {
        const tr = this.view.state.tr.delete(pmPos, pmPos + removed);
        this.view.dispatch(tr);
      }
    }
  }

  indentMore(): void {
    const cursor = this.getCursor();
    this.indentLine(cursor.line, true);
  }

  indentLess(): void {
    const cursor = this.getCursor();
    this.indentLine(cursor.line, false);
  }

  // ── Search ────────────────────────────────────────────────────────

  getSearchCursor(query: RegExp, pos?: Pos): VimSearchCursor {
    const fullText = this.getValue();
    const startIdx = pos ? this.model.toIndex(pos) : 0;
    const cursor = new PMSearchCursor(fullText, query, startIdx);

    return {
      findNext: () => cursor.findNext(),
      findPrevious: () => cursor.findPrevious(),
      find: (reverse?: boolean) => cursor.find(reverse),
      from: () => this.model.fromIndex(cursor.from()),
      to: () => this.model.fromIndex(cursor.to()),
      replace: (text: string) => {
        const r = cursor.replace(text);
        const fromPos = this.model.fromIndex(r.from);
        const toPos = this.model.fromIndex(r.to);
        this.replaceRange(text, fromPos, toPos);
      },
      get match() {
        return cursor.match;
      },
    };
  }

  addOverlay(overlay: { query: RegExp }): void {
    this._searchOverlay = overlay;
    this.signal("searchOverlayChange", overlay);
  }

  removeOverlay(): void {
    this._searchOverlay = null;
    this.signal("searchOverlayChange", null);
  }

  getSearchOverlay(): SearchOverlay | null {
    return this._searchOverlay;
  }

  showMatchesOnScrollbar(): void {
    // No-op — not needed for rich text editor
  }

  // ── Bookmarks (marks that track through edits) ────────────────────

  setBookmark(cursor: Pos, _options?: { insertLeft?: boolean }): Marker {
    const pmPos = this.model.toPMPos(cursor.line, cursor.ch);
    const self = this;

    const marker = {
      pmPos,
      insertLeft: _options?.insertLeft ?? false,
      cleared: false,
    };
    this._markers.add(marker);

    return {
      find(): Pos | null {
        if (marker.cleared) return null;
        return self.model.fromPMPos(marker.pmPos);
      },
      clear(): void {
        marker.cleared = true;
        self._markers.delete(marker);
      },
    };
  }

  // ── Options ───────────────────────────────────────────────────────

  getOption(name: string): unknown {
    switch (name) {
      case "tabSize":
        return 2;
      case "indentWithTabs":
        return false;
      case "indentUnit":
        return 2;
      case "textwidth":
        return (this.state as Record<string, unknown>).textwidth ?? 80;
      case "firstLineNumber":
        return 1;
      case "readOnly":
        return false;
      default:
        return (this.state as Record<string, unknown>)[name];
    }
  }

  setOption(name: string, val: unknown): void {
    (this.state as Record<string, unknown>)[name] = val;
  }

  toggleOverwrite(on: boolean): void {
    (this.state as Record<string, unknown>).overwrite = on;
  }

  overWriteSelection(text: string): void {
    // Extend selection to cover next character, then replace
    const { from, to } = this.view.state.selection;
    const endPos = Math.min(from + text.length, this.view.state.doc.content.size);
    const tr = this.view.state.tr.insertText(text, from, Math.max(to, endPos));
    this.view.dispatch(tr);
  }

  // ── Token / syntax ────────────────────────────────────────────────

  getTokenTypeAt(pos: Pos): string {
    // Check if position is inside a code block (has "code" spec)
    const pmPos = this.model.toPMPos(pos.line, pos.ch);
    const $pos = this.view.state.doc.resolve(pmPos);
    for (let d = $pos.depth; d >= 0; d--) {
      if ($pos.node(d).type.spec.code) return "comment";
    }
    return "";
  }

  findMatchingBracket(pos: Pos): { to: Pos | undefined } {
    const lineText = this.getLine(pos.line);
    const ch = lineText[pos.ch];
    if (!ch) return { to: undefined };

    const PAIRS: Record<string, string> = {
      "(": ")",
      ")": "(",
      "[": "]",
      "]": "[",
      "{": "}",
      "}": "{",
    };
    const match = PAIRS[ch];
    if (!match) return { to: undefined };

    const isOpen = "([{".includes(ch);
    const text = this.getValue();
    const startIdx = this.model.toIndex(pos);
    let depth = 0;

    if (isOpen) {
      for (let i = startIdx + 1; i < text.length; i++) {
        if (text[i] === ch) depth++;
        else if (text[i] === match) {
          if (depth === 0) return { to: this.model.fromIndex(i) };
          depth--;
        }
      }
    } else {
      for (let i = startIdx - 1; i >= 0; i--) {
        if (text[i] === ch) depth++;
        else if (text[i] === match) {
          if (depth === 0) return { to: this.model.fromIndex(i) };
          depth--;
        }
      }
    }

    return { to: undefined };
  }

  scanForBracket(
    pos: Pos,
    dir: 1 | -1,
    _style?: unknown,
    _config?: unknown,
  ): { pos: Pos; ch: string } | false | null {
    const text = this.getValue();
    const startIdx = this.model.toIndex(pos);
    const brackets = "()[]{}";

    if (dir === 1) {
      for (let i = startIdx; i < text.length; i++) {
        if (brackets.includes(text[i])) {
          return { pos: this.model.fromIndex(i), ch: text[i] };
        }
      }
    } else {
      for (let i = startIdx; i >= 0; i--) {
        if (brackets.includes(text[i])) {
          return { pos: this.model.fromIndex(i), ch: text[i] };
        }
      }
    }
    return null;
  }

  // ── Multi-selection stubs (ProseMirror is single-selection) ───────

  isInMultiSelectMode(): boolean {
    return false;
  }

  get inVirtualSelectionMode(): boolean {
    return false;
  }

  forEachSelection(fn: Function): void {
    fn(this);
  }

  // ── Operation batching ────────────────────────────────────────────

  operation<T>(fn: () => T): T {
    this._inOperation = true;
    this.curOp = {};
    try {
      return fn();
    } finally {
      this.curOp = null;
      this._inOperation = false;
      if (this._pendingCursorActivity) {
        this._pendingCursorActivity = false;
        this.signal("cursorActivity", this);
      }
    }
  }

  // ── Dialog / notification (delegated to React layer) ──────────────

  openDialog(
    template: HTMLElement | string,
    callback: (value: string, event?: Event) => void,
    options?: {
      bottom?: boolean;
      value?: string;
      onClose?: () => void;
      onKeyDown?: (event: KeyboardEvent, value: string, close: () => void) => boolean;
      onKeyUp?: (event: KeyboardEvent, value: string, close: () => void) => boolean;
      selectValueOnOpen?: boolean;
    },
  ): () => void {
    // Signal the React layer to open a command line
    this.signal("dialog", { template, callback, options });
    return () => this.signal("dialog-close");
  }

  openNotification(template: HTMLElement | string, _options?: unknown): () => void {
    this.signal("notification", { template });
    return () => {};
  }

  // ── Misc ──────────────────────────────────────────────────────────

  focus(): void {
    this.view.focus();
  }

  blur(): void {
    (this.view.dom as HTMLElement).blur();
  }

  getInputField(): HTMLElement {
    return this.view.dom as HTMLElement;
  }

  getWrapperElement(): HTMLElement {
    return this.view.dom as HTMLElement;
  }

  setValue(text: string): void {
    const tr = this.view.state.tr.insertText(text, 0, this.view.state.doc.content.size);
    this.view.dispatch(tr);
  }

  foldCode(): void {
    // No-op — code folding not supported in rich text ProseMirror
  }

  hardWrap(_options: unknown): number {
    // No-op
    return 0;
  }

  getLastEditEnd(): Pos {
    // Return cursor position as fallback
    return this.getCursor();
  }

  save(): void {
    this.signal("save");
  }

  // ── Line handle tracking (simplified) ─────────────────────────────

  private _lineHandles: Map<number, { row: number; index: number }> = new Map();
  private _handleId = 0;

  getLineHandle(row: number): { row: number; index: number } {
    const handle = { row, index: this._handleId++ };
    this._lineHandles.set(handle.index, handle);
    return handle;
  }

  getLineNumber(handle: { row: number; index: number }): number | null {
    const tracked = this._lineHandles.get(handle.index);
    return tracked ? tracked.row : null;
  }

  releaseLineHandles(): void {
    this._lineHandles.clear();
  }

  // ── Change dispatch helper ────────────────────────────────────────

  private _dispatchChange(tr: Transaction, from: Pos, to: Pos, text: string): void {
    this._lastChange = { from, to, text: text.split("\n") };
    this.view.dispatch(tr);
    this.signal("change", this, this._lastChange);
  }

  // ── Methods called by the plugin integration layer ─────────────────

  /** Called when the document changes */
  onChange(): void {
    this.signal("change", this, this._lastChange);
    this._lastChange = null;
  }

  onSelectionChange(): void {
    if (this._inOperation) {
      this._pendingCursorActivity = true;
    } else {
      this.signal("cursorActivity", this);
    }
  }

  onBeforeEndOperation(): void {
    if (this._pendingCursorActivity) {
      this._pendingCursorActivity = false;
      this.signal("cursorActivity", this);
    }
  }
}
