import { VoiceBrainOrb } from "@features/voice/components/VoiceBrainOrb";
import { useTransparentBackground } from "@shared/hooks/useTransparentBackground";
import { useEffect } from "react";

const DEV_SSE_BASE = "http://localhost:3456";

function VoiceEventBridge() {
  useEffect(() => {
    if (window.__TAURI_INTERNALS__) return;
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

export default function VoiceOrbPage() {
  useTransparentBackground();

  return (
    <div style={{ width: "100vw", height: "100vh", overflow: "hidden" }}>
      <VoiceEventBridge />
      <VoiceBrainOrb />
    </div>
  );
}
