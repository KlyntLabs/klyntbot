// @vitest-environment jsdom

import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { act, renderHook, waitFor } from "@testing-library/react";
import type { ReactNode } from "react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

vi.mock("@/utils/tauri-bridge", () => ({
  ipc: vi.fn(),
  listen: vi.fn(async (event: string, handler: (payload: unknown) => void) => {
    const wrapped = (e: Event) => handler((e as CustomEvent).detail);
    window.addEventListener(event, wrapped as EventListener);
    return () => window.removeEventListener(event, wrapped as EventListener);
  }),
}));

import { ipc } from "@/utils/tauri-bridge";
import { useFocusTimer } from "./useFocusTimer";

const mockedIpc = vi.mocked(ipc);

function wrapper(client: QueryClient) {
  return ({ children }: { children: ReactNode }) => (
    <QueryClientProvider client={client}>{children}</QueryClientProvider>
  );
}

function createClient() {
  return new QueryClient({
    defaultOptions: { queries: { retry: 0 } },
  });
}

function emitFocusSync(payload: {
  phase: "working" | "break_pending" | "break" | "paused" | "suspended";
  remainingSecs: number;
  totalSecs: number;
  cyclePosition: number;
  longBreakAfter: number;
  paused: boolean;
  actionTitle: string | null;
  dndActive: boolean;
}) {
  act(() => {
    window.dispatchEvent(new CustomEvent("focus:sync", { detail: payload }));
  });
}

describe("useFocusTimer", () => {
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

  it("derives remainingSecs from focus:sync events", async () => {
    const client = createClient();
    const { result } = renderHook(() => useFocusTimer(), {
      wrapper: wrapper(client),
    });

    await waitFor(() => expect(result.current.phase).not.toBe("working"));
    expect(result.current.remainingSecs).toBeNull();

    emitFocusSync({
      phase: "working",
      remainingSecs: 120,
      totalSecs: 1500,
      cyclePosition: 1,
      longBreakAfter: 4,
      paused: false,
      actionTitle: "Test task",
      dndActive: false,
    });

    await waitFor(() => expect(result.current.remainingSecs).toBe(120));
    expect(result.current.phase).toBe("working");
    expect(result.current.actionTitle).toBe("Test task");
  });

  it("updates state when focus:phase_changed fires", async () => {
    const client = createClient();
    const { result } = renderHook(() => useFocusTimer(), {
      wrapper: wrapper(client),
    });

    await waitFor(() => expect(result.current.phase).toBe("idle"));

    emitFocusSync({
      phase: "break_pending",
      remainingSecs: 0,
      totalSecs: 300,
      cyclePosition: 1,
      longBreakAfter: 4,
      paused: false,
      actionTitle: null,
      dndActive: false,
    });

    await waitFor(() => expect(result.current.phase).toBe("break_pending"));
  });

  it("does not locally decrement remainingSecs between syncs", async () => {
    vi.useFakeTimers({ shouldAdvanceTime: true });
    const client = createClient();
    const { result } = renderHook(() => useFocusTimer(), {
      wrapper: wrapper(client),
    });

    await waitFor(() => expect(result.current.phase).toBe("idle"));

    emitFocusSync({
      phase: "working",
      remainingSecs: 120,
      totalSecs: 1500,
      cyclePosition: 1,
      longBreakAfter: 4,
      paused: false,
      actionTitle: null,
      dndActive: false,
    });

    await waitFor(() => expect(result.current.remainingSecs).toBe(120));

    vi.advanceTimersByTime(2000);
    // The hook should still show the authoritative sync value, not 118.
    expect(result.current.remainingSecs).toBe(120);

    vi.useRealTimers();
  });
});
