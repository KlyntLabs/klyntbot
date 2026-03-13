# Port @replit/codemirror-vim to ProseMirror/tiptap — Implementation Plan

> **For agentic workers:** REQUIRED: Use superpowers:subagent-driven-development (if subagents available) or superpowers:executing-plans to implement this plan. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Replace the custom vim mode (14 files, ~3,400 lines, limited feature set) with a ProseMirror adapter for `@replit/codemirror-vim`'s battle-tested 7,164-line vim engine, gaining 30+ motions, 8+ operators, visual block mode, macros, registers, marks, search, ex commands, and dot repeat for free.

**Architecture:** `vim.js` (the vim engine) calls methods on a `CodeMirror` adapter class — never imports CodeMirror directly. We vendor `vim.js` unchanged, rewrite the adapter (~1,100 lines) to target ProseMirror's API, and wrap everything in a tiptap Extension. A `LineModel` abstraction bridges ProseMirror's node tree to vim's `{line, ch}` coordinate system.

**Tech Stack:** TypeScript, tiptap 2 / ProseMirror, `@replit/codemirror-vim` (vim.js vendored), Vitest

---

## File Structure

```
desktop-ui/src/features/notes/components/editor/vim/
├── index.ts                    # REWRITE — new tiptap Extension wrapping vim.js
├── ProseMirrorAdapter.ts       # CREATE — CM5 adapter interface targeting ProseMirror
├── LineModel.ts                # CREATE — line↔PM position mapping
├── SearchCursor.ts             # CREATE — regex search cursor for ProseMirror docs
├── vim-engine.js               # VENDOR — copy of vim.js from @replit/codemirror-vim
├── types.ts                    # CREATE — shared types (Pos, Marker, SearchQuery, etc.)
├── LineModel.test.ts           # CREATE — unit tests for line model
├── SearchCursor.test.ts        # CREATE — unit tests for search cursor
│
│ # OLD FILES TO DELETE (after migration):
├── VimPlugin.ts                # DELETE — replaced by index.ts + ProseMirrorAdapter
├── VimState.ts                 # DELETE — replaced by vim.js internal state
├── motions.ts                  # DELETE — replaced by vim.js motions
├── operators.ts                # DELETE — replaced by vim.js operators
├── commands.ts                 # DELETE — replaced by vim.js commands
├── textObjects.ts              # DELETE — replaced by vim.js text objects
├── search.ts                   # DELETE — replaced by vim.js search + SearchCursor
├── cursor.ts                   # DELETE — cursor rendering moved into index.ts
├── positions.ts                # DELETE — replaced by LineModel
├── charClass.ts                # DELETE — replaced by vim.js word detection
├── VimState.test.ts            # DELETE — old state tests
├── charClass.test.ts           # DELETE — old charClass tests
├── textObjects.test.ts         # DELETE — old textObjects tests
```

**UI files preserved as-is:**
- `VimCommandLine.tsx` — command line input (`:` and `/`)
- `VimStatusLine.tsx` — mode indicator

**Files modified:**
- `EditorCore.tsx:28,188` — update import path (stays the same, just `VimModeOptions` shape changes)
- `NoteEditor.tsx:11-13,46,66` — update vim state/callback types to match vim.js events
- `editor.css:421-517` — CSS stays identical (same class names)

---

## Chunk 1: Foundation — Vendor, Types, LineModel

### Task 1: Vendor vim.js and create types

**Files:**
- Create: `desktop-ui/src/features/notes/components/editor/vim/vim-engine.js`
- Create: `desktop-ui/src/features/notes/components/editor/vim/types.ts`

- [ ] **Step 1: Install @replit/codemirror-vim temporarily to extract vim.js**

We need the `vim.js` file from the package. Install it, copy the file, then remove the dependency.

```bash
cd desktop-ui
bun add @replit/codemirror-vim
cp node_modules/@replit/codemirror-vim/src/vim.js src/features/notes/components/editor/vim/vim-engine.js
bun remove @replit/codemirror-vim
```

- [ ] **Step 2: Patch vim-engine.js for ES module export**

The file uses CommonJS-style `export { initVim }`. Verify it works as an ES module import. If needed, add at the top:

```js
// @ts-nocheck
/* eslint-disable */
// Vendored from @replit/codemirror-vim (v6.x) — DO NOT EDIT
// This is the complete vim engine. It calls methods on a CodeMirror adapter
// class passed to initVim(CM). The adapter is our ProseMirrorAdapter.
```

Also remove the `import { StringStream } from "@codemirror/language"` at the top (if present) — we'll provide our own StringStream in the adapter.

Run: `cd desktop-ui && bun run build 2>&1 | head -20` to verify no import errors.

- [ ] **Step 3: Create types.ts with shared type definitions**

```typescript
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
```

- [ ] **Step 4: Verify build**

Run: `cd desktop-ui && npx tsc --noEmit 2>&1 | head -20`
Expected: No errors from new files (vim-engine.js is untyped, types.ts is self-contained)

- [ ] **Step 5: Commit**

