# Vim Mode Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Add comprehensive vim keybindings to the TipTap notes editor with a toggle button in the toolbar, collapsible toolbar in vim mode, and classic `-- NORMAL --` mode indicator.

**Architecture:** Custom ProseMirror plugin with a pure vim state machine. Keystrokes in Normal/Visual mode are intercepted by the plugin, processed through a state machine, and translated into ProseMirror transactions. The state machine is pure (no side effects) and independently testable. UI components handle the mode indicator, command line, and toolbar collapse.

**Tech Stack:** TypeScript, ProseMirror (via `@tiptap/pm`), TipTap Extension API, React 19, Vitest (new dev dep for unit tests), Tailwind v4 CSS tokens.

**Reference:** Design doc at `docs/plans/2026-03-07-vim-mode-design.md`.

---

## Phase 1: Pure State Machine + Test Infrastructure

### Task 1: Add vitest for unit testing

**Files:**
- Modify: `desktop-ui/package.json`

**Step 1: Install vitest**

```bash
cd desktop-ui && bun add -d vitest
```

**Step 2: Add test script to package.json**

Add to `scripts`:
```json
"test": "vitest run",
"test:watch": "vitest"
```

**Step 3: Verify**

Run: `cd desktop-ui && bun run test`
Expected: "No test files found" (no tests yet, but vitest runs)

**Step 4: Commit**

```bash
git add desktop-ui/package.json desktop-ui/bun.lock
git commit -m "chore(notes): add vitest for unit testing"
```

---

### Task 2: VimState types and initial state

**Files:**
- Create: `desktop-ui/src/components/notes/editor/vim/VimState.ts`
- Create: `desktop-ui/src/components/notes/editor/vim/VimState.test.ts`

**Step 1: Write failing test**

Create `VimState.test.ts`:

```typescript
import { describe, expect, it } from "vitest";
import { type VimState, createVimState, resetOperatorState } from "./VimState";

describe("VimState", () => {
  it("creates initial state in normal mode", () => {
    const state = createVimState();
    expect(state.mode).toBe("normal");
    expect(state.count).toBeNull();
    expect(state.operator).toBeNull();
    expect(state.awaitingChar).toBeNull();
    expect(state.gPending).toBe(false);
    expect(state.visualAnchor).toBeNull();
    expect(state.searchPattern).toBeNull();
  });

  it("resets operator state", () => {
    const state = createVimState();
    state.count = 3;
    state.operator = "d";
    state.gPending = true;
    const reset = resetOperatorState(state);
    expect(reset.count).toBeNull();
    expect(reset.operator).toBeNull();
    expect(reset.gPending).toBe(false);
    expect(reset.mode).toBe("normal");
  });
});
```

**Step 2: Run test to verify it fails**

Run: `cd desktop-ui && bun run test`
Expected: FAIL — module not found

**Step 3: Implement VimState.ts**

```typescript
export type VimMode = "normal" | "insert" | "visual" | "visual-line";

export type Operator = "d" | "c" | "y" | ">" | "<" | "gu" | "gU";

export type AwaitingChar = "f" | "F" | "t" | "T" | "r";

export type SearchDirection = "forward" | "backward";

export interface RecordedAction {
  keys: string[];
}

export interface VimState {
  mode: VimMode;
  count: number | null;
  operator: Operator | null;
  awaitingChar: AwaitingChar | null;
  registers: Record<string, string>;
  marks: Record<string, number>;
  lastAction: RecordedAction | null;
  searchPattern: string | null;
  searchDirection: SearchDirection;
  visualAnchor: number | null;
  gPending: boolean;
  /** Keys accumulated for recording lastAction */
  recording: string[];
}

export function createVimState(): VimState {
  return {
    mode: "normal",
    count: null,
    operator: null,
    awaitingChar: null,
    registers: {},
    marks: {},
    lastAction: null,
    searchPattern: null,
    searchDirection: "forward",
    visualAnchor: null,
    gPending: false,
    recording: [],
  };
}

export function resetOperatorState(state: VimState): VimState {
  return {
    ...state,
    count: null,
    operator: null,
    awaitingChar: null,
    gPending: false,
  };
}

export function enterInsertMode(state: VimState): VimState {
  return {
    ...resetOperatorState(state),
    mode: "insert",
    lastAction: state.recording.length > 0 ? { keys: [...state.recording] } : state.lastAction,
    recording: [],
  };
}

export function enterNormalMode(state: VimState): VimState {
  return {
    ...resetOperatorState(state),
    mode: "normal",
    visualAnchor: null,
  };
}

export function enterVisualMode(state: VimState, anchor: number): VimState {
  return {
    ...resetOperatorState(state),
    mode: "visual",
    visualAnchor: anchor,
  };
}

export function enterVisualLineMode(state: VimState, anchor: number): VimState {
  return {
    ...resetOperatorState(state),
    mode: "visual-line",
    visualAnchor: anchor,
  };
}

/** Accumulate a digit into the count prefix. Returns updated state. */
export function accumulateCount(state: VimState, digit: number): VimState {
  const current = state.count ?? 0;
  return { ...state, count: current * 10 + digit };
}

/** Get the effective count (default 1 if null). */
export function effectiveCount(state: VimState): number {
  return state.count ?? 1;
}
```

**Step 4: Run test**

Run: `cd desktop-ui && bun run test`
Expected: PASS

**Step 5: Commit**

```bash
git add desktop-ui/src/components/notes/editor/vim/
git commit -m "feat(notes): vim state machine types and initial state"
```

---

### Task 3: Character classification utilities

**Files:**
- Create: `desktop-ui/src/components/notes/editor/vim/charClass.ts`
- Create: `desktop-ui/src/components/notes/editor/vim/charClass.test.ts`

**Step 1: Write failing tests**

```typescript
import { describe, expect, it } from "vitest";
import { CharClass, classifyChar, findWordBoundary } from "./charClass";

describe("classifyChar", () => {
  it("classifies word characters", () => {
    expect(classifyChar("a")).toBe(CharClass.Word);
    expect(classifyChar("Z")).toBe(CharClass.Word);
    expect(classifyChar("0")).toBe(CharClass.Word);
    expect(classifyChar("_")).toBe(CharClass.Word);
  });

  it("classifies whitespace", () => {
    expect(classifyChar(" ")).toBe(CharClass.Space);
    expect(classifyChar("\t")).toBe(CharClass.Space);
    expect(classifyChar("\n")).toBe(CharClass.Space);
  });

  it("classifies punctuation", () => {
    expect(classifyChar(".")).toBe(CharClass.Punct);
    expect(classifyChar("(")).toBe(CharClass.Punct);
    expect(classifyChar("!")).toBe(CharClass.Punct);
  });
});

describe("findWordBoundary", () => {
  it("finds next word start from beginning", () => {
    const text = "hello world";
    expect(findWordBoundary(text, 0, "forward", "start")).toBe(6);
  });

  it("finds next word start across punctuation", () => {
    const text = "foo.bar baz";
    expect(findWordBoundary(text, 0, "forward", "start")).toBe(3);
  });

  it("finds previous word start", () => {
    const text = "hello world";
    expect(findWordBoundary(text, 8, "backward", "start")).toBe(6);
  });

  it("finds word end", () => {
    const text = "hello world";
    expect(findWordBoundary(text, 0, "forward", "end")).toBe(4);
  });

  it("handles end of string", () => {
    const text = "hello";
    expect(findWordBoundary(text, 0, "forward", "start")).toBe(5);
  });
});
```

**Step 2: Run — expect FAIL**

**Step 3: Implement charClass.ts**

```typescript
export enum CharClass {
  Word = "word",
  Space = "space",
  Punct = "punct",
}

export function classifyChar(ch: string): CharClass {
  if (/[\w]/.test(ch)) return CharClass.Word;
  if (/\s/.test(ch)) return CharClass.Space;
  return CharClass.Punct;
}

/**
 * Find a word boundary in flat text.
 * - direction: "forward" or "backward"
 * - boundary: "start" (w/b) or "end" (e)
 *
 * For `w` (forward, start): skip current word/punct, skip whitespace, land on next word/punct start.
 * For `b` (backward, start): skip whitespace backward, skip prev word/punct backward, land on its start.
 * For `e` (forward, end): skip whitespace, skip word/punct, land on last char of word/punct.
 */
export function findWordBoundary(
  text: string,
  pos: number,
  direction: "forward" | "backward",
  boundary: "start" | "end",
): number {
  const len = text.length;
  if (len === 0) return pos;

  if (direction === "forward" && boundary === "start") {
    // w motion: skip current class, then skip spaces
    let i = pos;
    if (i >= len) return len;
    const startClass = classifyChar(text[i]);
    // Skip current class
    while (i < len && classifyChar(text[i]) === startClass) i++;
    // Skip spaces
    while (i < len && classifyChar(text[i]) === CharClass.Space) i++;
    return i;
  }

  if (direction === "forward" && boundary === "end") {
    // e motion: skip spaces, then skip word/punct, land on last char
    let i = pos + 1;
    if (i >= len) return Math.min(pos, len - 1);
    // Skip spaces
    while (i < len && classifyChar(text[i]) === CharClass.Space) i++;
    if (i >= len) return len - 1;
    const cls = classifyChar(text[i]);
    while (i + 1 < len && classifyChar(text[i + 1]) === cls) i++;
    return i;
  }

  if (direction === "backward" && boundary === "start") {
    // b motion: move back one, skip spaces, then skip word/punct backward
    let i = pos - 1;
    if (i < 0) return 0;
    // Skip spaces
    while (i > 0 && classifyChar(text[i]) === CharClass.Space) i--;
    if (i <= 0) return 0;
    const cls = classifyChar(text[i]);
    while (i > 0 && classifyChar(text[i - 1]) === cls) i--;
    return i;
  }

  return pos;
}

/**
 * WORD boundary (W/B/E) — only whitespace is a boundary.
 * Same logic but treats everything non-space as one class.
 */
export function findWORDBoundary(
  text: string,
  pos: number,
  direction: "forward" | "backward",
  boundary: "start" | "end",
): number {
  const isSpace = (i: number) => /\s/.test(text[i]);

  if (direction === "forward" && boundary === "start") {
    let i = pos;
    if (i >= text.length) return text.length;
    // Skip non-space
    while (i < text.length && !isSpace(i)) i++;
    // Skip space
    while (i < text.length && isSpace(i)) i++;
    return i;
  }

  if (direction === "forward" && boundary === "end") {
    let i = pos + 1;
    if (i >= text.length) return Math.min(pos, text.length - 1);
    while (i < text.length && isSpace(i)) i++;
    while (i + 1 < text.length && !isSpace(i + 1)) i++;
    return i;
  }

  if (direction === "backward" && boundary === "start") {
    let i = pos - 1;
    if (i < 0) return 0;
    while (i > 0 && isSpace(i)) i--;
    while (i > 0 && !isSpace(i - 1)) i--;
    return i;
  }

  return pos;
}
```

