import type { ReviewQuality } from "../../notes/hooks/useFlashcards";

interface RatingButtonsProps {
  onRate: (quality: ReviewQuality) => void;
  suggestedRating?: string;
}

const ratings: {
  quality: ReviewQuality;
  label: string;
  key: string;
  color: string;
  hoverBg: string;
}[] = [
  {
    quality: "again",
    label: "Again",
    key: "1",
    color: "text-red-400",
    hoverBg: "hover:bg-red-400/10",
  },
  {
    quality: "hard",
    label: "Hard",
    key: "2",
    color: "text-amber-400",
    hoverBg: "hover:bg-amber-400/10",
  },
  {
    quality: "good",
    label: "Good",
    key: "3",
    color: "text-emerald-400",
    hoverBg: "hover:bg-emerald-400/10",
  },
  {
    quality: "easy",
    label: "Easy",
    key: "4",
    color: "text-blue-400",
    hoverBg: "hover:bg-blue-400/10",
  },
];

export function RatingButtons({ onRate, suggestedRating }: RatingButtonsProps) {
  return (
    <div className="flex items-center gap-2 justify-center">
      {ratings.map((r) => {
        const isSuggested = suggestedRating === r.quality;
        return (
          <button
            key={r.quality}
            type="button"
            onClick={() => onRate(r.quality)}
            className={`glass-button px-4 py-2.5 flex flex-col items-center gap-0.5 min-w-[72px] transition-all duration-200 ${r.hoverBg} ${
              isSuggested ? "ring-1 ring-white/20" : ""
            }`}
          >
            <span className={`text-sm font-medium ${r.color}`}>{r.label}</span>
            <span className="text-2xs text-muted-foreground">
              {isSuggested ? `${r.key} \u2022 suggested` : r.key}
            </span>
          </button>
        );
      })}
    </div>
  );
}
