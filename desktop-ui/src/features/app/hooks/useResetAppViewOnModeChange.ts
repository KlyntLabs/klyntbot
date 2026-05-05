import { useEffect, useRef } from "react";
import { useAppMode } from "./useAppMode";
import type { AppView } from "../constants/appViews";

/**
 * Resets the centre-pane `appView` to "home" whenever the AppMode flips.
 * This prevents stranding the user on an assistant-only view (calendar)
 * after a switch to code mode (and vice-versa).
 */
export function useResetAppViewOnModeChange(
  setAppView: (next: AppView) => void,
): void {
  const { mode } = useAppMode();
  const previous = useRef(mode);
  useEffect(() => {
    if (previous.current !== mode) {
      previous.current = mode;
      setAppView("home");
    }
  }, [mode, setAppView]);
}
