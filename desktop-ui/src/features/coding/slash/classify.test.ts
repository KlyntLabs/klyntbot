import { describe, expect, test } from "vitest";
import { classify } from "./classify";

describe("slash classify", () => {
  test("returns null for non-slash input", () => {
    expect(classify("hello world")).toBeNull();
  });
  test("returns 'agent' for /plan", () => {
    expect(classify("/plan refactor parser")).toBe("agent");
  });
  test("returns 'direct' for /skills list", () => {
    expect(classify("/skills list")).toBe("direct");
  });
  test("returns 'direct' for /status", () => {
    expect(classify("/status")).toBe("direct");
  });
  test("returns null for unknown command", () => {
    expect(classify("/foobarbaz xyz")).toBeNull();
  });
  test("trims leading whitespace before checking first char", () => {
    expect(classify(" /plan x")).toBeNull(); // not first non-whitespace; rule 1
  });
  test("/sk (partial prefix) returns null", () => {
    expect(classify("/sk")).toBeNull();
  });
  test("/sessions star returns 'direct'", () => {
    expect(classify("/sessions star")).toBe("direct");
  });
  test("/sessions returns null (branch, no leaf path)", () => {
    expect(classify("/sessions")).toBeNull();
  });
  test("/help returns 'direct'", () => {
    expect(classify("/help")).toBe("direct");
  });
});
