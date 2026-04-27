// @vitest-environment jsdom

import { describe, expect, it } from "vitest";
import { formatRemaining } from "./formatRemaining";

describe("formatRemaining", () => {
  it("returns 0s for past times", () => {
    const past = new Date(Date.now() - 60000).toISOString();
    expect(formatRemaining(past)).toBe("0s");
  });

  it("formats seconds only", () => {
    const future = new Date(Date.now() + 45000).toISOString();
    expect(formatRemaining(future)).toBe("45s");
  });

  it("formats minutes only", () => {
    const future = new Date(Date.now() + 5 * 60000).toISOString();
    expect(formatRemaining(future)).toBe("5m");
  });

  it("formats hours and minutes", () => {
    const future = new Date(Date.now() + 2 * 3600_000 + 30 * 60000).toISOString();
    expect(formatRemaining(future)).toBe("2h 30m");
  });

  it("formats hours with zero minutes", () => {
    const future = new Date(Date.now() + 3 * 3600_000 + 10 * 1000).toISOString();
    expect(formatRemaining(future)).toBe("3h 0m");
  });
});
