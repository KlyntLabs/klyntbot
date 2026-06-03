import { useEffect } from "react";

// Sets `data-launcher="true"` on <html> so the rules in aux-window.css
// apply: transparent background chain + 16px rounded clip on html/body/#root
// (matches Tauri's HUD effect radius from lazy_window.rs::hud_effects).
//
// The CSS approach is required — inline styles don't reliably override
// macOS WebKit's compositor mask on transparent windows; only the
// data-attribute selector with the rules in the side-imported CSS file
// produces the visibly-rounded card.
export function useTransparentBackground() {
  useEffect(() => {
    const html = document.documentElement;
    html.dataset.launcher = "true";
    return () => {
      delete html.dataset.launcher;
    };
  }, []);
}
