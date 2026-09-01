// @vitest-environment jsdom

import { render } from "@testing-library/react";
import { describe, expect, it } from "vitest";
import { HooksSection } from "./HooksSection";

describe("HooksSection", () => {
  it("renders static placeholder", () => {
    const { container } = render(<HooksSection />);
    expect(container.textContent).toMatch(/No.*hooks\.toml.*found/);
    expect(container.textContent).toMatch(/Hooks are user-managed/);
  });
});
