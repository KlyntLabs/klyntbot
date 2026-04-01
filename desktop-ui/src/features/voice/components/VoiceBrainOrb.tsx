import { useVoiceConversation } from "@features/voice/hooks/useVoiceConversation";
import { useEvent } from "@shared/hooks/useEvent";
import { unlockAudioContext } from "@shared/lib/audio";
import { isTauri } from "@shared/lib/utils";
import { useEffect, useRef } from "react";

import { VoiceOrbCanvas } from "./VoiceOrbCanvas";

const AUTO_HIDE_DELAY_MS = 3000;

export function VoiceBrainOrb() {
  const { phase, audioLevel, start, end, sessionInfo } = useVoiceConversation();
  const prevPhaseRef = useRef(phase);

  // Unlock AudioContext on mount (orb opens via global hotkey, not a click).
  useEffect(() => {
    unlockAudioContext();
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

  // Enable dragging in Tauri.
  const onMouseDown = async () => {
    if (window.__TAURI_INTERNALS__) {
      const { getCurrentWindow } = await import("@tauri-apps/api/window");
      getCurrentWindow().startDragging();
    }
  };

  return (
    <div onMouseDown={onMouseDown} style={{ width: "100%", height: "100%", cursor: "grab" }}>
      <VoiceOrbCanvas phase={phase} audioLevel={audioLevel} />
    </div>
  );
}
