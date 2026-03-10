import type { Node as PMNode } from "@tiptap/pm/model";
import { Plugin, PluginKey } from "@tiptap/pm/state";
import { Decoration, DecorationSet } from "@tiptap/pm/view";
import { buildFlatTextMap, type FlatTextMap } from "./positions";

export const vimSearchPluginKey = new PluginKey("vimSearch");

export function createVimSearchPlugin(getPattern: () => string | null) {
  // Cache the flat text map keyed by doc identity (doc is immutable in PM)
  let cachedDoc: PMNode | null = null;
  let cachedMap: FlatTextMap | null = null;

  function getFlatTextMap(doc: PMNode, state: Parameters<typeof buildFlatTextMap>[0]): FlatTextMap {
    if (cachedDoc === doc && cachedMap) return cachedMap;
    cachedMap = buildFlatTextMap(state);
    cachedDoc = doc;
    return cachedMap;
  }

  return new Plugin({
    key: vimSearchPluginKey,
    props: {
      decorations(state) {
        const pattern = getPattern();
        if (!pattern) return DecorationSet.empty;

        try {
          const regex = new RegExp(pattern, "gi");
          const map = getFlatTextMap(state.doc, state);
          const decorations: Decoration[] = [];

          for (let match = regex.exec(map.text); match !== null; match = regex.exec(map.text)) {
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
        regex.lastIndex = 0;
        match = regex.exec(text);
      }
      return match ? match.index : null;
    }

    const matches: number[] = [];
    for (let m = regex.exec(text); m !== null; m = regex.exec(text)) {
      matches.push(m.index);
      if (m.index === regex.lastIndex) regex.lastIndex++;
    }

    for (let i = matches.length - 1; i >= 0; i--) {
      if (matches[i] < fromFlat) return matches[i];
    }
    return matches.length > 0 ? matches[matches.length - 1] : null;
  } catch {
    return null;
  }
}
