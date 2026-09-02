import { useEvent } from "@shared/hooks/useEvent";
import type { FocusStatePayload } from "@shared/types";
import { useState } from "react";

const STATE_CONFIG: Record<string, { label: string; color: string; pulse: boolean }> = {
  building: { label: "Building focus", color: "var(--ds-accent)", pulse: true },
  focused: { label: "Deep focus", color: "var(--ds-status-success)", pulse: false },
  cooldown: { label: "Cooldown", color: "var(--ds-text-secondary)", pulse: true },
};

export function FocusStateIndicator() {
  const [focusState, setFocusState] = useState<FocusStatePayload | null>(null);

  useEvent<FocusStatePayload>("focus:state_changed", (payload) => {
    // Clear indicator when returning to unfocused
    if (payload.state === "unfocused" || payload.state === "ended") {
      setFocusState(null);
    } else {
      setFocusState(payload);
    }
  });

  if (!focusState) return null;

  const config = STATE_CONFIG[focusState.state];
  if (!config) return null;

  return (
    <div className="flex items-center gap-1.5 px-2.5 py-1 rounded-full bg-[var(--surface-glass-subtle)]">
      <span
        className={`w-1.5 h-1.5 rounded-full ${config.pulse ? "animate-pulse" : ""}`}
        style={{ backgroundColor: config.color }}
      />
      <span className="text-ui-xs font-medium" style={{ color: config.color }}>
        {config.label}
      </span>
    </div>
  );
}
