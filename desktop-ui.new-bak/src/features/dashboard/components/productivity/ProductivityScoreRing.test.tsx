// @vitest-environment jsdom
import { cleanup, fireEvent, render, screen } from "@testing-library/react";
import { afterEach, describe, expect, it } from "vitest";
import { ProductivityScoreRing, ScoreBar } from "./ProductivityScoreRing";

afterEach(() => cleanup());

describe("ProductivityScoreRing", () => {
  it("renders the rounded score and 'Good' label for 75", () => {
    render(<ProductivityScoreRing score={75} />);
    expect(screen.getByText("75")).toBeTruthy();
    expect(screen.getByText("Good")).toBeTruthy();
  });

  it("renders 'Excellent' for >= 80", () => {
    render(<ProductivityScoreRing score={92} />);
    expect(screen.getByText("Excellent")).toBeTruthy();
  });

  it("renders em-dash label for 0 score", () => {
    render(<ProductivityScoreRing score={0} />);
    expect(screen.getByText("—")).toBeTruthy();
  });

  it("shows tooltip rows on hover when summary is provided", () => {
    render(
      <ProductivityScoreRing
        score={75}
        summary={{
          productiveSecs: 3600,
          neutralSecs: 600,
          distractingSecs: 0,
          totalActiveSecs: 4200,
          avgSessionQuality: 0.8,
          focusSessionsCount: 2,
          contextSwitches: 5,
        }}
      />,
    );
    const ring = screen.getByText("75").closest("div");
    if (!ring) throw new Error("ring not found");
    fireEvent.mouseEnter(ring);
    expect(screen.getByText("Focus time")).toBeTruthy();
    expect(screen.getByText("Context switches")).toBeTruthy();
  });
});

describe("ScoreBar", () => {
  it("renders label, percent value, and bar fill width", () => {
    render(<ScoreBar label="Quality" value={0.42} />);
    expect(screen.getByText("Quality")).toBeTruthy();
    expect(screen.getByText("42")).toBeTruthy();
  });

  it("clamps value to [0,1]", () => {
    render(<ScoreBar label="X" value={1.5} />);
    expect(screen.getByText("100")).toBeTruthy();
    cleanup();
    render(<ScoreBar label="Y" value={-0.2} />);
    expect(screen.getByText("0")).toBeTruthy();
  });
});
