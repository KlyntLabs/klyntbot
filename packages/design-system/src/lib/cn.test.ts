import { describe, expect, it } from "vitest";
import { cn } from "./cn";

const dsFontSizes = [
  "text-ui",
  "text-ui-sm",
  "text-ui-xs",
  "text-title-sm",
  "text-body",
  "text-display",
];

describe("cn", () => {
  it.each(dsFontSizes)("keeps both a design-system size (%s) and a text colour", (size) => {
    expect(cn(size, "text-fg")).toBe(`${size} text-fg`);
  });

  it.each(dsFontSizes)("keeps both when the colour comes first (%s)", (size) => {
    expect(cn("text-fg", size)).toBe(`text-fg ${size}`);
  });

  it("keeps a design-system size alongside other classes and a colour", () => {
    expect(cn("text-title-sm font-semibold", "text-fg")).toBe(
      "text-title-sm font-semibold text-fg",
    );
  });

  it("still collapses a genuine font-size conflict, keeping the last", () => {
    expect(cn("text-ui", "text-title")).toBe("text-title");
    expect(cn("text-title-sm", "text-ui-xs")).toBe("text-ui-xs");
    expect(cn("text-sm", "text-ui")).toBe("text-ui");
  });

  it("still collapses a genuine text-colour conflict, keeping the last", () => {
    expect(cn("text-fg", "text-fg-secondary")).toBe("text-fg-secondary");
  });

  it("never lets a size and a colour evict each other, in either order", () => {
    expect(cn("text-ui", "text-fg-secondary", "text-title")).toBe("text-fg-secondary text-title");
    expect(cn("text-fg", "text-ui", "text-fg-secondary")).toBe("text-ui text-fg-secondary");
  });
});
