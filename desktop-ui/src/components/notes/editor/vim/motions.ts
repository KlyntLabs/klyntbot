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
export type MotionFn = (view: EditorView, state: EditorState, vim: VimState) => MotionResult;

// ---------------------------------------------------------------------------
// h / l — left / right within line
// ---------------------------------------------------------------------------

function motionH(_view: EditorView, state: EditorState, vim: VimState): MotionResult {
  const head = cursorPos(state);
  const start = lineStart(state, head);
  const count = effectiveCount(vim);
  return { pos: clampPos(state, Math.max(start, head - count)) };
}

function motionL(_view: EditorView, state: EditorState, vim: VimState): MotionResult {
  const head = cursorPos(state);
  const end = lineEnd(state, head);
  const count = effectiveCount(vim);
  // In normal mode, cursor cannot sit on the newline (end pos is exclusive
  // for the last char), so we clamp to end - 1 when there is content.
  const maxPos = end > lineStart(state, head) ? end - 1 : end;
  return { pos: clampPos(state, Math.min(maxPos, head + count)) };
}

// ---------------------------------------------------------------------------
// j / k — vertical movement
// ---------------------------------------------------------------------------

function motionJ(_view: EditorView, state: EditorState, vim: VimState): MotionResult {
  const head = cursorPos(state);
  const count = effectiveCount(vim);
  let pos = head;
  for (let i = 0; i < count; i++) {
    pos = verticalMove(state, pos, 1);
  }
  return { pos: clampPos(state, pos), linewise: true };
}

function motionK(_view: EditorView, state: EditorState, vim: VimState): MotionResult {
  const head = cursorPos(state);
  const count = effectiveCount(vim);
  let pos = head;
  for (let i = 0; i < count; i++) {
    pos = verticalMove(state, pos, -1);
  }
  return { pos: clampPos(state, pos), linewise: true };
}

// ---------------------------------------------------------------------------
// Word motions: w, W, b, B, e, E
// ---------------------------------------------------------------------------

function wordMotion(
  _view: EditorView,
  state: EditorState,
  vim: VimState,
  direction: "forward" | "backward",
  boundary: "start" | "end",
  big: boolean,
): MotionResult {
  const map = buildFlatTextMap(state);
  const head = cursorPos(state);
  const flatIdx = map.toFlat.get(head) ?? 0;
  const count = effectiveCount(vim);
  const finder = big ? findWORDBoundary : findWordBoundary;
  let pos = flatIdx;
  for (let i = 0; i < count; i++) {
    pos = finder(map.text, pos, direction, boundary);
  }
  // Convert flat index back to PM position
  const pmPos = pos < map.toPM.length ? map.toPM[pos] : state.doc.content.size;
  return { pos: clampPos(state, pmPos) };
}

function motionW(view: EditorView, state: EditorState, vim: VimState): MotionResult {
  return wordMotion(view, state, vim, "forward", "start", false);
}

function motionWBig(view: EditorView, state: EditorState, vim: VimState): MotionResult {
  return wordMotion(view, state, vim, "forward", "start", true);
}

function motionB(view: EditorView, state: EditorState, vim: VimState): MotionResult {
  return wordMotion(view, state, vim, "backward", "start", false);
}

function motionBBig(view: EditorView, state: EditorState, vim: VimState): MotionResult {
  return wordMotion(view, state, vim, "backward", "start", true);
}

function motionE(view: EditorView, state: EditorState, vim: VimState): MotionResult {
  return wordMotion(view, state, vim, "forward", "end", false);
}

function motionEBig(view: EditorView, state: EditorState, vim: VimState): MotionResult {
  return wordMotion(view, state, vim, "forward", "end", true);
}

// ---------------------------------------------------------------------------
// 0 / $ / ^ — line start / end / first non-whitespace
// ---------------------------------------------------------------------------

function motion0(_view: EditorView, state: EditorState, _vim: VimState): MotionResult {
  const head = cursorPos(state);
  return { pos: lineStart(state, head) };
}

