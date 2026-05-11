import { fireEvent, render, screen } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";
import { SmartAllowAlwaysButton } from "./SmartAllowAlwaysButton";

describe("SmartAllowAlwaysButton", () => {
  it("renders plain 'Allow always' when no suggestedGrant", () => {
    const onRespond = vi.fn();
    render(
      <SmartAllowAlwaysButton
        requestId="r1"
        suggestedGrant={null}
        onRespond={onRespond}
        onOpenStarlarkEditor={vi.fn()}
      />,
    );
    const btn = screen.getByRole("button", { name: /Allow always/ });
    expect(btn).toBeInTheDocument();
    fireEvent.click(btn);
    expect(onRespond).toHaveBeenCalledWith("r1", { kind: "allow_always" });
  });

  it("renders suggested pattern and calls commit on click", () => {
    const onRespond = vi.fn();
    render(
      <SmartAllowAlwaysButton
        requestId="r1"
        suggestedGrant={{
          pattern: "Edit on src/components/**",
          scope: { kind: "tool_glob", tool: "edit", glob: "src/components/**" },
          reason: "3 prior approvals",
        }}
        onRespond={onRespond}
        onOpenStarlarkEditor={vi.fn()}
      />,
    );
    expect(screen.getByText(/Edit on src\/components/)).toBeInTheDocument();
    fireEvent.click(screen.getByRole("button", { name: /Allow always:/ }));
    expect(onRespond).toHaveBeenCalledWith("r1", {
      kind: "allow_always",
      rule: "Edit on src/components/**",
    });
  });

  it("opens pattern picker when caret is clicked", () => {
    render(
      <SmartAllowAlwaysButton
        requestId="r1"
        suggestedGrant={{
          pattern: "Edit on src/**",
          scope: { kind: "tool_glob", tool: "edit", glob: "src/**" },
          reason: "Mirror suggestion",
        }}
        onRespond={vi.fn()}
        onOpenStarlarkEditor={vi.fn()}
      />,
    );
    fireEvent.click(screen.getByLabelText("Refine pattern"));
    expect(screen.getByText("Custom Starlark rule...")).toBeInTheDocument();
  });
});
