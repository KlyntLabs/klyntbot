import { describe, expect, test } from "vitest";
import { flatCatalog, REGISTRY } from "./registry";

describe("slash registry", () => {
  test("flatCatalog enumerates leaves", () => {
    const flat = flatCatalog();
    expect(flat.length).toBeGreaterThan(8);
    expect(flat.some((c) => c.command === "/skills list")).toBe(true);
    expect(flat.some((c) => c.command === "/plan")).toBe(true);
  });
  test("no leaf has a command name colliding with a branch's top key", () => {
    for (const [key, node] of Object.entries(REGISTRY)) {
      if (node.kind === "leaf") {
        expect(node.command.startsWith(key)).toBe(true);
      } else {
        for (const childKey of Object.keys(node.children)) {
          expect(childKey.includes(" ")).toBe(false);
        }
      }
    }
  });
});
