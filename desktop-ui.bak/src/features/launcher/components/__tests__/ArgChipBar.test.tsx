import { fireEvent, render, screen } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";
import { ArgChipBar } from "../ArgChipBar";

const specs = [
  { name: "duration", placeholder: "30m", kind: { type: "text" as const }, required: true },
  { name: "reason", placeholder: "why", kind: { type: "text" as const }, required: false },
];

describe("ArgChipBar", () => {
  it("tab moves focus between chips", () => {
    render(<ArgChipBar specs={specs} onSubmit={() => {}} onCancel={() => {}} />);
    const inputs = screen.getAllByRole("textbox");
    inputs[0].focus();
    fireEvent.keyDown(inputs[0], { key: "Tab" });
    expect(document.activeElement).toBe(inputs[1]);
  });

  it("enter on last chip submits", () => {
    const onSubmit = vi.fn();
    render(<ArgChipBar specs={specs} onSubmit={onSubmit} onCancel={() => {}} />);
    const inputs = screen.getAllByRole("textbox");
    fireEvent.change(inputs[0], { target: { value: "30m" } });
    fireEvent.keyDown(inputs[1], { key: "Enter" });
    expect(onSubmit).toHaveBeenCalledWith({ duration: "30m", reason: "" });
  });

  it("backspace from empty first chip cancels", () => {
    const onCancel = vi.fn();
    render(<ArgChipBar specs={specs} onSubmit={() => {}} onCancel={onCancel} />);
    const inputs = screen.getAllByRole("textbox");
    fireEvent.keyDown(inputs[0], { key: "Backspace" });
    expect(onCancel).toHaveBeenCalled();
  });
});
