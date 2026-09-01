import { fireEvent, render, screen } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";
import { ToggleField } from "./ToggleField";

describe("ToggleField", () => {
  it("renders label and fires onChange", () => {
    const onChange = vi.fn();
    render(<ToggleField label="Enabled" value={false} onChange={onChange} />);
    expect(screen.getByText("Enabled")).toBeInTheDocument();
    fireEvent.click(screen.getByRole("switch"));
    expect(onChange).toHaveBeenCalledWith(true);
  });

  it("renders description when provided", () => {
    render(
      <ToggleField label="Enabled" description="Turn this on" value={false} onChange={vi.fn()} />,
    );
    expect(screen.getByText("Turn this on")).toBeInTheDocument();
  });

  it("toggles off when already on", () => {
    const onChange = vi.fn();
    render(<ToggleField label="Enabled" value={true} onChange={onChange} />);
    fireEvent.click(screen.getByRole("switch"));
    expect(onChange).toHaveBeenCalledWith(false);
  });
});
