import { render, screen } from "@testing-library/react";
import { describe, expect, it } from "vitest";
import { ReviewResultPart } from "./ReviewResultPart";

describe("ReviewResultPart", () => {
  it("renders summary", () => {
    render(<ReviewResultPart reviewId="r1" summary="ok" issues={[]} />);
    expect(screen.getByText("ok")).toBeInTheDocument();
    expect(screen.getByText("No issues found.")).toBeInTheDocument();
  });

  it("groups issues by severity", () => {
    render(
      <ReviewResultPart
        reviewId="r1"
        summary="2 issues"
        issues={[
          { severity: "error", file: "a.rs", line: 1, description: "bug", suggestion: null },
          { severity: "info", file: null, line: null, description: "nit", suggestion: null },
        ]}
      />,
    );
    expect(screen.getByText(/Errors/)).toBeInTheDocument();
    expect(screen.getByText(/Info/)).toBeInTheDocument();
  });
});
