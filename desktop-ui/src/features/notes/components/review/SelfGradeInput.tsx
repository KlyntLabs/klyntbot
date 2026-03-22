import { ChevronRight } from "lucide-react";
import { useState } from "react";
import type { Flashcard, ReviewQuality } from "../../hooks/useFlashcards";

interface SelfGradeInputProps {
  card: Flashcard;
  onRate: (quality: ReviewQuality) => void;
}

const RATINGS: { quality: ReviewQuality; label: string }[] = [
  { quality: "again", label: "Again" },
  { quality: "hard", label: "Hard" },
  { quality: "good", label: "Good" },
  { quality: "easy", label: "Easy" },
];

export function SelfGradeInput({ card, onRate }: SelfGradeInputProps) {
  const [revealed, setRevealed] = useState(false);

  if (!revealed) {
    return (
      <button
        type="button"
        onClick={() => setRevealed(true)}
        className="flex items-center justify-center gap-1 text-[10px] px-3 py-2 rounded-md bg-white/[0.06] text-muted-foreground hover:text-foreground hover:bg-white/[0.08] w-full"
      >
        <ChevronRight size={10} />
        Show Answer
      </button>
    );
  }

  return (
    <div className="flex flex-col gap-2">
      {/* Revealed answer */}
      <div className="rounded-lg bg-white/[0.04] border border-border p-3">
        <p className="text-[11px] text-foreground whitespace-pre-wrap">{card.back}</p>
      </div>

      {/* Rating buttons */}
      <div className="flex gap-2 justify-center">
        {RATINGS.map(({ quality, label }) => (
          <button
            key={quality}
            type="button"
            onClick={() => onRate(quality)}
            className="text-[10px] px-3 py-1.5 rounded-md bg-white/[0.04] text-muted-foreground hover:text-foreground hover:bg-white/[0.08]"
          >
            {label}
          </button>
        ))}
      </div>
    </div>
  );
}
