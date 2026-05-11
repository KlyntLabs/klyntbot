import { fireEvent, render, screen } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";
import { PatternPicker } from "./PatternPicker";

describe("PatternPicker", () => {
  it("renders suggested pattern and alternatives for exact path", () => {
    const onCommit = vi.fn();
    render(
      <PatternPicker
        suggested={{
          pattern: "edit on src/main.rs",
          scope: { kind: "exact_tool_path", tool: "edit", path: "src/main.rs" },
          reason: "Exact path match",
        }}
        onCommit={onCommit}
        onCustom={vi.fn()}
      />,
    );
    expect(screen.getByText("edit on src/main.rs")).toBeInTheDocument();
    expect(screen.getByText("Exact path match")).toBeInTheDocument();
    expect(screen.getByText("broaden to folder")).toBeInTheDocument();
  });

  it("renders alternatives for folder scope", () => {
    render(
      <PatternPicker
        suggested={{
          pattern: "edit on src/**",
          scope: { kind: "tool_folder", tool: "edit", folder: "src" },
          reason: "Folder match",
        }}
        onCommit={vi.fn()}
        onCustom={vi.fn()}
      />,
    );
    expect(screen.getByText("deeper recursion")).toBeInTheDocument();
  });

  it("calls onCommit with selected pattern", () => {
    const onCommit = vi.fn();
    render(
      <PatternPicker
        suggested={{
          pattern: "edit on src/**",
          scope: { kind: "tool_glob", tool: "edit", glob: "src/**" },
          reason: "Glob match",
        }}
        onCommit={onCommit}
        onCustom={vi.fn()}
      />,
    );
    fireEvent.click(screen.getByRole("button", { name: /edit on src/ }));
    expect(onCommit).toHaveBeenCalledWith("edit on src/**");
  });

  it("calls onCustom for custom rule", () => {
    const onCustom = vi.fn();
    render(
      <PatternPicker
        suggested={{
          pattern: "edit on src/**",
          scope: { kind: "tool_glob", tool: "edit", glob: "src/**" },
          reason: "Glob match",
        }}
        onCommit={vi.fn()}
        onCustom={onCustom}
      />,
    );
    fireEvent.click(screen.getByRole("button", { name: /Custom Starlark/ }));
    expect(onCustom).toHaveBeenCalled();
  });
});
