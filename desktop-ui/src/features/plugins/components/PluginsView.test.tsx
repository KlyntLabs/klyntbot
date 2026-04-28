import { render, screen } from "@testing-library/react";
import { describe, it, expect } from "vitest";
import { PluginsView } from "./PluginsView";

describe("PluginsView", () => {
  it("renders the plugins host with a Coding Memory tab", () => {
    render(<PluginsView />);
    expect(screen.getByRole("heading", { name: /plugins/i })).toBeInTheDocument();
    expect(screen.getByRole("tab", { name: /coding memory/i })).toBeInTheDocument();
  });

  it("renders Coding Memory plugin by default", () => {
    render(<PluginsView />);
    expect(screen.getByTestId("plugins-active-pane")).toHaveAttribute("data-plugin", "coding-memory");
  });
});
