import { render, screen } from "@testing-library/react";
import { describe, expect, it } from "vitest";
import { DiffPreview } from "./DiffPreview";

describe("DiffPreview", () => {
  it("renders path and line stats", () => {
    render(
      <DiffPreview
        kind="diff"
        path="src/main.rs"
        unified_diff="+fn main() {}"
        lines_added={1}
        lines_removed={0}
        is_new_file={false}
        is_truncated={false}
      />,
    );
    expect(screen.getByText("src/main.rs")).toBeInTheDocument();
    expect(screen.getByText("+1")).toBeInTheDocument();
    expect(screen.getByText("-0")).toBeInTheDocument();
  });

  it("shows new file badge when is_new_file is true", () => {
    render(
      <DiffPreview
        kind="diff"
        path="new.txt"
        unified_diff="+hello"
        lines_added={1}
        lines_removed={0}
        is_new_file={true}
        is_truncated={false}
      />,
    );
    expect(screen.getByText("new file")).toBeInTheDocument();
  });

  it("shows truncated footer when is_truncated is true", () => {
    render(
      <DiffPreview
        kind="diff"
        path="big.txt"
        unified_diff="+line"
        lines_added={1}
        lines_removed={0}
        is_new_file={false}
        is_truncated={true}
      />,
    );
    expect(screen.getByText(/Truncated/)).toBeInTheDocument();
  });
});
