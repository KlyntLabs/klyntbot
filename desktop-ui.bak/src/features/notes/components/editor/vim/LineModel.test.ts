import { type Node as PMNode, Schema } from "@tiptap/pm/model";
import { describe, expect, it } from "vitest";
import { LineModel } from "./LineModel";

// Minimal ProseMirror schema for testing
const schema = new Schema({
  nodes: {
    doc: { content: "block+" },
    paragraph: {
      content: "inline*",
      group: "block",
      toDOM: () => ["p", 0] as const,
    },
    code_block: {
      content: "text*",
      group: "block",
      code: true,
      toDOM: () => ["pre", ["code", 0]] as const,
    },
    heading: {
      content: "inline*",
      group: "block",
      attrs: { level: { default: 1 } },
      toDOM: (n: PMNode) => [`h${n.attrs.level}`, 0] as const,
    },
    text: { group: "inline" },
  },
});

function doc(...children: PMNode[]) {
  return schema.node("doc", null, children);
}
function p(text: string) {
  return text ? schema.node("paragraph", null, [schema.text(text)]) : schema.node("paragraph");
}
function codeBlock(text: string) {
  return text ? schema.node("code_block", null, [schema.text(text)]) : schema.node("code_block");
}
function heading(text: string, level = 1) {
  return schema.node("heading", { level }, text ? [schema.text(text)] : []);
}

describe("LineModel", () => {
  it("maps simple paragraphs to lines", () => {
    const d = doc(p("hello"), p("world"));
    const model = new LineModel(d);
    expect(model.lineCount()).toBe(2);
    expect(model.getLine(0)).toBe("hello");
    expect(model.getLine(1)).toBe("world");
  });

  it("maps code block newlines to separate lines", () => {
    const d = doc(codeBlock("line1\nline2\nline3"));
    const model = new LineModel(d);
    expect(model.lineCount()).toBe(3);
    expect(model.getLine(0)).toBe("line1");
    expect(model.getLine(1)).toBe("line2");
    expect(model.getLine(2)).toBe("line3");
  });

  it("maps mixed blocks correctly", () => {
    const d = doc(p("para"), codeBlock("a\nb"), heading("title"));
    const model = new LineModel(d);
    expect(model.lineCount()).toBe(4);
    expect(model.getLine(0)).toBe("para");
    expect(model.getLine(1)).toBe("a");
    expect(model.getLine(2)).toBe("b");
    expect(model.getLine(3)).toBe("title");
  });

  it("handles empty paragraphs as empty lines", () => {
    const d = doc(p("above"), p(""), p("below"));
    const model = new LineModel(d);
    expect(model.lineCount()).toBe(3);
    expect(model.getLine(1)).toBe("");
  });

  it("converts line/ch to PM position and back", () => {
    const d = doc(p("hello"), p("world"));
    const model = new LineModel(d);

    // "hello" starts at PM pos 1 (after doc open tag)
    const pmPos = model.toPMPos(0, 2); // line 0, ch 2 = 'l' in "hello"
    expect(pmPos).toBeGreaterThan(0);

    const { line, ch } = model.fromPMPos(pmPos);
    expect(line).toBe(0);
    expect(ch).toBe(2);
  });

  it("converts positions in code blocks", () => {
    const d = doc(codeBlock("abc\ndef"));
    const model = new LineModel(d);

    // line 1, ch 1 = 'e' in "def"
    const pmPos = model.toPMPos(1, 1);
    const back = model.fromPMPos(pmPos);
    expect(back.line).toBe(1);
    expect(back.ch).toBe(1);
  });

  it("clamps out-of-range positions", () => {
    const d = doc(p("hi"));
    const model = new LineModel(d);
    const clipped = model.clipPos({ line: 99, ch: 99 });
    expect(clipped.line).toBe(0);
    expect(clipped.ch).toBe(2); // "hi" has length 2
  });

  it("lineCount returns 1 for empty doc with one empty paragraph", () => {
    const d = doc(p(""));
    const model = new LineModel(d);
    expect(model.lineCount()).toBe(1);
    expect(model.getLine(0)).toBe("");
  });
});
