import { describe, expect, it } from "vitest";
import { findTextObject } from "./textObjects";

describe("findTextObject", () => {
  // -----------------------------------------------------------------------
  // Word objects (w)
  // -----------------------------------------------------------------------
  describe("word (w)", () => {
    it("iw on a word returns the word range", () => {
      // "hello world", pos 1 (on 'e') -> {0, 5}
      expect(findTextObject("hello world", 1, "i", "w")).toEqual({
        from: 0,
        to: 5,
      });
    });

    it("aw on a word includes trailing whitespace", () => {
      // "hello world", pos 1 -> {0, 6}
      expect(findTextObject("hello world", 1, "a", "w")).toEqual({
        from: 0,
        to: 6,
      });
    });

    it("iw on second word", () => {
      expect(findTextObject("hello world", 7, "i", "w")).toEqual({
        from: 6,
        to: 11,
      });
    });

    it("aw on last word with no trailing space includes leading space", () => {
      expect(findTextObject("hello world", 7, "a", "w")).toEqual({
        from: 5,
        to: 11,
      });
    });

    it("iw on whitespace returns whitespace range", () => {
      expect(findTextObject("hello   world", 6, "i", "w")).toEqual({
        from: 5,
        to: 8,
      });
    });

    it("iw on punctuation returns punctuation run", () => {
      expect(findTextObject("foo...bar", 4, "i", "w")).toEqual({
        from: 3,
        to: 6,
      });
    });

    it("iw at start of text", () => {
      expect(findTextObject("hello", 0, "i", "w")).toEqual({
        from: 0,
        to: 5,
      });
    });

    it("returns null for empty text", () => {
      expect(findTextObject("", 0, "i", "w")).toBeNull();
    });

    it("returns null for out-of-bounds position", () => {
      expect(findTextObject("hello", 10, "i", "w")).toBeNull();
    });
  });

  // -----------------------------------------------------------------------
  // WORD objects (W)
  // -----------------------------------------------------------------------
  describe("WORD (W)", () => {
    it("iW treats punctuation as part of WORD", () => {
      expect(findTextObject("foo.bar baz", 2, "i", "W")).toEqual({
        from: 0,
        to: 7,
      });
    });

    it("aW includes trailing whitespace", () => {
      expect(findTextObject("foo.bar baz", 2, "a", "W")).toEqual({
        from: 0,
        to: 8,
      });
    });
  });

  // -----------------------------------------------------------------------
  // Bracket objects: parentheses ( ) b
  // -----------------------------------------------------------------------
  describe("parentheses ( ) b", () => {
    it("i( returns inner content of parens", () => {
      // "foo(bar baz)qux", pos 5 -> {4, 11}
      expect(findTextObject("foo(bar baz)qux", 5, "i", "(")).toEqual({
        from: 4,
        to: 11,
      });
    });

    it("a( returns parens and content", () => {
      // "foo(bar baz)qux", pos 5 -> {3, 12}
      expect(findTextObject("foo(bar baz)qux", 5, "a", "(")).toEqual({
        from: 3,
        to: 12,
      });
    });

    it("i) works same as i(", () => {
      expect(findTextObject("foo(bar baz)qux", 5, "i", ")")).toEqual({
        from: 4,
        to: 11,
      });
    });

    it("ib works same as i(", () => {
      expect(findTextObject("foo(bar baz)qux", 5, "i", "b")).toEqual({
        from: 4,
        to: 11,
      });
    });

    it("nested brackets: i( finds innermost pair", () => {
      // "((inner))", pos 3 -> inner pair is positions 1..7 with content 2..7
      // The inner "(" is at 1, inner ")" is at 7
      expect(findTextObject("((inner))", 3, "i", "(")).toEqual({
        from: 2,
        to: 7,
      });
    });

    it("nested brackets: cursor on outer paren", () => {
      // "((inner))", pos 0 -> outer pair 0..8
      expect(findTextObject("((inner))", 0, "i", "(")).toEqual({
        from: 1,
        to: 8,
      });
    });

    it("no matching bracket returns null", () => {
      expect(findTextObject("foo(bar", 5, "i", "(")).toBeNull();
    });

    it("cursor outside brackets returns null", () => {
      expect(findTextObject("x (y) z", 0, "i", "(")).toBeNull();
    });

    it("empty parens inner", () => {
      expect(findTextObject("()", 0, "i", "(")).toEqual({
        from: 1,
        to: 1,
      });
    });

    it("cursor on closing bracket", () => {
      expect(findTextObject("(abc)", 4, "i", "(")).toEqual({
        from: 1,
        to: 4,
      });
    });
  });

  // -----------------------------------------------------------------------
  // Bracket objects: square brackets [ ]
  // -----------------------------------------------------------------------
  describe("brackets [ ]", () => {
    it("i[ returns inner content", () => {
      expect(findTextObject("a[bc]d", 2, "i", "[")).toEqual({
        from: 2,
        to: 4,
      });
    });

    it("a] returns bracket and content", () => {
      expect(findTextObject("a[bc]d", 2, "a", "]")).toEqual({
        from: 1,
        to: 5,
      });
    });
  });

  // -----------------------------------------------------------------------
  // Bracket objects: braces { } B
  // -----------------------------------------------------------------------
  describe("braces { } B", () => {
    it("i{ returns inner content", () => {
      // "x { y } z", pos 4 -> {3, 6}
      expect(findTextObject("x { y } z", 4, "i", "{")).toEqual({
        from: 3,
        to: 6,
      });
    });

    it("a{ returns braces and content", () => {
      expect(findTextObject("x { y } z", 4, "a", "{")).toEqual({
        from: 2,
        to: 7,
      });
    });

    it("iB works same as i{", () => {
      expect(findTextObject("x { y } z", 4, "i", "B")).toEqual({
        from: 3,
        to: 6,
      });
    });

    it("a} works same as a{", () => {
      expect(findTextObject("x { y } z", 4, "a", "}")).toEqual({
        from: 2,
        to: 7,
      });
    });
  });

  // -----------------------------------------------------------------------
  // Bracket objects: angle brackets < >
  // -----------------------------------------------------------------------
  describe("angle brackets < >", () => {
    it("i< returns inner content", () => {
      expect(findTextObject("<div>", 2, "i", "<")).toEqual({
        from: 1,
        to: 4,
      });
    });

    it("a> returns brackets and content", () => {
      expect(findTextObject("<div>", 2, "a", ">")).toEqual({
        from: 0,
        to: 5,
      });
    });
  });

  // -----------------------------------------------------------------------
  // Quote objects: " ' `
  // -----------------------------------------------------------------------
  describe('quotes (")', () => {
    it('i" returns inner content of double quotes', () => {
      // 'say "hello" end', pos 6 -> {5, 10}
      expect(findTextObject('say "hello" end', 6, "i", '"')).toEqual({
        from: 5,
        to: 10,
      });
    });

    it('a" returns quotes and content', () => {
      // 'say "hello" end', pos 6 -> {4, 11}
      expect(findTextObject('say "hello" end', 6, "a", '"')).toEqual({
        from: 4,
        to: 11,
      });
    });

    it("cursor on opening quote", () => {
      expect(findTextObject('say "hello" end', 4, "i", '"')).toEqual({
        from: 5,
        to: 10,
      });
    });

    it("cursor on closing quote", () => {
      expect(findTextObject('say "hello" end', 10, "i", '"')).toEqual({
        from: 5,
        to: 10,
      });
    });

    it("handles escaped quotes", () => {
      // 'say "he\"llo" end' — the \" should not count as a boundary
      const text = 'say "he\\"llo" end';
      // Positions: s(0) a(1) y(2) (3) "(4) h(5) e(6) \(7) "(8) l(9) l(10) o(11) "(12) (13) e(14) n(15) d(16)
      // Unescaped quotes at: 4, 12. Escaped quote at: 8 (preceded by \)
      expect(findTextObject(text, 6, "i", '"')).toEqual({
        from: 5,
        to: 12,
      });
    });

    it("empty quotes inner", () => {
      expect(findTextObject('""', 0, "i", '"')).toEqual({
        from: 1,
        to: 1,
      });
    });

    it("cursor outside quotes returns null", () => {
      expect(findTextObject('x "y" z', 0, "i", '"')).toBeNull();
    });

    it("no closing quote returns null", () => {
      expect(findTextObject('"hello', 2, "i", '"')).toBeNull();
    });
  });

  describe("quotes (')", () => {
    it("i' returns inner content of single quotes", () => {
      expect(findTextObject("say 'hello' end", 6, "i", "'")).toEqual({
        from: 5,
        to: 10,
      });
    });

    it("a' includes quote delimiters", () => {
      expect(findTextObject("say 'hello' end", 6, "a", "'")).toEqual({
        from: 4,
        to: 11,
      });
    });
  });

  describe("quotes (`)", () => {
    it("i` returns inner content of backtick quotes", () => {
      expect(findTextObject("use `code` here", 6, "i", "`")).toEqual({
        from: 5,
        to: 9,
      });
    });
  });

  // -----------------------------------------------------------------------
  // Sentence objects (s)
  // -----------------------------------------------------------------------
  describe("sentence (s)", () => {
    it("is returns the current sentence", () => {
      const text = "Hello world. Foo bar.";
      // pos 2 is in "Hello world."
      const result = findTextObject(text, 2, "i", "s");
      expect(result).toEqual({ from: 0, to: 12 });
    });

    it("as includes trailing whitespace", () => {
      const text = "Hello world. Foo bar.";
      const result = findTextObject(text, 2, "a", "s");
      expect(result).toEqual({ from: 0, to: 13 });
    });

    it("is on second sentence", () => {
      const text = "Hello world. Foo bar.";
      // pos 14 is in "Foo bar."
      const result = findTextObject(text, 14, "i", "s");
      expect(result).toEqual({ from: 13, to: 21 });
    });

    it("handles exclamation marks", () => {
      const text = "Stop! Go now.";
      const result = findTextObject(text, 2, "i", "s");
      expect(result).toEqual({ from: 0, to: 5 });
    });

    it("handles question marks", () => {
      const text = "Why? Because.";
      const result = findTextObject(text, 1, "i", "s");
      expect(result).toEqual({ from: 0, to: 4 });
    });
  });

  // -----------------------------------------------------------------------
  // Paragraph objects (p)
  // -----------------------------------------------------------------------
  describe("paragraph (p)", () => {
    it("ip returns current paragraph", () => {
      // "line1\nline2\n\nline3", pos 2
      // Paragraph 1 is "line1\nline2" (positions 0..10)
      const text = "line1\nline2\n\nline3";
      const result = findTextObject(text, 2, "i", "p");
      expect(result).toEqual({ from: 0, to: 11 });
    });

    it("ap includes trailing blank lines", () => {
      const text = "line1\nline2\n\nline3";
      const result = findTextObject(text, 2, "a", "p");
      // Paragraph "line1\nline2" + trailing "\n" blank line
      expect(result).toEqual({ from: 0, to: 12 });
    });

    it("ip on second paragraph", () => {
      const text = "line1\nline2\n\nline3\nline4";
      // pos 14 is in "line3", second paragraph is "line3\nline4" at positions 13..24
      const result = findTextObject(text, 14, "i", "p");
      expect(result).toEqual({ from: 13, to: 24 });
    });

    it("ip on single-line text", () => {
      const text = "hello world";
      const result = findTextObject(text, 3, "i", "p");
      expect(result).toEqual({ from: 0, to: 11 });
    });

    it("ip with multiple blank lines between paragraphs", () => {
      const text = "para1\n\n\npara2";
      const result = findTextObject(text, 2, "i", "p");
      expect(result).toEqual({ from: 0, to: 5 });
    });
  });

  // -----------------------------------------------------------------------
  // Edge cases
  // -----------------------------------------------------------------------
  describe("edge cases", () => {
    it("unknown type returns null", () => {
      expect(findTextObject("hello", 2, "i", "z")).toBeNull();
    });

    it("empty text returns null for all types", () => {
      for (const type of ["w", "W", "s", "p", "(", "[", "{", "<", '"']) {
        expect(findTextObject("", 0, "i", type)).toBeNull();
      }
    });

    it("deeply nested brackets", () => {
      const text = "(a(b(c)d)e)";
      // Cursor on 'c' at pos 5, innermost is ( at 4, ) at 6
      expect(findTextObject(text, 5, "i", "(")).toEqual({
        from: 5,
        to: 6,
      });
      expect(findTextObject(text, 5, "a", "(")).toEqual({
        from: 4,
        to: 7,
      });
    });

    it("multiple quote pairs, cursor in second pair", () => {
      const text = '"first" "second"';
      // pos 10 is in "second"
      expect(findTextObject(text, 10, "i", '"')).toEqual({
        from: 9,
        to: 15,
      });
    });

    it("brackets with content across multiple nesting levels", () => {
      const text = "{outer {inner} more}";
      // pos 2 is in "outer", inside outer braces
      expect(findTextObject(text, 2, "i", "{")).toEqual({
        from: 1,
        to: 19,
      });
    });
  });
});
