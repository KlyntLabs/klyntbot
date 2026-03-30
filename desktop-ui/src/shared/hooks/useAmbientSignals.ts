import { useEvent } from "@shared/hooks/useEvent";
import { useCallback, useRef, useState } from "react";

// ── Types ────────────────────────────────────────────────────────────

export type SignalMode = "pulse" | "badge" | "deferred" | "merged";

export interface SignalSummary {
  signalType: string;
  entityPair: string;
  headline: string;
}

export interface BrainAmbientEvent {
  mode: SignalMode;
  signals: SignalSummary[];
  tooltip: string;
  detailRoute: string | null;
}

export interface AmbientSignalState {
  current: BrainAmbientEvent | null;
  badgeCount: number;
  isPulsing: boolean;
  isFocusDeferred: boolean;
  badgeSignals: SignalSummary[];
  acknowledge: () => void;
  clearBadge: () => void;
}

interface FocusStatePayload {
  active: boolean;
}

// ── Hook ─────────────────────────────────────────────────────────────

const PULSE_DURATION_MS = 2000;

export function useAmbientSignals(): AmbientSignalState {
  const [current, setCurrent] = useState<BrainAmbientEvent | null>(null);
  const [badgeCount, setBadgeCount] = useState(0);
  const [isPulsing, setIsPulsing] = useState(false);
  const [isFocusDeferred, setIsFocusDeferred] = useState(false);
  const [badgeSignals, setBadgeSignals] = useState<SignalSummary[]>([]);

  const pulseTimerRef = useRef<ReturnType<typeof setTimeout> | null>(null);

  const triggerPulse = useCallback(() => {
    setIsPulsing(true);
    if (pulseTimerRef.current !== null) {
      clearTimeout(pulseTimerRef.current);
    }
    pulseTimerRef.current = setTimeout(() => {
      setIsPulsing(false);
      pulseTimerRef.current = null;
    }, PULSE_DURATION_MS);
  }, []);

  useEvent<BrainAmbientEvent>("brain:ambient", (payload) => {
    setCurrent(payload);

    switch (payload.mode) {
      case "pulse":
        triggerPulse();
        break;

      case "merged":
        triggerPulse();
        break;

      case "badge":
        setBadgeCount((n) => n + 1);
        setBadgeSignals((prev) => [...prev, ...payload.signals]);
        break;

      case "deferred":
        // Focus just ended — clear deferred flag and trigger pulse
        setIsFocusDeferred(false);
        triggerPulse();
        break;
    }
  });

  useEvent<FocusStatePayload>("focus:state", (payload) => {
    setIsFocusDeferred(payload.active);
  });

  const acknowledge = useCallback(() => {
    setIsPulsing(false);
    setCurrent(null);
    if (pulseTimerRef.current !== null) {
      clearTimeout(pulseTimerRef.current);
      pulseTimerRef.current = null;
    }
  }, []);

  const clearBadge = useCallback(() => {
    setBadgeCount(0);
    setBadgeSignals([]);
  }, []);

  return {
    current,
    badgeCount,
    isPulsing,
    isFocusDeferred,
    badgeSignals,
    acknowledge,
    clearBadge,
  };
}
