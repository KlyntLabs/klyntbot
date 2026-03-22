import type { AnswerMode, GradeResult } from "@shared/types/notes";
import type { CardPhase } from "../../hooks/useActiveReview";
import type { Flashcard, ReviewQuality } from "../../hooks/useFlashcards";
import { CardFront } from "./CardFront";
import { ClozeInput } from "./ClozeInput";
import { GradeActions } from "./GradeActions";
import { GradeDisplay } from "./GradeDisplay";
import { MultipleChoiceInput } from "./MultipleChoiceInput";
import { SelfGradeInput } from "./SelfGradeInput";
import { SocraticPanel } from "./SocraticPanel";
import { TypedAnswerInput } from "./TypedAnswerInput";
import { VoiceInput } from "./VoiceInput";

interface ReviewCardProps {
  card: Flashcard;
  cardPhase: CardPhase;
  mode: AnswerMode;
  gradeResult: GradeResult | null;
  lastAnswer: string;
  onSubmitAnswer: (answer: string) => void;
  onConfirmRating: (quality?: ReviewQuality) => void;
  onExplain: () => void;
  onSaveInsight: () => void;
  onJumpToSource: () => void;
  onSelfRate: (quality: ReviewQuality) => void;
}

function AnswerInput({
  card,
  mode,
  onSubmit,
  onSelfRate,
}: {
  card: Flashcard;
  mode: AnswerMode;
  onSubmit: (answer: string) => void;
  onSelfRate: (quality: ReviewQuality) => void;
}) {
  switch (mode) {
    case "self_grade":
      return <SelfGradeInput card={card} onRate={onSelfRate} />;
    case "multiple_choice":
      return <MultipleChoiceInput correctAnswer={card.back} distractors={[]} onSelect={onSubmit} />;
    case "cloze_fill": {
      const clozeText = card.cardType === "cloze" ? card.front : card.front;
      return <ClozeInput clozeText={clozeText} onSubmit={onSubmit} />;
    }
    case "voice":
      return <VoiceInput onSubmit={onSubmit} />;
    case "typed":
    case "auto":
    default:
      return <TypedAnswerInput onSubmit={onSubmit} />;
  }
}

export function ReviewCard({
  card,
  cardPhase,
  mode,
  gradeResult,
  lastAnswer,
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
      {cardPhase === "answering" && (
        <AnswerInput card={card} mode={mode} onSubmit={onSubmitAnswer} onSelfRate={onSelfRate} />
      )}

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
            {cardPhase === "socratic" && (
              <SocraticPanel
                cardId={card.id}
                userAnswer={lastAnswer}
                gradeExplanation={gradeResult.explanation ?? ""}
              />
            )}
          </>
        )}
    </div>
  );
}