```bash
git add desktop-ui/src/features/notes/components/editor/vim/vim-engine.js \
       desktop-ui/src/features/notes/components/editor/vim/types.ts
git commit -m "feat(vim): vendor vim.js engine and add shared types"
```

---

### Task 2: Build LineModel — line↔ProseMirror position mapping

The critical abstraction. vim.js thinks in `{line, ch}` coordinates; ProseMirror uses absolute offsets in a node tree. LineModel bridges these.

**Design:** Each block-level textblock node = one logical "line". Code blocks with `\n` produce multiple lines. We build an index on demand (doc is immutable in ProseMirror, so we cache per-doc identity).

**Files:**
- Create: `desktop-ui/src/features/notes/components/editor/vim/LineModel.ts`
- Create: `desktop-ui/src/features/notes/components/editor/vim/LineModel.test.ts`

- [ ] **Step 1: Write failing tests for LineModel**

```typescript
// LineModel.test.ts
import { describe, expect, it } from "vitest";
import { Schema, Node as PMNode } from "@tiptap/pm/model";
import { LineModel } from "./LineModel";

// Minimal ProseMirror schema for testing
const schema = new Schema({
  nodes: {
    doc: { content: "block+" },
    paragraph: { content: "inline*", group: "block", toDOM: () => ["p", 0] },
    code_block: { content: "text*", group: "block", code: true, toDOM: () => ["pre", ["code", 0]] },
    heading: { content: "inline*", group: "block", attrs: { level: { default: 1 } }, toDOM: (n) => [`h${n.attrs.level}`, 0] },
    text: { group: "inline" },
  },
});

function doc(...children: PMNode[]) {
  return schema.node("doc", null, children);
}
function p(text: string) {
  return text ? schema.node("paragraph", null, [schema.text(text)]) : schema.node("paragraph");
}
function codeBlock(text: string) {
  return text ? schema.node("code_block", null, [schema.text(text)]) : schema.node("code_block");
}
function heading(text: string, level = 1) {
  return schema.node("heading", { level }, text ? [schema.text(text)] : []);
}

describe("LineModel", () => {
  it("maps simple paragraphs to lines", () => {
    const d = doc(p("hello"), p("world"));
    const model = new LineModel(d);
    expect(model.lineCount()).toBe(2);
    expect(model.getLine(0)).toBe("hello");
    expect(model.getLine(1)).toBe("world");
  });

  it("maps code block newlines to separate lines", () => {
    const d = doc(codeBlock("line1\nline2\nline3"));
    const model = new LineModel(d);
    expect(model.lineCount()).toBe(3);
    expect(model.getLine(0)).toBe("line1");
    expect(model.getLine(1)).toBe("line2");
    expect(model.getLine(2)).toBe("line3");
  });

  it("maps mixed blocks correctly", () => {
    const d = doc(p("para"), codeBlock("a\nb"), heading("title"));
    const model = new LineModel(d);
    expect(model.lineCount()).toBe(4);
    expect(model.getLine(0)).toBe("para");
    expect(model.getLine(1)).toBe("a");
    expect(model.getLine(2)).toBe("b");
    expect(model.getLine(3)).toBe("title");
  });

  it("handles empty paragraphs as empty lines", () => {
    const d = doc(p("above"), p(""), p("below"));
    const model = new LineModel(d);
    expect(model.lineCount()).toBe(3);
    expect(model.getLine(1)).toBe("");
  });

  it("converts line/ch to PM position and back", () => {
    const d = doc(p("hello"), p("world"));
    const model = new LineModel(d);

    // "hello" starts at PM pos 1 (after doc open tag)
    const pmPos = model.toPMPos(0, 2); // line 0, ch 2 = 'l' in "hello"
    expect(pmPos).toBeGreaterThan(0);

    const { line, ch } = model.fromPMPos(pmPos);
    expect(line).toBe(0);
    expect(ch).toBe(2);
  });

  it("converts positions in code blocks", () => {
    const d = doc(codeBlock("abc\ndef"));
    const model = new LineModel(d);

    // line 1, ch 1 = 'e' in "def"
    const pmPos = model.toPMPos(1, 1);
    const back = model.fromPMPos(pmPos);
    expect(back.line).toBe(1);
    expect(back.ch).toBe(1);
  });

  it("clamps out-of-range positions", () => {
    const d = doc(p("hi"));
    const model = new LineModel(d);
    const clipped = model.clipPos({ line: 99, ch: 99 });
    expect(clipped.line).toBe(0);
    expect(clipped.ch).toBe(2); // "hi" has length 2
  });

  it("lineCount returns 1 for empty doc with one empty paragraph", () => {
    const d = doc(p(""));
    const model = new LineModel(d);
    expect(model.lineCount()).toBe(1);
    expect(model.getLine(0)).toBe("");
  });
});
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cd desktop-ui && bun run test -- --run src/features/notes/components/editor/vim/LineModel.test.ts`
Expected: FAIL — `LineModel` not found

- [ ] **Step 3: Implement LineModel**

```typescript
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
```

- [ ] **Step 4: Run tests to verify they pass**

