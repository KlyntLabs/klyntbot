import { ThinkingDots } from "@shared/ui/ThinkingDots";
import { Play, RotateCcw, X } from "lucide-react";
import { createPortal } from "react-dom";

// ── Types ────────────────────────────────────────────────

interface PreviewSegment {
  index: number;
  text: string;
  type: string;
  suggestedFocus: string;
}

export interface PracticePreviewProps {
  segments: PreviewSegment[];
  estimatedMins: number;
  existingSession?: { currentIndex: number; averageScore?: number } | null;
  loading?: boolean;
  onStart: () => void;
  onResume: () => void;
  onCancel: () => void;
}

// ── Helpers ──────────────────────────────────────────────

function deriveFocusSummary(segments: PreviewSegment[]): string {
  const counts: Record<string, number> = {};
  for (const s of segments) {
    counts[s.suggestedFocus] = (counts[s.suggestedFocus] ?? 0) + 1;
  }
  const sorted = Object.entries(counts).sort((a, b) => b[1] - a[1]);
  const top = sorted.slice(0, 2).map(([f]) => f);
  return top.length > 0 ? top.join(" & ") : "";
}

function typeDot(type: string): string {
  switch (type) {
    case "heading":
      return "bg-purple";
    case "pattern":
      return "bg-amber-400";
    case "cultural":
      return "bg-emerald-400";
    default:
      return "bg-white/20";
  }
}

// ── Component ────────────────────────────────────────────

export function PracticePreview({
  segments,
  estimatedMins,
  existingSession,
  loading,
  onStart,
  onResume,
  onCancel,
}: PracticePreviewProps) {
  const hasExistingSession = existingSession != null;
  const focusSummary = deriveFocusSummary(segments);

  return createPortal(
    <div className="fixed inset-0 flex items-center justify-center z-50">
      {/* Backdrop */}
      {/* biome-ignore lint/a11y/noStaticElementInteractions: backdrop dismiss */}
      <div
        className="absolute inset-0 bg-black/60 backdrop-blur-[2px]"
        onClick={onCancel}
        onKeyDown={() => {}}
      />

      <div
        className="relative max-w-[520px] w-full mx-4 rounded-xl overflow-hidden animate-[glass-appear_0.2s_ease-out]"
        style={{
          background: "var(--ds-glass-bg-strong)",
          border: "1px solid rgba(255,255,255,0.08)",
          boxShadow:
            "0 24px 80px rgba(0,0,0,0.5), 0 0 1px rgba(255,255,255,0.1) inset, 0 0 40px rgba(167,139,250,0.06)",
        }}
      >
        {loading ? (
          <div className="flex flex-col items-center justify-center py-16 gap-3">
            <ThinkingDots />
            <p className="text-ui-xs text-fg-secondary">Preparing your session...</p>
          </div>
        ) : hasExistingSession ? (
          <ResumeView
            segments={segments}
            existingSession={existingSession}
            onResume={onResume}
            onStart={onStart}
            onCancel={onCancel}
          />
        ) : (
          <FirstVisitView
            segments={segments}
            estimatedMins={estimatedMins}
            focusSummary={focusSummary}
            onStart={onStart}
            onCancel={onCancel}
          />
        )}
      </div>
    </div>,
    document.body,
  );
}

// ── First-visit view ─────────────────────────────────────

