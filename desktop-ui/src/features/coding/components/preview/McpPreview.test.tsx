import { render, screen } from "@testing-library/react";
import { describe, expect, it } from "vitest";
import { McpPreview } from "./McpPreview";

describe("McpPreview", () => {
  it("renders server and tool name", () => {
    render(
      <McpPreview
        kind="mcp"
        server="linear"
        tool="create_issue"
        args={{ title: "Bug" }}
        schema={null}
      />,
    );
    expect(screen.getByText(/linear/)).toBeInTheDocument();
    expect(screen.getByText(/create_issue/)).toBeInTheDocument();
  });

  it("renders args as formatted json", () => {
    render(
      <McpPreview
        kind="mcp"
        server="github"
        tool="search_issues"
        args={{ q: "is:open" }}
        schema={null}
      />,
    );
    expect(screen.getByText(/is:open/)).toBeInTheDocument();
  });
});