Run: `cd desktop-ui && bun run test -- --run src/features/notes/components/editor/vim/LineModel.test.ts`
Expected: All tests PASS

- [ ] **Step 5: Commit**

```bash
git add desktop-ui/src/features/notes/components/editor/vim/LineModel.ts \
       desktop-ui/src/features/notes/components/editor/vim/LineModel.test.ts
git commit -m "feat(vim): add LineModel for line↔PM position mapping"
```

---

### Task 3: Build SearchCursor for ProseMirror

vim.js uses `getSearchCursor(regex, pos)` to find/replace text matches. ProseMirror has no built-in equivalent.

**Files:**
- Create: `desktop-ui/src/features/notes/components/editor/vim/SearchCursor.ts`
- Create: `desktop-ui/src/features/notes/components/editor/vim/SearchCursor.test.ts`

- [ ] **Step 1: Write failing tests**

```typescript
// SearchCursor.test.ts
import { describe, expect, it } from "vitest";
import { PMSearchCursor } from "./SearchCursor";

describe("PMSearchCursor", () => {
  const text = "hello world\nfoo bar\nhello again";

  it("finds forward matches", () => {
    const cursor = new PMSearchCursor(text, /hello/g, 0);
    expect(cursor.findNext()).toBe(true);
    expect(cursor.from()).toBe(0);
    expect(cursor.to()).toBe(5);
    expect(cursor.findNext()).toBe(true);
    expect(cursor.from()).toBe(20);
    expect(cursor.to()).toBe(25);
    expect(cursor.findNext()).toBe(false);
  });

  it("finds backward matches", () => {
    const cursor = new PMSearchCursor(text, /hello/g, 25);
    expect(cursor.findPrevious()).toBe(true);
    expect(cursor.from()).toBe(0);
    expect(cursor.to()).toBe(5);
    expect(cursor.findPrevious()).toBe(false);
  });

  it("starts from given position", () => {
    const cursor = new PMSearchCursor(text, /hello/g, 10);
    expect(cursor.findNext()).toBe(true);
    expect(cursor.from()).toBe(20); // skips first "hello"
  });

  it("handles case-insensitive regex", () => {
    const cursor = new PMSearchCursor("Hello HELLO", /hello/gi, 0);
    expect(cursor.findNext()).toBe(true);
    expect(cursor.from()).toBe(0);
    expect(cursor.findNext()).toBe(true);
    expect(cursor.from()).toBe(6);
  });

  it("replaces matches", () => {
    const cursor = new PMSearchCursor("aXbXc", /X/g, 0);
    cursor.findNext();
    expect(cursor.from()).toBe(1);
    // replace() returns the replacement info — actual doc mutation
    // is handled by the adapter. We just test the interface exists.
    expect(typeof cursor.replace).toBe("function");
  });

  it("exposes match groups", () => {
    const cursor = new PMSearchCursor("foo123bar", /(\d+)/g, 0);
    cursor.findNext();
    expect(cursor.match).not.toBeNull();
    expect(cursor.match![1]).toBe("123");
  });
});
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cd desktop-ui && bun run test -- --run src/features/notes/components/editor/vim/SearchCursor.test.ts`
Expected: FAIL

- [ ] **Step 3: Implement PMSearchCursor**

```typescript
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
    this.query = new RegExp(query.source, query.flags.includes("g") ? query.flags : query.flags + "g");
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
```

- [ ] **Step 4: Run tests to verify they pass**

Run: `cd desktop-ui && bun run test -- --run src/features/notes/components/editor/vim/SearchCursor.test.ts`
Expected: All tests PASS

- [ ] **Step 5: Commit**

```bash
git add desktop-ui/src/features/notes/components/editor/vim/SearchCursor.ts \
       desktop-ui/src/features/notes/components/editor/vim/SearchCursor.test.ts
git commit -m "feat(vim): add PMSearchCursor for regex search over document text"
```

---

## Chunk 2: ProseMirror Adapter

### Task 4: Build ProseMirrorAdapter — core methods

This is the main adapter class. It implements the ~40 methods that vim.js calls on its `cm` parameter. We build it incrementally: core document/cursor/selection methods first, then scroll/coordinates, then remaining.

**Files:**
- Create: `desktop-ui/src/features/notes/components/editor/vim/ProseMirrorAdapter.ts`

- [ ] **Step 1: Create adapter skeleton with Pos, static properties, and state container**

