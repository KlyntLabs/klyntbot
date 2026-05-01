// @vitest-environment jsdom
import { act, renderHook } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it } from "vitest";
import { __testing, APP_MODE_STORAGE_KEY, setAppMode, useAppMode } from "./useAppMode";

describe("useAppMode", () => {
  beforeEach(() => {
    window.localStorage.clear();
    __testing.reset("assistant");
  });

  afterEach(() => {
    window.localStorage.clear();
    __testing.reset("assistant");
  });

  it("defaults to assistant when no value is stored", () => {
    const { result } = renderHook(() => useAppMode());
    expect(result.current.mode).toBe("assistant");
  });

  it("hydrates from localStorage when rehydrate is called", () => {
    window.localStorage.setItem(APP_MODE_STORAGE_KEY, "code");
    __testing.rehydrateFromStorage();
    const { result } = renderHook(() => useAppMode());
    expect(result.current.mode).toBe("code");
  });

  it("ignores invalid stored values and falls back to default", () => {
    window.localStorage.setItem(APP_MODE_STORAGE_KEY, "garbage");
    __testing.rehydrateFromStorage();
    const { result } = renderHook(() => useAppMode());
    expect(result.current.mode).toBe("assistant");
  });

  it("setMode updates state and persists to localStorage", () => {
    const { result } = renderHook(() => useAppMode());
    act(() => result.current.setMode("code"));
    expect(result.current.mode).toBe("code");
    expect(window.localStorage.getItem(APP_MODE_STORAGE_KEY)).toBe("code");
  });

  it("setMode is a no-op when called with the current mode", () => {
    const { result } = renderHook(() => useAppMode());
    act(() => result.current.setMode("assistant"));
    expect(result.current.mode).toBe("assistant");
    expect(window.localStorage.getItem(APP_MODE_STORAGE_KEY)).toBeNull();
  });

  it("notifies multiple subscribers on change", () => {
    const a = renderHook(() => useAppMode());
    const b = renderHook(() => useAppMode());
    act(() => setAppMode("code"));
    expect(a.result.current.mode).toBe("code");
    expect(b.result.current.mode).toBe("code");
  });

  it("ignores invalid values passed to setAppMode", () => {
    const { result } = renderHook(() => useAppMode());
    // @ts-expect-error — testing runtime guard
    act(() => setAppMode("nope"));
    expect(result.current.mode).toBe("assistant");
  });
});
