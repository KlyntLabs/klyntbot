import { fireEvent, render, screen } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";
import { StuckThreadBanner } from "./StuckThreadBanner";

describe("StuckThreadBanner", () => {
  it("renders duration in seconds", () => {
    render(<StuckThreadBanner durationMs={7_500} onReset={() => {}} />);
    expect(screen.getByText(/processing for 8s/i)).toBeTruthy();
  });

  it("fires onReset on button click", () => {
    const onReset = vi.fn();
    render(<StuckThreadBanner durationMs={5_000} onReset={onReset} />);
    fireEvent.click(screen.getByRole("button"));
    expect(onReset).toHaveBeenCalledTimes(1);
  });
});
