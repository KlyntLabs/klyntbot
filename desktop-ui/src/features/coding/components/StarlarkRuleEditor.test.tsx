// @vitest-environment jsdom

import { fireEvent, render, screen } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";
import { StarlarkRuleEditor } from "./StarlarkRuleEditor";

describe("StarlarkRuleEditor", () => {
  it("renders textarea with default source", () => {
    render(<StarlarkRuleEditor requestId="r1" onCommit={vi.fn()} onCancel={vi.fn()} />);
    expect(screen.getByText("Add Starlark rule")).toBeInTheDocument();
    const textarea = screen.getByRole("textbox") as HTMLTextAreaElement;
    expect(textarea.value).toContain('prefix_rule(["git", "status"], decision="allow")');
  });

  it("calls onCancel when cancel button clicked", () => {
    const onCancel = vi.fn();
    render(<StarlarkRuleEditor requestId="r1" onCommit={vi.fn()} onCancel={onCancel} />);
    fireEvent.click(screen.getByText("Cancel"));
    expect(onCancel).toHaveBeenCalled();
  });

  it("allows editing the rule source", () => {
    render(<StarlarkRuleEditor requestId="r1" onCommit={vi.fn()} onCancel={vi.fn()} />);
    const textarea = screen.getByRole("textbox") as HTMLTextAreaElement;
    fireEvent.change(textarea, {
      target: { value: 'prefix_rule(["cargo", "build"], decision="allow")' },
    });
    expect(textarea.value).toBe('prefix_rule(["cargo", "build"], decision="allow")');
  });
});
