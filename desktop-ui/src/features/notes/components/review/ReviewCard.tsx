import { ipc } from "@shared/hooks/useIpc";
import type { AnswerMode, GradeResult } from "@shared/types/notes";
import { useEffect, useState } from "react";
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
  propagationCount?: number;
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
  lastAnswer,
  propagationCount = 0,
  onSubmitAnswer,
  onConfirmRating,
  onExplain,
  onSaveInsight,
  onJumpToSource,
  onSelfRate,
}: ReviewCardProps) {
  const [distractors, setDistractors] = useState<string[]>([]);

  // biome-ignore lint/correctness/useExhaustiveDependencies: cardPhase intentionally excluded to prevent re-firing on phase transitions
  useEffect(() => {
    if (mode !== "multiple_choice" || cardPhase !== "answering") return;
    setDistractors([]);
    ipc<{ distractors: string[]; cached: boolean }>("flashcard_generate_distractors", {
      cardId: card.id,
      count: 3,
    })
      .then((res) => setDistractors(res.distractors))
      .catch(() => setDistractors([]));
  }, [mode, card.id]);

  function renderAnswerInput() {
    switch (mode) {
      case "self_grade":
        return <SelfGradeInput card={card} onRate={onSelfRate} />;
      case "multiple_choice":
        if (distractors.length === 0) {
          return (
            <div className="flex items-center gap-2 py-3 justify-center">
              <span className="text-2xs text-dim">Generating options...</span>
              <span className="size-3 rounded-full border border-accent/40 border-t-accent animate-spin" />
            </div>
          );
        }
        return (
          <MultipleChoiceInput
            correctAnswer={card.back}
            distractors={distractors}
            onSelect={onSubmitAnswer}
          />
        );
      case "cloze_fill": {
        const clozeText = card.front;
        return <ClozeInput clozeText={clozeText} onSubmit={onSubmitAnswer} />;
      }
      case "voice":
        return <VoiceInput onSubmit={onSubmitAnswer} />;
      case "typed":
      case "auto":
      default:
        return <TypedAnswerInput onSubmit={onSubmitAnswer} />;
    }
  }

  return (
    <div className="flex flex-col gap-3">
      {/* Card front — always visible */}
      <CardFront card={card} />

      {/* Answer input — shown during answering phase */}
      {cardPhase === "answering" && renderAnswerInput()}

      {/* Grading spinner */}
      {cardPhase === "grading" && (
        <div className="flex items-center justify-center gap-2 py-3">
          <span className="text-2xs text-dim">Grading…</span>
          <span className="size-3 rounded-full border border-accent/40 border-t-accent animate-spin" />
        </div>
      )}

      {/* Grade result + actions */}
      {(cardPhase === "graded" || cardPhase === "socratic" || cardPhase === "confirming") &&
        gradeResult && (
          <>
            <GradeDisplay result={gradeResult} propagationCount={propagationCount} />
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
