import { describe, expect, it } from "vitest";
import { parseDurationToEndsAt } from "../parseDuration";

describe("parseDurationToEndsAt", () => {
  it("parses 30m", () => {
    const before = Date.now();
    const result = parseDurationToEndsAt("30m");
    const after = Date.now();
    expect(result).not.toBeNull();
    const ts = new Date(result!).getTime();
    expect(ts).toBeGreaterThanOrEqual(before + 30 * 60_000);
    expect(ts).toBeLessThanOrEqual(after + 30 * 60_000 + 100);
  });

  it("parses 2h", () => {
    const before = Date.now();
    const result = parseDurationToEndsAt("2h");
    const after = Date.now();
    expect(result).not.toBeNull();
    const ts = new Date(result!).getTime();
    expect(ts).toBeGreaterThanOrEqual(before + 2 * 3_600_000);
    expect(ts).toBeLessThanOrEqual(after + 2 * 3_600_000 + 100);
  });

  it("parses 1d", () => {
    const before = Date.now();
    const result = parseDurationToEndsAt("1d");
    const after = Date.now();
    expect(result).not.toBeNull();
    const ts = new Date(result!).getTime();
    expect(ts).toBeGreaterThanOrEqual(before + 86_400_000);
    expect(ts).toBeLessThanOrEqual(after + 86_400_000 + 100);
  });

  it("returns null for invalid input", () => {
    expect(parseDurationToEndsAt("until 5pm")).toBeNull();
    expect(parseDurationToEndsAt("foo")).toBeNull();
    expect(parseDurationToEndsAt("")).toBeNull();
  });
});
