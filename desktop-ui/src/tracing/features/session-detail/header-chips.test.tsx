import { render, screen } from "@testing-library/react";
import { describe, expect, it } from "vitest";
import { HeaderChips } from "./header-chips";

describe("HeaderChips", () => {
  it("renders chips in order", () => {
    render(
      <HeaderChips
        chips={["turns", "errors", "model"]}
        stats={{
          turns: 3,
          steps: 0,
          tool_calls: 0,
          errors: 1,
          compactions: 0,
          duration_sec: 0,
          input_tokens: 0,
          output_tokens: 0,
          wire_size: 0,
          context_size: 0,
          state_size: 0,
          total_size: 0,
        }}
        model="claude-opus-4-7"
      />,
    );
    expect(screen.getByText("Turns")).toBeInTheDocument();
    expect(screen.getByText("Errors")).toBeInTheDocument();
    expect(screen.getByText("Model")).toBeInTheDocument();
    expect(screen.getByText("claude-opus-4-7")).toBeInTheDocument();
  });
});