**Step 4: Run tests**

Run: `cd desktop-ui && bun run test`
Expected: PASS

**Step 5: Commit**

```bash
git add desktop-ui/src/components/notes/editor/vim/charClass*
git commit -m "feat(notes): vim character classification and word boundary utils"
```

---

### Task 4: Text object range calculations

**Files:**
- Create: `desktop-ui/src/components/notes/editor/vim/textObjects.ts`
- Create: `desktop-ui/src/components/notes/editor/vim/textObjects.test.ts`

**Step 1: Write failing tests**

```typescript
import { describe, expect, it } from "vitest";
import { findTextObject } from "./textObjects";

describe("findTextObject", () => {
  describe("word objects", () => {
    it("inner word", () => {
      const text = "hello world";
      expect(findTextObject(text, 1, "i", "w")).toEqual({ from: 0, to: 5 });
    });

    it("around word", () => {
      const text = "hello world";
      expect(findTextObject(text, 1, "a", "w")).toEqual({ from: 0, to: 6 });
    });
  });

  describe("bracket objects", () => {
    it("inner parens", () => {
      const text = "foo(bar baz)qux";
      expect(findTextObject(text, 5, "i", "(")).toEqual({ from: 4, to: 11 });
    });

    it("around parens", () => {
      const text = "foo(bar baz)qux";
      expect(findTextObject(text, 5, "a", "(")).toEqual({ from: 3, to: 12 });
    });

    it("inner braces", () => {
      const text = 'x { y } z';
      expect(findTextObject(text, 4, "i", "{")).toEqual({ from: 3, to: 6 });
    });

    it("nested brackets — finds innermost", () => {
      const text = "((inner))";
      expect(findTextObject(text, 3, "i", "(")).toEqual({ from: 2, to: 7 });
    });
  });

  describe("quote objects", () => {
    it("inner double quote", () => {
      const text = 'say "hello" end';
      expect(findTextObject(text, 6, "i", '"')).toEqual({ from: 5, to: 10 });
    });

    it("around double quote", () => {
      const text = 'say "hello" end';
      expect(findTextObject(text, 6, "a", '"')).toEqual({ from: 4, to: 11 });
    });
  });

  describe("paragraph objects", () => {
    it("inner paragraph", () => {
      const text = "line1\nline2\n\nline3";
      expect(findTextObject(text, 2, "i", "p")).toEqual({ from: 0, to: 11 });
    });
  });
});
```

**Step 2: Run — expect FAIL**

**Step 3: Implement textObjects.ts**

```typescript
import { CharClass, classifyChar } from "./charClass";

export interface TextRange {
  from: number;
  to: number;
}

type Scope = "i" | "a"; // inner or around
type ObjectType = "w" | "W" | "s" | "p" | "(" | ")" | "b" | "[" | "]" | "{" | "}" | "B" | '"' | "'" | "`" | "<" | ">";

const BRACKET_PAIRS: Record<string, [string, string]> = {
  "(": ["(", ")"],
  ")": ["(", ")"],
  b: ["(", ")"],
  "[": ["[", "]"],
  "]": ["[", "]"],
  "{": ["{", "}"],
  "}": ["{", "}"],
  B: ["{", "}"],
  "<": ["<", ">"],
  ">": ["<", ">"],
};

const QUOTE_CHARS = new Set(['"', "'", "`"]);

export function findTextObject(
  text: string,
  pos: number,
  scope: Scope,
  type: string,
): TextRange | null {
  if (type === "w") return wordObject(text, pos, scope, false);
  if (type === "W") return wordObject(text, pos, scope, true);
  if (type === "p") return paragraphObject(text, pos, scope);
  if (type === "s") return sentenceObject(text, pos, scope);
  if (BRACKET_PAIRS[type]) return bracketObject(text, pos, scope, BRACKET_PAIRS[type]);
  if (QUOTE_CHARS.has(type)) return quoteObject(text, pos, scope, type);
  return null;
}

function wordObject(text: string, pos: number, scope: Scope, bigWord: boolean): TextRange | null {
  if (pos >= text.length) return null;

  const classify = bigWord
    ? (ch: string) => (/\s/.test(ch) ? CharClass.Space : CharClass.Word)
    : classifyChar;

  const cls = classify(text[pos]);
  let from = pos;
  let to = pos;

  // Expand backward while same class
  while (from > 0 && classify(text[from - 1]) === cls) from--;
  // Expand forward while same class
  while (to + 1 < text.length && classify(text[to + 1]) === cls) to++;
  to++; // exclusive end

  if (scope === "a") {
    // Include trailing whitespace, or leading if no trailing
    if (to < text.length && classify(text[to]) === CharClass.Space) {
      while (to < text.length && classify(text[to]) === CharClass.Space) to++;
    } else if (from > 0 && classify(text[from - 1]) === CharClass.Space) {
      while (from > 0 && classify(text[from - 1]) === CharClass.Space) from--;
    }
  }

  return { from, to };
}

function bracketObject(
  text: string,
  pos: number,
  scope: Scope,
  pair: [string, string],
): TextRange | null {
  const [open, close] = pair;

  // Search backward for opening bracket
  let depth = 0;
  let openPos = -1;
  for (let i = pos; i >= 0; i--) {
    if (text[i] === close && i !== pos) depth++;
    if (text[i] === open) {
      if (depth === 0) {
        openPos = i;
        break;
      }
      depth--;
    }
  }
  if (openPos === -1) return null;

  // Search forward for closing bracket
  depth = 0;
  let closePos = -1;
  for (let i = openPos + 1; i < text.length; i++) {
    if (text[i] === open) depth++;
    if (text[i] === close) {
      if (depth === 0) {
        closePos = i;
        break;
      }
      depth--;
    }
  }
  if (closePos === -1) return null;

  return scope === "i"
    ? { from: openPos + 1, to: closePos }
    : { from: openPos, to: closePos + 1 };
}

function quoteObject(text: string, pos: number, scope: Scope, quote: string): TextRange | null {
  // Find the pair of quotes surrounding pos
  // Scan backward for opening quote
  let openPos = -1;
  for (let i = pos; i >= 0; i--) {
    if (text[i] === quote && (i === 0 || text[i - 1] !== "\\")) {
      if (i === pos) continue; // skip if cursor is on quote — check if it's opening
      openPos = i;
      break;
    }
  }

  // If cursor is on the quote, it might be the opening one
  if (openPos === -1 && text[pos] === quote) {
    openPos = pos;
  }

  if (openPos === -1) {
    // Try forward search for opening quote
    for (let i = pos; i < text.length; i++) {
      if (text[i] === quote) {
        openPos = i;
        break;
      }
    }
  }

  if (openPos === -1) return null;

  // Find closing quote
  let closePos = -1;
  for (let i = openPos + 1; i < text.length; i++) {
    if (text[i] === quote && text[i - 1] !== "\\") {
      closePos = i;
      break;
    }
  }
  if (closePos === -1) return null;

  // Verify cursor is between quotes
  if (pos < openPos || pos > closePos) return null;

  return scope === "i"
    ? { from: openPos + 1, to: closePos }
    : { from: openPos, to: closePos + 1 };
}

function paragraphObject(text: string, pos: number, scope: Scope): TextRange | null {
  const lines = text.split("\n");
  let charCount = 0;
  let currentLine = 0;

  // Find which line pos is on
  for (let i = 0; i < lines.length; i++) {
    if (charCount + lines[i].length >= pos) {
      currentLine = i;
      break;
    }
    charCount += lines[i].length + 1; // +1 for \n
  }

  // Find paragraph boundaries (blank lines)
  let startLine = currentLine;
  while (startLine > 0 && lines[startLine - 1].trim() !== "") startLine--;

  let endLine = currentLine;
  while (endLine < lines.length - 1 && lines[endLine + 1].trim() !== "") endLine++;

  // Calculate char positions
  let from = 0;
  for (let i = 0; i < startLine; i++) from += lines[i].length + 1;

  let to = from;
  for (let i = startLine; i <= endLine; i++) {
    to += lines[i].length;
    if (i < endLine) to += 1; // newline between paragraph lines
  }

  if (scope === "a") {
    // Include trailing blank lines
    let nextLine = endLine + 1;
    while (nextLine < lines.length && lines[nextLine].trim() === "") {
      to += lines[nextLine].length + 1;
      nextLine++;
    }
  }

  return { from, to };
}

function sentenceObject(text: string, pos: number, scope: Scope): TextRange | null {
  // Simplified: sentence ends at . ! ? followed by space or end
  const sentenceEnd = /[.!?](\s|$)/g;

  // Find sentence start (after prev sentence end or doc start)
  let from = 0;
  let match: RegExpExecArray | null;
  sentenceEnd.lastIndex = 0;
  while ((match = sentenceEnd.exec(text)) !== null) {
    const endPos = match.index + 1;
    if (endPos <= pos) {
      from = endPos;
      // Skip whitespace after end
      while (from < text.length && /\s/.test(text[from])) from++;
    } else {
      break;
    }
  }

  // Find sentence end
  sentenceEnd.lastIndex = pos;
  match = sentenceEnd.exec(text);
  const to = match ? match.index + 1 : text.length;

  if (scope === "a") {
    // Include trailing whitespace
    let aTo = to;
    while (aTo < text.length && /\s/.test(text[aTo])) aTo++;
    return { from, to: aTo };
  }

  return { from, to };
}
```

**Step 4: Run tests**

Run: `cd desktop-ui && bun run test`
Expected: PASS

**Step 5: Commit**

```bash
git add desktop-ui/src/components/notes/editor/vim/textObjects*
git commit -m "feat(notes): vim text object range calculations"
```

---

## Phase 2: ProseMirror Integration

### Task 5: Position mapping helpers

**Files:**
- Create: `desktop-ui/src/components/notes/editor/vim/positions.ts`

These helpers bridge between the vim world (flat text positions, lines/columns) and ProseMirror's node-based positions.

**Step 1: Implement positions.ts**

```typescript
import type { EditorState } from "@tiptap/pm/state";
import type { EditorView } from "@tiptap/pm/view";

