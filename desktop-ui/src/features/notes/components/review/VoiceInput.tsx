import { Mic, Square } from "lucide-react";
import { useEffect, useRef, useState } from "react";

declare global {
  interface Window {
    SpeechRecognition: typeof SpeechRecognition;
    webkitSpeechRecognition: typeof SpeechRecognition;
  }
}

interface VoiceInputProps {
  onSubmit: (transcript: string) => void;
}

type RecordingState = "idle" | "recording" | "done";

export function VoiceInput({ onSubmit }: VoiceInputProps) {
  const SpeechRecognitionClass =
    typeof window !== "undefined"
      ? (window.SpeechRecognition ?? window.webkitSpeechRecognition ?? null)
      : null;

  const [recordingState, setRecordingState] = useState<RecordingState>("idle");
  const [transcript, setTranscript] = useState("");
  const recognitionRef = useRef<InstanceType<typeof SpeechRecognition> | null>(null);

  // Cleanup on unmount
  useEffect(() => {
    return () => {
      recognitionRef.current?.abort();
    };
  }, []);

  if (!SpeechRecognitionClass) {
    return (
      <div className="flex items-center justify-center gap-2 rounded-lg bg-white/[0.03] border border-border px-3 py-3">
        <Mic size={12} className="text-dim shrink-0" />
        <span className="text-2xs text-dim">Voice input is not supported in this browser.</span>
      </div>
    );
  }

  const startRecording = () => {
    const recognition = new SpeechRecognitionClass();
    recognition.continuous = true;
    recognition.interimResults = true;
    recognition.lang = "en-US";

    recognition.onresult = (event: SpeechRecognitionEvent) => {
      let interim = "";
      let final = "";
      for (let i = event.resultIndex; i < event.results.length; i++) {
        const result = event.results[i];
        if (result !== undefined) {
          if (result.isFinal) {
            final += result[0]?.transcript ?? "";
          } else {
            interim += result[0]?.transcript ?? "";
          }
        }
      }
      setTranscript((prev) => {
        const base = prev;
        return (base + final + interim).trim();
      });
    };

    recognition.onerror = () => {
      setRecordingState("idle");
    };

    recognition.onend = () => {
      setRecordingState((prev) => (prev === "recording" ? "done" : prev));
    };

    recognitionRef.current = recognition;
    recognition.start();
    setRecordingState("recording");
    setTranscript("");
  };

  const stopRecording = () => {
    recognitionRef.current?.stop();
    setRecordingState("done");
  };

  const handleSubmit = () => {
    if (transcript.trim()) {
      onSubmit(transcript.trim());
    }
  };

  const handleReset = () => {
    recognitionRef.current?.abort();
    setRecordingState("idle");
    setTranscript("");
  };

  return (
    <div className="flex flex-col gap-2">
      {/* Transcript area */}
      <div className="min-h-[60px] rounded-lg bg-white/[0.04] border border-border px-3 py-2 text-xs text-foreground">
        {transcript ? (
          <p className="whitespace-pre-wrap">{transcript}</p>
        ) : (
          <p className="text-dim">
            {recordingState === "recording" ? "Listening…" : "Press the mic to start recording"}
          </p>
        )}
      </div>

      {/* Controls */}
      <div className="flex items-center gap-2">
        {recordingState === "idle" && (
          <button
            type="button"
            onClick={startRecording}
            className="flex items-center gap-1.5 text-2xs px-3 py-1.5 rounded-full bg-white/[0.06] text-muted-foreground hover:bg-white/[0.10] hover:text-foreground"
          >
            <Mic size={11} />
            Record
          </button>
        )}

        {recordingState === "recording" && (
          <button
            type="button"
            onClick={stopRecording}
            className="flex items-center gap-1.5 text-2xs px-3 py-1.5 rounded-full bg-red-500/20 text-red-400 hover:bg-red-500/30 animate-pulse"
          >
            <Square size={11} />
            Stop
          </button>
        )}

        {recordingState === "done" && (
          <>
            <button
              type="button"
              onClick={handleSubmit}
              disabled={!transcript.trim()}
              className="flex-1 text-2xs px-3 py-1.5 rounded-md bg-accent/20 text-accent hover:bg-accent/30 disabled:opacity-40 disabled:cursor-not-allowed"
            >
              Submit
            </button>
            <button
              type="button"
              onClick={handleReset}
              className="text-2xs px-2 py-1.5 rounded-md bg-white/[0.06] text-dim hover:text-foreground"
            >
              Retry
            </button>
          </>
        )}

        {recordingState !== "done" && transcript && (
          <button
            type="button"
            onClick={handleSubmit}
            className="ml-auto text-2xs px-3 py-1.5 rounded-md bg-accent/20 text-accent hover:bg-accent/30"
          >
            Submit
          </button>
        )}
      </div>
    </div>
  );
}
