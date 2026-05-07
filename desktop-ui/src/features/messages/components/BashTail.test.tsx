import { render, screen } from "@testing-library/react";
import { describe, expect, it } from "vitest";
import { BashTail } from "./BashTail";

describe("BashTail", () => {
  it("renders the last 3 non-empty lines", () => {
    const output = ["line 1", "line 2", "line 3", "line 4", "line 5"].join("\n");
    render(<BashTail output={output} />);
    expect(screen.queryByText("line 1")).toBeNull();
    expect(screen.queryByText("line 2")).toBeNull();
    expect(screen.getByText("line 3")).toBeInTheDocument();
    expect(screen.getByText("line 4")).toBeInTheDocument();
    expect(screen.getByText("line 5")).toBeInTheDocument();
  });

  it("renders nothing when output is empty", () => {
    const { container } = render(<BashTail output="" />);
    expect(container.firstChild).toBeNull();
  });

  it("preserves whitespace on each line", () => {
    render(<BashTail output={"  indented\nstart\n"} />);
    expect(screen.getByText(/indented/)).toBeInTheDocument();
  });
});
