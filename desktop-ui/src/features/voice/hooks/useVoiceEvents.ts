import { ipc } from "@shared/hooks/useIpc";
import { useCallback, useEffect, useState } from "react";

export type VoiceSessionState = "idle" | "capturing" | "processing" | "response";

export interface RoutingChip {
  skill: string;
  confidence: number;
  label: string;
}

export function useVoiceEvents() {
  const [sessionState, setSessionState] = useState<VoiceSessionState>("idle");
  const [transcript, setTranscript] = useState("");
  const [routingChips, setRoutingChips] = useState<RoutingChip[]>([]);
  const [memoryEcho, setMemoryEcho] = useState<string | null>(null);
  const [audioLevel, setAudioLevel] = useState(0);
  const [engineKind, setEngineKind] = useState<"local" | "cloud">("local");
  const [responseText, setResponseText] = useState("");
  const [segments, setSegments] = useState<Array<{ text: string; confidence: number }>>([]);
  const [ttsAudio, setTtsAudio] = useState<{
    base64: string;
    sampleRate: number;
    text: string;
  } | null>(null);
  const [modelDownloading, setModelDownloading] = useState(false);

  useEffect(() => {
    let unlisten: (() => void) | undefined;

    import("@tauri-apps/api/event").then(({ listen }) => {
      listen<Record<string, unknown>>("voice:event", (event) => {
        const payload = event.payload;

        switch (payload.type) {
          case "captureStarted":
            setSessionState("capturing");
            setTranscript("");
            setRoutingChips([]);
            setMemoryEcho(null);
            setSegments([]);
            setTtsAudio(null);
            setEngineKind((payload.engine as string) === "cloud" ? "cloud" : "local");
            break;
          case "audioLevel":
            setAudioLevel(payload.rms as number);
            break;
          case "partialTranscript":
            setTranscript(payload.text as string);
            setSegments((payload.segments as Array<{ text: string; confidence: number }>) ?? []);
            break;
          case "routingSuggestion":
            setRoutingChips((prev) => {
              if (prev.find((c) => c.skill === payload.skill)) return prev;
              return [
                ...prev,
                {
                  skill: payload.skill as string,
                  confidence: payload.confidence as number,
                  label: payload.label as string,
                },
              ];
            });
            break;
          case "memoryEcho":
            setMemoryEcho(payload.text as string);
            break;
          case "captureEnded":
          case "processingInBackground":
            setSessionState("processing");
            break;
          case "finalized":
            setSessionState("response");
            setTranscript(payload.text as string);
            setResponseText((payload.responsePreview as string) || (payload.text as string));
            break;
          case "speakResponse":
            setSessionState("response");
            setTtsAudio({
              base64: payload.audioBase64 as string,
              sampleRate: payload.sampleRate as number,
              text: payload.text as string,
            });
            break;
          case "error": {
            const message = payload.message as string;
            if (message.includes("No transcription engine") || message.includes("Download")) {
              setModelDownloading(true);
            }
            console.error("[VoiceBrain]", message);
            break;
          }
        }
      }).then((fn) => {
        unlisten = fn;
      });
    });

    return () => {
      unlisten?.();
    };
  }, []);

  const dismiss = useCallback(async () => {
    await ipc("voice_dismiss");
    setSessionState("idle");
  }, []);

  return {
    sessionState,
    transcript,
    routingChips,
    memoryEcho,
    audioLevel,
    engineKind,
    responseText,
    segments,
    ttsAudio,
    modelDownloading,
    dismiss,
  };
}
