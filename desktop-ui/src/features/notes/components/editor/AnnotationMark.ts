import { Mark, mergeAttributes } from "@tiptap/core";

export interface AnnotationMarkOptions {
  HTMLAttributes: Record<string, unknown>;
}

declare module "@tiptap/core" {
  interface Commands<ReturnType> {
    annotationMark: {
      setAnnotation: (annotationId: string) => ReturnType;
      unsetAnnotation: (annotationId: string) => ReturnType;
    };
  }
}

export const AnnotationMark = Mark.create<AnnotationMarkOptions>({
  name: "annotation",

  addOptions() {
    return { HTMLAttributes: {} };
  },

  addAttributes() {
    return {
      annotationId: {
        default: null,
        parseHTML: (el) => el.getAttribute("data-annotation-id"),
        renderHTML: (attrs) => ({ "data-annotation-id": attrs.annotationId }),
      },
      pending: {
        default: false,
        renderHTML: (attrs) => (attrs.pending ? { "data-pending": "true" } : {}),
      },
    };
  },

  parseHTML() {
    return [{ tag: "span[data-annotation-id]" }];
  },

  renderHTML({ HTMLAttributes }) {
    return [
      "span",
      mergeAttributes(this.options.HTMLAttributes, HTMLAttributes, {
        class: "annotation-highlight",
      }),
      0,
    ];
  },

  addCommands() {
    return {
      setAnnotation:
        (annotationId: string) =>
        ({ commands }) =>
          commands.setMark(this.name, { annotationId, pending: true }),
      unsetAnnotation:
        (annotationId: string) =>
        ({ tr, state }) => {
          state.doc.descendants((node, pos) => {
            for (const mark of node.marks) {
              if (mark.type.name === this.name && mark.attrs.annotationId === annotationId) {
                tr.removeMark(pos, pos + node.nodeSize, mark);
              }
            }
          });
          return true;
        },
    };
  },

  addKeyboardShortcuts() {
    return {
      "Alt-a": () => {
        window.dispatchEvent(new CustomEvent("editor-action", { detail: { action: "annotate" } }));
        return true;
      },
      "Alt-f": () => {
        window.dispatchEvent(new CustomEvent("editor-action", { detail: { action: "flashcard" } }));
        return true;
      },
      "Alt-l": () => {
        window.dispatchEvent(
          new CustomEvent("editor-action", {
            detail: { action: "linked-view" },
          }),
        );
        return true;
      },
    };
  },
});
