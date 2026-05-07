import { render, screen } from "@testing-library/react";
import { describe, expect, it } from "vitest";
import { StatusUpdateCard } from "./status-update-card";

describe("StatusUpdateCard", () => {
  it("renders turn_duration summary", () => {
    render(<StatusUpdateCard event={{ index: 0, timestamp: 0, type: "system", payload: { subtype: "turn_duration", durationMs: 2000, messageCount: 5 } }} />);
    expect(screen.getByText(/2.00s/)).toBeInTheDocument();
  });
});
