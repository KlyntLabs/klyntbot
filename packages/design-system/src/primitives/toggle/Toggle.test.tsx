import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { describe, expect, it, vi } from "vitest";
import { Toggle } from "./Toggle";

describe("Toggle", () => {
  it("renders unchecked with wash track", () => {
    render(<Toggle checked={false} onChange={() => {}} />);
    const el = screen.getByRole("switch");
    expect(el).toHaveAttribute("aria-checked", "false");
    expect(el.className).toMatch(/bg-control-active/);
  });

  it("uses brand fill when checked", () => {
    render(<Toggle checked onChange={() => {}} />);
    const el = screen.getByRole("switch");
    expect(el).toHaveAttribute("aria-checked", "true");
    expect(el.className).toMatch(/bg-brand/);
  });

  it("calls onChange with inverted value", async () => {
    const user = userEvent.setup();
    const onChange = vi.fn();
    render(<Toggle checked={false} onChange={onChange} />);
    await user.click(screen.getByRole("switch"));
    expect(onChange).toHaveBeenCalledWith(true);
  });
});
