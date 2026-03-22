import type { SessionStats } from "@shared/types/notes";
import { useEffect, useState } from "react";

// ── Types ────────────────────────────────────────────────────────────────────

export interface SessionSummaryProps {
  stats: SessionStats;
  onClose: () => void;
  onSaveInsight: () => void;
  onReviewWeak: () => void;
  onSaveReflection?: (text: string) => void;
}

// ── Score ring helpers ───────────────────────────────────────────────────────

const RING_RADIUS = 36;
const RING_CIRCUMFERENCE = 2 * Math.PI * RING_RADIUS;

function scoreRingColor(pct: number): string {
  if (pct >= 85) return "#34d399"; // green — success token
  if (pct >= 60) return "#fbbf24"; // yellow — warning token
  return "#f97316"; // orange
}

// ── Component ────────────────────────────────────────────────────────────────

export function SessionSummary({
  stats,
  onClose,
  onSaveInsight,
  onReviewWeak,
  onSaveReflection,
}: SessionSummaryProps) {
  const [beat1, setBeat1] = useState(false);
  const [beat2, setBeat2] = useState(false);
  const [beat3, setBeat3] = useState(false);
  const [showPulse, setShowPulse] = useState(false);
  const [reflectionText, setReflectionText] = useState("");
  const [pulseDismissed, setPulseDismissed] = useState(false);

  // ── Derived values ────────────────────────────────────────────────────────

  const avgScore = stats.cardsReviewed > 0 ? stats.totalScore / stats.cardsReviewed : 0;
  const avgPct = Math.round(avgScore * 100);

  const durationMin = Math.max(1, Math.round((Date.now() - stats.startTime) / 60_000));

  const modeCount = Object.keys(stats.modeUsage).length;
  const modeEntries = Object.entries(stats.modeUsage);

  const ringDash = RING_CIRCUMFERENCE * (avgPct / 100);

  // ── Pulse conditions ──────────────────────────────────────────────────────

  const shouldShowPulse =
    avgScore < 0.75 || stats.weakCards.length > 2 || stats.propagationCount > 5;

  // ── Timed beats ───────────────────────────────────────────────────────────

  // biome-ignore lint/correctness/useExhaustiveDependencies: beats should only trigger once on mount
  useEffect(() => {
    setBeat1(true);

    const t2 = setTimeout(() => setBeat2(true), 1000);
    const t3 = setTimeout(() => setBeat3(true), 2000);
    const tp = setTimeout(() => {
      if (shouldShowPulse) setShowPulse(true);
    }, 3000);

    return () => {
      clearTimeout(t2);
      clearTimeout(t3);
      clearTimeout(tp);
    };
  }, []);

  // ── Render ────────────────────────────────────────────────────────────────

  return (
    <div className="flex flex-col items-center gap-4 p-4 text-center">
      {/* Beat 1 — score ring */}
      {beat1 && (
        <div className="animate-in fade-in duration-500 flex flex-col items-center gap-2">
          <svg
            width="88"
            height="88"
            viewBox="0 0 88 88"
            role="img"
            aria-label={`Session score: ${avgPct}%`}
          >
            {/* Track */}
            <circle
              cx="44"
              cy="44"
              r={RING_RADIUS}
              fill="none"
              stroke="rgba(255,255,255,0.08)"
              strokeWidth="6"
            />
            {/* Progress arc */}
            <circle
              cx="44"
              cy="44"
              r={RING_RADIUS}
              fill="none"
              stroke={scoreRingColor(avgPct)}
              strokeWidth="6"
              strokeLinecap="round"
              strokeDasharray={`${ringDash} ${RING_CIRCUMFERENCE}`}
              strokeDashoffset={RING_CIRCUMFERENCE * 0.25}
              style={{ transition: "stroke-dasharray 800ms cubic-bezier(0.4,0,0.2,1)" }}
            />
            {/* Center label */}
            <text
              x="44"
              y="44"
              textAnchor="middle"
              dominantBaseline="central"
              fill={scoreRingColor(avgPct)}
              fontSize="16"
              fontWeight="600"
            >
              {avgPct}%
            </text>
          </svg>
          <p className="text-[11px] text-dim">Session score</p>
        </div>
      )}

      {/* Beat 2 — stats cards */}
      {beat2 && (
        <div className="animate-in fade-in duration-500 flex flex-col items-center gap-2 w-full">
          <div className="flex items-center gap-1.5 text-[11px] text-muted-foreground">
            <span className="font-medium text-foreground">{stats.cardsReviewed}</span>
            <span>cards</span>
            <span className="text-dim">·</span>
            <span className="font-medium text-foreground">{durationMin}</span>
            <span>min</span>
          </div>

          {modeCount > 1 && (
            <div className="flex flex-wrap justify-center gap-1.5">
              {modeEntries.map(([mode, usage]) => {
                const modePct =
                  usage.count > 0 ? Math.round((usage.totalScore / usage.count) * 100) : 0;
                return (
                  <span
                    key={mode}
                    className="text-[9px] px-1.5 py-0.5 rounded bg-white/[0.05] text-dim"
                  >
                    {mode.replace("_", " ")} {modePct}%
                  </span>
                );
              })}
            </div>
          )}
        </div>
      )}

      {/* Beat 3 — second-brain narrative */}
      {beat3 && (
        <div className="animate-in fade-in duration-500 flex flex-col items-center gap-1.5">
          {stats.propagationCount > 0 && (
            <p className="text-[10px] text-muted-foreground">
              <span className="text-foreground font-medium">{stats.propagationCount}</span>
              {" knowledge connections strengthened"}
            </p>
          )}
          {stats.weakCards.length > 0 && (
            <p className="text-[10px] text-muted-foreground">
              <span className="text-yellow-400 font-medium">{stats.weakCards.length}</span>
              {" weak spot"}
              {stats.weakCards.length !== 1 ? "s" : ""}
              {" surfaced"}
            </p>
          )}
          {stats.propagationCount === 0 && stats.weakCards.length === 0 && (
            <p className="text-[10px] text-dim">All cards held strong.</p>
          )}
        </div>
      )}

      {/* Reflection pulse */}
      {showPulse && !pulseDismissed && (
        <div className="animate-in fade-in duration-500 w-full rounded-lg bg-white/[0.04] border border-white/[0.07] p-3 flex flex-col gap-2 text-left">
          <p className="text-[10px] text-muted-foreground leading-snug">
            What felt different about today's answers?
          </p>
          <textarea
            value={reflectionText}
            onChange={(e) => setReflectionText(e.target.value)}
            placeholder="Optional reflection…"
            rows={2}
            className="w-full bg-transparent resize-none text-[10px] text-foreground placeholder:text-dim outline-none border-none"
          />
          <div className="flex justify-end">
            <button
              type="button"
              onClick={() => {
                if (reflectionText.trim() && onSaveReflection) {
                  onSaveReflection(reflectionText.trim());
                }
                setPulseDismissed(true);
              }}
              className="text-[9px] text-dim hover:text-muted-foreground"
            >
              {reflectionText.trim() ? "Save & close" : "Skip"}
            </button>
          </div>
        </div>
      )}

      {/* Actions — shown after beat 3 */}
      {beat3 && (
        <div className="animate-in fade-in duration-500 flex flex-col gap-1.5 w-full pt-1">
          {stats.weakCards.length > 0 && (
            <button
              type="button"
              onClick={onReviewWeak}
              className="w-full text-[10px] py-1.5 rounded-md bg-yellow-400/10 text-yellow-300 hover:bg-yellow-400/20 transition-colors"
            >
              Review {stats.weakCards.length} weak spot
              {stats.weakCards.length !== 1 ? "s" : ""}
            </button>
          )}
          <button
            type="button"
            onClick={onSaveInsight}
            className="w-full text-[10px] py-1.5 rounded-md bg-white/[0.06] text-muted-foreground hover:text-foreground transition-colors"
          >
            Save as insight
          </button>
          <button
            type="button"
            onClick={onClose}
            className="w-full text-[10px] py-1.5 rounded-md bg-white/[0.03] text-dim hover:text-muted-foreground transition-colors"
          >
            Done
          </button>
        </div>
      )}
    </div>
  );
}
