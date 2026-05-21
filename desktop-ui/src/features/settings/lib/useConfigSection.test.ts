import { act, renderHook, waitFor } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";

const invoke = vi.fn();
vi.mock("@/api/client", () => ({ invoke: (...a: unknown[]) => invoke(...a) }));

import { useConfigSection } from "./useConfigSection";

describe("useConfigSection", () => {
  beforeEach(() => invoke.mockReset());

  it("loads a section on mount", async () => {
    invoke.mockResolvedValueOnce({ silenceDurationMs: 1500 });
    const { result } = renderHook(() => useConfigSection<{ silenceDurationMs: number }>("voice"));
    await waitFor(() => expect(result.current.loading).toBe(false));
    expect(invoke).toHaveBeenCalledWith("config_get_section", {
      section: "voice",
    });
    expect(result.current.value).toEqual({ silenceDurationMs: 1500 });
  });

  it("patches and updates value", async () => {
    invoke.mockResolvedValueOnce({ silenceDurationMs: 1500 }); // initial load
    invoke.mockResolvedValueOnce({ silenceDurationMs: 1200 }); // update echo
    const { result } = renderHook(() => useConfigSection<{ silenceDurationMs: number }>("voice"));
    await waitFor(() => expect(result.current.loading).toBe(false));
    await act(async () => {
      await result.current.patch({ silenceDurationMs: 1200 });
    });
    expect(invoke).toHaveBeenCalledWith("config_update_section", {
      section: "voice",
      patch: { silenceDurationMs: 1200 },
    });
    expect(result.current.value).toEqual({ silenceDurationMs: 1200 });
  });

  it("sets patching flag during update", async () => {
    invoke.mockResolvedValueOnce({ enabled: false }); // initial load
    let resolveUpdate: (v: { enabled: boolean }) => void;
    const updatePromise = new Promise<{ enabled: boolean }>((r) => {
      resolveUpdate = r;
    });
    invoke.mockImplementationOnce(() => updatePromise);

    const { result } = renderHook(() => useConfigSection<{ enabled: boolean }>("ui"));
    await waitFor(() => expect(result.current.loading).toBe(false));

    act(() => {
      result.current.patch({ enabled: true });
    });

    expect(result.current.patching).toBe(true);

    await act(async () => {
      resolveUpdate!({ enabled: true });
      await updatePromise;
    });

    expect(result.current.patching).toBe(false);
  });

  it("skips patch as no-op when value is unchanged", async () => {
    invoke.mockResolvedValueOnce({ theme: "dark" }); // initial load
    const { result } = renderHook(() => useConfigSection<{ theme: string }>("ui"));
    await waitFor(() => expect(result.current.loading).toBe(false));

    await act(async () => {
      await result.current.patch({ theme: "dark" });
    });

    expect(invoke).not.toHaveBeenCalledWith("config_update_section", expect.anything());
    expect(result.current.value).toEqual({ theme: "dark" });
  });

  it("only applies the most recent patch result (generation tracking)", async () => {
    invoke.mockResolvedValueOnce({ count: 0 }); // initial load

    let resolveFirst: (v: { count: number }) => void;
    let resolveSecond: (v: { count: number }) => void;
    const firstPromise = new Promise<{ count: number }>((r) => {
      resolveFirst = r;
    });
    const secondPromise = new Promise<{ count: number }>((r) => {
      resolveSecond = r;
    });

    invoke.mockImplementationOnce(() => firstPromise);
    invoke.mockImplementationOnce(() => secondPromise);

    const { result } = renderHook(() => useConfigSection<{ count: number }>("test"));
    await waitFor(() => expect(result.current.loading).toBe(false));

    // Fire first patch
    act(() => {
      result.current.patch({ count: 1 });
    });
    // Fire second patch before first resolves
    act(() => {
      result.current.patch({ count: 2 });
    });

    // Resolve second (newer) first
    await act(async () => {
      resolveSecond!({ count: 2 });
      await secondPromise;
    });
    expect(result.current.value).toEqual({ count: 2 });

    // Resolve first (older) — should not overwrite
    await act(async () => {
      resolveFirst!({ count: 1 });
      await firstPromise;
    });
    expect(result.current.value).toEqual({ count: 2 });
  });

  it("surfaces patch errors", async () => {
    invoke.mockResolvedValueOnce({ ok: true }); // initial load
    invoke.mockRejectedValueOnce(new Error("save failed"));

    const { result } = renderHook(() => useConfigSection<{ ok: boolean }>("test"));
    await waitFor(() => expect(result.current.loading).toBe(false));

    await act(async () => {
      try {
        await result.current.patch({ ok: false });
      } catch {
        // expected
      }
    });

    expect(result.current.error).toBe("Error: save failed");
  });
});