function motionDollar(_view: EditorView, state: EditorState, vim: VimState): MotionResult {
  const head = cursorPos(state);
  const count = effectiveCount(vim);
  // With count > 1, $ moves down (count-1) lines then to end
  const pos = head;
  if (count > 1) {
    // Resolve to find current block, then step forward count-1 blocks
    const $pos = state.doc.resolve(pos);
    let blockNode = $pos.node($pos.depth);
    let blockStart = $pos.start($pos.depth);
    for (let i = 1; i < count; i++) {
      const nextPos = blockStart + blockNode.nodeSize;
      if (nextPos >= state.doc.content.size) break;
      const $next = state.doc.resolve(nextPos + 1);
      blockNode = $next.node($next.depth);
      blockStart = $next.start($next.depth);
    }
    return { pos: blockStart + blockNode.content.size };
  }
  const end = lineEnd(state, head);
  return { pos: end };
}

function motionCaret(_view: EditorView, state: EditorState, _vim: VimState): MotionResult {
  const head = cursorPos(state);
  const start = lineStart(state, head);
  const end = lineEnd(state, head);
  // Find first non-whitespace character on the line
  const lineText = state.doc.textBetween(start, end);
  const match = lineText.search(/\S/);
  return { pos: match === -1 ? start : start + match };
}

// ---------------------------------------------------------------------------
// G — go to line / document end
// ---------------------------------------------------------------------------

function findLastTextblock(state: EditorState): number {
  let lastStart = -1;
  state.doc.descendants((node, pos) => {
    if (node.isTextblock) lastStart = pos + 1;
    return true;
  });
  return lastStart;
}

function motionG(_view: EditorView, state: EditorState, vim: VimState): MotionResult {
  if (vim.count !== null) {
    // Go to line N (1-indexed): resolve the Nth block-level node
    const targetLine = vim.count;
    let lineNum = 0;
    let targetPos = 0;
    state.doc.descendants((node, pos) => {
      if (node.isBlock && node.isTextblock) {
        lineNum++;
        if (lineNum === targetLine) {
          targetPos = pos + 1; // inside the block
          return false;
        }
      }
      return true;
    });
    if (lineNum < targetLine) {
      // Beyond document: go to last line
      const last = findLastTextblock(state);
      return { pos: last >= 0 ? last : state.doc.content.size, linewise: true };
    }
    return { pos: clampPos(state, targetPos), linewise: true };
  }
  // No count: go to start of last textblock (like vim's G)
  const last = findLastTextblock(state);
  return { pos: last >= 0 ? last : state.doc.content.size, linewise: true };
}

// ---------------------------------------------------------------------------
// { / } — paragraph up / down
// ---------------------------------------------------------------------------

function motionParagraphUp(_view: EditorView, state: EditorState, vim: VimState): MotionResult {
  const head = cursorPos(state);
  const count = effectiveCount(vim);
  let pos = head;

  for (let c = 0; c < count; c++) {
    // Walk backward through document to find previous empty block (paragraph boundary)
    const $pos = state.doc.resolve(pos);
    const blockIndex = $pos.index($pos.depth > 0 ? $pos.depth - 1 : 0);
    const parent = $pos.node($pos.depth > 0 ? $pos.depth - 1 : 0);

    // Move backward to find an empty block
    let found = false;
    // Skip current block if we're at its start
    if (blockIndex > 0) {
      for (let i = blockIndex - 1; i >= 0; i--) {
        const child = parent.child(i);
        if (child.isTextblock && child.content.size === 0) {
          // Found empty block — compute its position
          let childPos = 0;
          for (let j = 0; j < i; j++) {
            childPos += parent.child(j).nodeSize;
          }
          // Offset by parent's start position
          const parentStart = $pos.depth > 0 ? $pos.start($pos.depth - 1) : 0;
          pos = parentStart + childPos + 1;
          found = true;
          break;
        }
      }
    }
    if (!found) {
      pos = 1; // beginning of document (inside first block)
      break;
    }
  }

  return { pos: clampPos(state, pos), linewise: true };
}

function motionParagraphDown(_view: EditorView, state: EditorState, vim: VimState): MotionResult {
  const head = cursorPos(state);
  const count = effectiveCount(vim);
  let pos = head;

  for (let c = 0; c < count; c++) {
    const $pos = state.doc.resolve(pos);
    const parentDepth = $pos.depth > 0 ? $pos.depth - 1 : 0;
    const blockIndex = $pos.index(parentDepth);
    const parent = $pos.node(parentDepth);

    let found = false;
    for (let i = blockIndex + 1; i < parent.childCount; i++) {
      const child = parent.child(i);
      if (child.isTextblock && child.content.size === 0) {
        let childPos = 0;
        for (let j = 0; j < i; j++) {
          childPos += parent.child(j).nodeSize;
        }
        const parentStart = parentDepth > 0 ? $pos.start(parentDepth) : 0;
        pos = parentStart + childPos + 1;
        found = true;
        break;
      }
    }
    if (!found) {
      pos = state.doc.content.size;
      break;
    }
  }

  return { pos: clampPos(state, pos), linewise: true };
}

