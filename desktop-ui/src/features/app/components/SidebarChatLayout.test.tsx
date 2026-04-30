// @vitest-environment jsdom
import { cleanup, render, screen } from "@testing-library/react";
import { afterEach, describe, expect, it, vi } from "vitest";
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
});
