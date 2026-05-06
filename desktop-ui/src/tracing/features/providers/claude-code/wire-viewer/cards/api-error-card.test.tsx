import { render, screen } from "@testing-library/react";
import { describe, expect, it } from "vitest";
import { ApiErrorCard } from "./api-error-card";

describe("ApiErrorCard", () => {
  it("renders retry counter", () => {
    render(<ApiErrorCard event={{ index: 0, timestamp: 0, type: "system", payload: { retryAttempt: 1, maxRetries: 10, retryInMs: 590 } }} />);
    expect(screen.getByText(/retry 1\/10/)).toBeInTheDocument();
  });
});
