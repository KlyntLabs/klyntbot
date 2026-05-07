import { render, screen, fireEvent } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";
import type { ConversationItem } from "@/types";
import type { BurstGroup } from "../utils/groupBursts";
import { BurstRow } from "./BurstRow";

function read(id: string, path: string): Extract<ConversationItem, { kind: "tool" }> {
  return {
    id,
    kind: "tool",
    toolType: "fileChange",
    title: "File changes",
    detail: path,
    status: "completed",
    output: "",
    changes: [{ path, kind: "read", diff: "" }],
  };
}

function makeBurst(): BurstGroup {
  return {
    id: "burst-a",
    kind: "burst",
    family: "filesystem",
    name: "Read",
    items: [read("a", "x.ts"), read("b", "y.ts"), read("c", "z.ts"), read("d", "p.ts"), read("e", "q.ts")],
  };
}

describe("BurstRow", () => {
  it("renders header showing first 3 paths plus +N more", () => {
    render(
      <BurstRow group={makeBurst()} expandedItems={new Set()} onToggle={vi.fn()} />,
    );
    expect(screen.getByText(/Read:/)).toBeInTheDocument();
    expect(screen.getByText(/x\.ts/)).toBeInTheDocument();
    expect(screen.getByText(/\+2 more/)).toBeInTheDocument();
  });

  it("expands sub-rows when group id is in expandedItems", () => {
    render(
      <BurstRow group={makeBurst()} expandedItems={new Set(["burst-a"])} onToggle={vi.fn()} />,
    );
    expect(screen.getByText(/p\.ts/)).toBeInTheDocument();
    expect(screen.getByText(/q\.ts/)).toBeInTheDocument();
  });

  it("calls onToggle with group id on click", () => {
    const onToggle = vi.fn();
    render(<BurstRow group={makeBurst()} expandedItems={new Set()} onToggle={onToggle} />);
    fireEvent.click(screen.getByRole("button", { name: /toggle/i }));
    expect(onToggle).toHaveBeenCalledWith("burst-a");
  });
});
