import { Extension } from "@tiptap/core";
import { Plugin, PluginKey } from "@tiptap/pm/state";
import { ulid } from "ulid";

export const UniqueID = Extension.create({
  name: "uniqueID",

  addGlobalAttributes() {
    return [
      {
        types: ["heading"],
        attributes: {
          id: {
            default: null,
            parseHTML: (element) => element.getAttribute("data-id"),
            renderHTML: (attributes) => {
              if (!attributes.id) return {};
              return { "data-id": attributes.id };
            },
          },
        },
      },
    ];
  },

  addProseMirrorPlugins() {
    return [
      new Plugin({
        key: new PluginKey("uniqueID"),
        appendTransaction: (transactions, _oldState, newState) => {
          if (!transactions.some((tr) => tr.docChanged)) return null;
          const { tr } = newState;
          let modified = false;
          newState.doc.descendants((node, pos) => {
            if (node.type.name === "heading" && !node.attrs.id) {
              tr.setNodeMarkup(pos, undefined, {
                ...node.attrs,
                id: ulid(),
              });
              modified = true;
            }
          });
          return modified ? tr : null;
        },
      }),
    ];
  },
});
