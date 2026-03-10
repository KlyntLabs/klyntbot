import { describe, expect, it } from "vitest";
import { CharClass, classifyChar, findWORDBoundary, findWordBoundary } from "./charClass";

describe("classifyChar", () => {
  it("classifies lowercase letters as Word", () => {
    for (const ch of "abcxyz") {
      expect(classifyChar(ch)).toBe(CharClass.Word);
    }
  });

  it("classifies uppercase letters as Word", () => {
    for (const ch of "ABCXYZ") {
      expect(classifyChar(ch)).toBe(CharClass.Word);
    }
  });

  it("classifies digits as Word", () => {
    for (const ch of "0123456789") {
      expect(classifyChar(ch)).toBe(CharClass.Word);
    }
  });

  it("classifies underscore as Word", () => {
    expect(classifyChar("_")).toBe(CharClass.Word);
  });

  it("classifies space as Space", () => {
    expect(classifyChar(" ")).toBe(CharClass.Space);
  });

  it("classifies tab as Space", () => {
    expect(classifyChar("\t")).toBe(CharClass.Space);
  });

  it("classifies newline as Space", () => {
    expect(classifyChar("\n")).toBe(CharClass.Space);
  });

  it("classifies punctuation characters as Punct", () => {
    for (const ch of ".,;:!?()[]{}@#$%^&*-+=/<>~`\"'|\\") {
      expect(classifyChar(ch)).toBe(CharClass.Punct);
    }
  });
});

describe("findWordBoundary", () => {
  describe("forward/start (w motion)", () => {
    it('moves from start of "hello" to start of "world"', () => {
      // "hello world" — from 0 skip "hello" (word), skip " " (space), land on 6
      expect(findWordBoundary("hello world", 0, "forward", "start")).toBe(6);
    });

    it("stops at punctuation boundary", () => {
      // "foo.bar baz" — from 0 skip "foo" (word), land on 3 (the dot)
      expect(findWordBoundary("foo.bar baz", 0, "forward", "start")).toBe(3);
    });

    it("skips from punctuation to next word", () => {
      // "foo.bar baz" — from 3 (dot), skip "." (punct), land on 4 ("bar")
      expect(findWordBoundary("foo.bar baz", 3, "forward", "start")).toBe(4);
    });

    it("skips whitespace from a space position", () => {
      // "hello world" — from 5 (space), skip space, land on 6
      expect(findWordBoundary("hello world", 5, "forward", "start")).toBe(6);
    });

    it("returns len when at end of string", () => {
      expect(findWordBoundary("hello", 4, "forward", "start")).toBe(5);
    });

    it("returns len when pos >= len", () => {
      expect(findWordBoundary("hello", 5, "forward", "start")).toBe(5);
      expect(findWordBoundary("hello", 10, "forward", "start")).toBe(5);
    });

    it("handles single character string", () => {
      expect(findWordBoundary("a", 0, "forward", "start")).toBe(1);
    });

    it("handles empty string", () => {
      expect(findWordBoundary("", 0, "forward", "start")).toBe(0);
    });

    it("skips multiple spaces", () => {
      expect(findWordBoundary("a   b", 0, "forward", "start")).toBe(4);
    });
  });

  describe("backward/start (b motion)", () => {
    it('moves from mid-"world" to start of "world"', () => {
      // "hello world" — from 8 ("r"), skip backward through "wor" (same word class), land on 6
      expect(findWordBoundary("hello world", 8, "backward", "start")).toBe(6);
    });

    it("moves from start of second word to start of first word", () => {
      // "hello world" — from 6 ("w"), i=5 (space), skip space->4 ("o"), skip "hello"->0
      expect(findWordBoundary("hello world", 6, "backward", "start")).toBe(0);
    });

    it("stays at 0 when already at start", () => {
      expect(findWordBoundary("hello", 0, "backward", "start")).toBe(0);
    });

    it("moves from pos 1 to 0", () => {
      expect(findWordBoundary("hello", 1, "backward", "start")).toBe(0);
    });

    it("handles punctuation boundary", () => {
      // "foo.bar" — from 4 ("b"), i=3 ("."), that's punct class, skip punct->3, then i-1=2 is "o" (word, different), land on 3
      expect(findWordBoundary("foo.bar", 4, "backward", "start")).toBe(3);
    });

    it("handles empty string", () => {
      expect(findWordBoundary("", 0, "backward", "start")).toBe(0);
    });

    it("handles single character", () => {
      expect(findWordBoundary("a", 0, "backward", "start")).toBe(0);
    });

    it("skips multiple spaces backward", () => {
      // "a   b" — from 4, i=3 (space), skip spaces->0 ("a"), that's word, skip->0
      expect(findWordBoundary("a   b", 4, "backward", "start")).toBe(0);
    });
  });

  describe("forward/end (e motion)", () => {
    it('lands on last char of "hello"', () => {
      // "hello world" — from 0, i=1, not space, cls=word, skip while text[i+1] is word -> i=4
      expect(findWordBoundary("hello world", 0, "forward", "end")).toBe(4);
    });

    it("lands on last char of next word when on last char of current", () => {
      // "hello world" — from 4 ("o"), i=5 (space), skip space->6 ("w"), skip while word->10
      expect(findWordBoundary("hello world", 4, "forward", "end")).toBe(10);
    });

    it("lands on punctuation end", () => {
      // "foo...bar" — from 2 ("o"), i=3 ("."), cls=punct, skip while punct->5, but text[6]="b" is word, so i=5
      expect(findWordBoundary("foo...bar", 2, "forward", "end")).toBe(5);
    });

    it("returns len-1 when near end of string", () => {
      expect(findWordBoundary("hello", 3, "forward", "end")).toBe(4);
    });

    it("stays at last char when already at end", () => {
      // from 4 (last char), i=5 >= len, return min(4, 4) = 4
      expect(findWordBoundary("hello", 4, "forward", "end")).toBe(4);
    });

    it("handles empty string", () => {
      expect(findWordBoundary("", 0, "forward", "end")).toBe(0);
    });

    it("handles single character", () => {
      // from 0, i=1 >= len(1), return min(0, 0) = 0
      expect(findWordBoundary("a", 0, "forward", "end")).toBe(0);
    });

    it("skips leading spaces to next word end", () => {
      // "  hello" — from 0, i=1 (space), skip spaces->2 ("h"), skip word->6, but len=7, text[i+1] check: i goes to 6
      expect(findWordBoundary("  hello", 0, "forward", "end")).toBe(6);
    });
  });
});

