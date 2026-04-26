import type { GradeResult } from "@shared/types/notes";
import type { ReviewQuality } from "../../hooks/useFlashcards";

interface GradeActionsProps {
  result: GradeResult;
  onConfirm: (quality?: ReviewQuality) => void;
  onExplain: () => void;
  onSaveInsight: () => void;
  onJumpToSource: () => void;
}

const OVERRIDE_RATINGS: { key: string; quality: ReviewQuality; label: string }[] = [
  { key: "1", quality: "again", label: "1:again" },
  { key: "2", quality: "hard", label: "2:hard" },
  { key: "3", quality: "good", label: "3:good" },
  { key: "4", quality: "easy", label: "4:easy" },
];

export function GradeActions({
  result,
  onConfirm,
  onExplain,
  onSaveInsight,
  onJumpToSource,
}: GradeActionsProps) {
  const suggestedLabel = result.suggestedRating
    ? result.suggestedRating.charAt(0).toUpperCase() + result.suggestedRating.slice(1)
    : "Good";

  return (
    <div className="flex flex-col gap-2.5">
      {/* Primary confirm button */}
      <button
        type="button"
        onClick={() => onConfirm()}
        className="w-full text-[11px] font-medium px-3 py-2 rounded-lg bg-accent/20 text-accent hover:bg-accent/30 text-center"
      >
        Confirm: {suggestedLabel} (Enter)
      </button>

      {/* Rating overrides */}
      <div className="flex items-center gap-1.5 justify-center">
        {OVERRIDE_RATINGS.map(({ key, quality, label }) => (
          <button
            key={key}
            type="button"
            onClick={() => onConfirm(quality)}
            className="text-[9px] px-2 py-1 rounded-md bg-white/[0.04] text-muted-foreground hover:text-foreground hover:bg-white/[0.08]"
          >
            {label}
          </button>
        ))}
      </div>

      {/* Action links */}
      <div className="flex items-center gap-3 justify-center">
        <button
          type="button"
          onClick={onExplain}
          className="text-[9px] text-dim hover:text-foreground"
        >
          (e) Explain
        </button>
        <button
          type="button"
          onClick={onSaveInsight}
          className="text-[9px] text-dim hover:text-foreground"
        >
          (s) Save insight
        </button>
        <button
          type="button"
          onClick={onJumpToSource}
          className="text-[9px] text-dim hover:text-foreground"
        >
          (j) Source note
        </button>
      </div>
    </div>
  );
}