/** Get the flat text content with a mapping from flat index to PM position. */
export interface FlatTextMap {
  text: string;
  /** flatIndex -> PM position. Length = text.length + 1 (includes end). */
  toPM: number[];
  /** PM position -> flat index (sparse — only mapped positions). */
  toFlat: Map<number, number>;
}

export function buildFlatTextMap(state: EditorState): FlatTextMap {
  const toPM: number[] = [];
  const toFlat = new Map<number, number>();
  let flat = "";

  state.doc.descendants((node, pos) => {
    if (node.isText && node.text) {
      for (let i = 0; i < node.text.length; i++) {
        const pmPos = pos + i;
        toFlat.set(pmPos, flat.length);
        toPM.push(pmPos);
        flat += node.text[i];
      }
      return false;
    }
    if (node.isBlock && flat.length > 0 && !flat.endsWith("\n")) {
      // Add newline between block nodes
      toPM.push(pos);
      toFlat.set(pos, flat.length);
      flat += "\n";
    }
    return true;
  });

  // End position
  toPM.push(state.doc.content.size);
  toFlat.set(state.doc.content.size, flat.length);

  return { text: flat, toPM, toFlat };
}

/** Get the PM position at the start of the line containing `pos`. */
export function lineStart(state: EditorState, pos: number): number {
  const $pos = state.doc.resolve(pos);
  return $pos.start($pos.depth);
}

/** Get the PM position at the end of the line containing `pos`. */
export function lineEnd(state: EditorState, pos: number): number {
  const $pos = state.doc.resolve(pos);
  return $pos.end($pos.depth);
}

/** Get current cursor position (head of selection). */
export function cursorPos(state: EditorState): number {
  return state.selection.head;
}

/** Get {line, col} from a view (uses DOM coordinates). */
export function getLineCol(view: EditorView, pos: number): { line: number; col: number } {
  const coords = view.coordsAtPos(pos);
  const startCoords = view.coordsAtPos(lineStart(view.state, pos));
  // Approximate line from y-coordinate
  const lineHeight = 28; // approximate
  const line = Math.round((coords.top - view.dom.getBoundingClientRect().top) / lineHeight);
  const col = pos - lineStart(view.state, pos);
  return { line, col };
}

/** Move up/down by `lines` lines, trying to preserve column. */
export function verticalMove(
  view: EditorView,
  pos: number,
  lines: number,
): number {
  // Use ProseMirror's coordinate-based approach for accurate vertical movement
  const coords = view.coordsAtPos(pos);
  const lineHeight = (view.dom.querySelector(".ProseMirror p") as HTMLElement)?.offsetHeight ?? 28;
  const targetY = coords.top + lines * lineHeight;
  const targetPos = view.posAtCoords({ left: coords.left, top: targetY });
  return targetPos?.pos ?? pos;
}

/** Clamp a position to valid document range. */
export function clampPos(state: EditorState, pos: number): number {
  return Math.max(0, Math.min(pos, state.doc.content.size));
}
```

**Step 2: Verify**

Run: `cd desktop-ui && bun run build`
Expected: PASS (no type errors)

**Step 3: Commit**

```bash
git add desktop-ui/src/components/notes/editor/vim/positions.ts
git commit -m "feat(notes): vim position mapping helpers for ProseMirror"
```

---

### Task 6: Motions — cursor movement functions

**Files:**
- Create: `desktop-ui/src/components/notes/editor/vim/motions.ts`

Each motion function returns a new PM position given current state and view.

**Step 1: Implement motions.ts**

```typescript
import type { EditorState } from "@tiptap/pm/state";
import type { EditorView } from "@tiptap/pm/view";
import { findWORDBoundary, findWordBoundary } from "./charClass";
import {
  buildFlatTextMap,
  clampPos,
  cursorPos,
  lineEnd,
  lineStart,
  verticalMove,
} from "./positions";
import type { VimState } from "./VimState";
import { effectiveCount } from "./VimState";

export type MotionResult = { pos: number; linewise?: boolean };

type MotionFn = (
  view: EditorView,
  state: EditorState,
  vim: VimState,
) => MotionResult;

/** Move left by count chars. */
export const motionH: MotionFn = (view, state, vim) => {
  const pos = cursorPos(state);
  const start = lineStart(state, pos);
  return { pos: Math.max(start, pos - effectiveCount(vim)) };
};

/** Move down by count lines. */
export const motionJ: MotionFn = (view, state, vim) => {
  return { pos: verticalMove(view, cursorPos(state), effectiveCount(vim)) };
};

/** Move up by count lines. */
export const motionK: MotionFn = (view, state, vim) => {
  return { pos: verticalMove(view, cursorPos(state), -effectiveCount(vim)) };
};

/** Move right by count chars. */
export const motionL: MotionFn = (view, state, vim) => {
  const pos = cursorPos(state);
  const end = lineEnd(state, pos);
  return { pos: Math.min(end, pos + effectiveCount(vim)) };
};

/** Word forward (w). */
export const motionW: MotionFn = (view, state, vim) => {
  const map = buildFlatTextMap(state);
  const flatIdx = map.toFlat.get(cursorPos(state)) ?? 0;
  let idx = flatIdx;
  for (let i = 0; i < effectiveCount(vim); i++) {
    idx = findWordBoundary(map.text, idx, "forward", "start");
  }
  return { pos: map.toPM[Math.min(idx, map.text.length)] ?? state.doc.content.size };
};

/** WORD forward (W). */
export const motionWW: MotionFn = (view, state, vim) => {
  const map = buildFlatTextMap(state);
  let idx = map.toFlat.get(cursorPos(state)) ?? 0;
  for (let i = 0; i < effectiveCount(vim); i++) {
    idx = findWORDBoundary(map.text, idx, "forward", "start");
  }
  return { pos: map.toPM[Math.min(idx, map.text.length)] ?? state.doc.content.size };
};

/** Word backward (b). */
export const motionB: MotionFn = (view, state, vim) => {
  const map = buildFlatTextMap(state);
  let idx = map.toFlat.get(cursorPos(state)) ?? 0;
  for (let i = 0; i < effectiveCount(vim); i++) {
    idx = findWordBoundary(map.text, idx, "backward", "start");
  }
  return { pos: map.toPM[Math.max(idx, 0)] ?? 0 };
};

/** WORD backward (B). */
export const motionBB: MotionFn = (view, state, vim) => {
  const map = buildFlatTextMap(state);
  let idx = map.toFlat.get(cursorPos(state)) ?? 0;
  for (let i = 0; i < effectiveCount(vim); i++) {
    idx = findWORDBoundary(map.text, idx, "backward", "start");
  }
  return { pos: map.toPM[Math.max(idx, 0)] ?? 0 };
};

/** Word end (e). */
export const motionE: MotionFn = (view, state, vim) => {
  const map = buildFlatTextMap(state);
  let idx = map.toFlat.get(cursorPos(state)) ?? 0;
  for (let i = 0; i < effectiveCount(vim); i++) {
    idx = findWordBoundary(map.text, idx, "forward", "end");
  }
  return { pos: map.toPM[Math.min(idx, map.text.length - 1)] ?? state.doc.content.size };
};

/** WORD end (E). */
export const motionEE: MotionFn = (view, state, vim) => {
  const map = buildFlatTextMap(state);
  let idx = map.toFlat.get(cursorPos(state)) ?? 0;
  for (let i = 0; i < effectiveCount(vim); i++) {
    idx = findWORDBoundary(map.text, idx, "forward", "end");
  }
  return { pos: map.toPM[Math.min(idx, map.text.length - 1)] ?? state.doc.content.size };
};

/** Line start (0). */
export const motion0: MotionFn = (_view, state) => {
  return { pos: lineStart(state, cursorPos(state)) };
};

/** First non-whitespace (^). */
export const motionCaret: MotionFn = (_view, state) => {
  const start = lineStart(state, cursorPos(state));
  const end = lineEnd(state, cursorPos(state));
  const text = state.doc.textBetween(start, end);
  const match = text.match(/^\s*/);
  const offset = match ? match[0].length : 0;
  return { pos: start + offset };
};

/** Line end ($). */
export const motionDollar: MotionFn = (_view, state, vim) => {
  let pos = cursorPos(state);
  for (let i = 0; i < effectiveCount(vim) - 1; i++) {
    pos = lineEnd(state, pos) + 1;
  }
  return { pos: lineEnd(state, pos) };
};

/** Document start (gg). */
export const motionGG: MotionFn = (_view, state, vim) => {
  if (vim.count !== null) {
    // Go to line N — approximate via proportion
    const target = Math.max(0, vim.count - 1);
    // Walk through block nodes to find Nth line
    let lineCount = 0;
    let targetPos = 0;
    state.doc.descendants((node, pos) => {
      if (node.isBlock && node.isTextblock) {
        if (lineCount === target) {
          targetPos = pos + 1;
          return false;
        }
        lineCount++;
      }
      return true;
    });
    return { pos: targetPos };
  }
  return { pos: 0 };
};

/** Document end (G). */
export const motionG: MotionFn = (_view, state, vim) => {
  if (vim.count !== null) return motionGG(_view, state, vim);
  return { pos: state.doc.content.size };
};

