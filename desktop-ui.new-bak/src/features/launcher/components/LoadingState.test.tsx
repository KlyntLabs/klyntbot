// @vitest-environment jsdom

import { render, screen } from "@testing-library/react";
import { describe, expect, it } from "vitest";
import { LoadingState } from "./LoadingState";

describe("LoadingState", () => {
  it("renders 6 skeleton rows", () => {
    const { container } = render(<LoadingState />);
    const rows = container.querySelectorAll(".lc-loading-row");
    expect(rows).toHaveLength(6);
  });

  it("has status and aria-busy", () => {
    render(<LoadingState />);
    const el = screen.getByRole("status");
    expect(el).toHaveAttribute("aria-busy", "true");
  });
});
