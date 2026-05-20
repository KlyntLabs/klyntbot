// @vitest-environment jsdom

import { render, screen } from "@testing-library/react";
import { describe, expect, it } from "vitest";
import { HooksSection } from "./HooksSection";

describe("HooksSection", () => {
  it("renders static placeholder", () => {
    render(<HooksSection />);
    expect(screen.getByText(/No.*hooks.toml.*found/)).toBeInTheDocument();
    expect(screen.getByText(/Hooks are user-managed/)).toBeInTheDocument();
  });
});
