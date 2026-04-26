import { describe, expect, it } from "vitest";
import { formatRemaining } from "../formatRemaining";

describe("formatRemaining", () => {
  it("formats hours and minutes", () => {
    const endsAt = new Date(Date.now() + (1 * 3600 + 23 * 60) * 1000).toISOString();
    expect(formatRemaining(endsAt)).toBe("1h 23m");
  });

  it("formats minutes only", () => {
    const endsAt = new Date(Date.now() + 23 * 60 * 1000).toISOString();
    expect(formatRemaining(endsAt)).toBe("23m");
  });

  it("formats seconds only", () => {
    const endsAt = new Date(Date.now() + 45 * 1000).toISOString();
    expect(formatRemaining(endsAt)).toBe("45s");
  });

  it("returns 0s when expired", () => {
    const endsAt = new Date(Date.now() - 1000).toISOString();
    expect(formatRemaining(endsAt)).toBe("0s");
  });
});