/** Paragraph forward (}). */
export const motionParaDown: MotionFn = (_view, state, vim) => {
  const map = buildFlatTextMap(state);
  let idx = map.toFlat.get(cursorPos(state)) ?? 0;
  for (let i = 0; i < effectiveCount(vim); i++) {
    // Skip current non-blank lines
    while (idx < map.text.length && map.text[idx] !== "\n") idx++;
    // Skip blank lines
    while (idx < map.text.length && map.text[idx] === "\n") idx++;
    // Skip to next blank line
    while (idx < map.text.length && map.text[idx] !== "\n") idx++;
  }
  return { pos: map.toPM[Math.min(idx, map.text.length)] ?? state.doc.content.size };
};

/** Paragraph backward ({). */
export const motionParaUp: MotionFn = (_view, state, vim) => {
  const map = buildFlatTextMap(state);
  let idx = map.toFlat.get(cursorPos(state)) ?? 0;
  for (let i = 0; i < effectiveCount(vim); i++) {
    // Skip to previous blank line boundary
    if (idx > 0) idx--;
    while (idx > 0 && map.text[idx] !== "\n") idx--;
    while (idx > 0 && map.text[idx] === "\n") idx--;
    while (idx > 0 && map.text[idx - 1] !== "\n") idx--;
  }
  return { pos: map.toPM[Math.max(idx, 0)] ?? 0 };
};

/** Find char forward (f{char}). */
export function motionFindChar(
  state: EditorState,
  char: string,
  forward: boolean,
  till: boolean,
  count: number,
): MotionResult | null {
  const pos = cursorPos(state);
  const start = lineStart(state, pos);
  const end = lineEnd(state, pos);
  const lineText = state.doc.textBetween(start, end);
  const col = pos - start;

  let found = -1;
  let hits = 0;

  if (forward) {
    for (let i = col + 1; i < lineText.length; i++) {
      if (lineText[i] === char) {
        hits++;
        if (hits === count) {
          found = i;
          break;
        }
      }
    }
  } else {
    for (let i = col - 1; i >= 0; i--) {
      if (lineText[i] === char) {
        hits++;
        if (hits === count) {
          found = i;
          break;
        }
      }
    }
  }

  if (found === -1) return null;
  if (till) found += forward ? -1 : 1;
  return { pos: start + found };
}

/** Match bracket (%). */
export const motionMatchBracket: MotionFn = (_view, state) => {
  const pos = cursorPos(state);
  const map = buildFlatTextMap(state);
  const flatIdx = map.toFlat.get(pos);
  if (flatIdx === undefined) return { pos };

  const ch = map.text[flatIdx];
  const pairs: Record<string, { match: string; dir: 1 | -1 }> = {
    "(": { match: ")", dir: 1 },
    ")": { match: "(", dir: -1 },
    "[": { match: "]", dir: 1 },
    "]": { match: "[", dir: -1 },
    "{": { match: "}", dir: 1 },
    "}": { match: "{", dir: -1 },
  };

  const pair = pairs[ch];
  if (!pair) return { pos };

  let depth = 0;
  let i = flatIdx;
  while (i >= 0 && i < map.text.length) {
    if (map.text[i] === ch) depth++;
    if (map.text[i] === pair.match) depth--;
    if (depth === 0) return { pos: map.toPM[i] ?? pos };
    i += pair.dir;
  }

  return { pos };
};

/** Registry of motion keys -> functions. */
export const MOTIONS: Record<string, MotionFn> = {
  h: motionH,
  j: motionJ,
  k: motionK,
  l: motionL,
  w: motionW,
  W: motionWW,
  b: motionB,
  B: motionBB,
  e: motionE,
  E: motionEE,
  "0": motion0,
  "^": motionCaret,
  $: motionDollar,
  G: motionG,
  "%": motionMatchBracket,
  "{": motionParaUp,
  "}": motionParaDown,
};
```

**Step 2: Verify**

Run: `cd desktop-ui && bun run build`
Expected: PASS

**Step 3: Commit**

```bash
git add desktop-ui/src/components/notes/editor/vim/motions.ts
git commit -m "feat(notes): vim motion functions for ProseMirror"
```

---

### Task 7: Operators and commands

**Files:**
- Create: `desktop-ui/src/components/notes/editor/vim/operators.ts`
- Create: `desktop-ui/src/components/notes/editor/vim/commands.ts`

**Step 1: Implement operators.ts**

Operators take a range and produce a ProseMirror transaction.

```typescript
import { TextSelection } from "@tiptap/pm/state";
import type { EditorView } from "@tiptap/pm/view";
import { lineEnd, lineStart } from "./positions";
import type { VimState } from "./VimState";

/** Delete a range and yank to register. */
export function operatorDelete(
  view: EditorView,
  from: number,
  to: number,
  vim: VimState,
  linewise: boolean,
): VimState {
  const { state } = view;
  const text = state.doc.textBetween(from, to);
  const reg = { ...vim.registers, "": text };

  if (linewise) {
    // Expand to full lines
    from = lineStart(state, from);
    to = lineEnd(state, to);
    // Include the newline after if possible
    if (to < state.doc.content.size) to += 1;
  }

  const tr = state.tr.delete(from, to);
  tr.setSelection(TextSelection.near(tr.doc.resolve(Math.min(from, tr.doc.content.size))));
  view.dispatch(tr);

  return { ...vim, registers: reg };
}

/** Yank a range to register (no deletion). */
export function operatorYank(
  view: EditorView,
  from: number,
  to: number,
  vim: VimState,
): VimState {
  const text = view.state.doc.textBetween(from, to);
  return { ...vim, registers: { ...vim.registers, "": text } };
}

/** Change a range: delete + enter insert mode. */
export function operatorChange(
  view: EditorView,
  from: number,
  to: number,
  vim: VimState,
): VimState {
  const newVim = operatorDelete(view, from, to, vim, false);
  return { ...newVim, mode: "insert" };
}

/** Indent a range. */
export function operatorIndent(
  view: EditorView,
  from: number,
  to: number,
  _vim: VimState,
  outdent: boolean,
): void {
  // Use TipTap's built-in list commands if available, otherwise manipulate content
  const { tr, doc } = view.state;
  doc.nodesBetween(from, to, (node, pos) => {
    if (node.isTextblock) {
      const text = node.textContent;
      if (outdent) {
        // Remove leading 2 spaces or tab
        if (text.startsWith("  ")) {
          tr.replaceWith(pos + 1, pos + 3, []);
        } else if (text.startsWith("\t")) {
          tr.replaceWith(pos + 1, pos + 2, []);
        }
      } else {
        // Add 2 spaces at start
        tr.insertText("  ", pos + 1);
      }
    }
    return true;
  });
  view.dispatch(tr);
}

/** Toggle case of text in range. */
export function operatorToggleCase(view: EditorView, from: number, to: number): void {
  const text = view.state.doc.textBetween(from, to);
  const toggled = text
    .split("")
    .map((ch) => (ch === ch.toLowerCase() ? ch.toUpperCase() : ch.toLowerCase()))
    .join("");
  const tr = view.state.tr.insertText(toggled, from, to);
  view.dispatch(tr);
}

/** Lowercase range. */
export function operatorLowercase(view: EditorView, from: number, to: number): void {
  const text = view.state.doc.textBetween(from, to);
  view.dispatch(view.state.tr.insertText(text.toLowerCase(), from, to));
}

/** Uppercase range. */
export function operatorUppercase(view: EditorView, from: number, to: number): void {
  const text = view.state.doc.textBetween(from, to);
  view.dispatch(view.state.tr.insertText(text.toUpperCase(), from, to));
}
```

**Step 2: Implement commands.ts**

Direct commands (enter insert, paste, undo, join, etc.).

```typescript
import { TextSelection } from "@tiptap/pm/state";
import type { EditorView } from "@tiptap/pm/view";
import { lineEnd, lineStart } from "./positions";
import type { VimState } from "./VimState";
import { effectiveCount, enterInsertMode } from "./VimState";
import { operatorDelete } from "./operators";

/** Enter insert mode at cursor. */
export function cmdInsert(view: EditorView, vim: VimState): VimState {
  return enterInsertMode(vim);
}

/** Enter insert mode after cursor. */
export function cmdAppend(view: EditorView, vim: VimState): VimState {
  const pos = view.state.selection.head;
  const end = lineEnd(view.state, pos);
  const newPos = Math.min(pos + 1, end);
  const tr = view.state.tr.setSelection(TextSelection.create(view.state.doc, newPos));
  view.dispatch(tr);
  return enterInsertMode(vim);
}

/** Enter insert mode at line start. */
export function cmdInsertLineStart(view: EditorView, vim: VimState): VimState {
  const pos = view.state.selection.head;
  const start = lineStart(view.state, pos);
  // Skip whitespace
  const text = view.state.doc.textBetween(start, lineEnd(view.state, pos));
  const offset = text.match(/^\s*/)?.[0].length ?? 0;
  const tr = view.state.tr.setSelection(TextSelection.create(view.state.doc, start + offset));
  view.dispatch(tr);
  return enterInsertMode(vim);
}

/** Enter insert mode at line end. */
export function cmdAppendLineEnd(view: EditorView, vim: VimState): VimState {
  const pos = view.state.selection.head;
  const end = lineEnd(view.state, pos);
  const tr = view.state.tr.setSelection(TextSelection.create(view.state.doc, end));
  view.dispatch(tr);
  return enterInsertMode(vim);
}

/** Open line below and enter insert. */
export function cmdOpenBelow(view: EditorView, vim: VimState): VimState {
  const pos = view.state.selection.head;
  const end = lineEnd(view.state, pos);
  // Insert newline after current line
  const tr = view.state.tr.insert(end, view.state.schema.nodes.paragraph.create());
  tr.setSelection(TextSelection.near(tr.doc.resolve(end + 1)));
  view.dispatch(tr);
  return enterInsertMode(vim);
}

/** Open line above and enter insert. */
export function cmdOpenAbove(view: EditorView, vim: VimState): VimState {
  const pos = view.state.selection.head;
  const start = lineStart(view.state, pos);
  const insertPos = Math.max(0, start - 1);
  const tr = view.state.tr.insert(insertPos, view.state.schema.nodes.paragraph.create());
  tr.setSelection(TextSelection.near(tr.doc.resolve(insertPos + 1)));
  view.dispatch(tr);
  return enterInsertMode(vim);
}

