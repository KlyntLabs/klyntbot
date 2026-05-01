// @vitest-environment jsdom

import { fireEvent, render, screen } from "@testing-library/react";
import { describe, expect, test } from "vitest";
import { RecallTrayCard } from "./RecallTrayCard";

describe("RecallTrayCard", () => {
  test("renders summary and expands on click", () => {
    render(
      <RecallTrayCard
        memoryIds={["m1"]}
        coverageScore={0.62}
        snippets={[{ kind: "edit", summary: "fix parser", source: "src/main.rs" }]}
      />,
    );

    expect(screen.getByText("1 snippet injected")).toBeTruthy();
    expect(screen.getByText("62% coverage")).toBeTruthy();

    const header = screen.getByRole("button");
    fireEvent.click(header);

    expect(screen.getByText("fix parser")).toBeTruthy();
    expect(screen.getByText("src/main.rs")).toBeTruthy();
  });
});
