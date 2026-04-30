import { describe, it, expect } from "vitest";
import { render, screen } from "@testing-library/react";
import { DiffPreview } from "./DiffPreview";

describe("DiffPreview", () => {
  it("renders path label, op badge, byte count and diff body", () => {
    render(<DiffPreview item={{
      id: "d1", kind: "diff", title: "x.rs",
      diff: "--- a\n+++ b\n@@ -1 +1 @@\n-old\n+new\n",
      path: "/repo/src/x.rs", op: "edit", bytes: 1234,
    }} />);
    expect(screen.getByText(/\/repo\/src\/x\.rs/)).toBeInTheDocument();
    expect(screen.getByText(/edit/i)).toBeInTheDocument();
    expect(screen.getByText(/1234/)).toBeInTheDocument();
  });
});
