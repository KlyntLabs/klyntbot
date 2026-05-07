import { render, screen } from "@testing-library/react";
import { describe, expect, it } from "vitest";
import type { ConversationItem } from "@/types";
import { ToolRowBody } from "./ToolRowBody";

function tool(overrides: Partial<Extract<ConversationItem, { kind: "tool" }>>) {
  return {
    id: "t1",
    kind: "tool" as const,
    toolType: "commandExecution",
    title: "",
    detail: "",
    status: "completed",
    output: "",
    ...overrides,
  };
}

describe("ToolRowBody", () => {
  it("renders command output in code style for commandExecution", () => {
    const item = tool({
      toolType: "commandExecution",
      title: "Command: echo hi",
      output: "hi\n",
    });
    render(<ToolRowBody item={item} />);
    expect(screen.getByText(/hi/)).toBeInTheDocument();
  });

  it("renders 'No output' placeholder when output is empty", () => {
    const item = tool({ toolType: "commandExecution", title: "Command: true", output: "" });
    render(<ToolRowBody item={item} />);
    expect(screen.getByText(/No output/i)).toBeInTheDocument();
  });

  it("renders fileChange diff via PierreDiffBlock when changes present", () => {
    const item = tool({
      toolType: "fileChange",
      title: "File changes",
      changes: [{ path: "a.ts", kind: "edit", diff: "@@ -1 +1 @@\n-a\n+b" }],
    });
    render(<ToolRowBody item={item} />);
    expect(screen.getByText(/a\.ts/)).toBeInTheDocument();
  });
});
