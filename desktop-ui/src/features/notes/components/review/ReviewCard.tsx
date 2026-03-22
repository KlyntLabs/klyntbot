import type { AnswerMode, GradeResult } from "@shared/types/notes";
import type { CardPhase } from "../../hooks/useActiveReview";
import type { Flashcard, ReviewQuality } from "../../hooks/useFlashcards";
import { CardFront } from "./CardFront";
import { GradeActions } from "./GradeActions";
import { GradeDisplay } from "./GradeDisplay";
import { SelfGradeInput } from "./SelfGradeInput";
import { TypedAnswerInput } from "./TypedAnswerInput";

interface ReviewCardProps {
  card: Flashcard;
  cardPhase: CardPhase;
  mode: AnswerMode;
  gradeResult: GradeResult | null;
  onSubmitAnswer: (answer: string) => void;
  onConfirmRating: (quality?: ReviewQuality) => void;
  onExplain: () => void;
  onSaveInsight: () => void;
  onJumpToSource: () => void;
  onSelfRate: (quality: ReviewQuality) => void;
}

export function ReviewCard({
  card,
  cardPhase,
  mode,
  gradeResult,
  onSubmitAnswer,
  onConfirmRating,
  onExplain,
  onSaveInsight,
  onJumpToSource,
  onSelfRate,
}: ReviewCardProps) {
  return (
    <div className="flex flex-col gap-3">
      {/* Card front — always visible */}
      <CardFront card={card} />

      {/* Answer input — shown during answering phase */}
      {cardPhase === "answering" &&
        (mode === "self_grade" ? (
          <SelfGradeInput card={card} onRate={onSelfRate} />
        ) : (
          <TypedAnswerInput onSubmit={onSubmitAnswer} />
        ))}

      {/* Grading spinner */}
      {cardPhase === "grading" && (
        <div className="flex items-center justify-center gap-2 py-3">
          <span className="text-[10px] text-dim">Grading…</span>
          <span className="w-3 h-3 rounded-full border border-accent/40 border-t-accent animate-spin" />
        </div>
      )}

      {/* Grade result + actions */}
      {(cardPhase === "graded" || cardPhase === "socratic" || cardPhase === "confirming") &&
        gradeResult && (
          <>
            <GradeDisplay result={gradeResult} />
            <GradeActions
              result={gradeResult}
              onConfirm={onConfirmRating}
              onExplain={onExplain}
              onSaveInsight={onSaveInsight}
              onJumpToSource={onJumpToSource}
            />
          </>
        )}
    </div>
  );
}