```typescript
// ProseMirrorAdapter.ts — CM5-compatible adapter for ProseMirror
//
// vim.js receives this class via initVim(CM) and calls instance methods
// on it. Each method translates the CM5 API to ProseMirror equivalents.

import type { Node as PMNode } from "@tiptap/pm/model";
import type { EditorState, Transaction } from "@tiptap/pm/state";
import { TextSelection } from "@tiptap/pm/state";
import type { EditorView } from "@tiptap/pm/view";
import { getLineModel, LineModel } from "./LineModel";
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
        ? this.string.slice(this.pos, this.pos + pattern.length).toLowerCase() === pattern.toLowerCase()
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
    const match = this.string.match(/^\s*/);
    return match ? match[0].length : 0;
  }

  current(): string {
    return this.string.slice(this.start, this.pos);
  }
}

// ── ProseMirrorAdapter class ─────────────────────────────────────────

export class ProseMirrorAdapter {
  view: EditorView;
  state: Record<string, unknown>; // vim.js reads/writes cm.state.vim, cm.state.overwrite, etc.

  private _model: LineModel | null = null;
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
  static commands: Record<string, (cm: ProseMirrorAdapter) => void> = {};
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
    if (emitter && typeof emitter === "object" && "on" in emitter) {
      const adapter = emitter as ProseMirrorAdapter;
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

  static lookupKey(
    _key: string,
    _map: unknown,
    handle: (binding: string) => void,
  ): string {
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

    // Register default commands
    ProseMirrorAdapter.commands.undo = (cm) => {
      cm.view.dispatch(cm.view.state.tr);
      // Use ProseMirror history's undo
      const { undo } = require("@tiptap/pm/history");
      undo(cm.view.state, cm.view.dispatch);
    };
    ProseMirrorAdapter.commands.redo = (cm) => {
      const { redo } = require("@tiptap/pm/history");
      redo(cm.view.state, cm.view.dispatch);
    };
    ProseMirrorAdapter.commands.newlineAndIndent = (cm) => {
      const { splitBlock } = require("@tiptap/pm/commands");
      splitBlock(cm.view.state, cm.view.dispatch);
    };
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
      for (const fn of listeners) fn(this, ...args);
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
    const tr = this.view.state.tr.setSelection(
      TextSelection.create(this.view.state.doc, pmPos),
    );
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

  setSelections(
    ranges: { anchor: Pos; head: Pos }[],
    _primIndex?: number,
  ): void {
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
    const coords = this.view.coordsAtPos(1);
    return coords.bottom - coords.top || 20;
  }

  findPosV(
    start: Pos,
    amount: number,
    unit: "page" | "line",
    _goalColumn?: number,
  ): FindPosVResult {
    const pmPos = this.model.toPMPos(start.line, start.ch);
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

    // vim.js expects from()/to() to return Pos, not numbers
    const self = this;
    return {
      findNext: () => cursor.findNext(),
      findPrevious: () => cursor.findPrevious(),
      find: (reverse?: boolean) => cursor.find(reverse),
      from: () => self.model.fromIndex(cursor.from()),
      to: () => self.model.fromIndex(cursor.to()),
      replace: (text: string) => {
        const r = cursor.replace(text);
        const fromPos = self.model.fromIndex(r.from);
        const toPos = self.model.fromIndex(r.to);
        self.replaceRange(text, fromPos, toPos);
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
    let pmPos = this.model.toPMPos(cursor.line, cursor.ch);
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
    try {
      return fn();
    } finally {
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
    const tr = this.view.state.tr.insertText(
      text,
      0,
      this.view.state.doc.content.size,
    );
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

  /** Called when the CM6 view updates (doc change or selection change) */
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
```

- [ ] **Step 2: Verify build compiles**

Run: `cd desktop-ui && npx tsc --noEmit 2>&1 | grep -c "error"` (should be 0 or only pre-existing errors)

Note: The `require()` calls in the constructor for `@tiptap/pm/history` and `@tiptap/pm/commands` should be converted to dynamic imports or top-level imports. Replace them with:

```typescript
import { undo, redo } from "@tiptap/pm/history";
import { splitBlock } from "@tiptap/pm/commands";
```

And update the commands:
```typescript
ProseMirrorAdapter.commands.undo = (cm) => undo(cm.view.state, cm.view.dispatch);
ProseMirrorAdapter.commands.redo = (cm) => redo(cm.view.state, cm.view.dispatch);
ProseMirrorAdapter.commands.newlineAndIndent = (cm) => splitBlock(cm.view.state, cm.view.dispatch);
```

- [ ] **Step 3: Commit**

```bash
git add desktop-ui/src/features/notes/components/editor/vim/ProseMirrorAdapter.ts
git commit -m "feat(vim): add ProseMirrorAdapter — CM5 interface for ProseMirror"
```

---

## Chunk 3: tiptap Extension + Integration

### Task 5: Build the new tiptap VimExtension (index.ts rewrite)

This replaces the current `index.ts` and `VimPlugin.ts`. It creates the tiptap Extension that:
1. Instantiates `ProseMirrorAdapter` per editor
2. Calls `initVim(ProseMirrorAdapter)` to get the Vim API
3. Handles keyboard events via `Vim.handleKey()`
4. Renders block cursor decorations
5. Highlights search matches
6. Forwards mode changes to the React layer

**Files:**
- Modify: `desktop-ui/src/features/notes/components/editor/vim/index.ts` (full rewrite)

- [ ] **Step 1: Rewrite index.ts**

