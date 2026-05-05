/** @vitest-environment jsdom */
import { cleanup, render, screen } from "@testing-library/react";
import { afterEach, describe, expect, it, vi } from "vitest";
import { __testing as appModeTesting } from "../hooks/useAppMode";
import { SidebarChatLayout } from "./SidebarChatLayout";

vi.mock("@tauri-apps/api/core");

afterEach(() => {
  cleanup();
  vi.restoreAllMocks();
  appModeTesting.reset("assistant");
});

const baseProps = {
  onOpenSettings: vi.fn(),
  onNewChat: vi.fn(),
  onSelectPlugins: vi.fn(),
  onSelectCalendar: vi.fn(),
  threads: [],
  selectedSessionKey: null,
  onSelectThread: vi.fn(),
  activeNavId: null,
};

describe("SidebarChatLayout — mode-aware nav", () => {
  it("shows Calendar + Automations + Project in assistant mode", () => {
    appModeTesting.reset("assistant");
    render(<SidebarChatLayout {...baseProps} />);
    expect(screen.getByText("Calendar")).toBeTruthy();
    expect(screen.getByText("Automations")).toBeTruthy();
    expect(screen.getByText("Project")).toBeTruthy();
  });

  it("hides Calendar + Automations in code mode; shows Project", () => {
    appModeTesting.reset("code");
    render(<SidebarChatLayout {...baseProps} />);
    expect(screen.queryByText("Calendar")).toBeNull();
    expect(screen.queryByText("Automations")).toBeNull();
    expect(screen.getByText("Project")).toBeTruthy();
  });

  it("shows Search in both modes", () => {
    for (const m of ["assistant", "code"] as const) {
      appModeTesting.reset(m);
      const { unmount } = render(<SidebarChatLayout {...baseProps} />);
      expect(screen.getByText("Search")).toBeTruthy();
      unmount();
    }
  });
});
