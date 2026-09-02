import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { describe, expect, it, vi } from "vitest";
import { Checkbox } from "./Checkbox";

describe("Checkbox", () => {
  it("renders unchecked without brand fill", () => {
    render(<Checkbox checked={false} onCheckedChange={() => {}} />);
    const el = screen.getByRole("checkbox");
    expect(el).not.toBeChecked();
    expect(el.className).toMatch(/border-separator/);
  });

  it("applies brand fill when checked", () => {
    render(<Checkbox checked onCheckedChange={() => {}} />);
    const el = screen.getByRole("checkbox");
    expect(el).toBeChecked();
    expect(el.className).toMatch(/data-\[state=checked\]:bg-brand/);
  });

  it("notifies onCheckedChange", async () => {
    const user = userEvent.setup();
    const onCheckedChange = vi.fn();
    render(<Checkbox checked={false} onCheckedChange={onCheckedChange} />);
    await user.click(screen.getByRole("checkbox"));
    expect(onCheckedChange).toHaveBeenCalledWith(true);
  });
});
