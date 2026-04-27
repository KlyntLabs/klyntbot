// @vitest-environment jsdom

import { render, screen } from "@testing-library/react";
import { describe, expect, it } from "vitest";
import { EmptyState } from "./EmptyState";

describe("EmptyState", () => {
  it("renders the query and hints", () => {
    render(<EmptyState query="hello world" />);

    expect(screen.getByText('No results for "hello world"')).toBeInTheDocument();
    expect(screen.getByText("f/")).toBeInTheDocument();
    expect(screen.getByText("g/")).toBeInTheDocument();
    expect(screen.getByText("h/")).toBeInTheDocument();
    expect(screen.getByText("@")).toBeInTheDocument();
    expect(screen.getByText(">")).toBeInTheDocument();
    expect(screen.getByText("?")).toBeInTheDocument();
  });

  it("has status role", () => {
    render(<EmptyState query="x" />);
    expect(screen.getByRole("status")).toBeInTheDocument();
  });
});
