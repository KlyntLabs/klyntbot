// @vitest-environment jsdom
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { act, renderHook, waitFor } from "@testing-library/react";
import type { ReactNode } from "react";
import { afterEach, describe, expect, it, vi } from "vitest";

vi.mock("@/utils/tauri-bridge", () => ({
  ipc: vi.fn(),
}));

import { ipc } from "@/utils/tauri-bridge";
import { qk } from "../queryKeys";
import { useTauriMutation } from "../useTauriMutation";

const mockedIpc = vi.mocked(ipc);

afterEach(() => {
  mockedIpc.mockReset();
});

function wrap(client: QueryClient) {
  return ({ children }: { children: ReactNode }) => (
    <QueryClientProvider client={client}>{children}</QueryClientProvider>
  );
}

describe("useTauriMutation", () => {
  it("calls ipc with the cmd + args", async () => {
    mockedIpc.mockResolvedValueOnce({ ok: true });
    const client = new QueryClient();
    const { result } = renderHook(
      () => useTauriMutation<unknown, { id: string }>({ command: "task_toggle_complete" }),
      { wrapper: wrap(client) },
    );

    await act(async () => {
      await result.current.mutate({ id: "t1" });
    });

    expect(mockedIpc).toHaveBeenCalledWith("task_toggle_complete", { id: "t1" });
  });

  it("auto-invalidates tasks.all after a task_* mutation", async () => {
    mockedIpc.mockResolvedValueOnce({ ok: true });
    const client = new QueryClient();
    const spy = vi.spyOn(client, "invalidateQueries");

    const { result } = renderHook(
      () => useTauriMutation<unknown, { id: string }>({ command: "task_toggle_complete" }),
      { wrapper: wrap(client) },
    );

    await act(async () => {
      await result.current.mutate({ id: "t1" });
    });

    expect(spy).toHaveBeenCalledWith({ queryKey: ["tasks"] });
  });

  it("applies optimistic patch + rolls back on error", async () => {
    const client = new QueryClient();
    client.setQueryData(qk.tasks.today(), [{ id: "t1", completed: false }]);
    mockedIpc.mockRejectedValueOnce(new Error("boom"));

    const { result } = renderHook(
      () =>
        useTauriMutation<unknown, { id: string }>({
          command: "task_toggle_complete",
          optimistic: {
            queryKey: qk.tasks.today(),
            update: (vars, prev: Array<{ id: string; completed: boolean }>) =>
              prev.map((t) => (t.id === vars.id ? { ...t, completed: true } : t)),
          },
        }),
      { wrapper: wrap(client) },
    );

    await act(async () => {
      await result.current.mutate({ id: "t1" }).catch(() => {});
    });

    // Roll-back: cache restored to pre-mutation state
    expect(client.getQueryData(qk.tasks.today())).toEqual([{ id: "t1", completed: false }]);
  });

  it("optimistic patch survives a successful mutation", async () => {
    const client = new QueryClient();
    client.setQueryData(qk.tasks.today(), [{ id: "t1", completed: false }]);
    mockedIpc.mockResolvedValueOnce({ ok: true });

    const { result } = renderHook(
      () =>
        useTauriMutation<unknown, { id: string }>({
          command: "task_toggle_complete",
          optimistic: {
            queryKey: qk.tasks.today(),
            update: (vars, prev: Array<{ id: string; completed: boolean }>) =>
              prev.map((t) => (t.id === vars.id ? { ...t, completed: true } : t)),
          },
        }),
      { wrapper: wrap(client) },
    );

    await act(async () => {
      await result.current.mutate({ id: "t1" });
    });

    await waitFor(() =>
      expect(client.getQueryData(qk.tasks.today())).toEqual([{ id: "t1", completed: true }]),
    );
  });
});

describe("useTauriMutation — mutationFn escape hatch", () => {
  it("uses mutationFn when provided", async () => {
    const mutationFn = vi.fn().mockResolvedValue({ ok: true });
    const client = new QueryClient();

    const { result } = renderHook(
      () =>
        useTauriMutation<{ ok: boolean }, { name: string }>({
          mutationFn,
          invalidates: [],
        }),
      { wrapper: wrap(client) },
    );

    await act(async () => {
      await result.current.mutate({ name: "abc" });
    });

    expect(mutationFn).toHaveBeenCalledWith({ name: "abc" });
    expect(mockedIpc).not.toHaveBeenCalled();
  });

  it("throws if neither command nor mutationFn is provided", () => {
    expect(() => {
      useTauriMutation({} as never);
    }).toThrow("useTauriMutation: either `command` or `mutationFn` must be provided");
  });
});
