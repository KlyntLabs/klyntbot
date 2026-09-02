import { render, screen } from "@testing-library/react";
import { describe, expect, it } from "vitest";
import { Tooltip } from "./Tooltip";

describe("Tooltip", () => {
  it("renders trigger children", () => {
    render(
      <Tooltip content="More info">
        <button type="button">Hint</button>
      </Tooltip>,
    );
    expect(screen.getByRole("button", { name: "Hint" })).toBeInTheDocument();
  });
});
