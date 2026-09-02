import { render } from "@testing-library/react";
import { describe, expect, it } from "vitest";
import { Spinner } from "./Spinner";

describe("Spinner", () => {
  it("renders spinning svg at default md size", () => {
    const { container } = render(<Spinner />);
    const el = container.querySelector("svg");
    expect(el).not.toBeNull();
    expect(el?.getAttribute("class")).toMatch(/animate-spin/);
    expect(el?.getAttribute("class")).toMatch(/size-6/);
  });

  it("applies sm size", () => {
    const { container } = render(<Spinner size="sm" />);
    expect(container.querySelector("svg")?.getAttribute("class")).toMatch(/size-4/);
  });
});
