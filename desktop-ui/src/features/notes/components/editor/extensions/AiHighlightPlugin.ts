import { Plugin, PluginKey } from "@tiptap/pm/state";
import type { EditorView } from "@tiptap/pm/view";
import { Decoration, DecorationSet } from "@tiptap/pm/view";
import { Extension } from "@tiptap/react";

// ── Types ──────────────────────────────────────────────────────────────

export interface AiHighlight {
  from: number;
  to: number;
  similarity: number;
}

// ── Plugin Key ─────────────────────────────────────────────────────────

const aiHighlightKey = new PluginKey("aiHighlight");

// ── Public API ─────────────────────────────────────────────────────────

/** Set AI highlight decorations on the editor. Pass an empty array to clear. */
export function setAiHighlights(editor: { view: EditorView }, highlights: AiHighlight[]) {
  const { view } = editor;
  const tr = view.state.tr.setMeta(aiHighlightKey, highlights);
  view.dispatch(tr);
}

/** Clear all AI highlight decorations. */
export function clearAiHighlights(editor: { view: EditorView }) {
  setAiHighlights(editor, []);
}

// ── ProseMirror Plugin ─────────────────────────────────────────────────

function createAiHighlightPlugin() {
  return new Plugin({
    key: aiHighlightKey,

    state: {
      init() {
        return DecorationSet.empty;
      },

      apply(tr, decorationSet) {
        const meta = tr.getMeta(aiHighlightKey) as AiHighlight[] | undefined;

        if (meta !== undefined) {
          if (meta.length === 0) {
            return DecorationSet.empty;
          }

          const decorations = meta.map((h) =>
            Decoration.inline(h.from, h.to, {
              class: "ai-highlight",
              "data-similarity": String(Math.round(h.similarity * 100)),
            }),
          );

          return DecorationSet.create(tr.doc, decorations);
        }

        // Map existing decorations through document changes
        return decorationSet.map(tr.mapping, tr.doc);
      },
    },

    props: {
      decorations(state) {
        return this.getState(state);
      },
    },
  });
}

// ── Tiptap Extension ──────────────────────────────────────────────────

export const AiHighlightExtension = Extension.create({
  name: "aiHighlight",

  addProseMirrorPlugins() {
    return [createAiHighlightPlugin()];
  },
});
