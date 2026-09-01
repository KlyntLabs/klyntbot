import { isTauri } from "@shared/lib/utils";
import { useEffect } from "react";

/**
 * Sets the document background to transparent so the window content
 * floats over the desktop. Used by popover/overlay windows (tray, launcher).
 *
 * Pass `nativeVibrancy: true` for windows that use Tauri `windowEffects`
 * (e.g. tray). This sets a `data-vibrancy` attribute so CSS disables
 * the CSS `backdrop-filter` and lets the native material show through.
 */
export function useTransparentBackground(options?: { nativeVibrancy?: boolean }) {
  const vibrancy = options?.nativeVibrancy ?? false;
  useEffect(() => {
    document.documentElement.style.background = "transparent";
    document.body.style.background = "transparent";
    if (isTauri && vibrancy) {
      document.documentElement.dataset.vibrancy = "";
    }
    return () => {
      document.documentElement.style.background = "";
      document.body.style.background = "";
      delete document.documentElement.dataset.vibrancy;
    };
  }, [vibrancy]);
}