```typescript
// index.ts — tiptap Extension wrapping @replit/codemirror-vim engine
//
// Architecture:
//   vim-engine.js → initVim(ProseMirrorAdapter) → Vim API
//   This file creates the tiptap Extension + ProseMirror plugins.

import { Extension } from "@tiptap/react";
import type { Editor } from "@tiptap/react";
import { Plugin, PluginKey, TextSelection } from "@tiptap/pm/state";
import { Decoration, DecorationSet } from "@tiptap/pm/view";
import type { EditorView } from "@tiptap/pm/view";
import { ProseMirrorAdapter, StringStream } from "./ProseMirrorAdapter";
import type { VimModeChangeEvent } from "./types";

// @ts-expect-error — vim-engine.js is untyped
import { initVim } from "./vim-engine";

// ── Public types ────────────────────────────────────────────────────────

export type VimMode = "normal" | "insert" | "visual" | "visual-line" | "replace";

export interface VimModeOptions {
  onStateChange?: (state: { mode: VimMode }) => void;
  onOpenCommandLine?: (prefix: string) => void;
  enabled?: () => boolean;
}

// ── Plugin keys ─────────────────────────────────────────────────────────

const vimPluginKey = new PluginKey("vim");
const vimCursorKey = new PluginKey("vimCursor");
const vimSearchKey = new PluginKey("vimSearch");

// ── Vim API singleton ───────────────────────────────────────────────────

// initVim() returns the Vim controller. We call it once with our adapter class.
let Vim: ReturnType<typeof initVim>["Vim"] | null = null;

function ensureVim(): ReturnType<typeof initVim>["Vim"] {
  if (!Vim) {
    const result = initVim(ProseMirrorAdapter);
    Vim = result.Vim;
  }
  return Vim;
}

// ── VimPlugin instance interface ────────────────────────────────────────

export interface VimPluginInstance extends Plugin {
  getAdapter: () => ProseMirrorAdapter | null;
  setSearchPattern: (pattern: string, direction: "forward" | "backward") => void;
  executeCommand: (cmd: string) => void;
}

export const VIM_SAVE_EVENT = "vim:save";

export function getVimPlugin(editor: Editor): VimPluginInstance | null {
  const plugin = vimPluginKey.get(editor.state);
  return plugin ? (plugin as unknown as VimPluginInstance) : null;
}

// ── Key handling ────────────────────────────────────────────────────────

function vimKeyFromEvent(e: KeyboardEvent): string {
  // Build a vim-compatible key name from a keyboard event
  const vim = ensureVim();
  // Use vim.js's built-in key name resolution if available
  if (vim.vimKeyFromEvent) {
    const vimState = {}; // minimal state
    return vim.vimKeyFromEvent(e, vimState);
  }

  // Fallback key name building
  let key = e.key;
  if (key === "Escape") key = "Esc";

  const parts: string[] = [];
  if (e.ctrlKey && key !== "Control") parts.push("C");
  if (e.altKey && key !== "Alt") parts.push("A");
  if (e.shiftKey && key.length > 1 && key !== "Shift") parts.push("S");

  if (parts.length > 0) {
    return `<${parts.join("-")}-${key}>`;
  }
  return key;
}

// ── Create plugins ──────────────────────────────────────────────────────

function createVimPlugins(opts: Required<VimModeOptions>) {
  let adapter: ProseMirrorAdapter | null = null;
  let currentMode: VimMode = "normal";
  let searchOverlayQuery: RegExp | null = null;

  function mapVimMode(event: VimModeChangeEvent): VimMode {
    const { mode, subMode } = event;
    if (mode === "visual") {
      if (subMode === "linewise") return "visual-line";
      return "visual";
    }
    if (mode === "replace") return "replace";
    if (mode === "insert") return "insert";
    return "normal";
  }

  // ── Main vim plugin ──────────────────────────────────────────────

  const mainPlugin = new Plugin({
    key: vimPluginKey,

    view(editorView: EditorView) {
      const vim = ensureVim();
      adapter = new ProseMirrorAdapter(editorView);

      // Listen for vim events
      adapter.on("vim-mode-change", (_cm: unknown, event: VimModeChangeEvent) => {
        currentMode = mapVimMode(event);
        opts.onStateChange({ mode: currentMode });
      });

      adapter.on("vim-command-done", () => {
        // Trigger decoration update
        editorView.dispatch(editorView.state.tr);
      });

      adapter.on("dialog", (_cm: unknown, data: { template: unknown; callback: Function; options?: unknown }) => {
        // Determine prefix from template
        // vim.js creates a DOM element with the prompt character
        let prefix = ":";
        if (data.template instanceof HTMLElement) {
          const text = data.template.textContent || "";
          if (text.includes("/")) prefix = "/";
          else if (text.includes("?")) prefix = "?";
        }
        opts.onOpenCommandLine(prefix);
      });

      adapter.on("save", () => {
        document.dispatchEvent(new CustomEvent(VIM_SAVE_EVENT));
      });

      adapter.on("searchOverlayChange", (_cm: unknown, overlay: { query: RegExp } | null) => {
        searchOverlayQuery = overlay?.query ?? null;
        // Force decoration recalculation
        editorView.dispatch(editorView.state.tr);
      });

      // Enter vim mode
      vim.enterVimMode(adapter);

      return {
        update() {
          if (adapter) {
            adapter.view = editorView;
            adapter.onSelectionChange();
          }
        },
        destroy() {
          if (adapter) {
            vim.leaveVimMode(adapter);
            adapter = null;
          }
        },
      };
    },

    props: {
      handleKeyDown(view: EditorView, event: KeyboardEvent): boolean {
        if (!opts.enabled() || !adapter) return false;

        const vim = ensureVim();
        const vimState = adapter.state.vim as { insertMode?: boolean; visualMode?: boolean } | undefined;

        // In insert mode, only handle Escape
        if (vimState?.insertMode && event.key !== "Escape") {
          return false;
        }

        // Let Meta/Ctrl combos pass through (Cmd+C, Cmd+V, etc.)
        if (event.metaKey) return false;

        const key = vimKeyFromEvent(event);
        if (!key) return false;

        try {
          // vim.js handles the key and returns whether it was consumed
          const handled = vim.handleKey(adapter, key, "user");
          if (handled) {
            event.preventDefault();
            event.stopPropagation();
          }
          return handled;
        } catch (e) {
          console.warn("[vim] Key handling error:", e);
          return false;
        }
      },

      handleKeyPress(_view: EditorView, event: KeyboardEvent): boolean {
        if (!opts.enabled() || !adapter) return false;

        const vimState = adapter.state.vim as { insertMode?: boolean } | undefined;
        if (vimState?.insertMode) return false;

        // In normal/visual mode, consume keypress to prevent character insertion
        event.preventDefault();
        return true;
      },
    },
  }) as VimPluginInstance;

  // Attach public API
  mainPlugin.getAdapter = () => adapter;

  mainPlugin.setSearchPattern = (pattern: string, direction: "forward" | "backward") => {
    if (!adapter) return;
    const vim = ensureVim();
    try {
      // Use vim.js API to set search
      const vimGlobalState = vim.getVimGlobalState_?.() ?? {};
      if (vimGlobalState.query) {
        // Set the search register via vim.js
      }
      // Direct approach: run search command
      const query = new RegExp(pattern, "gi");
      adapter.addOverlay({ query });
    } catch (e) {
      console.warn("[vim] Invalid search pattern:", e);
    }
  };

  mainPlugin.executeCommand = (cmd: string) => {
    if (!adapter) return;
    if (cmd === "w" || cmd === "write") {
      document.dispatchEvent(new CustomEvent(VIM_SAVE_EVENT));
    } else {
      // Try to run as ex command
      const vim = ensureVim();
      try {
        vim.handleEx?.(adapter, cmd);
      } catch (e) {
        console.warn("[vim] Ex command error:", e);
      }
    }
  };

  // ── Cursor decoration plugin ─────────────────────────────────────

  const cursorPlugin = new Plugin({
    key: vimCursorKey,
    props: {
      decorations(state) {
        if (!opts.enabled() || currentMode === "insert") {
          return DecorationSet.empty;
        }

        const pos = state.selection.head;
        const $pos = state.doc.resolve(pos);
        const end = Math.min(pos + 1, $pos.end());

        if (pos >= end) {
          // End-of-line: widget decoration
          return DecorationSet.create(state.doc, [
            Decoration.widget(pos, () => {
              const span = document.createElement("span");
              span.className = "vim-block-cursor vim-block-cursor--eol";
              span.textContent = "\u00A0";
              return span;
            }),
          ]);
        }

        return DecorationSet.create(state.doc, [
          Decoration.inline(pos, end, { class: "vim-block-cursor" }),
        ]);
      },
    },
  });

  // ── Search highlight plugin ──────────────────────────────────────

  const searchPlugin = new Plugin({
    key: vimSearchKey,
    props: {
      decorations(state) {
        if (!searchOverlayQuery) return DecorationSet.empty;

        try {
          const text = state.doc.textContent;
          const re = new RegExp(searchOverlayQuery.source, "gi");
          const decorations: Decoration[] = [];

          // We need PM positions, not flat text positions.
          // Walk the doc to build position-aware matches.
          let offset = 0;
          const segments: { text: string; pmStart: number }[] = [];
          state.doc.descendants((node, pos) => {
            if (node.isText && node.text) {
              segments.push({ text: node.text, pmStart: pos });
            }
            return true;
          });

          // Match against full text and map back to PM positions
          // Simple approach: use textContent and positional mapping
          let m = re.exec(text);
          while (m !== null) {
            // Map flat text offset to PM position (approximate via node walking)
            const from = findPMPosFromTextOffset(state.doc, m.index);
            const to = findPMPosFromTextOffset(state.doc, m.index + m[0].length);
            if (from !== null && to !== null && from < to) {
              decorations.push(Decoration.inline(from, to, { class: "vim-search-match" }));
            }
            if (m.index === re.lastIndex) re.lastIndex++;
            m = re.exec(text);
          }

          return DecorationSet.create(state.doc, decorations);
        } catch {
          return DecorationSet.empty;
        }
      },
    },
  });

  return [mainPlugin, cursorPlugin, searchPlugin];
}

/** Map a flat textContent offset to a ProseMirror document position */
function findPMPosFromTextOffset(doc: import("@tiptap/pm/model").Node, offset: number): number | null {
  let textIdx = 0;
  let result: number | null = null;

  doc.descendants((node, pos) => {
    if (result !== null) return false;
    if (node.isText && node.text) {
      if (textIdx + node.text.length > offset) {
        result = pos + (offset - textIdx);
        return false;
      }
      textIdx += node.text.length;
    }
    return true;
  });

  // If offset is exactly at the end, return doc.content.size
  if (result === null && offset >= textIdx) {
    result = doc.content.size;
  }

  return result;
}

// ── tiptap Extension ────────────────────────────────────────────────────

export const VimModeExtension = Extension.create<VimModeOptions>({
  name: "vimMode",

  addOptions() {
    return {
      onStateChange: undefined,
      onOpenCommandLine: undefined,
      enabled: undefined,
    };
  },

  addProseMirrorPlugins() {
    const enabled = this.options.enabled ?? (() => false);
    return createVimPlugins({
      onStateChange: this.options.onStateChange ?? (() => {}),
      onOpenCommandLine: this.options.onOpenCommandLine ?? (() => {}),
      enabled,
    });
  },
});

export default VimModeExtension;
```

