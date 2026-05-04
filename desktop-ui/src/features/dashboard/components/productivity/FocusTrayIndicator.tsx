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
    <div className="dashboard__focus-tray-pill">
      <span className="dashboard__focus-state-pill-dot dashboard__focus-state-pill-dot--pulsing" />
      <span>Focus</span>
    </div>
  );
}
