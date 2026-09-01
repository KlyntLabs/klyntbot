import { render, screen } from "@testing-library/react";
import { describe, expect, it } from "vitest";
import { ThinkingCard } from "./thinking-card";

const ev = {
  index: 0,
  timestamp: 0,
  type: "assistant.thinking",
  payload: { type: "thinking", thinking: "considering options" },
};

describe("ThinkingCard", () => {
  it("renders the thinking text when expanded", () => {
    render(<ThinkingCard event={ev} defaultExpanded />);
    expect(screen.getByText(/considering options/)).toBeInTheDocument();
  });
  it("collapses by default", () => {
    render(<ThinkingCard event={ev} />);
    expect(screen.queryByText(/considering options/)).not.toBeInTheDocument();
  });
});
