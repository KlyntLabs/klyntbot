// @vitest-environment jsdom
import { cleanup, fireEvent, render, screen } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { __testing } from "../hooks/useAppMode";
import { SidebarChatLayout } from "./SidebarChatLayout";

afterEach(() => cleanup());

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

describe("SidebarChatLayout", () => {
  beforeEach(() => {
    window.localStorage.clear();
    __testing.reset("assistant");
  });

  it("renders the Calendar nav item", () => {
    render(<SidebarChatLayout {...baseProps} />);
    expect(screen.getByText("Calendar")).toBeTruthy();
  });

  it("applies the --active modifier class to the matching nav item", () => {
    render(<SidebarChatLayout {...baseProps} activeNavId="calendar" />);
    const calendarBtn = screen.getByText("Calendar").closest("button");
    expect(calendarBtn?.className).toContain("sidebar-chat__nav-item--active");
  });

  it("does not apply --active when activeNavId is null", () => {
    render(<SidebarChatLayout {...baseProps} />);
    const buttons = document.querySelectorAll(".sidebar-chat__nav-item");
    buttons.forEach((b) => {
      expect(b.className).not.toContain("sidebar-chat__nav-item--active");
    });
  });

  it("calls onSelectCalendar when Calendar nav item is clicked", () => {
    const onSelectCalendar = vi.fn();
    render(<SidebarChatLayout {...baseProps} onSelectCalendar={onSelectCalendar} />);
    (screen.getByText("Calendar").closest("button") as HTMLButtonElement).click();
    expect(onSelectCalendar).toHaveBeenCalled();
  });

  it("renders the App mode tablist in the topbar with both segments", () => {
    render(<SidebarChatLayout {...baseProps} />);
    const list = screen.getByRole("tablist", { name: "App mode" });
    expect(list).toBeTruthy();
    expect(screen.getByRole("tab", { name: "Assistant" })).toBeTruthy();
    expect(screen.getByRole("tab", { name: "Code" })).toBeTruthy();
  });

  it("clicking the Code tab flips the global mode to code", () => {
    render(<SidebarChatLayout {...baseProps} />);
    const code = screen.getByRole("tab", { name: "Code" });
    expect(code.getAttribute("aria-selected")).toBe("false");
    fireEvent.click(code);
    expect(code.getAttribute("aria-selected")).toBe("true");
    expect(window.localStorage.getItem("klynt.appMode")).toBe("code");
  });

  it("renders the same nav items in code mode", () => {
    __testing.reset("code");
    render(<SidebarChatLayout {...baseProps} />);
    expect(screen.getByText("New chat")).toBeTruthy();
    expect(screen.getByText("Search")).toBeTruthy();
    expect(screen.getByText("Calendar")).toBeTruthy();
    expect(screen.getByText("Plugins")).toBeTruthy();
    expect(screen.getByText("Automations")).toBeTruthy();
    expect(screen.getByText("Project")).toBeTruthy();
  });
});
