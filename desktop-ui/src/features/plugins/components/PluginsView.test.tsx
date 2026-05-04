// @vitest-environment jsdom
// @vitest-environment jsdom
import { render, screen, within } from "@testing-library/react";
import { describe, expect, it } from "vitest";
import { PluginsView } from "./PluginsView";

describe("PluginsView", () => {
  it("renders the plugins host with a Coding Memory tab", () => {
    render(<PluginsView />);
    expect(screen.getByRole("heading", { name: /plugins/i })).toBeInTheDocument();
    expect(screen.getByRole("tab", { name: /coding memory/i })).toBeInTheDocument();
  });

  it("renders Coding Memory plugin by default", () => {
    render(<PluginsView />);
    expect(screen.getByTestId("plugins-active-pane")).toHaveAttribute(
      "data-plugin",
      "coding-memory",
    );
  });

  it("does not render Klynt CLI as a primary plugin tab", () => {
    render(<PluginsView />);
    const primaryTabs = screen.getByRole("tablist", { name: /plugins/i });
    expect(within(primaryTabs).queryByRole("tab", { name: /klynt cli/i })).not.toBeInTheDocument();
  });

  it("renders Skills and MCP Servers as Soon-badged tabs", () => {
    render(<PluginsView />);
    expect(screen.getByRole("tab", { name: /skills/i })).toBeInTheDocument();
    expect(screen.getByRole("tab", { name: /mcp servers/i })).toBeInTheDocument();
  });
});
