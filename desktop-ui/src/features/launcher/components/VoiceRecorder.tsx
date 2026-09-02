import { useEvent } from "@shared/hooks/useEvent";
import { ipc } from "@shared/hooks/useIpc";
import { Mic, Square } from "lucide-react";
import { useCallback, useEffect, useRef, useState } from "react";

type VoiceSessionState = "idle" | "capturing" | "processing" | "response";

interface VoiceRecorderProps {
  onTranscriptReady: (transcript: string) => void;
  onCancel: () => void;
}

function Waveform({ level }: { level: number }) {
  const bars = 24;
  return (
    <div className="flex items-end justify-center gap-[3px] h-12">
      {Array.from({ length: bars }).map((_, i) => {
        const base = 4;
        const amplitude = level * 44 * (0.4 + 0.6 * Math.sin(i * 0.5 + Date.now() * 0.003));
        const height = Math.max(base, base + amplitude);
        return (
          <div
            key={i}
            className="w-[3px] rounded-full bg-brand/80 transition-all duration-75"
            style={{ height: `${height}px` }}
          />
        );
      })}
    </div>
  );
}

export function VoiceRecorder({ onTranscriptReady, onCancel }: VoiceRecorderProps) {
  const [sessionState, setSessionState] = useState<VoiceSessionState>("idle");
  const [transcript, setTranscript] = useState("");
  const [audioLevel, setAudioLevel] = useState(0);

  useEvent<Record<string, unknown>>("voice:event", (payload) => {
    switch (payload.type) {
      case "captureStarted":
        setSessionState("capturing");
        setTranscript("");
        break;
      case "audioLevel":
        setAudioLevel(payload.rms as number);
        break;
      case "partialTranscript":
        setTranscript(payload.text as string);
        break;
      case "captureEnded":
      case "processingInBackground":
        setSessionState("processing");
        break;
      case "finalized":
        setSessionState("response");
        setTranscript(payload.text as string);
        break;
    }
  });

  const hasStarted = useRef(false);
  const animationFrame = useRef<number>();

  const forceUpdate = useCallback(() => {
    animationFrame.current = requestAnimationFrame(forceUpdate);
  }, []);

  useEffect(() => {
    if (hasStarted.current) return;
    hasStarted.current = true;
    ipc("voice_start_dictation").catch((e: unknown) => {
      console.error("[VoiceRecorder] Failed to start capture:", e);
      onCancel();
    });
    animationFrame.current = requestAnimationFrame(forceUpdate);
    return () => {
      if (animationFrame.current) cancelAnimationFrame(animationFrame.current);
    };
  }, [onCancel, forceUpdate]);

  useEffect(() => {
    if (sessionState === "response" && transcript) {
      onTranscriptReady(transcript);
    }
  }, [sessionState, transcript, onTranscriptReady]);

  const handleStop = useCallback(() => {
    ipc("voice_stop_dictation").catch((e: unknown) => {
      console.error("[VoiceRecorder] Failed to stop capture:", e);
    });
  }, []);

  const handleCancel = useCallback(() => {
    ipc("voice_cancel_dictation").catch(() => {});
    onCancel();
  }, [onCancel]);

  useEffect(() => {
    const handler = (e: KeyboardEvent) => {
      if (e.key === "Escape") {
        e.preventDefault();
        handleCancel();
      } else if (e.key === "Enter" && !e.shiftKey) {
        e.preventDefault();
        handleStop();
      }
    };
    window.addEventListener("keydown", handler);
    return () => window.removeEventListener("keydown", handler);
  }, [handleStop, handleCancel]);

  const isCapturing = sessionState === "capturing";
  const isProcessing = sessionState === "processing" || sessionState === "response";

  return (
    <div className="flex flex-col items-center justify-center py-8 px-6 gap-5">
      <div className="relative">
        <div
          className={`size-16 rounded-full flex items-center justify-center ${
            isCapturing ? "bg-brand/20" : "bg-muted/20"
          }`}
        >
          <Mic
            className={`size-7 ${isCapturing ? "text-brand" : "text-muted-foreground"}`}
            strokeWidth={1.5}
          />
        </div>
        {isCapturing && (
          <div className="absolute inset-0 rounded-full border-2 border-brand/40 animate-ping" />
        )}
      </div>

      {isCapturing && <Waveform level={audioLevel} />}

      <div className="text-center">
        {isCapturing && <p className="text-sm text-muted-foreground font-light">Listening...</p>}
        {isProcessing && (
          <p className="text-sm text-muted-foreground font-light animate-pulse">Transcribing...</p>
        )}
      </div>

      {transcript && (
        <p className="text-xs text-muted-foreground/60 text-center max-w-[400px] line-clamp-2 italic">
          {transcript}
        </p>
      )}

      <div className="flex items-center gap-3">
        {isCapturing && (
          <button
            type="button"
            onClick={handleStop}
            className="flex items-center gap-2 px-4 py-2 rounded-full bg-brand text-white text-xs font-medium hover:bg-brand/90 transition-colors"
          >
            <Square className="size-3" fill="currentColor" />
            Done
          </button>
        )}
      </div>

      <div className="flex items-center gap-4 text-[11px] text-muted-foreground/50">
        <span className="flex items-center gap-1">
          <kbd className="px-1 py-0.5 glass-badge text-[10px]">Enter</kbd> Stop
        </span>
        <span className="flex items-center gap-1">
          <kbd className="px-1 py-0.5 glass-badge text-[10px]">Esc</kbd> Cancel
        </span>
      </div>
    </div>
  );
}
