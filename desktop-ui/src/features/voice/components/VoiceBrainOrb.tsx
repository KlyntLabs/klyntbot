import type { ConversationPhase } from "@features/voice/hooks/useVoiceConversation";
import { useVoiceConversation } from "@features/voice/hooks/useVoiceConversation";
import { playTtsAudio } from "@shared/lib/audio";
import { useEffect, useRef, useState } from "react";

// ─── Sub-components ──────────────────────────────────────────────────────────

function Waveform({ level, colorClass }: { level: number; colorClass: string }) {
  const bars = 12;
  return (
    <div className="flex items-center gap-0.5 h-4">
      {Array.from({ length: bars }).map((_, i) => {
        const height = Math.max(2, level * 16 * (0.5 + 0.5 * Math.sin(i * 0.8)));
        return (
          <div
            key={i}
            className={`w-0.5 rounded-full transition-all duration-75 ${colorClass}`}
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
        <div
          key={chip.skill}
          className="glass-panel border border-border/40 px-2.5 py-0.5 rounded-full text-xs text-muted-foreground"
        >
          {chip.label}
        </div>
      ))}
    </div>
  );
}

// ─── Phase dot color mapping ──────────────────────────────────────────────────

function phaseDotClass(phase: ConversationPhase): string {
  switch (phase) {
    case "listening":
      return "bg-destructive animate-pulse";
    case "reflecting":
      return "bg-warning animate-pulse";
    case "speaking":
      return "bg-primary animate-pulse";
    default:
      return "bg-muted";
  }
}

function phaseWaveformColor(phase: ConversationPhase): string {
  switch (phase) {
    case "speaking":
      return "bg-primary/60";
    default:
      return "bg-destructive/60";
  }
}

// ─── Main component ───────────────────────────────────────────────────────────

export function VoiceBrainOrb() {
  const {
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
    pause,
    resume,
    interrupt,
    newSession,
    continueTts,
    start,
    end,
  } = useVoiceConversation();

  // Track whether the session was warm-reattached
  const [isContinuing, setIsContinuing] = useState(false);
  // Track whether we're paused (phase stays at current value, but we toggled pause)
  const [paused, setPaused] = useState(false);

  // Listen for a phaseChanged event that signals continuing — we detect it via
  // sessionInfo changes combined with turnCount > 0 when phase first enters listening
  const prevPhaseRef = useRef<ConversationPhase>("idle");
  useEffect(() => {
    if (phase === "listening" && prevPhaseRef.current === "idle" && sessionInfo) {
      setIsContinuing(sessionInfo.turnCount > 0);
    }
    prevPhaseRef.current = phase;
  }, [phase, sessionInfo]);

  // Auto-play TTS is handled inside the hook already (playTtsAudio called in hook).
  // We only need it here for the replay button — ttsAudio.base64 carries the data.

  const handlePauseResume = async () => {
    if (paused) {
      await resume();
      setPaused(false);
    } else {
      await pause();
      setPaused(true);
    }
  };

  const handleNewSession = async () => {
    const info = await newSession();
    setIsContinuing(info.turnCount > 0);
    setPaused(false);
  };

  // Setup required → show model download progress screen
  if (setupRequired) {
    return (
      <div className="bg-card border border-border/50 rounded-2xl p-4 w-[320px] shadow-xl text-center">
        <div className="text-sm font-medium text-card-foreground mb-2">
          Waking up your second brain...
        </div>
        <div className="text-xs text-muted-foreground mb-3">Local voice model downloading</div>
        <div className="h-1 bg-accent rounded-full overflow-hidden">
          <div className="h-full bg-primary rounded-full animate-pulse w-1/2" />
        </div>
        <button
          type="button"
          onClick={() => {
            start().catch(() => {
              // Cloud not available either — stay on setup screen
            });
          }}
          className="mt-3 text-xs text-primary hover:underline"
        >
          Speak anyway (cloud)
        </button>
      </div>
    );
  }

  // Idle with no session → render nothing
  if (phase === "idle" && !sessionInfo) return null;

  const showWaveform = phase === "listening" || phase === "speaking";
  const showReflecting = phase === "reflecting";

  return (
    // biome-ignore lint/a11y/noStaticElementInteractions: orb background acts as dismiss zone
    <div
      className="bg-card border border-border/50 rounded-2xl w-[320px] shadow-xl select-none
        animate-in fade-in zoom-in-95 duration-200 overflow-hidden"
      onClick={phase === "speaking" ? interrupt : end}
      onKeyDown={(e) => {
        if (e.key === "Escape") {
          if (phase === "speaking") {
            interrupt();
          } else {
            end();
          }
        }
      }}
    >
      {/* ── Title bar ──────────────────────────────────────────────────────── */}
      <div
        className="flex items-center gap-2 px-3 pt-3 pb-2"
        onClick={(e) => e.stopPropagation()}
        onKeyDown={(e) => e.stopPropagation()}
        role="presentation"
      >
        <div className={`w-2 h-2 flex-shrink-0 rounded-full ${phaseDotClass(phase)}`} />

        <span className="text-xs font-medium text-foreground truncate flex-1">
          {sessionInfo?.title ?? "Voice"}
        </span>

        {isContinuing && (
          <button
            type="button"
            onClick={async (e) => {
              e.stopPropagation();
              if (window.__TAURI_INTERNALS__) {
                const { WebviewWindow } = await import("@tauri-apps/api/webviewWindow");
                const mainWindow = WebviewWindow.getByLabel("main");
                if (mainWindow) {
                  await mainWindow.setFocus();
                }
              }
            }}
            className="text-[10px] text-success hover:text-success/80 bg-accent/40 px-1.5 py-0.5
              rounded-full flex-shrink-0 flex items-center gap-0.5 transition-colors"
            title="Open in chat"
          >
            continuing ↗
          </button>
        )}

        {engineKind === "cloud" && (
          <span className="text-[10px] text-muted-foreground/40 flex-shrink-0">cloud</span>
        )}

        <div className="flex items-center gap-1 flex-shrink-0">
          <button
            type="button"
            onClick={handleNewSession}
            className="text-xs text-muted-foreground hover:text-primary transition-colors px-1.5 py-0.5
              rounded hover:bg-accent/40"
            aria-label="New session"
            title="New session"
          >
            ⊕
          </button>
          <button
            type="button"
            onClick={handlePauseResume}
            className="text-xs text-muted-foreground hover:text-primary transition-colors px-1.5 py-0.5
              rounded hover:bg-accent/40"
            aria-label={paused ? "Resume" : "Pause"}
            title={paused ? "Resume" : "Pause"}
          >
            {paused ? "▶" : "⏸"}
          </button>
        </div>
      </div>

      {/* ── Phase visual ───────────────────────────────────────────────────── */}
      <div className="px-3 pb-2 flex items-center justify-center gap-2 min-h-[28px]">
        {showWaveform && <Waveform level={audioLevel} colorClass={phaseWaveformColor(phase)} />}
        {phase === "speaking" && (
          <button
            type="button"
            onClick={(e) => {
              e.stopPropagation();
              interrupt();
            }}
            className="text-xs text-muted-foreground hover:text-foreground transition-colors"
            aria-label="Stop speaking"
          >
            ■ Stop
          </button>
        )}
        {showReflecting && (
          <div className="flex items-center gap-2">
            <div className="w-2.5 h-2.5 rounded-full bg-warning animate-pulse" />
            <span className="text-xs text-warning/80 animate-pulse">Reflecting...</span>
          </div>
        )}
        {paused && phase === "idle" && (
          <div className="flex items-center gap-1.5">
            <div className="w-2 h-2 rounded-full bg-muted" />
            <span className="text-xs text-muted-foreground">Paused</span>
          </div>
        )}
      </div>

      {/* ── Transcript / response text ─────────────────────────────────────── */}
      <div
        className="px-3 pb-2 text-sm font-mono min-h-[40px]"
        onClick={(e) => e.stopPropagation()}
        onKeyDown={(e) => e.stopPropagation()}
        role="presentation"
      >
        {phase === "speaking" && ttsAudio ? (
          <span className="text-foreground">{ttsAudio.text}</span>
        ) : segments.length > 0 ? (
          <WordHighlights segments={segments} />
        ) : transcript ? (
          <span className="text-foreground">{transcript}</span>
        ) : phase === "listening" ? (
          <span className="text-muted-foreground">Listening...</span>
        ) : null}
      </div>

      {/* ── Routing chips ──────────────────────────────────────────────────── */}
      {routingChips.length > 0 && (
        <div
          className="px-3 pb-2"
          onClick={(e) => e.stopPropagation()}
          onKeyDown={(e) => e.stopPropagation()}
          role="presentation"
        >
          <RoutingChips chips={routingChips} />
        </div>
      )}

      {/* ── Memory echo ────────────────────────────────────────────────────── */}
      {memoryEcho && (
        <div
          className="animate-in fade-in duration-300 flex items-center gap-1.5 text-xs text-muted-foreground
            px-3 pb-2"
          onClick={(e) => e.stopPropagation()}
          onKeyDown={(e) => e.stopPropagation()}
          role="presentation"
        >
          <span className="rounded bg-accent px-1.5 py-0.5 text-[10px] font-medium text-accent-foreground/70 flex-shrink-0">
            Mirror
          </span>
          <span className="italic">{memoryEcho}</span>
        </div>
      )}

      {/* ── TTS replay button ──────────────────────────────────────────────── */}
      {phase === "speaking" && ttsAudio && (
        <div
          className="px-3 pb-2"
          onClick={(e) => e.stopPropagation()}
          onKeyDown={(e) => e.stopPropagation()}
          role="presentation"
        >
          <button
            type="button"
            onClick={() => playTtsAudio(ttsAudio.base64, ttsAudio.sampleRate).catch(() => {})}
            className="text-xs text-muted-foreground hover:text-primary transition-colors"
            aria-label="Replay spoken response"
          >
            ↻ Replay
          </button>
        </div>
      )}

      {/* ── Continue button ────────────────────────────────────────────────── */}
      {continueAvailable && (
        <div
          className="px-3 pb-2"
          onClick={(e) => e.stopPropagation()}
          onKeyDown={(e) => e.stopPropagation()}
          role="presentation"
        >
          <button
            type="button"
            onClick={continueTts}
            className="w-full text-left text-xs text-primary hover:text-primary/80 transition-colors
              bg-primary/10 hover:bg-primary/20 px-3 py-1.5 rounded-lg font-medium"
          >
            ▸ Continue
          </button>
        </div>
      )}

      {/* ── Hint bar ───────────────────────────────────────────────────────── */}
      <div className="text-[10px] text-muted-foreground/40 text-center px-3 pb-3">
        {phase === "speaking"
          ? "Esc stop · ⌥⇧V close"
          : "Esc close · ⌥⇧V close"}
      </div>
    </div>
  );
}
