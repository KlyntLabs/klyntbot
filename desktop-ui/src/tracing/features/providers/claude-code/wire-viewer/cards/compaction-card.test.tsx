import { render, screen } from "@testing-library/react";
import { describe, expect, it } from "vitest";
import { CompactionCard } from "./compaction-card";

describe("CompactionCard", () => {
  it("renders pre→post tokens", () => {
    render(<CompactionCard event={{ index: 0, timestamp: 0, type: "system", payload: { compactMetadata: { trigger: "manual", preTokens: 1000, postTokens: 100, durationMs: 2000 } } }} />);
    expect(screen.getByText(/1,000/)).toBeInTheDocument();
    expect(screen.getByText("manual")).toBeInTheDocument();
  });
});
