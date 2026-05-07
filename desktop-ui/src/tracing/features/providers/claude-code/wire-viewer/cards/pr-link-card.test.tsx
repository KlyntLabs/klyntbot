import { render, screen } from "@testing-library/react";
import { describe, expect, it } from "vitest";
import { PrLinkCard } from "./pr-link-card";

describe("PrLinkCard", () => {
  it("renders PR number and link", () => {
    render(<PrLinkCard event={{ index: 0, timestamp: 0, type: "pr-link", payload: { prNumber: 42, prUrl: "https://x", prRepository: "o/r" } }} />);
    expect(screen.getByText(/#42/)).toBeInTheDocument();
    expect(screen.getByText("o/r")).toBeInTheDocument();
  });
});
