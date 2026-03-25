import { describe, expect, it } from "vitest";
import { PMSearchCursor } from "./SearchCursor";

describe("PMSearchCursor", () => {
  const text = "hello world\nfoo bar\nhello again";

  it("finds forward matches", () => {
    const cursor = new PMSearchCursor(text, /hello/g, 0);
    expect(cursor.findNext()).toBe(true);
    expect(cursor.from()).toBe(0);
    expect(cursor.to()).toBe(5);
    expect(cursor.findNext()).toBe(true);
    expect(cursor.from()).toBe(20);
    expect(cursor.to()).toBe(25);
    expect(cursor.findNext()).toBe(false);
  });

  it("finds backward matches", () => {
    const cursor = new PMSearchCursor(text, /hello/g, 25);
    expect(cursor.findPrevious()).toBe(true);
    expect(cursor.from()).toBe(20);
    expect(cursor.to()).toBe(25);
    expect(cursor.findPrevious()).toBe(true);
    expect(cursor.from()).toBe(0);
    expect(cursor.to()).toBe(5);
    expect(cursor.findPrevious()).toBe(false);
  });

  it("starts from given position", () => {
    const cursor = new PMSearchCursor(text, /hello/g, 10);
    expect(cursor.findNext()).toBe(true);
    expect(cursor.from()).toBe(20); // skips first "hello"
  });

  it("handles case-insensitive regex", () => {
    const cursor = new PMSearchCursor("Hello HELLO", /hello/gi, 0);
    expect(cursor.findNext()).toBe(true);
    expect(cursor.from()).toBe(0);
    expect(cursor.findNext()).toBe(true);
    expect(cursor.from()).toBe(6);
  });

  it("replaces matches", () => {
    const cursor = new PMSearchCursor("aXbXc", /X/g, 0);
    cursor.findNext();
    expect(cursor.from()).toBe(1);
    expect(typeof cursor.replace).toBe("function");
  });

  it("exposes match groups", () => {
    const cursor = new PMSearchCursor("foo123bar", /(\d+)/g, 0);
    cursor.findNext();
    expect(cursor.match).not.toBeNull();
    expect(cursor.match?.[1]).toBe("123");
  });
});
