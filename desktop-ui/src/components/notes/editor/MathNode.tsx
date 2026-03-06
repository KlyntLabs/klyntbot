import { Node } from "@tiptap/core";
import katex from "katex";
import "katex/dist/katex.min.css";

export const MathBlock = Node.create({
  name: "mathBlock",
  group: "block",
  atom: true,

  addAttributes() {
    return { tex: { default: "" } };
  },

  parseHTML() {
    return [
      {
        tag: "div[data-math-block]",
        getAttrs: (el) => ({
          tex: decodeURIComponent((el as HTMLElement).getAttribute("data-math-block") || ""),
        }),
      },
    ];
  },

  renderHTML({ HTMLAttributes }) {
    const tex = HTMLAttributes.tex || "";
    return [
      "div",
      {
        "data-math-block": encodeURIComponent(tex),
        class: "math-block",
        contenteditable: "false",
      },
      katex.renderToString(tex, { displayMode: true, throwOnError: false }),
    ];
  },

  addNodeView() {
    return ({ node }) => {
      const dom = document.createElement("div");
      dom.classList.add("math-block");
      dom.contentEditable = "false";
      dom.setAttribute("data-math-block", encodeURIComponent(node.attrs.tex));
      dom.innerHTML = katex.renderToString(node.attrs.tex, {
        displayMode: true,
        throwOnError: false,
      });
      return { dom };
    };
  },
});

export const MathInline = Node.create({
  name: "mathInline",
  group: "inline",
  inline: true,
  atom: true,

  addAttributes() {
    return { tex: { default: "" } };
  },

  parseHTML() {
    return [
      {
        tag: "span[data-math-inline]",
        getAttrs: (el) => ({
          tex: decodeURIComponent((el as HTMLElement).getAttribute("data-math-inline") || ""),
        }),
      },
    ];
  },

  renderHTML({ HTMLAttributes }) {
    const tex = HTMLAttributes.tex || "";
    return [
      "span",
      {
        "data-math-inline": encodeURIComponent(tex),
        class: "math-inline",
        contenteditable: "false",
      },
      katex.renderToString(tex, { displayMode: false, throwOnError: false }),
    ];
  },

  addNodeView() {
    return ({ node }) => {
      const dom = document.createElement("span");
      dom.classList.add("math-inline");
      dom.contentEditable = "false";
      dom.setAttribute("data-math-inline", encodeURIComponent(node.attrs.tex));
      dom.innerHTML = katex.renderToString(node.attrs.tex, {
        displayMode: false,
        throwOnError: false,
      });
      return { dom };
    };
  },
});
