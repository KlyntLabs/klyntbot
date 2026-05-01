// @vitest-environment jsdom
import { describe, it, expect, vi } from "vitest";
import { render, screen, waitFor, act } from "@testing-library/react";
import { HooksSection } from "./HooksSection";

vi.mock("@/api/client", () => ({
  invoke: vi.fn(async (cmd: string) => {
    if (cmd === "coding_hooks_list") {
      return {
        path: "/home/user/.klyntbot/hooks.toml",
        exists: true,
        content: '[[hook]]\nevent = "PreToolUse"\nmatcher = "Bash(*)"\ncommand = "scripts/log.sh"',
      };
    }
    return {};
  }),
}));

describe("HooksSection", () => {
  it("renders loading state initially", () => {
    render(<HooksSection />);
    expect(screen.getByText("Loading...")).toBeInTheDocument();
  });

  it("renders hooks.toml content after load", async () => {
    await act(async () => {
      render(<HooksSection />);
    });
    await waitFor(() => {
      expect(screen.getByText(/hooks.toml/)).toBeInTheDocument();
    });
    expect(screen.getByText(/PreToolUse/)).toBeInTheDocument();
  });
});
