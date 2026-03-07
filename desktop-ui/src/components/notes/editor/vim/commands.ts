import { redo, undo } from "@tiptap/pm/history";
import { TextSelection } from "@tiptap/pm/state";
import type { EditorView } from "@tiptap/pm/view";
import { operatorChange, operatorDelete, operatorToggleCase, operatorYank } from "./operators";
import { clampPos, cursorPos, lineEnd, lineStart } from "./positions";
import type { VimState } from "./VimState";
import { effectiveCount, enterInsertMode, enterNormalMode } from "./VimState";

// ---------------------------------------------------------------------------
// Insert-mode entry commands
// ---------------------------------------------------------------------------

/** `i` — enter insert mode at cursor */
export function cmdInsert(_view: EditorView, vim: VimState): VimState {
  return enterInsertMode(vim);
}

/** `a` — enter insert mode after cursor */
export function cmdAppend(view: EditorView, vim: VimState): VimState {
  const { state } = view;
  const head = cursorPos(state);
  const end = lineEnd(state, head);
  const newPos = Math.min(head + 1, end);
  const tr = state.tr.setSelection(TextSelection.create(state.doc, newPos));
  view.dispatch(tr);
  return enterInsertMode(vim);
}

/** `I` — enter insert mode at first non-whitespace on line */
export function cmdInsertLineStart(view: EditorView, vim: VimState): VimState {
  const { state } = view;
  const head = cursorPos(state);
  const start = lineStart(state, head);
  const end = lineEnd(state, head);
  // Find first non-whitespace
  const lineText = state.doc.textBetween(start, end);
  const match = lineText.search(/\S/);
  const pos = match === -1 ? start : start + match;
  const tr = state.tr.setSelection(TextSelection.create(state.doc, pos));
  view.dispatch(tr);
  return enterInsertMode(vim);
}

/** `A` — enter insert mode at end of line */
export function cmdAppendLineEnd(view: EditorView, vim: VimState): VimState {
  const { state } = view;
  const head = cursorPos(state);
  const end = lineEnd(state, head);
  const tr = state.tr.setSelection(TextSelection.create(state.doc, end));
  view.dispatch(tr);
  return enterInsertMode(vim);
}

/** `o` — open line below */
export function cmdOpenBelow(view: EditorView, vim: VimState): VimState {
  const { state } = view;
  const head = cursorPos(state);
  const $pos = state.doc.resolve(head);
  // Insert a new paragraph after the current block
  const blockEnd = $pos.after($pos.depth);
  const paragraph = state.schema.nodes.paragraph?.create();
  if (!paragraph) return vim;
  const tr = state.tr.insert(blockEnd, paragraph);
  // Place cursor inside the new paragraph
  tr.setSelection(TextSelection.create(tr.doc, blockEnd + 1));
  view.dispatch(tr);
  return enterInsertMode(vim);
}

/** `O` — open line above */
export function cmdOpenAbove(view: EditorView, vim: VimState): VimState {
  const { state } = view;
  const head = cursorPos(state);
  const $pos = state.doc.resolve(head);
  // Insert a new paragraph before the current block
  const blockStart = $pos.before($pos.depth);
  const paragraph = state.schema.nodes.paragraph?.create();
  if (!paragraph) return vim;
  const tr = state.tr.insert(blockStart, paragraph);
  // Place cursor inside the new paragraph
  tr.setSelection(TextSelection.create(tr.doc, blockStart + 1));
  view.dispatch(tr);
  return enterInsertMode(vim);
}

// ---------------------------------------------------------------------------
// Delete char commands
// ---------------------------------------------------------------------------

/** `x` — delete character under cursor */
export function cmdDeleteChar(view: EditorView, vim: VimState): VimState {
  const { state } = view;
  const head = cursorPos(state);
  const end = lineEnd(state, head);
  const count = effectiveCount(vim);
  const deleteTo = Math.min(head + count, end);
  if (head >= deleteTo) return enterNormalMode(vim);
  return operatorDelete(view, head, deleteTo, vim, false);
}

/** `X` — delete character before cursor */
export function cmdDeleteCharBefore(view: EditorView, vim: VimState): VimState {
  const { state } = view;
  const head = cursorPos(state);
  const start = lineStart(state, head);
  const count = effectiveCount(vim);
  const deleteFrom = Math.max(head - count, start);
  if (deleteFrom >= head) return enterNormalMode(vim);
  return operatorDelete(view, deleteFrom, head, vim, false);
}

// ---------------------------------------------------------------------------
// Line commands: dd, yy, cc
// ---------------------------------------------------------------------------

/** Expand from `head` downward to cover `count` lines, returning `toPos`. */
function expandLineRange(state: EditorView["state"], head: number, count: number): number {
  let toPos = head;
  for (let i = 0; i < count; i++) {
    const end = lineEnd(state, toPos);
    toPos = end;
    if (i < count - 1) {
      const nextPos = end + 1;
      if (nextPos <= state.doc.content.size) {
        toPos = nextPos;
      }
    }
  }
  return toPos;
}

/** `dd` — delete current line(s) */
export function cmdDeleteLine(view: EditorView, vim: VimState): VimState {
  const { state } = view;
  const head = cursorPos(state);
  const toPos = expandLineRange(state, head, effectiveCount(vim));
  return operatorDelete(view, head, toPos, vim, true);
}