function FirstVisitView({
  segments,
  estimatedMins,
  focusSummary,
  onStart,
  onCancel,
}: {
  segments: PreviewSegment[];
  estimatedMins: number;
  focusSummary: string;
  onStart: () => void;
  onCancel: () => void;
}) {
  return (
    <>
      {/* Header */}
      <div className="px-6 pt-5 pb-3 flex items-start justify-between">
        <div>
          <p className="text-purple text-ui-xs uppercase tracking-[0.12em] font-medium">
            Practice Session
          </p>
          <div className="flex items-baseline gap-2 mt-1.5">
            <span className="text-brand text-base font-semibold">{segments.length} units</span>
            <span className="text-fg-secondary text-sm">~{estimatedMins} min</span>
          </div>
          {focusSummary && <p className="text-fg-secondary text-ui-sm mt-0.5">{focusSummary}</p>}
        </div>
        <button
          type="button"
          onClick={onCancel}
          className="text-fg-secondary hover:text-brand p-1 -m-1 rounded transition-colors"
        >
          <X size={14} strokeWidth={1.5} />
        </button>
      </div>

      {/* Segment list */}
      <div
        className="mx-4 mb-4 rounded-lg overflow-hidden"
        style={{ background: "rgba(0,0,0,0.2)" }}
      >
        <div className="max-h-[300px] overflow-y-auto py-1.5">
          {segments.map((seg) => (
            <div
              key={seg.index}
              className="flex items-center gap-2.5 py-1.5 px-3.5 hover:bg-white/[0.03] transition-colors"
            >
              <span className="text-fg-secondary/50 text-ui-xs w-5 text-right shrink-0 tabular-nums">
                {seg.index + 1}
              </span>
              <span className={`w-1.5 h-1.5 rounded-full shrink-0 ${typeDot(seg.type)}`} />
              <span className="text-ui-sm text-fg-secondary truncate flex-1 min-w-0">
                {seg.text}
              </span>
            </div>
          ))}
        </div>
      </div>

      {/* Actions */}
      <div className="px-6 pb-5 flex items-center gap-3">
        <button
          type="button"
          onClick={onCancel}
          className="text-ui-sm text-fg-secondary hover:text-brand transition-colors"
        >
          Edit segments
        </button>
        <button
          type="button"
          onClick={onStart}
          className="flex-1 flex items-center justify-center gap-1.5 py-2.5 rounded-lg text-ui-sm font-medium text-white transition-all hover:brightness-110"
          style={{
            background: "linear-gradient(135deg, #a78bfa 0%, #7c3aed 100%)",
            boxShadow: "0 0 20px rgba(167,139,250,0.25)",
          }}
        >
          <Play size={11} fill="currentColor" />
          Start Practice
        </button>
      </div>
    </>
  );
}

// ── Resume view ──────────────────────────────────────────

function ResumeView({
  segments,
  existingSession,
  onResume,
  onStart,
  onCancel,
}: {
  segments: PreviewSegment[];
  existingSession: { currentIndex: number; averageScore?: number };
  onResume: () => void;
  onStart: () => void;
  onCancel: () => void;
}) {
  const total = segments.length;
  const current = existingSession.currentIndex;
  const pct = Math.round((current / total) * 100);
  const scoreText =
    existingSession.averageScore != null ? `${Math.round(existingSession.averageScore)}%` : null;

  return (
    <>
      {/* Header */}
      <div className="px-6 pt-5 pb-3 flex items-start justify-between">
        <div>
          <p className="text-purple text-ui-xs uppercase tracking-[0.12em] font-medium">
            Resume Session
          </p>
          <div className="flex items-baseline gap-2 mt-1.5">
            <span className="text-brand text-base font-semibold">
              {current}/{total} completed
            </span>
            {scoreText && <span className="text-fg-secondary text-ui-sm">{scoreText}</span>}
          </div>
        </div>
        <button
          type="button"
          onClick={onCancel}
          className="text-fg-secondary hover:text-brand p-1 -m-1 rounded transition-colors"
        >
          <X size={14} strokeWidth={1.5} />
        </button>
      </div>

      {/* Progress bar */}
      <div className="mx-6 mb-5">
        <div
          className="h-1 rounded-full overflow-hidden"
          style={{ background: "rgba(255,255,255,0.06)" }}
        >
          <div
            className="h-full rounded-full bg-purple transition-all"
            style={{ width: `${pct}%` }}
          />
        </div>
        <p className="text-ui-xs text-fg-secondary mt-1.5">Pick up where you left off</p>
      </div>

      {/* Actions */}
      <div className="px-6 pb-5 flex flex-col gap-2.5">
        <button
          type="button"
          onClick={onResume}
          className="w-full flex items-center justify-center gap-1.5 py-2.5 rounded-lg text-ui-sm font-medium text-white transition-all hover:brightness-110"
          style={{
            background: "linear-gradient(135deg, #a78bfa 0%, #7c3aed 100%)",
            boxShadow: "0 0 20px rgba(167,139,250,0.25)",
          }}
        >
          <Play size={11} fill="currentColor" />
          Resume
        </button>
        <div className="flex items-center justify-center gap-4">
          <button
            type="button"
            onClick={onStart}
            className="flex items-center gap-1 text-ui-xs text-fg-secondary hover:text-brand transition-colors"
          >
            <RotateCcw size={10} />
            Start fresh
          </button>
          <button
            type="button"
            onClick={onCancel}
            className="text-ui-xs text-fg-secondary hover:text-brand transition-colors"
          >
            Cancel
          </button>
        </div>
      </div>
    </>
  );
}
