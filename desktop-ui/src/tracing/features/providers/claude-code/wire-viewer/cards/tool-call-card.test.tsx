import { render, screen } from "@testing-library/react";
import { describe, expect, it } from "vitest";
import { ToolCallCard } from "./tool-call-card";

describe("ToolCallCard", () => {
  it("renders tool name", () => {
    render(
      <ToolCallCard
        event={{
          index: 0,
          timestamp: 0,
          type: "assistant.tool_use",
          payload: { name: "Bash", input: { command: "ls" } },
        }}
      />,
    );
    expect(screen.getByText("Bash")).toBeInTheDocument();
  });
});