describe("findWORDBoundary", () => {
  describe("forward/start (W motion)", () => {
    it("skips punctuation as non-space", () => {
      // "foo.bar baz" — from 0, skip non-space "foo.bar"->7, skip space->8
      expect(findWORDBoundary("foo.bar baz", 0, "forward", "start")).toBe(8);
    });

    it("behaves like w for simple words", () => {
      expect(findWORDBoundary("hello world", 0, "forward", "start")).toBe(6);
    });

    it("returns len at end of string", () => {
      expect(findWORDBoundary("hello", 0, "forward", "start")).toBe(5);
    });

    it("handles empty string", () => {
      expect(findWORDBoundary("", 0, "forward", "start")).toBe(0);
    });

    it("returns text.length when pos >= length", () => {
      expect(findWORDBoundary("abc", 5, "forward", "start")).toBe(3);
    });

    it("skips from space to next WORD", () => {
      // "a  b" from 1 (space): skip non-space (only "a" already past), actually pos=1 is space
      // isSpace(1)=true so first while doesn't execute, skip spaces->3
      expect(findWORDBoundary("a  b", 1, "forward", "start")).toBe(3);
    });
  });

  describe("forward/end (E motion)", () => {
    it("lands on last non-space char of WORD", () => {
      // "foo.bar baz" — from 0, i=1, not space, skip non-space while text[i+1] non-space -> i=6
      expect(findWORDBoundary("foo.bar baz", 0, "forward", "end")).toBe(6);
    });

    it("handles simple words", () => {
      expect(findWORDBoundary("hello world", 0, "forward", "end")).toBe(4);
    });

    it("handles end of string", () => {
      expect(findWORDBoundary("hello", 4, "forward", "end")).toBe(4);
    });

    it("handles single character", () => {
      expect(findWORDBoundary("a", 0, "forward", "end")).toBe(0);
    });
  });

  describe("backward/start (B motion)", () => {
    it("skips punctuation as non-space", () => {
      // "foo.bar baz" — from 8, i=7 (space), skip space->6 ("r"), skip non-space backward while text[i-1] non-space -> i=0
      expect(findWORDBoundary("foo.bar baz", 8, "backward", "start")).toBe(0);
    });

    it("handles simple words", () => {
      expect(findWORDBoundary("hello world", 8, "backward", "start")).toBe(6);
    });

    it("stays at 0 when at start", () => {
      expect(findWORDBoundary("hello", 0, "backward", "start")).toBe(0);
    });

    it("handles empty string", () => {
      expect(findWORDBoundary("", 0, "backward", "start")).toBe(0);
    });
  });
});
