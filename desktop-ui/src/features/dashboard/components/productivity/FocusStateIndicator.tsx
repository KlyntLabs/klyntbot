import { useEffect, useState } from "react";
import type { FocusStatePayload } from "@/bindings";
import { subscribeFocusStateChanged } from "@/services/events";
import { cn } from "@/utils/cn";

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

  if (!focusState) return <div data-testid="focus-state-indicator" className="hidden" />;
  const config = STATE_CONFIG[focusState.state];
  if (!config) return null;

  return (
    <div className="px-4 py-1 flex justify-start">
      <div className="inline-flex items-center gap-1.5 px-2.5 py-1 rounded-full bg-surface-card-strong border border-ds-border-subtle text-ui-2xs font-medium">
        <span
          className={cn("w-1.5 h-1.5 rounded-full", config.pulse && "animate-[pulse_2s_ease-in-out_infinite]")}
          style={{ backgroundColor: config.color }}
        />
        <span style={{ color: config.color }}>{config.label}</span>
      </div>
    </div>
  );
}
