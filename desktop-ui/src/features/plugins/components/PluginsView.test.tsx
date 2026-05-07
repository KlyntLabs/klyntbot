// @vitest-environment jsdom
import { render, screen } from "@testing-library/react";
import { describe, expect, it } from "vitest";
import { PluginsView } from "./PluginsView";

describe("PluginsView", () => {
  it("renders Coding Memory plugin by default", () => {
    render(<PluginsView />);
    expect(screen.getByTestId("plugins-active-pane")).toHaveAttribute(
      "data-plugin",
      "coding-memory",
    );
  });
});
