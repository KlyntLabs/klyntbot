import { describe, it, expect, vi, beforeEach } from "vitest";
import { render, screen, fireEvent, waitFor } from "@testing-library/react";
import { AgentsMdPanel } from "./AgentsMdPanel";

vi.mock("@tauri-apps/api/core", () => ({
  invoke: vi.fn(),
}));

import { invoke } from "@tauri-apps/api/core";

const initial = [
  { path: "/repo/AGENTS.md", dir: "/repo", contents: "rule one" },
];

describe("AgentsMdPanel", () => {
  beforeEach(() => vi.mocked(invoke).mockReset());

  it("renders empty state when no sources", () => {
    render(<AgentsMdPanel threadId="t1" initialSources={[]} />);
    expect(screen.getByText(/No AGENTS\.md found/)).toBeInTheDocument();
  });

  it("renders count + sources when expanded", () => {
    render(<AgentsMdPanel threadId="t1" initialSources={initial} />);
    expect(screen.getByText("1")).toBeInTheDocument();
    fireEvent.click(screen.getByRole("button", { name: /Loaded context/ }));
    expect(screen.getByText(/AGENTS\.md/)).toBeInTheDocument();
  });

  it("refresh calls coding_thread_refresh_agents_md", async () => {
    vi.mocked(invoke).mockResolvedValueOnce([
      { path: "/repo/AGENTS.md", dir: "/repo", contents: "rule TWO" },
    ]);
    render(<AgentsMdPanel threadId="t1" initialSources={initial} />);
    fireEvent.click(screen.getByText("Refresh"));
    await waitFor(() => expect(invoke).toHaveBeenCalledWith(
      "coding_thread_refresh_agents_md", { threadId: "t1" }
    ));
  });
});
