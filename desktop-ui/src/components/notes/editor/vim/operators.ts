import { TextSelection } from "@tiptap/pm/state";
import type { EditorView } from "@tiptap/pm/view";
import { lineEnd, lineStart } from "./positions";
import type { VimState } from "./VimState";
import { enterInsertMode, enterNormalMode } from "./VimState";

// ---------------------------------------------------------------------------
// Delete operator (d)
// ---------------------------------------------------------------------------

export function operatorDelete(
  view: EditorView,
  from: number,
  to: number,
  vim: VimState,
  linewise: boolean,
): VimState {
  const { state } = view;
  let deleteFrom = from;
  let deleteTo = to;

  if (linewise) {
    // Expand to full lines
    deleteFrom = lineStart(state, from);
    deleteTo = lineEnd(state, to);
    // Include the trailing newline if possible (delete the block node boundary)
    const $to = state.doc.resolve(deleteTo);
    const blockEnd = $to.after($to.depth);
    if (blockEnd <= state.doc.content.size) {
      deleteTo = blockEnd;
    }
    // Also include from the block start
    const $from = state.doc.resolve(deleteFrom);
    deleteFrom = $from.before($from.depth);
  }

  // Yank the text before deleting
  const yanked = state.doc.textBetween(deleteFrom, deleteTo, "\n");
  const newVim: VimState = {
    ...enterNormalMode(vim),
    registers: {
      ...vim.registers,
      '"': yanked,
    },
  };

  const tr = state.tr.delete(deleteFrom, deleteTo);
  // Place cursor at the deletion point
  const cursorAt = Math.min(deleteFrom, tr.doc.content.size);
  if (cursorAt >= 0 && cursorAt <= tr.doc.content.size) {
    tr.setSelection(TextSelection.near(tr.doc.resolve(cursorAt)));
  }
  view.dispatch(tr);

  return newVim;
}

// ---------------------------------------------------------------------------
// Yank operator (y)
// ---------------------------------------------------------------------------

export function operatorYank(view: EditorView, from: number, to: number, vim: VimState): VimState {
  const { state } = view;
  const yanked = state.doc.textBetween(from, to, "\n");
  return {
    ...enterNormalMode(vim),
    registers: {
      ...vim.registers,
      '"': yanked,
    },
  };
}

// ---------------------------------------------------------------------------
// Change operator (c)
// ---------------------------------------------------------------------------

export function operatorChange(
  view: EditorView,
  from: number,
  to: number,
  vim: VimState,
): VimState {
  const { state } = view;

  // Yank the text before deleting
  const yanked = state.doc.textBetween(from, to, "\n");
  const newVim: VimState = {
    ...enterInsertMode(vim),
    registers: {
      ...vim.registers,
      '"': yanked,
    },
  };

  const tr = state.tr.delete(from, to);
  const cursorAt = Math.min(from, tr.doc.content.size);
  if (cursorAt >= 0 && cursorAt <= tr.doc.content.size) {
    tr.setSelection(TextSelection.near(tr.doc.resolve(cursorAt)));
  }
  view.dispatch(tr);

  return newVim;
}

// ---------------------------------------------------------------------------
// Indent / outdent operator (> / <)
// ---------------------------------------------------------------------------

export function operatorIndent(
  view: EditorView,
  from: number,
  to: number,
  vim: VimState,
  outdent: boolean,
): VimState {
  const { state } = view;
  const indent = "  "; // 2 spaces

  let tr = state.tr;

  // Find all textblock nodes in the range
  state.doc.nodesBetween(from, to, (node, pos) => {
    if (node.isTextblock) {
      const blockStart = pos + 1; // inside the block
      if (outdent) {
        // Remove up to 2 leading spaces
        const text = node.textContent;
        let removeCount = 0;
        for (let i = 0; i < Math.min(indent.length, text.length); i++) {
          if (text[i] === " ") removeCount++;
          else break;
        }
        if (removeCount > 0) {
          tr = tr.delete(tr.mapping.map(blockStart), tr.mapping.map(blockStart + removeCount));
        }
      } else {
        // Insert indent at block start
        tr = tr.insertText(indent, tr.mapping.map(blockStart));
      }
    }
  });

  view.dispatch(tr);
  return enterNormalMode(vim);
}

// ---------------------------------------------------------------------------
// Toggle case operator (~)
// ---------------------------------------------------------------------------

export function operatorToggleCase(view: EditorView, from: number, to: number): void {
  const { state } = view;
  const text = state.doc.textBetween(from, to);
  let toggled = "";
  for (const ch of text) {
    if (ch === ch.toLowerCase() && ch !== ch.toUpperCase()) {
      toggled += ch.toUpperCase();
    } else {
      toggled += ch.toLowerCase();
    }
  }
  const tr = state.tr.insertText(toggled, from, to);
  view.dispatch(tr);
}

// ---------------------------------------------------------------------------
// Lowercase operator (gu)
// ---------------------------------------------------------------------------

export function operatorLowercase(view: EditorView, from: number, to: number): void {
  const { state } = view;
  const text = state.doc.textBetween(from, to);
  const tr = state.tr.insertText(text.toLowerCase(), from, to);
  view.dispatch(tr);
}

// ---------------------------------------------------------------------------
// Uppercase operator (gU)
// ---------------------------------------------------------------------------

export function operatorUppercase(view: EditorView, from: number, to: number): void {
  const { state } = view;
  const text = state.doc.textBetween(from, to);
  const tr = state.tr.insertText(text.toUpperCase(), from, to);
  view.dispatch(tr);
}
