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
  activeColor: string;
  borderColor: string;
}[] = [
  {
    quality: "again",
    label: "Again",
    key: "1",
    color: "text-red-400",
    activeColor: "bg-red-400/10",
    borderColor: "border-red-400/30",
  },
  {
    quality: "hard",
    label: "Hard",
    key: "2",
    color: "text-amber-400",
    activeColor: "bg-amber-400/10",
    borderColor: "border-amber-400/30",
  },
  {
    quality: "good",
    label: "Good",
    key: "3",
    color: "text-emerald-400",
    activeColor: "bg-emerald-400/10",
    borderColor: "border-emerald-400/30",
  },
  {
    quality: "easy",
    label: "Easy",
    key: "4",
    color: "text-blue-400",
    activeColor: "bg-blue-400/10",
    borderColor: "border-blue-400/30",
  },
];

export function RatingButtons({ onRate, suggestedRating }: RatingButtonsProps) {
  return (
    <div className="flex items-stretch gap-2 justify-center animate-[fade-in-up_0.15s_ease-out]">
      {ratings.map((r) => {
        const isSuggested = suggestedRating === r.quality;
        return (
          <button
            key={r.quality}
            type="button"
            onClick={() => onRate(r.quality)}
            className={`flex-1 max-w-[100px] flex flex-col items-center gap-0.5 py-2.5 px-3 rounded-xl border transition-all duration-200 ${
              isSuggested
                ? `${r.activeColor} ${r.borderColor}`
                : "border-white/[0.08] hover:border-white/15 bg-white/[0.03] hover:bg-white/[0.06]"
            }`}
          >
            <span className={`text-[13px] font-medium ${r.color}`}>{r.label}</span>
            <span className="text-[10px] text-dim tabular-nums">
              {isSuggested ? `${r.key} · suggested` : r.key}
            </span>
          </button>
        );
      })}
    </div>
  );
}
