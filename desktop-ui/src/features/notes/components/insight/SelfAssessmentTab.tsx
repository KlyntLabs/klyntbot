import { BookOpen } from "lucide-react";
import type { QuizQuestion, TabStatus } from "../../hooks/useInsightReview";

interface SelfAssessmentTabProps {
  status: TabStatus;
  questions: QuizQuestion[];
  quizState: {
    answers: Record<string, string>;
    revealed: Set<string>;
    score: number;
    total: number;
  };
  onAnswer: (questionId: string, answer: string) => void;
  onReveal: (questionId: string) => void;
  onRevealAll: () => void;
  onSaveFlashcards: (deckName: string) => void;
}

const CHOICE_LABELS = ["A", "B", "C", "D"];

function SkeletonLoader() {
  return (
    <div className="space-y-3 animate-pulse">
      {[0, 1, 2].map((i) => (
        <div
          key={i}
          className="rounded-lg bg-card border border-border-subtle p-4 space-y-3"
        >
          <div className="h-3 bg-card rounded w-3/4" />
          <div className="h-3 bg-card rounded w-full" />
          <div className="h-3 bg-card rounded w-1/2" />
        </div>
      ))}
    </div>
  );
}

export function SelfAssessmentTab({
  status,
  questions,
  quizState,
  onAnswer,
  onReveal,
  onRevealAll,
  onSaveFlashcards,
}: SelfAssessmentTabProps) {
  if (status === "idle") {
    return (
      <p className="text-[11px] text-dim italic">
        Start an insight review to test your understanding
      </p>
    );
  }

  if (status === "loading") {
    return <SkeletonLoader />;
  }

  if (status === "error") {
    return (
      <p className="text-[11px] text-destructive">
        Failed to generate quiz questions. Try regenerating.
      </p>
    );
  }

  const anyRevealed = quizState.revealed.size > 0;

  return (
    <div className="space-y-3">
      {anyRevealed && (
        <div className="flex items-center justify-between mb-4">
          <div className="text-[13px] font-medium text-foreground">
            Score: {quizState.score} / {quizState.total}
          </div>
          {quizState.revealed.size >= 3 && quizState.revealed.size < questions.length && (
            <button
              type="button"
              onClick={onRevealAll}
              className="text-[10px] text-muted-foreground hover:text-foreground transition-colors"
            >
              Reveal all answers
            </button>
          )}
        </div>
      )}

      {questions.map((q) => {
        const isRevealed = quizState.revealed.has(q.id);
        const userAnswer = quizState.answers[q.id];

        return (
          <div
            key={q.id}
            className="rounded-lg bg-card border border-border-subtle p-4 space-y-3"
          >
            <div className="flex items-start justify-between gap-2">
              <span className="text-[12px] text-foreground leading-relaxed">{q.question}</span>
              <span className="text-[9px] px-1.5 py-0.5 rounded bg-accent text-dim shrink-0">
                {q.difficulty}
              </span>
            </div>

            {q.type === "multiple_choice" && q.choices != null ? (
              <div className="space-y-1.5">
                {q.choices.map((choice, idx) => {
                  const label = CHOICE_LABELS[idx] ?? String(idx + 1);
                  const isSelected = userAnswer === choice;
                  const isCorrect = choice === q.correctAnswer;

                  let choiceClass =
                    "w-full flex items-center gap-2 px-3 py-1.5 rounded-md border text-left text-[11px] transition-colors ";

                  if (isRevealed) {
                    if (isCorrect) {
                      choiceClass += "border-success/50 bg-success/10 text-foreground";
                    } else if (isSelected && !isCorrect) {
                      choiceClass += "border-destructive/50 bg-destructive/10 text-foreground";
                    } else {
                      choiceClass += "border-border-subtle bg-card text-dim";
                    }
                  } else {
                    if (isSelected) {
                      choiceClass +=
                        "border-border bg-muted text-foreground hover:bg-muted";
                    } else {
                      choiceClass +=
                        "border-border-subtle bg-card text-muted-foreground hover:bg-accent";
                    }
                  }

                  return (
                    <button
                      key={choice}
                      type="button"
                      disabled={isRevealed}
                      onClick={() => onAnswer(q.id, choice)}
                      className={choiceClass}
                    >
                      <span className="text-[10px] text-dim shrink-0 w-4">{label}</span>
                      <span>{choice}</span>
                    </button>
                  );
                })}
              </div>
            ) : (
              <input
                type="text"
                value={quizState.answers[q.id] ?? ""}
                onChange={(e) => onAnswer(q.id, e.target.value)}
                placeholder="Type your answer..."
                disabled={isRevealed}
                className="w-full bg-card border border-border rounded-md px-3 py-1.5 text-[11px] text-foreground placeholder:text-dim focus:border-purple-400/50 focus:outline-none disabled:opacity-50"
              />
            )}

            {userAnswer && !isRevealed && (
              <button
                type="button"
                onClick={() => onReveal(q.id)}
                className="text-[10px] px-3 py-1 rounded-md bg-purple/20 text-purple hover:bg-purple/30 transition-colors"
              >
                Check
              </button>
            )}

            {isRevealed && (
              <div className="pt-2 border-t border-border-subtle">
                <div className="text-[10px] text-muted-foreground leading-relaxed">
                  <span className="font-medium text-muted-foreground">Correct: </span>
                  {q.correctAnswer}
                </div>
                <div className="text-[10px] text-dim mt-1">{q.explanation}</div>
              </div>
            )}
          </div>
        );
      })}

      {questions.length > 0 && Object.keys(quizState.answers).length >= questions.length * 0.5 && (
        <button
          type="button"
          onClick={() => onSaveFlashcards(`insight-${Date.now()}`)}
          className="w-full mt-4 flex items-center justify-center gap-2 px-4 py-2.5 rounded-lg bg-brand/20 text-brand hover:bg-brand/30 transition-colors text-[11px] font-medium"
        >
          <BookOpen size={14} />
          Save as Flashcard Deck
        </button>
      )}
    </div>
  );
}
