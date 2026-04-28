import { useCallback, useEffect, useRef, useState } from "react";
import { ipc } from "@/utils/tauri-bridge";
import { showError } from "../lib/showError";

type Phase = "idle" | "recording" | "processing" | "error";

export function useVoiceRecording(onTranscript: (t: string) => void) {
  const [phase, setPhase] = useState<Phase>("idle");
  const [level, setLevel] = useState(0);
  const stoppedRef = useRef(false);

  const start = useCallback(async () => {
    stoppedRef.current = false;
    setPhase("recording");
    try {
      await ipc("voice_start_dictation", {});
    } catch (err) {
      setPhase("error");
      showError("Couldn't start recording:", err);
    }
  }, []);

  const stop = useCallback(async () => {
    if (stoppedRef.current) return;
    stoppedRef.current = true;
    setPhase("processing");
    try {
      const transcript: string = await ipc("voice_stop_dictation", {});
      onTranscript(transcript);
      setPhase("idle");
    } catch (err) {
      setPhase("error");
      showError("Couldn't process transcript:", err);
    }
  }, [onTranscript]);

  const cancel = useCallback(async () => {
    if (stoppedRef.current) return;
    stoppedRef.current = true;
    try {
      await ipc("voice_stop_dictation", {});
    } catch (err) {
      console.error("voice_stop_dictation on cancel failed:", err);
    }
    setPhase("idle");
  }, []);

  // Simulated audio level for waveform animation (no live level IPC available)
  useEffect(() => {
    if (phase !== "recording") {
      setLevel(0);
      return;
    }
    const id = setInterval(() => {
      setLevel(Math.random() * 0.6 + 0.2);
    }, 100);
    return () => clearInterval(id);
  }, [phase]);

  return { phase, level, start, stop, cancel };
}
