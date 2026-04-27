import { useEffect } from "react";
import { useVoiceRecording } from "../hooks/useVoiceRecording";

interface Props {
  onTranscriptReady: (transcript: string) => void;
  onCancel: () => void;
}

const PHASE_LABEL: Record<"idle" | "recording" | "processing" | "error", string> = {
  recording: "Listening… press Enter to send",
  processing: "Transcribing…",
  error: "Something went wrong",
  idle: "Press to start",
};

export function VoiceRecorder({ onTranscriptReady, onCancel }: Props) {
  const { phase, level, start, stop, cancel } = useVoiceRecording(onTranscriptReady);

  useEffect(() => { void start(); }, [start]);

  useEffect(() => {
    const onKey = (e: KeyboardEvent) => {
      if (e.key === "Escape") { void cancel(); onCancel(); }
      if (e.key === "Enter") { void stop(); }
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, [stop, cancel, onCancel]);

  return (
    <div className="lc-voice">
      <div className={`lc-voice-orb lc-voice-orb--${phase}`}
           style={{ transform: `scale(${1 + level * 0.4})` }}>
        🎙
      </div>
      <p className="lc-muted-sm">{PHASE_LABEL[phase]}</p>
      <p className="lc-hint-sm">Esc to cancel</p>
    </div>
  );
}
