import { render, screen } from "@testing-library/react";
import { describe, expect, it } from "vitest";
import { ToolResultCard } from "./tool-result-card";

describe("ToolResultCard", () => {
  it("renders content string", () => {
    render(
      <ToolResultCard
        event={{
          index: 0,
          timestamp: 0,
          type: "user.tool_result",
          payload: { tool_use_id: "t1", is_error: false, content: "stdout here" },
        }}
      />,
    );
    expect(screen.getByText(/stdout here/)).toBeInTheDocument();
  });
});
