import { render, screen } from "@testing-library/react";
import { describe, expect, it } from "vitest";
import { Input } from "./Input";

describe("Input", () => {
  it("renders with default wash surface and neutral focus ring", () => {
    render(<Input aria-label="Name" />);
    const el = screen.getByLabelText("Name");
    expect(el.className).toMatch(/bg-control-hover/);
    expect(el.className).toMatch(/border-separator/);
    expect(el.className).toMatch(/focus-visible:ring-separator/);
    expect(el.className).not.toMatch(/ring-brand/);
  });

  it("applies glass variant", () => {
    render(<Input aria-label="Search" variant="glass" />);
    expect(screen.getByLabelText("Search").className).toMatch(/glass-input/);
  });
});
