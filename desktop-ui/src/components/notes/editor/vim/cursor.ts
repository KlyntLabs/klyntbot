import { Plugin, PluginKey } from "@tiptap/pm/state";
import { Decoration, DecorationSet } from "@tiptap/pm/view";
import type { VimMode } from "./VimState";

export const vimCursorPluginKey = new PluginKey("vimCursor");

export function createVimCursorPlugin(getMode: () => VimMode, getEnabled: () => boolean) {
  return new Plugin({
    key: vimCursorPluginKey,
    props: {
      decorations(state) {
        if (!getEnabled()) return DecorationSet.empty;
        const mode = getMode();
        if (mode === "insert") return DecorationSet.empty;

        const pos = state.selection.head;
        const $pos = state.doc.resolve(pos);
        const end = Math.min(pos + 1, $pos.end());

        if (pos >= end) {
          // At end of line — use widget decoration
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
}
