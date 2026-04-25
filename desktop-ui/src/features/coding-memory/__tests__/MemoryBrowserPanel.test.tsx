import { render, screen } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";

vi.mock("../hooks", () => ({
  useMemoryBrowser: () => ({
    data: [
      { id: "1", subject: "repo:bot", predicate: "framework", object: "tauri" },
      { id: "2", subject: "repo:bot", predicate: "language", object: "rust" },
    ],
    isLoading: false,
  }),
}));

import { MemoryBrowserPanel } from "../MemoryBrowserPanel";

describe("MemoryBrowserPanel", () => {
  it("renders memory rows", () => {
    render(<MemoryBrowserPanel />);
    expect(screen.getByText("Memory Browser")).toBeInTheDocument();
    expect(screen.getByText("repo:bot → framework")).toBeInTheDocument();
    expect(screen.getByText("tauri")).toBeInTheDocument();
    expect(screen.getByText("repo:bot → language")).toBeInTheDocument();
    expect(screen.getByText("rust")).toBeInTheDocument();
  });
});
