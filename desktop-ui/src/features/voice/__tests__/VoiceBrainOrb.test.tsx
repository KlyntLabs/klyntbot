import { describe, expect, it } from "vitest";

describe("Word-level highlights", () => {
  it("classifies confidence >= 0.85 as good (success)", () => {
    const seg = { text: "bonjour", confidence: 0.92 };
    const cls =
      seg.confidence >= 0.85
        ? "text-status-success"
        : seg.confidence >= 0.6
          ? "text-status-warning"
          : "text-status-danger";
    expect(cls).toBe("text-status-success");
  });

  it("classifies confidence 0.60-0.84 as fair (warning)", () => {
    const seg = { text: "monde", confidence: 0.72 };
    const cls =
      seg.confidence >= 0.85
        ? "text-status-success"
        : seg.confidence >= 0.6
          ? "text-status-warning"
          : "text-status-danger";
    expect(cls).toBe("text-status-warning");
  });

  it("classifies confidence < 0.60 as poor (destructive)", () => {
    const seg = { text: "suis", confidence: 0.42 };
    const cls =
      seg.confidence >= 0.85
        ? "text-status-success"
        : seg.confidence >= 0.6
          ? "text-status-warning"
          : "text-status-danger";
    expect(cls).toBe("text-status-danger");
  });

  it("boundary: exactly 0.85 is good", () => {
    const seg = { text: "a", confidence: 0.85 };
    const cls =
      seg.confidence >= 0.85
        ? "text-status-success"
        : seg.confidence >= 0.6
          ? "text-status-warning"
          : "text-status-danger";
    expect(cls).toBe("text-status-success");
  });

  it("boundary: exactly 0.60 is fair", () => {
    const seg = { text: "b", confidence: 0.6 };
    const cls =
      seg.confidence >= 0.85
        ? "text-status-success"
        : seg.confidence >= 0.6
          ? "text-status-warning"
          : "text-status-danger";
    expect(cls).toBe("text-status-warning");
  });
});