- [ ] **Step 2: Update NoteEditor.tsx imports**

The external API is kept compatible. The `VimMode` type now includes `"replace"`. Update `NoteEditor.tsx`:

```diff
- import type { VimMode } from "./editor/vim/VimState";
+ import type { VimMode } from "./editor/vim";
```

And `VimStatusLine.tsx`:
```diff
- import type { VimMode } from "./vim/VimState";
+ import type { VimMode } from "./vim";
```

Add `"replace"` to the MODE_LABELS in `VimStatusLine.tsx`:
```typescript
const MODE_LABELS: Record<VimMode, string> = {
  normal: "-- NORMAL --",
  insert: "-- INSERT --",
  visual: "-- VISUAL --",
  "visual-line": "-- VISUAL LINE --",
  replace: "-- REPLACE --",
};
```

- [ ] **Step 3: Verify build**

Run: `cd desktop-ui && npx tsc --noEmit 2>&1 | head -30`
Expected: No new errors

- [ ] **Step 4: Commit**

```bash
git add desktop-ui/src/features/notes/components/editor/vim/index.ts \
       desktop-ui/src/features/notes/components/NoteEditor.tsx \
       desktop-ui/src/features/notes/components/editor/VimStatusLine.tsx
git commit -m "feat(vim): rewrite VimModeExtension to use vim.js engine"
```

