// @vitest-environment jsdom

import { act, render, screen, waitFor } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

vi.mock("@/utils/tauri-bridge", () => ({
  ipc: vi.fn(),
  listen: vi.fn(async (event: string, handler: (payload: unknown) => void) => {
    const wrapped = (e: Event) => handler((e as CustomEvent).detail);
    window.addEventListener(event, wrapped as EventListener);
    return () => window.removeEventListener(event, wrapped as EventListener);
  }),
}));

import { QueryProvider } from "@/lib/query/QueryProvider";
import { ipc } from "@/utils/tauri-bridge";
import { FocusPage } from "./FocusPage";

const mockedIpc = vi.mocked(ipc);

describe("FocusPage", () => {
  beforeEach(() => {
    localStorage.clear();
    mockedIpc.mockImplementation(async (cmd: string) => {
      switch (cmd) {
        case "focus_session_status":
          return { active: false, sync: null, session: null };
        case "focus_defaults_get":
          return {
            workMins: 25,
            shortBreakMins: 5,
            longBreakMins: 15,
            longBreakAfter: 4,
            autoStartWork: false,
            autoStartBreak: false,
          };
        case "productivity_sessions":
          return [];
        default:
          throw new Error(`unexpected ipc command: ${cmd}`);
      }
    });
  });

  afterEach(() => {
    mockedIpc.mockReset();
  });

  it("renders the focus timer route", async () => {
    render(
      <QueryProvider>
        <FocusPage />
      </QueryProvider>,
    );

    await waitFor(() => {
      expect(screen.getByText("Start")).toBeInTheDocument();
    });
    expect(screen.getByText("25:00")).toBeInTheDocument();
  });

  it("opens settings when the settings button is clicked", async () => {
    render(
      <QueryProvider>
        <FocusPage />
      </QueryProvider>,
    );

    await waitFor(() => {
      expect(screen.getByText("Start")).toBeInTheDocument();
    });

    act(() => {
      screen.getByTitle("Settings").click();
    });

    await waitFor(() => {
      expect(screen.getByText("Settings")).toBeInTheDocument();
    });
  });
});
