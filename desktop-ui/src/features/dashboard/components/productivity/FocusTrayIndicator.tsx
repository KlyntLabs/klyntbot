import { useEffect, useState } from "react";
import { subscribeFocusStateChanged } from "@/services/events";

export function FocusTrayIndicator() {
  const [inFocus, setInFocus] = useState(false);

  useEffect(() => {
    return subscribeFocusStateChanged((payload) => {
      setInFocus(payload.state !== "unfocused" && payload.state !== "ended");
    });
  }, []);

  if (!inFocus) return null;

  return (
    <div className="inline-flex items-center gap-1.5 px-2 py-1 rounded-full bg-[color-mix(in_srgb,var(--success)_10%,transparent)] text-success text-ui-2xs font-medium">
      <span className="w-1.5 h-1.5 rounded-full animate-[pulse_2s_ease-in-out_infinite]" />
      <span>Focus</span>
    </div>
  );
}
