/**
 * Rehype plugin that transforms [@type:id] markers in text nodes into
 * <memory-ref data-ref-type="type" data-ref-id="id"> elements.
 *
 * These are rendered by react-markdown's custom component mapping
 * as inline MemoryReference tooltips.
 */

import type { Element, ElementContent, Root, Text } from "hast";
import { visit } from "unist-util-visit";

const MEMORY_REF_PATTERN = /\[@(\w+):([a-f0-9-]+)\]/g;

export function rehypeMemoryRef() {
  return (tree: Root) => {
    visit(tree, "text", (node: Text, index, parent) => {
      if (!parent || index === undefined) return;
      const value = node.value;
      if (!value.includes("[@")) return;

      const parts: ElementContent[] = [];
      let lastIndex = 0;

      for (const match of value.matchAll(MEMORY_REF_PATTERN)) {
        const matchStart = match.index;
        const refType = match[1];
        const refId = match[2];

        // Text before the match
        if (matchStart > lastIndex) {
          parts.push({ type: "text", value: value.slice(lastIndex, matchStart) });
        }

        // The memory-ref element
        const element: Element = {
          type: "element",
          tagName: "memory-ref",
          properties: {
            "data-ref-type": refType,
            "data-ref-id": refId,
          },
          children: [],
        };
        parts.push(element);

        lastIndex = matchStart + match[0].length;
      }

      // No matches found — the pattern check passed but matchAll found nothing
      if (parts.length === 0) return;

      // Trailing text after last match
      if (lastIndex < value.length) {
        parts.push({ type: "text", value: value.slice(lastIndex) });
      }

      // Replace the text node with the new nodes
      parent.children.splice(index, 1, ...parts);
    });
  };
}
