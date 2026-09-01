import { describe, expect, it } from "vitest";
import { tagBgColor, tagColor } from "./tagColor";

describe("tagColor", () => {
  it("returns a consistent color for the same tag", () => {
    expect(tagColor("rust")).toBe(tagColor("rust"));
    expect(tagColor("python")).toBe(tagColor("python"));
  });

  it("returns different colors for different tags", () => {
    const colors = new Set(["rust", "python", "go", "java", "ruby"].map(tagColor));
    expect(colors.size).toBeGreaterThan(1);
  });

  it("returns a valid hex color", () => {
    expect(tagColor("test")).toMatch(/^#[0-9a-fA-F]{6}$/);
  });

  it("tagBgColor returns color with opacity suffix", () => {
    expect(tagBgColor("test")).toMatch(/^#[0-9a-fA-F]{6}25$/);
  });
});