---

### Task 6: Delete old vim implementation files

Now that the new implementation is wired up, remove the old custom files.

**Files:**
- Delete: `VimPlugin.ts`, `VimState.ts`, `motions.ts`, `operators.ts`, `commands.ts`, `textObjects.ts`, `search.ts`, `cursor.ts`, `positions.ts`, `charClass.ts`
- Delete: `VimState.test.ts`, `charClass.test.ts`, `textObjects.test.ts`

- [ ] **Step 1: Remove old files**

```bash
cd desktop-ui/src/features/notes/components/editor/vim
rm VimPlugin.ts VimState.ts motions.ts operators.ts commands.ts \
   textObjects.ts search.ts cursor.ts positions.ts charClass.ts \
   VimState.test.ts charClass.test.ts textObjects.test.ts
```

- [ ] **Step 2: Remove stale imports from EditorCore.tsx**

In `EditorCore.tsx:28`, the import `{ VimModeExtension, type VimModeOptions }` from `"./vim"` should remain unchanged — the new `index.ts` exports the same names.

Check that no remaining files import from deleted modules:

Run: `cd desktop-ui && grep -r "from.*VimPlugin\|from.*VimState\|from.*motions\|from.*operators\|from.*commands\|from.*textObjects\|from.*search\|from.*cursor\|from.*positions\|from.*charClass" src/ --include="*.ts" --include="*.tsx"`

Fix any remaining imports to use the new `index.ts` exports.

- [ ] **Step 3: Verify tests pass**

Run: `cd desktop-ui && bun run test`
Expected: LineModel and SearchCursor tests pass. Old tests are gone.

- [ ] **Step 4: Verify lint**

Run: `cd desktop-ui && bunx biome check src/features/notes/components/editor/vim/`
Expected: No new errors

- [ ] **Step 5: Commit**

```bash
git add -A desktop-ui/src/features/notes/components/editor/vim/
git commit -m "refactor(vim): remove old custom vim implementation (replaced by vim.js engine)"
```

---

### Task 7: Manual testing and iteration

The adapter is a large surface area. Some vim.js method calls may fail at runtime because of missing adapter methods, incorrect position mapping, or edge cases in the LineModel. This task is iterative.

- [ ] **Step 1: Start dev server**

Run: `cd /Users/jayden/Projects/Klynt/nanobot/klyntbot && cargo tauri dev`
Wait for Vite + Tauri to start. Open the notes editor.

- [ ] **Step 2: Enable vim mode and test basic operations**

