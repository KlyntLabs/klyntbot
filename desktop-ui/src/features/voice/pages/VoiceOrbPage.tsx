import { VoiceBrainOrb } from "@features/voice";
import { useTransparentBackground } from "@shared/hooks/useTransparentBackground";
import { useWindowAutoResize } from "@shared/hooks/useWindowAutoResize";
import { DEV_SSE_BASE, isTauri } from "@shared/lib/utils";
import { useEffect, useRef } from "react";

/**
 * Bridge voice SSE events in browser dev mode.
 * VoiceOrbPage lives outside AppShell so BrainEventBridge isn't mounted.
 */
function VoiceEventBridge() {
  useEffect(() => {
    if (isTauri) return;

    const source = new EventSource(`${DEV_SSE_BASE}/api/brain/events`);
    source.addEventListener("voice:event", (e: MessageEvent) => {
      try {
        const payload = JSON.parse(e.data);
        window.dispatchEvent(new CustomEvent("voice:event", { detail: payload }));
      } catch {
        // Ignore malformed SSE frames
      }
    });

    return () => source.close();
  }, []);

  return null;
}

export function VoiceOrbPage() {
  const contentRef = useRef<HTMLDivElement>(null);

  useTransparentBackground({ nativeVibrancy: true });
  useWindowAutoResize(contentRef, { width: 320, maxHeight: 400 });

  return (
    <div ref={contentRef}>
      <VoiceEventBridge />
      <VoiceBrainOrb />
    </div>
  );
}
