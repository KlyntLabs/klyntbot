// @vitest-environment jsdom

import { describe, expect, it } from "vitest";
import { formatRemaining } from "./formatRemaining";

describe("formatRemaining", () => {
  const now = new Date("2026-06-16T12:00:00.000Z").getTime();

  it("returns 0s for past times", () => {
    const past = new Date(now - 60000).toISOString();
    expect(formatRemaining(past, now)).toBe("0s");
  });

  it("formats seconds only", () => {
    const future = new Date(now + 45000).toISOString();
    expect(formatRemaining(future, now)).toBe("45s");
  });

  it("formats minutes only", () => {
    const future = new Date(now + 5 * 60000).toISOString();
    expect(formatRemaining(future, now)).toBe("5m");
  });

  it("formats hours and minutes", () => {
    const future = new Date(now + 2 * 3600_000 + 30 * 60000).toISOString();
    expect(formatRemaining(future, now)).toBe("2h 30m");
  });

  it("formats hours with zero minutes", () => {
    const future = new Date(now + 3 * 3600_000 + 10 * 1000).toISOString();
    expect(formatRemaining(future, now)).toBe("3h 0m");
  });
});