// ---------------------------------------------------------------------------
// % — matching bracket
// ---------------------------------------------------------------------------

const BRACKET_PAIRS: Record<string, string> = {
  "(": ")",
  ")": "(",
  "[": "]",
  "]": "[",
  "{": "}",
  "}": "{",
};
const OPEN_BRACKETS = new Set(["(", "[", "{"]);

function motionMatchBracket(_view: EditorView, state: EditorState, _vim: VimState): MotionResult {
  const head = cursorPos(state);
  const start = lineStart(state, head);
  const end = lineEnd(state, head);

  // Get the current line text to find the first bracket at or after cursor
  const lineText = state.doc.textBetween(start, end);
  const lineOffset = head - start;
  let bracketPos = -1;
  let bracketChar = "";
  for (let i = lineOffset; i < lineText.length; i++) {
    if (lineText[i] in BRACKET_PAIRS) {
      bracketPos = start + i;
      bracketChar = lineText[i];
      break;
    }
  }

  if (bracketPos === -1) return { pos: head };

  const matchChar = BRACKET_PAIRS[bracketChar];
  const forward = OPEN_BRACKETS.has(bracketChar);
  let depth = 0;

  // Build flat text map for efficient scanning across the entire document
  const map = buildFlatTextMap(state);
  const flatStart = map.toFlat.get(bracketPos);
  if (flatStart === undefined) return { pos: head };

  if (forward) {
    for (let i = flatStart + 1; i < map.text.length; i++) {
      const ch = map.text[i];
      if (ch === bracketChar) depth++;
      else if (ch === matchChar) {
        if (depth === 0) return { pos: map.toPM[i] };
        depth--;
      }
    }
  } else {
    for (let i = flatStart - 1; i >= 0; i--) {
      const ch = map.text[i];
      if (ch === bracketChar) depth++;
      else if (ch === matchChar) {
        if (depth === 0) return { pos: map.toPM[i] };
        depth--;
      }
    }
  }

  return { pos: head };
}

// ---------------------------------------------------------------------------
// f / F / t / T — find char
// ---------------------------------------------------------------------------

export function motionFindChar(
  state: EditorState,
  char: string,
  forward: boolean,
  till: boolean,
  count: number,
): MotionResult {
  const head = cursorPos(state);
  const start = lineStart(state, head);
  const end = lineEnd(state, head);
  let found = -1;
  let matches = 0;

  if (forward) {
    for (let p = head + 1; p <= end; p++) {
      const ch = state.doc.textBetween(p, Math.min(p + 1, state.doc.content.size));
      if (ch === char) {
        matches++;
        if (matches === count) {
          found = till ? p - 1 : p;
          break;
        }
      }
    }
  } else {
    for (let p = head - 1; p >= start; p--) {
      const ch = state.doc.textBetween(p, p + 1);
      if (ch === char) {
        matches++;
        if (matches === count) {
          found = till ? p + 1 : p;
          break;
        }
      }
    }
  }

  if (found === -1) return { pos: head };
  return { pos: clampPos(state, found) };
}

// ---------------------------------------------------------------------------
// gg — go to beginning of document (or line N with count)
// ---------------------------------------------------------------------------

function motionGG(_view: EditorView, state: EditorState, vim: VimState): MotionResult {
  if (vim.count !== null) {
    // Same as G with count — go to line N
    return motionG(_view, state, vim);
  }
  // No count: go to beginning of document
  // Position 1 = inside the first block node
  const firstPos = Math.min(1, state.doc.content.size);
  return { pos: firstPos, linewise: true };
}

// ---------------------------------------------------------------------------
// Motion registry
// ---------------------------------------------------------------------------

export const MOTIONS: Record<string, MotionFn> = {
  h: motionH,
  l: motionL,
  j: motionJ,
  k: motionK,
  w: motionW,
  W: motionWBig,
  b: motionB,
  B: motionBBig,
  e: motionE,
  E: motionEBig,
  "0": motion0,
  $: motionDollar,
  "^": motionCaret,
  G: motionG,
  "{": motionParagraphUp,
  "}": motionParagraphDown,
  "%": motionMatchBracket,
  gg: motionGG,
};
