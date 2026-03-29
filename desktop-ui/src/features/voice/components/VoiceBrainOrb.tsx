import { playTtsAudio } from "@shared/lib/audio";
import { useEffect } from "react";
import { useVoiceEvents } from "../hooks/useVoiceEvents";

function Waveform({ level }: { level: number }) {
  const bars = 12;
  return (
    <div className="flex items-center gap-0.5 h-4">
      {Array.from({ length: bars }).map((_, i) => {
        const height = Math.max(2, level * 16 * (0.5 + 0.5 * Math.sin(i * 0.8)));
        return (
          <div
            key={i}
            className="w-0.5 rounded-full bg-accent transition-all duration-75"
            style={{ height: `${height}px` }}
          />
        );
      })}
    </div>
  );
}

function WordHighlights({ segments }: { segments: Array<{ text: string; confidence: number }> }) {
  return (
    <span>
      {segments.map((seg, i) => {
        const cls =
          seg.confidence >= 0.85
            ? "text-success"
            : seg.confidence >= 0.6
              ? "text-warning"
              : "text-destructive";
        return (
          <span key={i} className={cls}>
            {seg.text}{" "}
          </span>
        );
      })}
    </span>
  );
}

function RoutingChips({ chips }: { chips: { skill: string; label: string }[] }) {
  return (
    <div className="flex gap-1.5 flex-wrap">
      {chips.map((chip) => (
        <div key={chip.skill} className="glass-panel px-2 py-0.5 rounded-full text-xs text-muted">
          {chip.label}
        </div>
      ))}
    </div>
  );
}

export function VoiceBrainOrb() {
  const {
    sessionState,
    transcript,
    routingChips,
    memoryEcho,
    audioLevel,
    engineKind,
    responseText,
    segments,
    ttsAudio,
    dismiss,
  } = useVoiceEvents();

  useEffect(() => {
    if (ttsAudio) {
      playTtsAudio(ttsAudio.base64, ttsAudio.sampleRate);
    }
  }, [ttsAudio]);

  if (sessionState === "idle") return null;

  return (
    <div
      className="glass-panel rounded-2xl p-3 w-[320px] select-none animate-in fade-in zoom-in-95 duration-200"
      role={sessionState === "response" ? "button" : undefined}
      tabIndex={sessionState === "response" ? 0 : undefined}
      onClick={sessionState === "response" ? dismiss : undefined}
      onKeyDown={
        sessionState === "response"
          ? (e) => {
              if (e.key === "Enter" || e.key === " ") dismiss();
            }
          : undefined
      }
    >
      {/* Header */}
      <div className="flex items-center gap-2 mb-2">
        <div
          className={`w-2 h-2 rounded-full ${
            sessionState === "capturing" ? "bg-red-500 animate-pulse" : "bg-muted"
          }`}
        />
        {sessionState === "capturing" && <Waveform level={audioLevel} />}
        {sessionState === "processing" && (
          <span className="text-xs text-muted animate-pulse">Processing...</span>
        )}
        {sessionState === "response" && <span className="text-xs text-muted">Response</span>}
        <div className="flex-1" />
        {engineKind === "cloud" && <span className="text-xs text-muted opacity-60">cloud</span>}
      </div>

      {/* Transcript */}
      <div className="text-sm font-mono min-h-[40px] mb-2">
        {sessionState === "response" ? (
          <span className="text-foreground">{responseText}</span>
        ) : segments.length > 0 ? (
          <WordHighlights segments={segments} />
        ) : (
          <span className="text-foreground">{transcript || "Listening..."}</span>
        )}
      </div>

      {/* Routing chips */}
      {routingChips.length > 0 && (
        <div className="mb-2">
          <RoutingChips chips={routingChips} />
        </div>
      )}

      {/* TTS replay */}
      {sessionState === "response" && ttsAudio && (
        <div className="mb-2">
          <button
            type="button"
            onClick={() => playTtsAudio(ttsAudio.base64, ttsAudio.sampleRate)}
            className="text-xs text-muted hover:text-primary transition-colors"
            aria-label="Replay spoken response"
          >
            ↻ Replay
          </button>
        </div>
      )}

      {/* Memory echo with Mirror badge */}
      {memoryEcho && (
        <div className="animate-in fade-in duration-300 flex items-center gap-1.5 text-xs text-muted mb-2">
          <span className="rounded bg-surface-overlay px-1 py-0.5 text-[10px] font-medium text-muted/70">
            Mirror
          </span>
          <span className="italic">{memoryEcho}</span>
        </div>
      )}

      {/* Hint bar */}
      <div className="text-[10px] text-muted opacity-40 text-center">
        {sessionState === "capturing" && "opt+shift+V to finish · tap to close"}
        {sessionState === "processing" && "Cancel & discard"}
        {sessionState === "response" && "tap anywhere to close"}
      </div>
    </div>
  );
}
