// @vitest-environment jsdom

import { render, screen } from "@testing-library/react";
import { describe, expect, test } from "vitest";
import { DeadEndWarning } from "./DeadEndWarning";

describe("DeadEndWarning", () => {
  test("renders approach summary and confidence", () => {
    render(
      <DeadEndWarning approachSummary="regex rewrite" priorAttemptId="a1" confidence={0.87} />,
    );

    expect(screen.getByText(/Prior attempt: regex rewrite/)).toBeTruthy();
    expect(screen.getByText(/87% confidence dead-end/)).toBeTruthy();
  });
});
