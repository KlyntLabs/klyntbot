import { useVoiceConversation } from "@features/voice/hooks/useVoiceConversation";
import { useEvent } from "@shared/hooks/useEvent";
import { unlockAudioContext } from "@shared/lib/audio";
import { isTauri } from "@shared/lib/utils";
import { useEffect, useRef } from "react";

import { VoiceOrbCanvas } from "./VoiceOrbCanvas";

const AUTO_HIDE_DELAY_MS = 3000;

export function VoiceBrainOrb() {
  const { phase, audioLevel, start, end, interrupt, newSession, sessionInfo } =
    useVoiceConversation();
  const prevPhaseRef = useRef(phase);
  const clickTimerRef = useRef<ReturnType<typeof setTimeout> | null>(null);

  // Unlock AudioContext on mount (orb opens via global hotkey, not a click).
  // Also register a one-shot click/keydown listener: WebKit requires a user gesture
  // for AudioContext.resume(), so we unlock on the first interaction in the window.
  useEffect(() => {
    unlockAudioContext();
    const handler = () => {
      unlockAudioContext();
      document.removeEventListener("click", handler);
      document.removeEventListener("keydown", handler);
    };
    document.addEventListener("click", handler);
    document.addEventListener("keydown", handler);
    return () => {
      document.removeEventListener("click", handler);
      document.removeEventListener("keydown", handler);
    };
  }, []);

  // Second unlock attempt after Rust-side set_focus().
  useEvent("voice:unlock-audio", unlockAudioContext);

  // Auto-start in browser dev mode.
  const startedRef = useRef(false);
  useEffect(() => {
    if (!isTauri && !startedRef.current && phase === "idle" && !sessionInfo) {
      startedRef.current = true;
      start().catch(() => {});
    }
  }, [phase, sessionInfo, start]);

  // Auto-hide: 3 seconds after speaking -> idle transition.
  useEffect(() => {
    const wasSpeaking = prevPhaseRef.current === "speaking";
    prevPhaseRef.current = phase;

    if (wasSpeaking && phase === "idle" && isTauri) {
      const timer = setTimeout(async () => {
        const { getCurrentWindow } = await import("@tauri-apps/api/window");
        getCurrentWindow().hide();
      }, AUTO_HIDE_DELAY_MS);
      return () => clearTimeout(timer);
    }
  }, [phase]);

  // Dismiss on Esc.
  useEffect(() => {
    const onKey = (e: KeyboardEvent) => {
      if (e.key === "Escape") {
        end();
      }
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, [end]);

  // Click: interrupt during speaking. Double-click: new session.
  const onClick = () => {
    if (clickTimerRef.current) {
      clearTimeout(clickTimerRef.current);
      clickTimerRef.current = null;
      newSession();
      return;
    }
    clickTimerRef.current = setTimeout(() => {
      clickTimerRef.current = null;
      if (phase === "speaking") {
        interrupt();
      }
    }, 250);
  };

  // Unlock audio on first interaction.
  const onMouseDown = () => {
    unlockAudioContext();
  };

  // Start window drag on mouse move (not mousedown) to avoid suppressing onClick.
  const onMouseMove = async (e: React.MouseEvent) => {
    if (e.buttons === 1 && window.__TAURI_INTERNALS__) {
      const { getCurrentWindow } = await import("@tauri-apps/api/window");
      getCurrentWindow().startDragging();
    }
  };

  return (
    <button
      type="button"
      onClick={onClick}
      onKeyDown={(e) => {
        if (e.key === "Enter" || e.key === " ") onClick();
      }}
      onMouseDown={onMouseDown}
      onMouseMove={onMouseMove}
      style={{
        width: "100%",
        height: "100%",
        cursor: "grab",
        background: "none",
        border: "none",
        padding: 0,
        display: "block",
      }}
    >
      <VoiceOrbCanvas phase={phase} audioLevel={audioLevel} />
    </button>
  );
}
