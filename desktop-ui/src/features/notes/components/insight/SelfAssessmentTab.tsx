import { useInsightChat } from "@features/notes/hooks/useInsightChat";
import { ipc } from "@shared/hooks/useIpc";
import { BookOpen } from "lucide-react";
import { useCallback, useState } from "react";
import type { QuizQuestion, TabStatus } from "../../hooks/useInsightReview";
import { InsightChatInput } from "./InsightChatInput";
import { ScenarioChallenge, type ScenarioData } from "./ScenarioChallenge";

interface SelfAssessmentTabProps {
  status: TabStatus;
  questions: QuizQuestion[];
  quizState: {
    answers: Record<string, string>;
    revealed: Set<string>;
    score: number;
    total: number;
  };
  noteId: string | null;
  squadId?: string | null;
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
        <div key={i} className="island p-4 space-y-3">
          <div className="h-3 bg-bg-elevated rounded w-3/4" />
          <div className="h-3 bg-bg-elevated rounded w-full" />
          <div className="h-3 bg-bg-elevated rounded w-1/2" />
        </div>
      ))}
    </div>
  );
}

export function SelfAssessmentTab({
  status,
  questions,
  quizState,
  noteId,
  squadId,
  onAnswer,
  onReveal,
  onRevealAll,
  onSaveFlashcards,
}: SelfAssessmentTabProps) {
  const quizSummary = questions.map((q) => `Q: ${q.question} A: ${q.correctAnswer}`).join("\n");
  const chat = useInsightChat(noteId, "assessment", status === "done", squadId, quizSummary);
  const [scenario, setScenario] = useState<ScenarioData | null>(null);
  const [scenarioLoading, setScenarioLoading] = useState(false);
  const [scenarioError, setScenarioError] = useState(false);

  const answeredCount = Object.keys(quizState.answers).length;
  const showScenarioButton = answeredCount >= questions.length * 0.5 && questions.length > 0;

  const handleGenerateScenario = useCallback(async () => {
    if (!noteId) return;
    setScenarioError(false);
    setScenarioLoading(true);
    try {
      const data = await ipc<ScenarioData>("note_insight_generate_scenario", { noteId });
      setScenario(data);
    } catch {
      setScenarioError(true);
    } finally {
      setScenarioLoading(false);
    }
  }, [noteId]);

  if (status === "idle") {
    return (
      <p className="text-ui-xs text-fg-dim italic">
        Start an insight review to test your understanding
      </p>
    );
  }

  if ((status === "done" || status === "error") && questions.length === 0) {
    return (
      <div className="space-y-2">
        <p className="text-ui-xs text-fg-secondary">
          Quiz questions couldn't be generated. Try regenerating this tab.
        </p>
      </div>
    );
  }

  if (status === "loading") {
    return <SkeletonLoader />;
  }

  if (status === "error") {
    return (
      <p className="text-ui-xs text-status-danger">
        Failed to generate quiz questions. Try regenerating.
      </p>
    );
  }

  const anyRevealed = quizState.revealed.size > 0;

  return (
    <div className="space-y-3">
      {anyRevealed && (
        <div className="flex items-center justify-between mb-4">
          <div className="text-ui font-medium text-fg">
            Score: {quizState.score} / {quizState.total}
          </div>
          {quizState.revealed.size >= 3 && quizState.revealed.size < questions.length && (
            <button
              type="button"
              onClick={onRevealAll}
              className="text-ui-xs text-fg-secondary hover:text-fg transition-colors"
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
          <div key={q.id} className="island p-4 space-y-3">
            <div className="flex items-start justify-between gap-2">
              <span className="text-ui-sm text-fg leading-relaxed">{q.question}</span>
              <span className="text-[9px] px-1.5 py-0.5 rounded bg-control-hover text-fg-dim shrink-0">
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
                    "w-full flex items-center gap-2 px-3 py-1.5 rounded-md border text-left text-ui-xs transition-colors ";

                  if (isRevealed) {
                    if (isCorrect) {
                      choiceClass += "border-status-success/50 bg-status-success/10 text-fg";
                    } else if (isSelected && !isCorrect) {
                      choiceClass += "border-status-danger/50 bg-status-danger/10 text-fg";
                    } else {
                      choiceClass += "border-separator bg-bg-elevated text-fg-dim";
                    }
                  } else {
                    if (isSelected) {
                      choiceClass +=
                        "border-separator bg-control-hover text-fg hover:bg-control-hover";
                    } else {
                      choiceClass +=
                        "border-separator bg-bg-elevated text-fg-secondary hover:bg-control-hover";
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
                      <span className="text-ui-xs text-fg-dim shrink-0 w-4">{label}</span>
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
                className="w-full bg-bg-elevated border border-separator rounded-md px-3 py-1.5 text-ui-xs text-fg placeholder:text-fg-dim focus:border-purple-400/50 focus:outline-none disabled:opacity-50"
              />
            )}

            {userAnswer && !isRevealed && (
              <button
                type="button"
                onClick={() => onReveal(q.id)}
                className="text-ui-xs px-3 py-1 rounded-md bg-purple/20 text-purple hover:bg-purple/30 transition-colors"
              >
                Check
              </button>
            )}

            {isRevealed && (
              <div className="pt-2 border-t border-separator">
                <div className="text-ui-xs text-fg-secondary leading-relaxed">
                  <span className="font-medium text-fg-secondary">Correct: </span>
                  {q.correctAnswer}
                </div>
                <div className="text-ui-xs text-fg-dim mt-1">{q.explanation}</div>
              </div>
            )}
          </div>
        );
      })}

      {/* Scenario Challenge */}
      {showScenarioButton && !scenario && (
        <button
          type="button"
          onClick={handleGenerateScenario}
          disabled={scenarioLoading}
          className="flex items-center gap-1.5 text-ui-xs text-brand hover:text-brand/80 transition-colors disabled:text-fg-dim"
        >
          {scenarioLoading ? (
            <>
              <span className="size-3 border border-brand/40 border-t-brand rounded-full animate-spin" />
              Generating scenario...
            </>
          ) : (
            <>
              <BookOpen size={12} />
              Generate Applied Scenario
            </>
          )}
        </button>
      )}
      {scenarioError && !scenario && (
        <p className="text-ui-xs text-status-danger mt-1">
          Failed to generate scenario. Try again.
        </p>
      )}

      {scenario && <ScenarioChallenge scenario={scenario} />}

      {questions.length > 0 && Object.keys(quizState.answers).length >= questions.length * 0.5 && (
        <button
          type="button"
          onClick={() => onSaveFlashcards(`insight-${Date.now()}`)}
          className="w-full mt-4 flex items-center justify-center gap-2 px-4 py-2.5 rounded-lg bg-brand/20 text-brand hover:bg-brand/30 transition-colors text-ui-xs font-medium"
        >
          <BookOpen size={14} />
          Save as Flashcard Deck
        </button>
      )}

      {status === "done" && (
        <InsightChatInput {...chat} placeholder="Ask about this assessment..." />
      )}
    </div>
  );
}
