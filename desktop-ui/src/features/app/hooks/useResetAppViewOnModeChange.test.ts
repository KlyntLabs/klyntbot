/** @vitest-environment jsdom */
import { act, renderHook } from "@testing-library/react";
import { useState } from "react";
import { afterEach, describe, expect, it } from "vitest";
import type { AppView } from "../constants/appViews";
import { __testing as appModeTesting, useAppMode } from "./useAppMode";
import { useResetAppViewOnModeChange } from "./useResetAppViewOnModeChange";

afterEach(() => appModeTesting.reset("assistant"));

describe("useResetAppViewOnModeChange", () => {
  it("resets to home when mode changes from assistant to code", () => {
    appModeTesting.reset("assistant");
    const { result } = renderHook(() => {
      const [view, setView] = useState<AppView>("calendar");
      useResetAppViewOnModeChange(setView);
      const { mode, setMode } = useAppMode();
      return { view, setView, mode, setMode };
    });
    expect(result.current.view).toBe("calendar");
    act(() => result.current.setMode("code"));
    expect(result.current.view).toBe("home");
  });
});
