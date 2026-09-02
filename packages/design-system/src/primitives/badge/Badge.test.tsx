import { render, screen } from "@testing-library/react";
import { describe, expect, it } from "vitest";
import { Badge } from "./Badge";

describe("Badge", () => {
  it("renders default wash variant", () => {
    render(<Badge>Idle</Badge>);
    const el = screen.getByText("Idle");
    expect(el.className).toMatch(/bg-control-hover/);
    expect(el.className).toMatch(/text-ui-sm/);
  });

  it("applies success and sm size", () => {
    render(
      <Badge variant="success" size="sm">
        Done
      </Badge>,
    );
    const el = screen.getByText("Done");
    expect(el.className).toMatch(/text-status-success/);
    expect(el.className).toMatch(/text-ui-xs/);
  });
});
