interface PracticeProgressHeaderProps {
  currentIndex: number;
  totalSegments: number;
  suggestedFocus: string;
  averageScore?: number; // 0-100
  onExit: () => void;
}

export function PracticeProgressHeader({
  currentIndex,
  totalSegments,
  suggestedFocus,
  averageScore,
  onExit,
}: PracticeProgressHeaderProps) {
  const progressPct = totalSegments > 0 ? ((currentIndex + 1) / totalSegments) * 100 : 0;
  const scoreDisplay = averageScore != null ? `${Math.round(averageScore)}%` : "";
  const centerLabel = `Sentence ${currentIndex + 1}/${totalSegments}${scoreDisplay ? ` \u00b7 ${scoreDisplay}` : ""}`;

  return (
    <div className="h-9 bg-bg-elevated border-b border-separator flex items-center justify-between px-3 shrink-0">
      {/* Left: suggested focus */}
      <span className="text-brand text-ui-sm truncate max-w-[30%]">Focus: {suggestedFocus}</span>

      {/* Center: progress indicator with fill bar */}
      <div className="relative flex items-center justify-center h-5 min-w-[160px] rounded-full overflow-hidden">
        <div
          className="absolute inset-0 bg-brand/10 rounded-full origin-left transition-[width] duration-300"
          style={{ width: `${progressPct}%` }}
        />
        <span className="relative text-ui-sm text-brand z-10">{centerLabel}</span>
      </div>

      {/* Right: exit button */}
      <button
        type="button"
        onClick={onExit}
        className="text-ui-sm text-brand bg-control-hover hover:bg-brand/20 hover:text-brand px-3 py-1 rounded-md transition-colors"
      >
        Exit &amp; Save
      </button>
    </div>
  );
}
