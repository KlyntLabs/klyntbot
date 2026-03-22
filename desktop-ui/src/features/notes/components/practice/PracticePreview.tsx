import { Loader2, Play, RotateCcw } from "lucide-react";
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

const TYPE_COLORS: Record<string, string> = {
  heading: "bg-purple/15 text-purple",
  sentence: "bg-info/15 text-info",
  pattern: "bg-warning/15 text-warning",
  cultural: "bg-success/15 text-success",
};

function typeBadgeClass(type: string): string {
  return TYPE_COLORS[type] ?? "bg-muted text-muted-foreground";
}

function deriveFocusSummary(segments: PreviewSegment[]): string {
  const counts: Record<string, number> = {};
  for (const s of segments) {
    const focus = s.suggestedFocus;
    counts[focus] = (counts[focus] ?? 0) + 1;
  }
  const sorted = Object.entries(counts).sort((a, b) => b[1] - a[1]);
  const top = sorted.slice(0, 2).map(([f]) => f);
  if (top.length === 0) return "";
  return `Focus: ${top.join(" & ")}`;
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
    <div className="fixed inset-0 flex items-center justify-center bg-black/40 z-50">
      {/* Backdrop dismiss */}
      <div
        className="absolute inset-0"
        onClick={onCancel}
        role="presentation"
        onKeyDown={() => {}}
      />

      <div className="relative glass-panel max-w-md w-full mx-4 rounded-2xl p-6 animate-[glass-appear_0.2s_ease-out]">
        {loading ? (
          <div className="flex flex-col items-center justify-center py-12 gap-3">
            <Loader2 size={24} className="text-brand animate-spin" strokeWidth={1.5} />
            <p className="text-sm text-muted-foreground">Preparing your session...</p>
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
    <div className="flex flex-col gap-4">
      {/* Header */}
      <p className="text-brand text-xs uppercase tracking-widest font-medium">
        Your Personal Language Gym
      </p>

      {/* Main stat */}
      <div>
        <p className="text-primary text-lg font-semibold">
          {segments.length} unit{segments.length !== 1 ? "s" : ""} &middot; ~{estimatedMins} min
        </p>
        {focusSummary && <p className="text-muted-foreground text-sm mt-0.5">{focusSummary}</p>}
      </div>

      {/* Scrollable unit list */}
      <div className="max-h-[240px] overflow-y-auto space-y-1 pr-1">
        {segments.map((seg) => (
          <div
            key={seg.index}
            className="flex items-center gap-2.5 py-1.5 px-2 rounded-lg hover:bg-white/[0.03] transition-colors"
          >
            <span className="text-dim text-xs w-5 text-right flex-shrink-0 tabular-nums">
              {seg.index + 1}
            </span>
            <span className="text-sm text-foreground truncate flex-1 min-w-0">{seg.text}</span>
            <span
              className={`text-[10px] px-1.5 py-0.5 rounded-md flex-shrink-0 ${typeBadgeClass(seg.type)}`}
            >
              {seg.type}
            </span>
          </div>
        ))}
      </div>

      {/* Actions */}
      <div className="flex flex-col items-center gap-3 pt-2">
        <div className="flex items-center gap-3 w-full">
          <button
            type="button"
            onClick={onCancel}
            className="text-sm text-muted-foreground hover:text-foreground transition-colors"
          >
            Edit segments
          </button>
          <button
            type="button"
            onClick={onStart}
            className="flex-1 flex items-center justify-center gap-2 px-5 py-2.5 rounded-xl bg-brand text-white text-sm font-medium shadow-[0_0_20px_var(--brand-glow)] hover:bg-brand-hover transition-colors"
          >
            <Play size={14} />
            Start Practice
          </button>
        </div>
        <button
          type="button"
          onClick={onCancel}
          className="text-xs text-muted-foreground hover:text-foreground transition-colors"
        >
          Cancel
        </button>
      </div>
    </div>
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
  const scoreText =
    existingSession.averageScore != null
      ? ` \u00B7 ${Math.round(existingSession.averageScore)}% last time`
      : "";

  return (
    <div className="flex flex-col gap-4">
      {/* Header */}
      <p className="text-brand text-xs uppercase tracking-widest font-medium">
        Session in Progress
      </p>

      {/* Resume banner */}
      <div className="glass-card p-4 flex items-center gap-3">
        <div className="flex-1 min-w-0">
          <p className="text-sm text-foreground font-medium">
            Resume {current}/{total}
            {scoreText}
          </p>
          <p className="text-xs text-muted-foreground mt-0.5">Pick up where you left off</p>
        </div>
      </div>

      {/* Actions */}
      <div className="flex flex-col items-center gap-3">
        <button
          type="button"
          onClick={onResume}
          className="w-full flex items-center justify-center gap-2 px-5 py-2.5 rounded-xl bg-brand text-white text-sm font-medium shadow-[0_0_20px_var(--brand-glow)] hover:bg-brand-hover transition-colors"
        >
          <Play size={14} />
          Resume
        </button>
        <button
          type="button"
          onClick={onStart}
          className="flex items-center gap-1.5 text-sm text-muted-foreground hover:text-foreground transition-colors"
        >
          <RotateCcw size={12} />
          Start fresh
        </button>
        <button
          type="button"
          onClick={onCancel}
          className="text-xs text-muted-foreground hover:text-foreground transition-colors"
        >
          Cancel
        </button>
      </div>
    </div>
  );
}