/** `yy` — yank current line(s) */
export function cmdYankLine(view: EditorView, vim: VimState): VimState {
  const { state } = view;
  const head = cursorPos(state);
  const toPos = expandLineRange(state, head, effectiveCount(vim));
  return operatorYank(view, lineStart(state, head), lineEnd(state, toPos), vim);
}

/** `cc` — change current line(s) */
export function cmdChangeLine(view: EditorView, vim: VimState): VimState {
  const { state } = view;
  const head = cursorPos(state);
  const toPos = expandLineRange(state, head, effectiveCount(vim));
  return operatorChange(view, lineStart(state, head), lineEnd(state, toPos), vim);
}

// ---------------------------------------------------------------------------
// Paste commands
// ---------------------------------------------------------------------------

/** `p` — paste after cursor */
export function cmdPaste(view: EditorView, vim: VimState): VimState {
  const text = vim.registers['"'];
  if (!text) return enterNormalMode(vim);

  const { state } = view;
  const head = cursorPos(state);
  const insertAt = Math.min(head + 1, state.doc.content.size);
  const content = text.repeat(effectiveCount(vim));

  const tr = state.tr.insertText(content, insertAt);
  // Place cursor at end of pasted text
  const cursorAt = insertAt + content.length - 1;
  tr.setSelection(TextSelection.create(tr.doc, clampPos(state, cursorAt)));
  view.dispatch(tr);

  return enterNormalMode(vim);
}

/** `P` — paste before cursor */
export function cmdPasteBefore(view: EditorView, vim: VimState): VimState {
  const text = vim.registers['"'];
  if (!text) return enterNormalMode(vim);

  const { state } = view;
  const head = cursorPos(state);
  const content = text.repeat(effectiveCount(vim));

  const tr = state.tr.insertText(content, head);
  // Place cursor at end of pasted text - 1
  const cursorAt = head + content.length - 1;
  tr.setSelection(TextSelection.create(tr.doc, Math.min(cursorAt, tr.doc.content.size)));
  view.dispatch(tr);

  return enterNormalMode(vim);
}

// ---------------------------------------------------------------------------
// Join lines (J)
// ---------------------------------------------------------------------------

/** `J` — join current line with next */
export function cmdJoinLines(view: EditorView, vim: VimState): VimState {
  const { state } = view;
  const head = cursorPos(state);
  const count = effectiveCount(vim);

  let tr = state.tr;
  for (let i = 0; i < count; i++) {
    // Find end of current block
    const mappedHead = tr.mapping.map(head);
    const $pos = tr.doc.resolve(mappedHead);
    const blockEnd = $pos.after($pos.depth);

    // Check if there's a next block
    if (blockEnd >= tr.doc.content.size) break;

    const $next = tr.doc.resolve(blockEnd + 1);
    const nextBlockStart = $next.start($next.depth);
    const nextBlockEnd = $next.end($next.depth);

    // Get the next line's text, trimming leading whitespace
    const nextText = tr.doc.textBetween(nextBlockStart, nextBlockEnd);
    const trimmed = nextText.replace(/^\s+/, "");

    // Delete from end of current block through end of next block content,
    // including the block boundaries
    const deleteFrom = $pos.end($pos.depth);
    const deleteTo = $next.after($next.depth);

    tr = tr.delete(deleteFrom, deleteTo);
    // Insert space + trimmed text
    if (trimmed.length > 0) {
      tr = tr.insertText(` ${trimmed}`, deleteFrom);
    }
  }

  view.dispatch(tr);
  return enterNormalMode(vim);
}

// ---------------------------------------------------------------------------
// Replace char (r)
// ---------------------------------------------------------------------------

/** `r{char}` — replace character under cursor */
export function cmdReplaceChar(view: EditorView, vim: VimState, char: string): VimState {
  const { state } = view;
  const head = cursorPos(state);
  const end = lineEnd(state, head);
  if (head >= end) return enterNormalMode(vim);

  const tr = state.tr.insertText(char, head, head + 1);
  // Keep cursor at same position
  tr.setSelection(TextSelection.create(tr.doc, head));
  view.dispatch(tr);

  return enterNormalMode(vim);
}

// ---------------------------------------------------------------------------
// Toggle case (~)
// ---------------------------------------------------------------------------

/** `~` — toggle case of character under cursor and advance */
export function cmdToggleCaseChar(view: EditorView, vim: VimState): VimState {
  const { state } = view;
  const head = cursorPos(state);
  const end = lineEnd(state, head);
  const count = effectiveCount(vim);
  const toggleTo = Math.min(head + count, end);

  if (head >= toggleTo) return enterNormalMode(vim);

  operatorToggleCase(view, head, toggleTo);

  // Move cursor to end of toggled range
  const newState = view.state;
  const newPos = Math.min(toggleTo, lineEnd(newState, toggleTo) - 1);
  const tr = newState.tr.setSelection(TextSelection.create(newState.doc, Math.max(head, newPos)));
  view.dispatch(tr);

  return enterNormalMode(vim);
}

// ---------------------------------------------------------------------------
// Undo / Redo
// ---------------------------------------------------------------------------

/** `u` — undo */
export function cmdUndo(view: EditorView, vim: VimState): VimState {
  undo(view.state, view.dispatch);
  return enterNormalMode(vim);
}

/** `Ctrl-r` — redo */
export function cmdRedo(view: EditorView, vim: VimState): VimState {
  redo(view.state, view.dispatch);
  return enterNormalMode(vim);
}
