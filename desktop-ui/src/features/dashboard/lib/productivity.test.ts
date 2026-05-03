import { describe, expect, it } from "vitest";
import {
  getAppColor,
  getCategoryColor,
  purityToOpacity,
  qualityToColor,
  resolveActivityColor,
  resolveCategoryLabel,
  scoreColor,
} from "./productivity";

describe("scoreColor", () => {
  it("returns success for >= 80", () => {
    expect(scoreColor(80)).toBe("var(--success)");
    expect(scoreColor(95)).toBe("var(--success)");
  });
  it("returns brand for 60-79", () => {
    expect(scoreColor(60)).toBe("var(--brand)");
    expect(scoreColor(79)).toBe("var(--brand)");
  });
  it("returns muted for 40-59", () => {
    expect(scoreColor(40)).toBe("var(--text-muted-foreground)");
  });
  it("returns destructive below 40", () => {
    expect(scoreColor(0)).toBe("var(--destructive)");
    expect(scoreColor(39)).toBe("var(--destructive)");
  });
});

describe("getAppColor", () => {
  it("matches lowercase native app names", () => {
    expect(getAppColor("Visual Studio Code", null)).toBe("#007ACC");
  });
  it("matches lowercase domain site names", () => {
    expect(getAppColor("youtube.com", null)).toBe("#FF0000");
  });
  it("falls back to category color when app unknown", () => {
    expect(getAppColor("UnknownApp", "coding")).toBe("#22C55E");
  });
  it("falls back to brand when nothing matches", () => {
    expect(getAppColor("UnknownApp", null)).toBe("var(--brand)");
  });
});

describe("getCategoryColor", () => {
  it("resolves a known category id", () => {
    expect(getCategoryColor("coding")).toBe("#22C55E");
  });
  it("normalizes display names with spaces and ampersands", () => {
    expect(getCategoryColor("Project Management")).toBe("#8B5CF6");
  });
  it("falls back to a rotating palette for unknown", () => {
    expect(getCategoryColor("zzz", 0)).toBe("#60A5FA");
  });
});

describe("resolveActivityColor", () => {
  it("returns surface-highest for idle", () => {
    expect(resolveActivityColor("anything", true)).toBe("var(--surface-highest)");
  });
  it("returns success for productive", () => {
    expect(resolveActivityColor("productive", false)).toBe("var(--success)");
  });
  it("returns destructive for distracting", () => {
    expect(resolveActivityColor("distracting", false)).toBe("var(--destructive)");
  });
  it("falls back to brand for unknown category type", () => {
    expect(resolveActivityColor("zzz", false)).toBe("var(--brand)");
  });
});

describe("resolveCategoryLabel", () => {
  it("maps known types", () => {
    expect(resolveCategoryLabel("productive")).toBe("Productive");
    expect(resolveCategoryLabel("distracting")).toBe("Distracting");
    expect(resolveCategoryLabel("neutral")).toBe("Neutral");
  });
  it("falls back to Uncategorized", () => {
    expect(resolveCategoryLabel("zzz")).toBe("Uncategorized");
  });
});

describe("qualityToColor", () => {
  it("returns an oklch() string", () => {
    const c = qualityToColor(50);
    expect(c.startsWith("oklch(")).toBe(true);
  });
  it("clamps to 0..100", () => {
    expect(qualityToColor(-50)).toBe(qualityToColor(0));
    expect(qualityToColor(150)).toBe(qualityToColor(100));
  });
});

describe("purityToOpacity", () => {
  it("returns 0.65 for null", () => {
    expect(purityToOpacity(null)).toBe(0.65);
  });
  it("maps 0 → 0.5 and 1 → 0.9", () => {
    expect(purityToOpacity(0)).toBeCloseTo(0.5);
    expect(purityToOpacity(1)).toBeCloseTo(0.9);
  });
});
