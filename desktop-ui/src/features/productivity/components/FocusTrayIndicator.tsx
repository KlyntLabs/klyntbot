import { useEvent } from "@shared/hooks/useEvent";
import type { FocusStatePayload } from "@shared/types";
import { useState } from "react";

/**
 * FocusTrayIndicator — minimal status indicator showing when auto-focus is active.
 * Renders a subtle green dot + "Focus" text, or hidden when not in focus.
 * Listens to focus:state-changed and focus:auto-started events from backend.
 */
export function FocusTrayIndicator() {
  const [inFocus, setInFocus] = useState(false);

  useEvent<FocusStatePayload>("focus:state_changed", (payload) => {
    // Show indicator for any non-unfocused state
    setInFocus(payload.state !== "unfocused" && payload.state !== "ended");
  });

  useEvent<{ sessionId: string; appName: string }>("focus:auto-started", () => {
    setInFocus(true);
  });

  if (!inFocus) return null;

  return (
    <div className="flex items-center gap-1.5 px-2 py-1 rounded-full bg-success/10 text-success text-[10px] font-medium">
      <span className="w-1.5 h-1.5 rounded-full bg-success animate-pulse" />
      Focus
    </div>
  );
}
