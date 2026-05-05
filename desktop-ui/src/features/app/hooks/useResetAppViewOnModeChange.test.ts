/** @vitest-environment jsdom */
import { renderHook, act } from "@testing-library/react";
import { afterEach, describe, expect, it, vi } from "vitest";
import { useState } from "react";
import { __testing as appModeTesting, useAppMode } from "./useAppMode";
import { useResetAppViewOnModeChange } from "./useResetAppViewOnModeChange";

afterEach(() => appModeTesting.reset("assistant"));

describe("useResetAppViewOnModeChange", () => {
  it("resets to home when mode changes from assistant to code", () => {
    appModeTesting.reset("assistant");
    const { result } = renderHook(() => {
      const [view, setView] = useState<string>("calendar");
      useResetAppViewOnModeChange(setView as (next: "home") => void);
      const { mode, setMode } = useAppMode();
      return { view, setView, mode, setMode };
    });
    expect(result.current.view).toBe("calendar");
    act(() => result.current.setMode("code"));
    expect(result.current.view).toBe("home");
  });
});
