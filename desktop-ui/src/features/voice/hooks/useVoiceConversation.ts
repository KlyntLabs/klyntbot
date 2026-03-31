import { useEvent } from "@shared/hooks/useEvent";
import { ipc } from "@shared/hooks/useIpc";
import { playTtsAudio, stopTtsAudio } from "@shared/lib/audio";
import { useCallback, useRef, useState } from "react";

export type ConversationPhase = "idle" | "listening" | "reflecting" | "speaking";

export interface RoutingChip {
  skill: string;
  confidence: number;
  label: string;
}

export interface SessionInfo {
  key: string;
  title: string;
  turnCount: number;
}

export interface TtsAudioData {
  base64: string;
  sampleRate: number;
  text: string;
}

export function useVoiceConversation() {
  const [phase, setPhase] = useState<ConversationPhase>("idle");
  const [transcript, setTranscript] = useState("");
  const [segments, setSegments] = useState<Array<{ text: string; confidence: number }>>([]);
  const [routingChips, setRoutingChips] = useState<RoutingChip[]>([]);
  const [memoryEcho, setMemoryEcho] = useState<string | null>(null);
  const [audioLevel, setAudioLevel] = useState(0);
  const [ttsAudio, setTtsAudio] = useState<TtsAudioData | null>(null);
  const [sessionInfo, setSessionInfo] = useState<SessionInfo | null>(null);
  const [continueAvailable, setContinueAvailable] = useState(false);
  const [engineKind, setEngineKind] = useState<"local" | "cloud">("local");
  const [setupRequired, setSetupRequired] = useState(false);

  const continueTimerRef = useRef<ReturnType<typeof setTimeout> | null>(null);

  // Handle incoming voice events
  const handleEvent = useCallback((payload: Record<string, unknown>) => {
    const type = payload.type as string;

    switch (type) {
      case "phaseChanged": {
        const newPhase = payload.phase as ConversationPhase;
        setPhase(newPhase);
        // If the conversation started, the engine is available — clear setup screen
        if (newPhase !== "idle") {
          setSetupRequired(false);
        }
        if (payload.sessionTitle || payload.turnCount !== undefined) {
          setSessionInfo((prev) => ({
            key: prev?.key ?? "",
            title: (payload.sessionTitle as string) ?? prev?.title ?? "",
            turnCount: (payload.turnCount as number) ?? 0,
          }));
        }
        if (newPhase === "listening") {
          setTranscript("");
          setSegments([]);
          setRoutingChips([]);
          setMemoryEcho(null);
          setContinueAvailable(false);
          if (continueTimerRef.current) {
            clearTimeout(continueTimerRef.current);
            continueTimerRef.current = null;
          }
        }
        break;
      }
      case "audioLevel":
        setAudioLevel(payload.rms as number);
        break;
      case "partialTranscript":
        setTranscript(payload.text as string);
        if (payload.segments) {
          setSegments(payload.segments as Array<{ text: string; confidence: number }>);
        }
        break;
      case "routingSuggestion":
        setRoutingChips((prev) => {
          const skill = payload.skill as string;
          if (prev.some((c) => c.skill === skill)) return prev;
          return [
            ...prev,
            {
              skill,
              confidence: payload.confidence as number,
              label: payload.label as string,
            },
          ];
        });
        break;
      case "memoryEcho":
        setMemoryEcho(payload.text as string);
        break;
      case "reflecting":
        setPhase("reflecting");
        break;
      case "speakResponse": {
        setPhase("speaking");
        const audio: TtsAudioData = {
          base64: payload.audioBase64 as string,
          sampleRate: (payload.sampleRate as number) ?? 16000,
          text: payload.text as string,
        };
        setTtsAudio(audio);
        playTtsAudio(audio.base64, audio.sampleRate);
        break;
      }
      case "ttsFadeOut":
        stopTtsAudio();
        // Delay clearing so CSS can animate a 300ms fade-out on the speaking visual
        setTimeout(() => setTtsAudio(null), 300);
        break;
      case "continueAvailable":
        setContinueAvailable(true);
        // Auto-hide after 8 seconds
        continueTimerRef.current = setTimeout(() => {
          setContinueAvailable(false);
          continueTimerRef.current = null;
        }, 8000);
        break;
      case "captureStarted":
        setEngineKind(payload.engine === "Cloud" ? "cloud" : "local");
        break;
      case "setupRequired":
        setSetupRequired(true);
        break;
      case "error":
        setPhase("idle");
        break;
    }
  }, []);

  // Listen to Tauri events or browser CustomEvents
  useEvent<Record<string, unknown>>("voice:event", handleEvent);

  // Actions
  const start = useCallback(async (): Promise<SessionInfo> => {
    const result = await ipc<{ sessionKey: string; sessionTitle: string; isContinuing: boolean }>(
      "voice_conversation_start",
    );
    const info: SessionInfo = { key: result.sessionKey, title: result.sessionTitle, turnCount: 0 };
    setSessionInfo(info);
    return info;
  }, []);

  const pause = useCallback(async () => {
    await ipc("voice_conversation_pause");
  }, []);

  const resume = useCallback(async () => {
    await ipc("voice_conversation_resume");
  }, []);

  const interrupt = useCallback(async () => {
    await ipc("voice_conversation_interrupt");
  }, []);

  const continueTts = useCallback(async () => {
    await ipc("voice_conversation_continue");
    setContinueAvailable(false);
    if (continueTimerRef.current) {
      clearTimeout(continueTimerRef.current);
      continueTimerRef.current = null;
    }
  }, []);

  const newSession = useCallback(async (): Promise<SessionInfo> => {
    const result = await ipc<{ sessionKey: string; sessionTitle: string; isContinuing: boolean }>(
      "voice_conversation_new_session",
    );
    const info: SessionInfo = { key: result.sessionKey, title: result.sessionTitle, turnCount: 0 };
    setSessionInfo(info);
    return info;
  }, []);

  const end = useCallback(async () => {
    stopTtsAudio();
    await ipc("voice_conversation_end");
    setPhase("idle");
    // Hide orb window if in Tauri
    if (window.__TAURI_INTERNALS__) {
      const { getCurrentWindow } = await import("@tauri-apps/api/window");
      getCurrentWindow().hide();
    }
  }, []);

  return {
    phase,
    transcript,
    segments,
    routingChips,
    memoryEcho,
    audioLevel,
    ttsAudio,
    sessionInfo,
    continueAvailable,
    engineKind,
    setupRequired,
    start,
    pause,
    resume,
    interrupt,
    continueTts,
    newSession,
    end,
  };
}