/** Delete char under cursor (x). */
export function cmdDeleteChar(view: EditorView, vim: VimState): VimState {
  const pos = view.state.selection.head;
  const end = lineEnd(view.state, pos);
  const count = Math.min(effectiveCount(vim), end - pos);
  if (count <= 0) return vim;
  return operatorDelete(view, pos, pos + count, vim, false);
}

/** Delete char before cursor (X). */
export function cmdDeleteCharBefore(view: EditorView, vim: VimState): VimState {
  const pos = view.state.selection.head;
  const start = lineStart(view.state, pos);
  const count = Math.min(effectiveCount(vim), pos - start);
  if (count <= 0) return vim;
  return operatorDelete(view, pos - count, pos, vim, false);
}

/** Delete entire line (dd). */
export function cmdDeleteLine(view: EditorView, vim: VimState): VimState {
  const pos = view.state.selection.head;
  const from = lineStart(view.state, pos);
  let to = lineEnd(view.state, pos);
  // Include newline
  if (to < view.state.doc.content.size) to += 1;
  return operatorDelete(view, from, to, vim, false);
}

/** Yank entire line (yy). */
export function cmdYankLine(view: EditorView, vim: VimState): VimState {
  const pos = view.state.selection.head;
  const from = lineStart(view.state, pos);
  const to = lineEnd(view.state, pos);
  const text = view.state.doc.textBetween(from, to);
  return { ...vim, registers: { ...vim.registers, "": text + "\n" } };
}

/** Change entire line (cc). */
export function cmdChangeLine(view: EditorView, vim: VimState): VimState {
  const pos = view.state.selection.head;
  const from = lineStart(view.state, pos);
  const to = lineEnd(view.state, pos);
  const newVim = operatorDelete(view, from, to, vim, false);
  return { ...newVim, mode: "insert" };
}

/** Paste after cursor (p). */
export function cmdPaste(view: EditorView, vim: VimState): VimState {
  const text = vim.registers[""] ?? "";
  if (!text) return vim;

  const pos = view.state.selection.head;
  const isLinewise = text.endsWith("\n");

  if (isLinewise) {
    const end = lineEnd(view.state, pos);
    const insertPos = Math.min(end + 1, view.state.doc.content.size);
    const tr = view.state.tr.insertText(text.replace(/\n$/, ""), insertPos);
    // Insert a paragraph break
    view.dispatch(tr);
  } else {
    const insertPos = pos + 1;
    const tr = view.state.tr.insertText(text, insertPos);
    tr.setSelection(TextSelection.create(tr.doc, insertPos + text.length - 1));
    view.dispatch(tr);
  }
  return vim;
}

/** Paste before cursor (P). */
export function cmdPasteBefore(view: EditorView, vim: VimState): VimState {
  const text = vim.registers[""] ?? "";
  if (!text) return vim;

  const pos = view.state.selection.head;
  const tr = view.state.tr.insertText(text.replace(/\n$/, ""), pos);
  tr.setSelection(TextSelection.create(tr.doc, pos));
  view.dispatch(tr);
  return vim;
}

/** Join current line with next (J). */
export function cmdJoinLines(view: EditorView, vim: VimState): VimState {
  const pos = view.state.selection.head;
  const end = lineEnd(view.state, pos);
  if (end >= view.state.doc.content.size) return vim;

  // Replace the newline + leading whitespace of next line with a single space
  const nextStart = end + 1;
  const $next = view.state.doc.resolve(nextStart);
  const nextText = $next.parent.textContent;
  const leadingWS = nextText.match(/^\s*/)?.[0].length ?? 0;

  const tr = view.state.tr.replaceWith(end, nextStart + leadingWS, view.state.schema.text(" "));
  view.dispatch(tr);
  return vim;
}

/** Replace char under cursor (r{char}). */
export function cmdReplaceChar(view: EditorView, vim: VimState, char: string): VimState {
  const pos = view.state.selection.head;
  const tr = view.state.tr.insertText(char, pos, pos + 1);
  tr.setSelection(TextSelection.create(tr.doc, pos));
  view.dispatch(tr);
  return vim;
}

/** Toggle case of char under cursor (~). */
export function cmdToggleCaseChar(view: EditorView, vim: VimState): VimState {
  const pos = view.state.selection.head;
  const end = lineEnd(view.state, pos);
  if (pos >= end) return vim;

  const ch = view.state.doc.textBetween(pos, pos + 1);
  const toggled = ch === ch.toLowerCase() ? ch.toUpperCase() : ch.toLowerCase();
  const tr = view.state.tr.insertText(toggled, pos, pos + 1);
  tr.setSelection(TextSelection.create(tr.doc, pos + 1));
  view.dispatch(tr);
  return vim;
}

/** Undo. */
export function cmdUndo(view: EditorView, vim: VimState): VimState {
  const { undo } = require("@tiptap/pm/history");
  undo(view.state, view.dispatch);
  return vim;
}

/** Redo. */
export function cmdRedo(view: EditorView, vim: VimState): VimState {
  const { redo } = require("@tiptap/pm/history");
  redo(view.state, view.dispatch);
  return vim;
}
```

**Step 2: Verify**

Run: `cd desktop-ui && bun run build`
Expected: PASS

**Step 3: Commit**

```bash
git add desktop-ui/src/components/notes/editor/vim/operators.ts desktop-ui/src/components/notes/editor/vim/commands.ts
git commit -m "feat(notes): vim operators and commands for ProseMirror"
```

---

### Task 8: Block cursor decoration

**Files:**
- Create: `desktop-ui/src/components/notes/editor/vim/cursor.ts`
- Modify: `desktop-ui/src/styles/editor.css`

**Step 1: Implement cursor.ts**

A ProseMirror decoration plugin that draws a block cursor in Normal/Visual mode.

```typescript
import { Plugin, PluginKey } from "@tiptap/pm/state";
import { Decoration, DecorationSet } from "@tiptap/pm/view";
import type { VimMode } from "./VimState";

export const vimCursorPluginKey = new PluginKey("vimCursor");

