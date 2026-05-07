import { render, screen } from "@testing-library/react";
import { describe, expect, it } from "vitest";
import { CommandPreview } from "./CommandPreview";

describe("CommandPreview", () => {
  it("renders command and cwd", () => {
    render(
      <CommandPreview
        kind="command"
        command="cargo build"
        cwd="/project"
        is_dangerous={false}
        risk_hits={[]}
      />,
    );
    expect(screen.getByText("cargo build")).toBeInTheDocument();
    expect(screen.getByText("/project")).toBeInTheDocument();
  });

  it("shows dangerous badge when is_dangerous is true", () => {
    render(
      <CommandPreview
        kind="command"
        command="rm -rf /"
        cwd="/"
        is_dangerous={true}
        risk_hits={["destructive recursive delete"]}
      />,
    );
    expect(screen.getByText("dangerous")).toBeInTheDocument();
    expect(screen.getByText("destructive recursive delete")).toBeInTheDocument();
  });
});
