// @vitest-environment jsdom

import { fireEvent, render, screen } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";
import { ApprovalCard } from "./ApprovalCard";

const baseItem = {
  id: "approval-r1",
  kind: "approval" as const,
  requestId: "r1",
  tool: "bash",
  args: { command: "cargo test" },
  cwd: "/repo",
  sandboxSummary: "Seatbelt cwd-only",
  layer: "layer2_starlark" as const,
  layerReason: "no rule matched",
  status: "pending" as const,
};

describe("ApprovalCard", () => {
  it("renders pending state with buttons", () => {
    render(<ApprovalCard item={baseItem} onRespond={vi.fn()} />);
    expect(screen.getByText(/cargo test/)).toBeInTheDocument();
    expect(screen.getByRole("button", { name: /allow once/i })).toBeInTheDocument();
    expect(screen.getByRole("button", { name: /deny/i })).toBeInTheDocument();
  });

  it("calls onRespond with allow_once when clicked", () => {
    const onRespond = vi.fn();
    render(<ApprovalCard item={baseItem} onRespond={onRespond} />);
    fireEvent.click(screen.getByRole("button", { name: /allow once/i }));
    expect(onRespond).toHaveBeenCalledWith("r1", { kind: "allow_once" });
  });

  it("keyboard 'a' triggers allow once", () => {
    const onRespond = vi.fn();
    render(<ApprovalCard item={baseItem} onRespond={onRespond} />);
    fireEvent.keyDown(window, { key: "a" });
    expect(onRespond).toHaveBeenCalledWith("r1", { kind: "allow_once" });
  });

  it("collapses to one-line summary when decided", () => {
    const decided = {
      ...baseItem,
      status: "approved-once" as const,
      decidedBy: "user" as const,
      decidedAt: new Date().toISOString(),
    };
    render(<ApprovalCard item={decided} onRespond={vi.fn()} />);
    expect(screen.queryByRole("button", { name: /allow once/i })).not.toBeInTheDocument();
    expect(screen.getByText(/approved/i)).toBeInTheDocument();
  });
});