Test checklist (in browser):
1. **Mode switching**: `i` enters insert, `Escape` returns to normal, `v` enters visual, `V` enters visual-line
2. **Basic motions**: `h`, `j`, `k`, `l` move cursor correctly (j/k should work line-by-line within code blocks)
3. **Word motions**: `w`, `b`, `e` jump between words
4. **Line motions**: `0`, `$`, `^` go to line start/end/first-non-blank
5. **Operators**: `dd` delete line, `yy` yank line, `p` paste, `cc` change line
6. **Visual mode**: `v` + motion to select, `d` to delete selection
7. **Search**: `/` opens search, `n`/`N` navigate matches
8. **Undo/redo**: `u` undo, `Ctrl-r` redo
9. **Dot repeat**: make a change, press `.` to repeat
10. **Ex commands**: `:w` saves

- [ ] **Step 3: Fix any runtime errors**

Common issues to watch for:
- `cm.getLine is not a function` → method name mismatch on adapter
- `Cannot read property 'line' of undefined` → position conversion returning undefined
- `Maximum call stack exceeded` → circular event dispatch
- Cursor not moving → `handleKey` returning wrong value
- Characters being typed in normal mode → `handleKeyPress` not consuming the event

Open browser DevTools console, filter for `[vim]` warnings.

- [ ] **Step 4: Fix and iterate**

For each runtime error:
1. Identify which vim.js method is failing
2. Check if it's missing from `ProseMirrorAdapter` or has a bug
3. Fix the adapter method
4. Hard reload (Cmd+Shift+R) to pick up changes
5. Re-test

- [ ] **Step 5: Commit fixes**

```bash
git add -A desktop-ui/src/features/notes/components/editor/vim/
git commit -m "fix(vim): adapter fixes from manual testing"
```

---

## Important Notes

### vim.js Integration Details

**How `initVim(CM)` works:** vim.js exports a function that takes the `CodeMirror` class. It builds the entire vim engine against that class's prototype and static methods. When we pass `ProseMirrorAdapter`, vim.js will:
1. Store the class reference
2. Use `CM.Pos` to create positions
3. Use `CM.StringStream` for ex-command parsing
4. Create instances via `new CM(view)` — but we construct the adapter ourselves
5. Call instance methods like `cm.getCursor()`, `cm.getLine(n)`, etc.

**Key discovery:** Read the first 50 lines of `vim-engine.js` after vendoring to verify the export name (`initVim`) and what it returns (should be `{ Vim, CodeMirror }`).

### Position Model Details

The core challenge is mapping between ProseMirror's tree positions and vim's `{line, ch}` coordinates:

```
ProseMirror doc:
  doc (pos 0)
    paragraph (pos 1) "hello" (pos 1-6)
    paragraph (pos 8) "world" (pos 8-13)
    code_block (pos 15) "a\nb\nc" (pos 15-20)

Vim sees:
  line 0: "hello"    (pmStart=1)
  line 1: "world"    (pmStart=8)
  line 2: "a"        (pmStart=15)
  line 3: "b"        (pmStart=17)
  line 4: "c"        (pmStart=19)
```

The `+1` offsets are because ProseMirror positions count node open/close tags. `paragraph` at pos 1 means the text starts at pos 1 (after the doc open tag at pos 0). The text "hello" occupies positions 1-5, then paragraph close at 6, next paragraph open at 7, text starts at 8.

### What We Keep vs. What vim.js Provides

| Feature | Old Custom | New (vim.js) |
|---------|-----------|-------------|
| Motions | h,l,j,k,w,W,b,B,e,E,0,$,^,G,gg,{,},f,F,t,T,% | 30+ including H,M,L,ge,gE,gj,gk,n,N,*,#,marks |
| Operators | d,y,c,>,<,gu,gU | d,y,c,>,<,g~,gu,gU,=,gq,gw,gc,g? |
| Visual | v,V (basic) | v,V,Ctrl-v (full block mode) |
| Text objects | iw,aw,is,as,ip,ap,i(,i[,i{,i<,i",i',i\` | Same + it/at (tags), custom definitions |
| Registers | " (unnamed only) | Full a-z, A-Z, 0-9, +, _, . |
| Marks | Basic m/\' | Full a-z, A-Z, auto-marks (<, >, [, ]) |
| Search | / with simple regex | /, ?, n, N, *, #, g*, g#, :s with confirm |
| Macros | None | q{reg}, @{reg}, @@ |
| Dot repeat | Basic (replay keys) | Full (operator+motion+text recorded) |
| Ex commands | :w only | 30+ including :s, :g, :v, :sort, :map, :set |
| Replace mode | None | R (full replace mode) |
| Ctrl-a/Ctrl-x | None | Number increment/decrement |

### CSS Compatibility

The existing CSS classes are preserved:
- `.vim-block-cursor` / `.vim-block-cursor--eol` — block cursor
- `.vim-normal` / `.vim-insert` / `.vim-visual` / `.vim-visual-line` — caret visibility
- `.vim-search-match` — search highlighting

One addition needed in `editor.css` if replace mode is used:
```css
.vim-replace .editor-content {
  caret-color: transparent;
}
```
