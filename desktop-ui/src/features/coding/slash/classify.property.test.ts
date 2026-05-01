import { describe, expect, test } from "vitest";
import { classify } from "./classify";

function randomString(length: number): string {
  const chars = "ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789 /-_";
  let result = "";
  for (let i = 0; i < length; i++) {
    result += chars.charAt(Math.floor(Math.random() * chars.length));
  }
  return result;
}

describe("K9: classify is deterministic and stable", () => {
  test("same input always returns same value across 500 random trials", () => {
    for (let i = 0; i < 500; i++) {
      const input = randomString(Math.floor(Math.random() * 80));
      const a = classify(input);
      const b = classify(input);
      expect(a).toBe(b);
    }
  });

  test("classify never throws on 500 random inputs", () => {
    for (let i = 0; i < 500; i++) {
      const input = randomString(Math.floor(Math.random() * 200));
      expect(() => classify(input)).not.toThrow();
    }
  });

  test("classify returns null on empty or whitespace-only inputs", () => {
    expect(classify("")).toBeNull();
    expect(classify("   ")).toBeNull();
    expect(classify("\n\n")).toBeNull();
  });

  test("registered commands always classify (not null)", () => {
    expect(classify("/skills list")).toBe("direct");
    expect(classify("/plan")).toBe("agent");
    expect(classify("/help")).toBe("direct");
  });
});
