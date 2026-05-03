import { useEffect, useState } from "react";
import type { FocusStatePayload } from "@/bindings";
import { subscribeFocusStateChanged } from "@/services/events";

const STATE_CONFIG: Record<string, { label: string; color: string; pulse: boolean }> = {
  building: { label: "Building focus", color: "var(--brand)", pulse: true },
  focused: { label: "Deep focus", color: "var(--success)", pulse: false },
  cooldown: { label: "Cooldown", color: "var(--text-muted-foreground)", pulse: true },
};

export function FocusStateIndicator() {
  const [focusState, setFocusState] = useState<FocusStatePayload | null>(null);

  useEffect(() => {
    return subscribeFocusStateChanged((payload) => {
      if (payload.state === "unfocused" || payload.state === "ended") {
        setFocusState(null);
      } else {
        setFocusState(payload);
      }
    });
  }, []);

  if (!focusState) return <div data-testid="focus-state-indicator" style={{ display: "none" }} />;
  const config = STATE_CONFIG[focusState.state];
  if (!config) return null;

  return (
    <div className="dashboard__focus-state-banner">
      <div className="dashboard__focus-state-pill">
        <span
          className={
            config.pulse
              ? "dashboard__focus-state-pill-dot dashboard__focus-state-pill-dot--pulsing"
              : "dashboard__focus-state-pill-dot"
          }
          style={{ backgroundColor: config.color }}
        />
        <span style={{ color: config.color }}>{config.label}</span>
      </div>
    </div>
  );
}
