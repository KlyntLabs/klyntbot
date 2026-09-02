import { invalidateQueries } from "@shared/hooks/useQuery";
import { ThinkingDots } from "@shared/ui/ThinkingDots";
import { ArrowLeft, CheckCircle } from "lucide-react";
import { useEffect, useRef, useState } from "react";
import { Link, useParams } from "react-router";
import { useReviewSession } from "../hooks/useReviewSession";
import { CardRenderer } from "./CardRenderer";
import { RatingButtons } from "./RatingButtons";

export function FocusedReview() {
  const { topicId } = useParams<{ topicId?: string }>();

  const [loading, setLoading] = useState(true);
  const isRating = useRef(false);

  const session = useReviewSession();
  const {
    cards,
    current,
    currentIndex,
    revealed,
    done,
    reveal,
    rate: sessionRate,
    startReview,
  } = session;

  // Start review on mount — use deck-specific fetch when topicId is present
  // biome-ignore lint/correctness/useExhaustiveDependencies: only run on mount/topicId change
  useEffect(() => {
    startReview(topicId ?? undefined).then(() => setLoading(false));
  }, [topicId]);

  const rate = async (quality: "again" | "hard" | "good" | "easy") => {
    if (isRating.current) return;
    isRating.current = true;
    try {
      await sessionRate(quality);
    } finally {
      isRating.current = false;
    }
  };

  // Keyboard shortcuts
  useEffect(() => {
    const handler = (e: KeyboardEvent) => {
      const tag = (e.target as HTMLElement)?.tagName;
      if (tag === "INPUT" || tag === "TEXTAREA") return;

      if (e.key === " " && !revealed && current) {
        e.preventDefault();
        reveal();
        return;
      }

      if (revealed && current) {
        const ratingMap: Record<string, "again" | "hard" | "good" | "easy"> = {
          "1": "again",
          "2": "hard",
          "3": "good",
          "4": "easy",
        };
        const quality = ratingMap[e.key];
        if (quality) {
          e.preventDefault();
          rate(quality);
        }
      }
    };
    document.addEventListener("keydown", handler);
    return () => document.removeEventListener("keydown", handler);
    // biome-ignore lint/correctness/useExhaustiveDependencies: rate uses a ref guard and is stable in behavior; React Compiler handles memoization
  }, [revealed, reveal, rate, current]);

  const handleFinish = () => {
    invalidateQueries("flashcard_");
    invalidateQueries("knowledge_");
  };

  if (loading) {
    return (
      <div className="flex-1 flex items-center justify-center">
        <ThinkingDots />
      </div>
    );
  }

  // No cards
  if (cards.length === 0) {
    return (
      <div className="flex-1 flex items-center justify-center">
        <div className="text-center space-y-3">
          <CheckCircle size={32} className="mx-auto text-emerald-400" strokeWidth={1.5} />
          <p className="text-sm text-fg-secondary">No cards due for review</p>
          <Link
            to="/learn"
            onClick={handleFinish}
            className="glass-button px-4 py-2 text-sm text-fg inline-block"
          >
            Back to Learning Hub
          </Link>
        </div>
      </div>
    );
  }

  // Done
  if (done) {
    return (
      <div className="flex-1 flex items-center justify-center">
        <div className="text-center space-y-3 animate-[fade-in-up_0.3s_ease-out]">
          <CheckCircle size={40} className="mx-auto text-emerald-400" strokeWidth={1.5} />
          <h2 className="text-xl font-semibold text-fg">Review Complete!</h2>
          <p className="text-sm text-fg-secondary">
            You reviewed {cards.length} card{cards.length !== 1 ? "s" : ""}
          </p>
          <Link
            to="/learn"
            onClick={handleFinish}
            className="glass-button px-5 py-2.5 text-sm text-fg inline-block"
          >
            Back to Learning Hub
          </Link>
        </div>
      </div>
    );
  }

  const progress = (currentIndex / cards.length) * 100;
  const heading = topicId ? `Reviewing: ${topicId}` : "Review All";

  return (
    <div className="flex-1 flex flex-col">
      {/* Top bar */}
      <div className="flex items-center justify-between px-4 py-3 border-b border-separator">
        <Link
          to="/learn"
          onClick={handleFinish}
          className="flex items-center gap-1.5 text-sm text-fg-secondary hover:text-fg transition-colors"
        >
          <ArrowLeft size={16} strokeWidth={1.5} />
          <span className="text-ui-sm">Back</span>
        </Link>

        <span className="text-ui font-medium text-fg">{heading}</span>

        <span className="text-ui-sm text-fg-secondary tabular-nums">
          Card {currentIndex + 1} of {cards.length}
        </span>
      </div>

      {/* Card area */}
      <div className="flex-1 flex flex-col items-center justify-center px-6 py-8">
        {current && (
          <div className="w-full max-w-lg">
            <div className="glass-card p-8">
              <CardRenderer card={current} revealed={revealed} />
            </div>
          </div>
        )}
      </div>

      {/* Bottom: Show Answer or Rating */}
      <div className="px-6 pb-4 space-y-3">
        {!revealed ? (
          <div className="flex justify-center">
            <button
              type="button"
              onClick={reveal}
              className="glass-button px-8 py-2.5 text-sm text-fg"
            >
              Show Answer
              <span className="text-ui-xs text-fg-secondary ml-2">Space</span>
            </button>
          </div>
        ) : (
          <RatingButtons onRate={rate} />
        )}
      </div>

      {/* Progress bar */}
      <div className="h-1 bg-white/[0.04]">
        <div
          className="h-full bg-brand transition-all duration-300 ease-out"
          style={{ width: `${progress}%` }}
        />
      </div>
    </div>
  );
}