export function createVimCursorPlugin(getMode: () => VimMode) {
  return new Plugin({
    key: vimCursorPluginKey,
    props: {
      decorations(state) {
        const mode = getMode();
        if (mode === "insert") return DecorationSet.empty;

        const pos = state.selection.head;
        // Create a 1-char-wide decoration at cursor position
        const $pos = state.doc.resolve(pos);
        const end = Math.min(pos + 1, $pos.end());

        if (pos >= end) {
          // At end of line — use widget decoration
          return DecorationSet.create(state.doc, [
            Decoration.widget(pos, () => {
              const span = document.createElement("span");
              span.className = "vim-block-cursor vim-block-cursor--eol";
              span.textContent = "\u00A0"; // nbsp
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
}
```

**Step 2: Add cursor CSS to editor.css**

Append to `desktop-ui/src/styles/editor.css`:

```css
/* Vim block cursor */
.vim-block-cursor {
  background: rgba(255, 255, 255, 0.7);
  color: rgba(0, 0, 0, 0.9);
  outline: none;
}

.vim-block-cursor--eol {
  display: inline-block;
  width: 0.6em;
  background: rgba(255, 255, 255, 0.7);
}

/* Hide native caret in vim normal/visual mode */
.editor-content.vim-normal,
.editor-content.vim-visual,
.editor-content.vim-visual-line {
  caret-color: transparent;
}

/* Restore caret in insert mode */
.editor-content.vim-insert {
  caret-color: var(--brand);
}
```

**Step 3: Verify**

Run: `cd desktop-ui && bun run build`
Expected: PASS

**Step 4: Commit**

```bash
git add desktop-ui/src/components/notes/editor/vim/cursor.ts desktop-ui/src/styles/editor.css
git commit -m "feat(notes): vim block cursor decoration and CSS"
```

---

### Task 9: Search — pattern matching and decorations

**Files:**
- Create: `desktop-ui/src/components/notes/editor/vim/search.ts`

**Step 1: Implement search.ts**

```typescript
import { Plugin, PluginKey } from "@tiptap/pm/state";
import { Decoration, DecorationSet } from "@tiptap/pm/view";
import { buildFlatTextMap } from "./positions";

export const vimSearchPluginKey = new PluginKey("vimSearch");

export function createVimSearchPlugin(getPattern: () => string | null) {
  return new Plugin({
    key: vimSearchPluginKey,
    props: {
      decorations(state) {
        const pattern = getPattern();
        if (!pattern) return DecorationSet.empty;

        try {
          const regex = new RegExp(pattern, "gi");
          const map = buildFlatTextMap(state);
          const decorations: Decoration[] = [];

          let match: RegExpExecArray | null;
          while ((match = regex.exec(map.text)) !== null) {
            const from = map.toPM[match.index];
            const to = map.toPM[match.index + match[0].length];
            if (from !== undefined && to !== undefined) {
              decorations.push(Decoration.inline(from, to, { class: "vim-search-match" }));
            }
            if (match.index === regex.lastIndex) regex.lastIndex++;
          }

          return DecorationSet.create(state.doc, decorations);
        } catch {
          return DecorationSet.empty;
        }
      },
    },
  });
}

/** Find next match position from a given doc position. */
export function findNextMatch(
  text: string,
  pattern: string,
  fromFlat: number,
  direction: "forward" | "backward",
): number | null {
  try {
    const regex = new RegExp(pattern, "gi");

    if (direction === "forward") {
      regex.lastIndex = fromFlat + 1;
      let match = regex.exec(text);
      if (!match) {
        // Wrap around
        regex.lastIndex = 0;
        match = regex.exec(text);
      }
      return match ? match.index : null;
    }

    // Backward: find all matches, return the one before fromFlat
    const matches: number[] = [];
    let m: RegExpExecArray | null;
    while ((m = regex.exec(text)) !== null) {
      matches.push(m.index);
      if (m.index === regex.lastIndex) regex.lastIndex++;
    }

    // Find last match before current position
    for (let i = matches.length - 1; i >= 0; i--) {
      if (matches[i] < fromFlat) return matches[i];
    }
    // Wrap around
    return matches.length > 0 ? matches[matches.length - 1] : null;
  } catch {
    return null;
  }
}
```

**Step 2: Add search highlight CSS to editor.css**

Append:

```css
/* Vim search highlights */
.vim-search-match {
  background: rgba(249, 115, 22, 0.25);
  border-radius: 2px;
  box-shadow: 0 0 0 1px rgba(249, 115, 22, 0.3);
}
```

**Step 3: Verify build, commit**

```bash
git add desktop-ui/src/components/notes/editor/vim/search.ts desktop-ui/src/styles/editor.css
git commit -m "feat(notes): vim search pattern matching and highlight decorations"
```

---

### Task 10: VimPlugin — the ProseMirror keydown handler

**Files:**
- Create: `desktop-ui/src/components/notes/editor/vim/VimPlugin.ts`

This is the central plugin that intercepts keystrokes and orchestrates the state machine, motions, operators, and commands.

**Step 1: Implement VimPlugin.ts**

```typescript
import { Plugin, PluginKey, TextSelection } from "@tiptap/pm/state";
import type { EditorView } from "@tiptap/pm/view";
import {
  accumulateCount,
  type AwaitingChar,
  createVimState,
  effectiveCount,
  enterInsertMode,
  enterNormalMode,
  enterVisualLineMode,
  enterVisualMode,
  type Operator,
  resetOperatorState,
  type VimState,
} from "./VimState";
import { MOTIONS, motionFindChar } from "./motions";
import {
  operatorChange,
  operatorDelete,
  operatorIndent,
  operatorLowercase,
  operatorToggleCase,
  operatorUppercase,
  operatorYank,
} from "./operators";
import {
  cmdAppend,
  cmdAppendLineEnd,
  cmdChangeLine,
  cmdDeleteChar,
  cmdDeleteCharBefore,
  cmdDeleteLine,
  cmdInsert,
  cmdInsertLineStart,
  cmdJoinLines,
  cmdOpenAbove,
  cmdOpenBelow,
  cmdPaste,
  cmdPasteBefore,
  cmdReplaceChar,
  cmdToggleCaseChar,
  cmdYankLine,
} from "./commands";
import { buildFlatTextMap, clampPos, cursorPos, lineEnd, lineStart } from "./positions";
import { findTextObject } from "./textObjects";
import { findNextMatch } from "./search";

export const vimPluginKey = new PluginKey("vim");

export interface VimPluginOptions {
  onStateChange: (state: VimState) => void;
  onOpenCommandLine: (prefix: string) => void;
}

const OPERATORS = new Set(["d", "c", "y"]);
const AWAITING_CHAR_KEYS = new Set(["f", "F", "t", "T", "r"]);
const TEXT_OBJECT_SCOPES = new Set(["i", "a"]);
const TEXT_OBJECT_TARGETS = new Set(["w", "W", "s", "p", "(", ")", "b", "[", "]", "{", "}", "B", '"', "'", "`", "<", ">"]);

export function createVimPlugin(options: VimPluginOptions) {
  let vim = createVimState();
  const notify = () => options.onStateChange({ ...vim });

  function getVimState(): VimState {
    return vim;
  }

  const plugin = new Plugin({
    key: vimPluginKey,
    props: {
      handleKeyDown(view: EditorView, event: KeyboardEvent): boolean {
        // In insert mode, only intercept Escape
        if (vim.mode === "insert") {
          if (event.key === "Escape") {
            event.preventDefault();
            vim = enterNormalMode(vim);
            // Move cursor back one (vim convention)
            const pos = cursorPos(view.state);
            const start = lineStart(view.state, pos);
            if (pos > start) {
              const tr = view.state.tr.setSelection(
                TextSelection.create(view.state.doc, pos - 1),
              );
              view.dispatch(tr);
            }
            notify();
            return true;
          }
          return false; // Let TipTap handle normal typing
        }

        // Ignore modifier-only keys and Ctrl/Meta combos (let browser handle)
        if (event.key === "Control" || event.key === "Meta" || event.key === "Alt" || event.key === "Shift") {
          return false;
        }
        if (event.ctrlKey && event.key === "r") {
          // Ctrl+R = redo
          event.preventDefault();
          const { redo } = require("@tiptap/pm/history") as typeof import("@tiptap/pm/history");
          redo(view.state, view.dispatch);
          return true;
        }
        if (event.metaKey || (event.ctrlKey && event.key !== "r")) {
          return false; // Let browser/system shortcuts through
        }

        event.preventDefault();
        const key = event.key;

        // ── Awaiting char (f/F/t/T/r) ─────────────────────────────────
        if (vim.awaitingChar) {
          if (key.length !== 1) {
            vim = resetOperatorState(vim);
            notify();
            return true;
          }
          handleAwaitingChar(view, key);
          notify();
          return true;
        }

        // ── g-prefix commands ──────────────────────────────────────────
        if (vim.gPending) {
          vim = { ...vim, gPending: false };
          if (key === "g") {
            // gg — go to top
            const motion = MOTIONS.G;
            if (motion) {
              const result = MOTIONS.G(view, view.state, { ...vim, count: vim.count ?? 0 === 0 ? null : vim.count });
              // Actually gg goes to start
              const pos = vim.count !== null
                ? MOTIONS.G(view, view.state, vim).pos
                : 0;
              applyMotionOrOperator(view, pos, false);
            }
            notify();
            return true;
          }
          if (key === "u") {
            // gu{motion} — lowercase
            vim = { ...vim, operator: "gu" };
            notify();
            return true;
          }
          if (key === "U") {
            // gU{motion} — uppercase
            vim = { ...vim, operator: "gU" };
            notify();
            return true;
          }
          vim = resetOperatorState(vim);
          notify();
          return true;
        }

        // ── Text object scope (i/a after operator) ────────────────────
        if (vim.operator && TEXT_OBJECT_SCOPES.has(key)) {
          // Next key should be the text object target
          vim = { ...vim, awaitingChar: key as AwaitingChar };
          // Repurpose awaitingChar for text object — we'll handle it specially
          // Actually, let's use a different approach: just wait for next key
          const origAwait = vim.awaitingChar;
          const handler = (e: KeyboardEvent) => {
            e.preventDefault();
            document.removeEventListener("keydown", handler, true);
            const target = e.key;
            vim = { ...vim, awaitingChar: null };
            if (TEXT_OBJECT_TARGETS.has(target)) {
              handleTextObject(view, key as "i" | "a", target);
            } else {
              vim = resetOperatorState(vim);
            }
            notify();
          };
          vim = { ...vim, awaitingChar: null };
          document.addEventListener("keydown", handler, true);
          return true;
        }

        // ── Count prefix ──────────────────────────────────────────────
        if (/^[1-9]$/.test(key) || (key === "0" && vim.count !== null)) {
          vim = accumulateCount(vim, Number.parseInt(key));
          notify();
          return true;
        }

        // ── Operators ─────────────────────────────────────────────────
        if (OPERATORS.has(key)) {
          if (vim.operator === key) {
            // Doubled operator (dd, yy, cc) — operate on line
            handleDoubledOperator(view, key as Operator);
            notify();
            return true;
          }
          vim = { ...vim, operator: key as Operator };
          notify();
          return true;
        }

        // ── Indent operators ──────────────────────────────────────────
        if (key === ">" || key === "<") {
          if (vim.operator === key) {
            // >> or << — indent/outdent current line
            const pos = cursorPos(view.state);
            operatorIndent(view, lineStart(view.state, pos), lineEnd(view.state, pos), vim, key === "<");
            vim = resetOperatorState(vim);
            notify();
            return true;
          }
          vim = { ...vim, operator: key as Operator };
          notify();
          return true;
        }

        // ── g prefix ──────────────────────────────────────────────────
        if (key === "g") {
          vim = { ...vim, gPending: true };
          notify();
          return true;
        }

        // ── Awaiting char triggers ────────────────────────────────────
        if (AWAITING_CHAR_KEYS.has(key)) {
          vim = { ...vim, awaitingChar: key as AwaitingChar };
          notify();
          return true;
        }

        // ── Motions ───────────────────────────────────────────────────
        const motionFn = MOTIONS[key];
        if (motionFn) {
          const result = motionFn(view, view.state, vim);
          applyMotionOrOperator(view, result.pos, result.linewise ?? false);
          notify();
          return true;
        }

        // ── Direct commands ───────────────────────────────────────────
        vim = handleCommand(view, key) ?? vim;
        notify();
        return true;
      },
    },
  });

  function handleAwaitingChar(view: EditorView, char: string) {
    const type = vim.awaitingChar!;
    vim = { ...vim, awaitingChar: null };

    if (type === "r") {
      vim = cmdReplaceChar(view, vim, char);
      vim = resetOperatorState(vim);
      return;
    }

    // f/F/t/T
    const forward = type === "f" || type === "t";
    const till = type === "t" || type === "T";
    const result = motionFindChar(view.state, char, forward, till, effectiveCount(vim));
    if (result) {
      applyMotionOrOperator(view, result.pos, false);
    }
    vim = resetOperatorState(vim);
  }

  function handleTextObject(view: EditorView, scope: "i" | "a", target: string) {
    const map = buildFlatTextMap(view.state);
    const flatPos = map.toFlat.get(cursorPos(view.state)) ?? 0;
    const range = findTextObject(map.text, flatPos, scope, target);

    if (!range) {
      vim = resetOperatorState(vim);
      return;
    }

    const pmFrom = map.toPM[range.from] ?? 0;
    const pmTo = map.toPM[Math.min(range.to, map.text.length)] ?? view.state.doc.content.size;

    if (vim.operator) {
      applyOperatorOnRange(view, pmFrom, pmTo);
    } else if (vim.mode === "visual" || vim.mode === "visual-line") {
      // Extend selection to text object range
      const tr = view.state.tr.setSelection(TextSelection.create(view.state.doc, pmFrom, pmTo));
      view.dispatch(tr);
    }
  }

  function handleDoubledOperator(view: EditorView, op: Operator) {
    switch (op) {
      case "d":
        vim = cmdDeleteLine(view, vim);
        break;
      case "y":
        vim = cmdYankLine(view, vim);
        break;
      case "c":
        vim = cmdChangeLine(view, vim);
        break;
    }
    vim = resetOperatorState(vim);
  }

  function applyMotionOrOperator(view: EditorView, targetPos: number, linewise: boolean) {
    const pos = clampPos(view.state, targetPos);

    if (vim.operator) {
      const from = Math.min(cursorPos(view.state), pos);
      const to = Math.max(cursorPos(view.state), pos);
      applyOperatorOnRange(view, from, to);
      return;
    }

    if (vim.mode === "visual" || vim.mode === "visual-line") {
      // Extend selection
      const anchor = vim.visualAnchor ?? cursorPos(view.state);
      const tr = view.state.tr.setSelection(TextSelection.create(view.state.doc, anchor, pos));
      view.dispatch(tr);
      vim = resetOperatorState(vim);
      return;
    }

    // Just move cursor
    const tr = view.state.tr.setSelection(TextSelection.create(view.state.doc, pos));
    view.dispatch(tr);
    vim = resetOperatorState(vim);
  }

  function applyOperatorOnRange(view: EditorView, from: number, to: number) {
    const op = vim.operator!;
    switch (op) {
      case "d":
        vim = operatorDelete(view, from, to, vim, false);
        break;
      case "c":
        vim = operatorChange(view, from, to, vim);
        break;
      case "y":
        vim = operatorYank(view, from, to, vim);
        break;
      case ">":
        operatorIndent(view, from, to, vim, false);
        break;
      case "<":
        operatorIndent(view, from, to, vim, true);
        break;
      case "gu":
        operatorLowercase(view, from, to);
        break;
      case "gU":
        operatorUppercase(view, from, to);
        break;
    }
    vim = resetOperatorState(vim);
  }

  function handleCommand(view: EditorView, key: string): VimState | null {
    switch (key) {
      case "i": return resetOperatorState(cmdInsert(view, vim));
      case "a": return resetOperatorState(cmdAppend(view, vim));
      case "I": return resetOperatorState(cmdInsertLineStart(view, vim));
      case "A": return resetOperatorState(cmdAppendLineEnd(view, vim));
      case "o": return resetOperatorState(cmdOpenBelow(view, vim));
      case "O": return resetOperatorState(cmdOpenAbove(view, vim));
      case "x": return resetOperatorState(cmdDeleteChar(view, vim));
      case "X": return resetOperatorState(cmdDeleteCharBefore(view, vim));
      case "p": return resetOperatorState(cmdPaste(view, vim));
      case "P": return resetOperatorState(cmdPasteBefore(view, vim));
      case "J": return resetOperatorState(cmdJoinLines(view, vim));
      case "~": return resetOperatorState(cmdToggleCaseChar(view, vim));
      case "u": {
        const { undo } = require("@tiptap/pm/history") as typeof import("@tiptap/pm/history");
        undo(view.state, view.dispatch);
        return resetOperatorState(vim);
      }
      case "v": {
        if (vim.mode === "visual") return enterNormalMode(vim);
        return enterVisualMode(vim, cursorPos(view.state));
      }
      case "V": {
        if (vim.mode === "visual-line") return enterNormalMode(vim);
        return enterVisualLineMode(vim, cursorPos(view.state));
      }
      case ".": {
        // Dot repeat — replay lastAction keys
        if (vim.lastAction) {
          for (const k of vim.lastAction.keys) {
            const event = new KeyboardEvent("keydown", { key: k });
            plugin.props.handleKeyDown!(view, event);
          }
        }
        return vim;
      }
      case "/": {
        options.onOpenCommandLine("/");
        return vim;
      }
      case ":": {
        options.onOpenCommandLine(":");
        return vim;
      }
      case "n": {
        // Next search match
        if (vim.searchPattern) {
          const map = buildFlatTextMap(view.state);
          const flatPos = map.toFlat.get(cursorPos(view.state)) ?? 0;
          const next = findNextMatch(map.text, vim.searchPattern, flatPos, vim.searchDirection);
          if (next !== null && map.toPM[next] !== undefined) {
            const tr = view.state.tr.setSelection(TextSelection.create(view.state.doc, map.toPM[next]));
            view.dispatch(tr);
          }
        }
        return resetOperatorState(vim);
      }
      case "N": {
        // Previous search match
        if (vim.searchPattern) {
          const map = buildFlatTextMap(view.state);
          const flatPos = map.toFlat.get(cursorPos(view.state)) ?? 0;
          const dir = vim.searchDirection === "forward" ? "backward" : "forward";
          const prev = findNextMatch(map.text, vim.searchPattern, flatPos, dir);
          if (prev !== null && map.toPM[prev] !== undefined) {
            const tr = view.state.tr.setSelection(TextSelection.create(view.state.doc, map.toPM[prev]));
            view.dispatch(tr);
          }
        }
        return resetOperatorState(vim);
      }
      case "m": {
        // Set mark — await next char
        vim = { ...vim, awaitingChar: "r" as AwaitingChar }; // Reuse awaiting, handle specially
        // Actually, let's use a one-shot listener
        const handler = (e: KeyboardEvent) => {
          e.preventDefault();
          document.removeEventListener("keydown", handler, true);
          if (e.key.length === 1 && /[a-z]/.test(e.key)) {
            vim = { ...vim, marks: { ...vim.marks, [e.key]: cursorPos(view.state) }, awaitingChar: null };
          }
          notify();
        };
        vim = { ...vim, awaitingChar: null };
        document.addEventListener("keydown", handler, true);
        return vim;
      }
      case "'": {
        // Jump to mark
        const handler = (e: KeyboardEvent) => {
          e.preventDefault();
          document.removeEventListener("keydown", handler, true);
          const markPos = vim.marks[e.key];
          if (markPos !== undefined) {
            const clamped = clampPos(view.state, markPos);
            const tr = view.state.tr.setSelection(TextSelection.create(view.state.doc, clamped));
            view.dispatch(tr);
          }
          notify();
        };
        document.addEventListener("keydown", handler, true);
        return vim;
      }
      case "Escape": {
        return enterNormalMode(vim);
      }
      default:
        return resetOperatorState(vim);
    }
  }

  // Public API for external consumers
  (plugin as any).getVimState = getVimState;
  (plugin as any).setSearchPattern = (pattern: string, direction: "forward" | "backward") => {
    vim = { ...vim, searchPattern: pattern, searchDirection: direction };
    notify();
  };
  (plugin as any).executeCommand = (cmd: string) => {
    if (cmd === "w") {
      // :w — trigger save via custom event
      document.dispatchEvent(new CustomEvent("vim:save"));
    }
  };

  return plugin;
}
```

**Step 2: Verify**

Run: `cd desktop-ui && bun run build`
Expected: PASS (may have import warnings — fix any type errors)

**Step 3: Commit**

```bash
git add desktop-ui/src/components/notes/editor/vim/VimPlugin.ts
git commit -m "feat(notes): vim ProseMirror plugin — keydown handler and state orchestration"
```

---

## Phase 3: TipTap Extension + UI

### Task 11: TipTap VimMode extension

**Files:**
- Create: `desktop-ui/src/components/notes/editor/vim/index.ts`

**Step 1: Implement index.ts**

Wraps VimPlugin + cursor plugin + search plugin into a single TipTap extension.

```typescript
import { Extension } from "@tiptap/react";
import type { VimMode, VimState } from "./VimState";
import { createVimPlugin, type VimPluginOptions } from "./VimPlugin";
import { createVimCursorPlugin } from "./cursor";
import { createVimSearchPlugin } from "./search";

export type { VimState, VimMode };

export interface VimModeOptions {
  onStateChange?: (state: VimState) => void;
  onOpenCommandLine?: (prefix: string) => void;
}

export const VimMode = Extension.create<VimModeOptions>({
  name: "vimMode",

  addOptions() {
    return {
      onStateChange: undefined,
      onOpenCommandLine: undefined,
    };
  },

  addProseMirrorPlugins() {
    const opts: VimPluginOptions = {
      onStateChange: this.options.onStateChange ?? (() => {}),
      onOpenCommandLine: this.options.onOpenCommandLine ?? (() => {}),
    };

    const vimPlugin = createVimPlugin(opts);
    const getMode = (): VimMode => (vimPlugin as any).getVimState().mode;
    const getPattern = (): string | null => (vimPlugin as any).getVimState().searchPattern;

    return [
      vimPlugin,
      createVimCursorPlugin(getMode),
      createVimSearchPlugin(getPattern),
    ];
  },
});

export default VimMode;
```

**Step 2: Verify**

Run: `cd desktop-ui && bun run build`
Expected: PASS

**Step 3: Commit**

```bash
git add desktop-ui/src/components/notes/editor/vim/index.ts
git commit -m "feat(notes): TipTap VimMode extension wrapper"
```

---

### Task 12: VimStatusLine component

**Files:**
- Create: `desktop-ui/src/components/notes/editor/VimStatusLine.tsx`

**Step 1: Implement**

```tsx
import type { VimMode } from "./vim";

interface VimStatusLineProps {
  mode: VimMode;
}

const MODE_LABELS: Record<VimMode, string> = {
  normal: "-- NORMAL --",
  insert: "-- INSERT --",
  visual: "-- VISUAL --",
  "visual-line": "-- VISUAL LINE --",
};

export function VimStatusLine({ mode }: VimStatusLineProps) {
  return (
    <span className="font-mono text-xs text-secondary tracking-wide select-none">
      {MODE_LABELS[mode]}
    </span>
  );
}
```

**Step 2: Commit**

```bash
git add desktop-ui/src/components/notes/editor/VimStatusLine.tsx
git commit -m "feat(notes): vim mode status line component"
```

---

### Task 13: VimCommandLine component

**Files:**
- Create: `desktop-ui/src/components/notes/editor/VimCommandLine.tsx`

**Step 1: Implement**

```tsx
import { useEffect, useRef, useState } from "react";

interface VimCommandLineProps {
  prefix: string; // "/" or ":"
  onSubmit: (value: string) => void;
  onCancel: () => void;
}

export function VimCommandLine({ prefix, onSubmit, onCancel }: VimCommandLineProps) {
  const [value, setValue] = useState("");
  const inputRef = useRef<HTMLInputElement>(null);

  useEffect(() => {
    inputRef.current?.focus();
  }, []);

  const handleKeyDown = (e: React.KeyboardEvent) => {
    if (e.key === "Enter") {
      e.preventDefault();
      onSubmit(value);
    } else if (e.key === "Escape") {
      e.preventDefault();
      onCancel();
    }
  };

  return (
    <div className="flex items-center gap-1.5 px-4 py-1.5 border-t border-white/[0.06] bg-black/20">
      <span className="font-mono text-xs text-muted select-none">{prefix}</span>
      <input
        ref={inputRef}
        type="text"
        value={value}
        onChange={(e) => setValue(e.target.value)}
        onKeyDown={handleKeyDown}
        onBlur={onCancel}
        className="flex-1 bg-transparent text-xs font-mono text-primary outline-none placeholder:text-dim"
        placeholder={prefix === "/" ? "Search..." : "Command..."}
      />
    </div>
  );
}
```

**Step 2: Commit**

```bash
git add desktop-ui/src/components/notes/editor/VimCommandLine.tsx
git commit -m "feat(notes): vim command line component for search and ex commands"
```

---

### Task 14: Modify EditorToolbar — conditional vim/full rendering

**Files:**
- Modify: `desktop-ui/src/components/notes/editor/EditorToolbar.tsx`

**Step 1: Read current file** (already read above)

**Step 2: Update EditorToolbar**

Add `vimEnabled`, `vimMode`, and `onToggleVim` props. When vim is enabled, render the minimal toolbar (status line + toggle). When vim is off, render the full toolbar + toggle button at the end.

The `Vi` toggle button goes at the far right of the toolbar, separated by a divider. Use a text-based button with "Vi" label.

```tsx
// Add to imports:
import type { VimMode } from "./vim";
import { VimStatusLine } from "./VimStatusLine";

// Update props interface:
interface EditorToolbarProps {
  editor: Editor | null;
  vimEnabled: boolean;
  vimMode: VimMode;
  onToggleVim: () => void;
}

// Update component:
export function EditorToolbar({ editor, vimEnabled, vimMode, onToggleVim }: EditorToolbarProps) {
  if (!editor) return null;

  const vimToggle = (
    <button
      type="button"
      onClick={onToggleVim}
      title={vimEnabled ? "Disable vim mode" : "Enable vim mode"}
      className={`px-2 h-7 rounded-lg flex items-center justify-center font-mono text-xs font-bold transition-all ${
        vimEnabled
          ? "bg-brand/15 text-brand shadow-[0_0_8px_rgba(249,115,22,0.12)]"
          : "text-muted hover:text-primary hover:bg-white/[0.08]"
      }`}
    >
      Vi
    </button>
  );

  if (vimEnabled) {
    return (
      <div className="glass-toolbar rounded-lg px-3 py-1 flex items-center justify-between">
        <VimStatusLine mode={vimMode} />
        {vimToggle}
      </div>
    );
  }

  return (
    <div className="glass-toolbar rounded-lg px-2 py-1 flex items-center gap-0.5 flex-wrap">
      {groups.map((group, gi) => (
        <div key={gi} className="flex items-center gap-0.5">
          {gi > 0 && <div className="w-px h-4 bg-white/[0.08] mx-1.5" />}
          {group.map((btn) => {
            const Icon = btn.icon;
            const active = btn.isActive?.(editor) ?? false;
            return (
              <button
                key={btn.label}
                type="button"
                onClick={() => btn.action(editor)}
                title={btn.label}
                className={`w-7 h-7 rounded-lg flex items-center justify-center transition-all ${
                  active
                    ? "bg-brand/15 text-brand shadow-[0_0_8px_rgba(249,115,22,0.12)]"
                    : "text-muted hover:text-primary hover:bg-white/[0.08]"
                }`}
              >
                <Icon className="w-3.5 h-3.5" strokeWidth={1.5} />
              </button>
            );
          })}
        </div>
      ))}
      <div className="w-px h-4 bg-white/[0.08] mx-1.5" />
      {vimToggle}
    </div>
  );
}
```

**Step 3: Verify**

Run: `cd desktop-ui && bun run build`
Expected: Will fail because NoteEditor needs updating (next task). That's fine — verify type errors in this file are clean.

**Step 4: Commit**

```bash
git add desktop-ui/src/components/notes/editor/EditorToolbar.tsx
git commit -m "feat(notes): conditional toolbar rendering for vim mode"
```

---

### Task 15: Wire VimMode into EditorCore and NoteEditor

**Files:**
- Modify: `desktop-ui/src/components/notes/editor/EditorCore.tsx`
- Modify: `desktop-ui/src/components/notes/NoteEditor.tsx`

**Step 1: Update EditorCore — add VimMode to extensions when enabled**

In `useNoteEditor`, accept a `vimOptions` parameter. When provided, include the `VimMode` extension in the extensions list. Also add a CSS class to `editorProps.attributes.class` based on the current vim mode.

Key changes to `EditorCore.tsx`:
- Import `VimMode` from `./vim`
- Add `vimEnabled` and vim callbacks to `UseNoteEditorOptions`
- Conditionally add `VimMode` extension to the extensions array
- Update the `editorProps.attributes.class` to include `vim-{mode}` class

In `getEditorExtensions`:
```typescript
// Add to EditorExtensionOptions:
vimOptions?: VimModeOptions;

// In getEditorExtensions, after the existing extensions:
if (opts.vimOptions) {
  extensions.push(VimMode.configure(opts.vimOptions));
}
```

In `useNoteEditor`:
```typescript
// Add to UseNoteEditorOptions:
vimEnabled?: boolean;
vimCallbacks?: { onStateChange: (s: VimState) => void; onOpenCommandLine: (p: string) => void };

// Pass to getEditorExtensions:
vimOptions: options.vimEnabled ? options.vimCallbacks : undefined,
```

Update `editorProps.attributes` to be dynamic:
```typescript
editorProps: {
  attributes: { class: `editor-content${vimEnabled ? ` vim-${vimMode}` : ""}` },
  // ... handlePaste stays the same
}
```

**Step 2: Update NoteEditor — add vim state management + command line**

Key changes to `NoteEditor.tsx`:
- Add `useState` for `vimEnabled` (initialized from `localStorage`)
- Add `useState` for `vimMode` (VimMode type)
- Add `useState` for `commandLinePrefix` (null | "/" | ":")
- Pass `vimEnabled`, vim callbacks to `useNoteEditor`
- Pass `vimEnabled`, `vimMode`, `onToggleVim` to `EditorToolbar`
- Render `<VimCommandLine>` at the bottom of the editor area when `commandLinePrefix` is set
- On toggle: save to `localStorage`, destroy and recreate editor (TipTap extensions are set at init)
- On command line submit: if `/`, set search pattern on vim plugin; if `:w`, trigger save
- Add CSS class `vim-{mode}` to editor content wrapper

Toggle handler:
```typescript
const toggleVim = useCallback(() => {
  const next = !vimEnabled;
  setVimEnabled(next);
  localStorage.setItem("klyntbot:notes:vimMode", String(next));
}, [vimEnabled]);
```

**Note:** Since TipTap extensions are configured at editor creation time, toggling vim mode requires recreating the editor. Use a React `key` prop on the editor wrapper keyed to `vimEnabled` to force remount:

```tsx
<EditorContentWrapper key={`editor-${vimEnabled}`} editor={editor} />
```

But since `useNoteEditor` is a hook, instead use a `useEffect` to destroy and recreate when `vimEnabled` changes. Or simpler: pass `vimEnabled` as a dependency of the editor hook so it re-initializes.

The cleanest approach: pass `vimEnabled` into `useNoteEditor`. Inside that hook, include it in the `useEditor` dependencies so it recreates when toggled. TipTap's `useEditor` already handles this — changing the `extensions` array triggers recreation.

**Step 3: Verify end-to-end**

Run: `cd desktop-ui && bun run build`
Expected: PASS

**Step 4: Run lint**

Run: `cd desktop-ui && bun run lint:fix`
Expected: Clean or auto-fixed

**Step 5: Commit**

```bash
git add desktop-ui/src/components/notes/editor/EditorCore.tsx desktop-ui/src/components/notes/NoteEditor.tsx
git commit -m "feat(notes): wire vim mode into editor core and note editor"
```

---

### Task 16: Final verify and test

**Step 1: Run all unit tests**

Run: `cd desktop-ui && bun run test`
Expected: All PASS

**Step 2: Run full build**

Run: `cd desktop-ui && bun run build`
Expected: PASS

**Step 3: Run lint**

Run: `cd desktop-ui && bun run lint:fix`
Expected: Clean

**Step 4: Manual test checklist**

Start dev server: `cargo run -p dev-api` + `cd desktop-ui && bun run dev`

Verify:
- [ ] `Vi` button visible in toolbar, far right, separated by divider
- [ ] Click `Vi` → toolbar collapses to `-- NORMAL --` + `Vi` button
- [ ] Click `Vi` again → full toolbar restores
- [ ] Vim mode persists across page refresh (localStorage)
- [ ] In normal mode: `hjkl` moves cursor, block cursor visible
- [ ] `w`, `b`, `e` — word motions work
- [ ] `dd` deletes line, `yy` + `p` yanks and pastes
- [ ] `i`, `a`, `o`, `O` enter insert mode, thin caret appears
- [ ] `Escape` returns to normal mode
- [ ] `v` enters visual mode, motions extend selection
- [ ] `dw`, `d$`, `d0` — operator + motion combos
- [ ] `ci"`, `di(`, `yiw` — text objects
- [ ] `3w` — count + motion
- [ ] `f{char}` — find char in line
- [ ] `/pattern` → search highlights appear, `n`/`N` navigates
- [ ] `u` / `Ctrl+R` — undo/redo
- [ ] `.` — dot repeat
- [ ] `ma` / `'a` — marks

**Step 5: Commit any fixes**

```bash
git add -A
git commit -m "fix(notes): vim mode polish and fixes"
```

---

## Verification Checklist

- [ ] `cd desktop-ui && bun run test` — all unit tests pass
- [ ] `cd desktop-ui && bun run build` — production build succeeds
- [ ] `cd desktop-ui && bun run lint:fix` — clean
- [ ] Manual test: vim toggle, motions, operators, text objects, search, visual mode
