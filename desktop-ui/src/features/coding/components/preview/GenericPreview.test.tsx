import { render, screen } from "@testing-library/react";
import { describe, expect, it } from "vitest";
import { GenericPreview } from "./GenericPreview";

describe("GenericPreview", () => {
  it("renders args as formatted json", () => {
    render(<GenericPreview kind="generic" args={{ foo: "bar", num: 42 }} />);
    expect(screen.getByText(/foo/)).toBeInTheDocument();
    expect(screen.getByText(/bar/)).toBeInTheDocument();
  });
});
