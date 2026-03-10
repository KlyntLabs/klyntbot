import type { EditorState } from "@tiptap/pm/state";

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

export function verticalMove(state: EditorState, pos: number, direction: 1 | -1): number {
  const $pos = state.doc.resolve(pos);

  // Find the textblock containing the cursor
  let blockDepth = $pos.depth;
  while (blockDepth > 0 && !$pos.node(blockDepth).isTextblock) {
    blockDepth--;
  }
  if (blockDepth === 0) {
    // Cursor is outside any textblock (e.g. at doc end) — find nearest textblock
    if (direction === -1 || pos >= state.doc.content.size) {
      let lastStart = -1;
      let lastEnd = -1;
      state.doc.nodesBetween(0, Math.min(pos, state.doc.content.size), (node, npos) => {
        if (node.isTextblock) {
          lastStart = npos + 1;
          lastEnd = npos + 1 + node.content.size;
        }
        return true;
      });
      if (lastStart >= 0) return Math.min(lastStart, lastEnd);
    }
    return pos;
  }

  const blockStart = $pos.start(blockDepth);
  const column = pos - blockStart;

  if (direction === 1) {
    // Down: find next textblock after current block
    const afterBlock = $pos.after(blockDepth);
    if (afterBlock >= state.doc.content.size) return pos;

    let found = -1;
    let foundEnd = -1;
    state.doc.nodesBetween(afterBlock, state.doc.content.size, (node, npos) => {
      if (found >= 0) return false;
      if (node.isTextblock) {
        found = npos + 1;
        foundEnd = npos + 1 + node.content.size;
        return false;
      }
      return true;
    });

    if (found < 0) return pos;
    return Math.min(found + column, foundEnd);
  }

  // Up: find previous textblock before current block
  const beforeBlock = $pos.before(blockDepth);
  if (beforeBlock <= 0) return pos;

  let found = -1;
  let foundEnd = -1;
  state.doc.nodesBetween(0, beforeBlock, (node, npos) => {
    if (node.isTextblock) {
      found = npos + 1;
      foundEnd = npos + 1 + node.content.size;
    }
    return true;
  });

  if (found < 0) return pos;
  return Math.min(found + column, foundEnd);
}

export function clampPos(state: EditorState, pos: number): number {
  return Math.max(0, Math.min(pos, state.doc.content.size));
}
