import type { EditorState } from "@tiptap/pm/state";
import type { EditorView } from "@tiptap/pm/view";

/** Flat text with bidirectional position mapping to ProseMirror positions. */
export interface FlatTextMap {
  text: string;
  /** flatIndex -> PM position. Length = text.length + 1. */
  toPM: number[];
  /** PM position -> flat index. */
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

export function lineStart(state: EditorState, pos: number): number {
  const $pos = state.doc.resolve(pos);
  return $pos.start($pos.depth);
}

export function lineEnd(state: EditorState, pos: number): number {
  const $pos = state.doc.resolve(pos);
  return $pos.end($pos.depth);
}

export function cursorPos(state: EditorState): number {
  return state.selection.head;
}

/**
 * Move the cursor up or down by one **visual line** using the view's
 * coordinate system. This works correctly for:
 * - Lines within code blocks (newlines in a single node)
 * - Wrapped paragraphs (soft-wrapped visual lines)
 * - Moving between different block types (heading → paragraph → code)
 */
export function verticalMove(view: EditorView, pos: number, direction: 1 | -1): number {
  const coords = view.coordsAtPos(pos);
  const lineHeight = coords.bottom - coords.top;

  // Jump to the vertical midpoint of the adjacent visual line.
  // Using half a line-height past the edge ensures we land squarely on the
  // next/previous line even when there's inter-line spacing or padding.
  const targetY =
    direction === 1
      ? coords.bottom + Math.max(lineHeight * 0.5, 2)
      : coords.top - Math.max(lineHeight * 0.5, 2);

  const result = view.posAtCoords({ left: coords.left, top: targetY });
  if (!result) return pos;

  return result.pos;
}

export function clampPos(state: EditorState, pos: number): number {
  return Math.max(0, Math.min(pos, state.doc.content.size));
}
