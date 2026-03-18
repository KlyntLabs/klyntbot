import { ipc } from "@shared/hooks/useIpc";
import { cn } from "@shared/lib/utils";
import type { Answer, AnswerValue, InteractionRequest, Question } from "@shared/types";
import { Check, X } from "lucide-react";
import { useCallback, useEffect, useRef, useState } from "react";

interface InteractionCardProps {
  sessionKey: string;
  requestId: string;
  request: InteractionRequest;
  onSubmitted: () => void;
}

function optionButtonClass(isSelected: boolean, isFocused: boolean) {
  return cn(
    "rounded-lg border text-[12px] font-light transition-colors",
    isSelected
      ? "border-brand bg-brand/10 text-foreground"
      : isFocused
        ? "border-border bg-muted text-foreground"
        : "border-transparent bg-muted/50 text-muted-foreground hover:bg-muted",
  );
}

export function InteractionCard({
  sessionKey,
  requestId,
  request,
  onSubmitted,
}: InteractionCardProps) {
  const [activeTab, setActiveTab] = useState(0);
  const [answers, setAnswers] = useState<Record<string, AnswerValue>>(() => {
    const init: Record<string, AnswerValue> = {};
    for (const q of request.questions) {
      if (q.answer_type.type === "yes_no" && q.answer_type.default != null) {
        init[q.id] = { type: "yes_no", answer: q.answer_type.default };
      }
    }
    return init;
  });
  const [focusIndex, setFocusIndex] = useState(0);
  const [submitting, setSubmitting] = useState(false);
  const cardRef = useRef<HTMLFieldSetElement>(null);

  const question = request.questions[activeTab];
  const optionCount =
    question.answer_type.type === "single_select" || question.answer_type.type === "multi_select"
      ? question.answer_type.options.length
      : question.answer_type.type === "yes_no"
        ? 2
        : 0;

  // Focus the card on mount for keyboard navigation
  useEffect(() => {
    cardRef.current?.focus();
  }, []);

  // Reset focus when switching tabs
  useEffect(() => {
    setFocusIndex(0);
  }, []);

  const setAnswer = useCallback((questionId: string, value: AnswerValue) => {
    setAnswers((prev) => ({ ...prev, [questionId]: value }));
  }, []);

  const handleSelect = useCallback(
    (q: Question, index: number) => {
      if (q.answer_type.type === "single_select") {
        setAnswer(q.id, { type: "selected", value: q.answer_type.options[index].value });
      } else if (q.answer_type.type === "multi_select") {
        const val = q.answer_type.options[index].value;
        setAnswers((prev) => {
          const current = prev[q.id];
          const values = current?.type === "multi_selected" ? [...current.values] : [];
          const idx = values.indexOf(val);
          if (idx >= 0) values.splice(idx, 1);
          else values.push(val);
          return { ...prev, [q.id]: { type: "multi_selected", values } };
        });
      }
    },
    [setAnswer],
  );

  const isLastQuestion = activeTab === request.questions.length - 1;

  const handleSubmit = useCallback(async () => {
    setSubmitting(true);
    const answerList: Answer[] = request.questions.map((q) => ({
      question_id: q.id,
      value: answers[q.id] ?? { type: "skipped" as const },
    }));
    try {
      await ipc("chat_respond_interaction", {
        sessionKey,
        requestId,
        response: { Completed: answerList },
      });
      onSubmitted();
    } catch {
      setSubmitting(false);
    }
  }, [sessionKey, requestId, request, answers, onSubmitted]);

  const handleCancel = useCallback(async () => {
    setSubmitting(true);
    try {
      await ipc("chat_respond_interaction", {
        sessionKey,
        requestId,
        response: "Cancelled",
      });
      onSubmitted();
    } catch {
      setSubmitting(false);
    }
  }, [sessionKey, requestId, onSubmitted]);

  // Ref so advance() always calls the latest handleSubmit (with fresh answers)
  const handleSubmitRef = useRef(handleSubmit);
  handleSubmitRef.current = handleSubmit;

  const advance = useCallback(() => {
    if (isLastQuestion) {
      // setTimeout lets React flush state updates before submitting
      setTimeout(() => handleSubmitRef.current(), 0);
    } else {
      setActiveTab((t) => t + 1);
    }
  }, [isLastQuestion]);

  const handleKeyDown = useCallback(
    (e: React.KeyboardEvent) => {
      // Don't intercept most keys when typing in form inputs (allow Tab through for navigation)
      const tag = (e.target as HTMLElement).tagName;
      if ((tag === "INPUT" || tag === "TEXTAREA") && e.key !== "Tab") return;

      switch (e.key) {
        case "ArrowDown":
        case "j":
          e.preventDefault();
          setFocusIndex((i) => Math.min(i + 1, optionCount - 1));
          break;
        case "ArrowUp":
        case "k":
          e.preventDefault();
          setFocusIndex((i) => Math.max(i - 1, 0));
          break;
        case "Enter":
        case " ":
          e.preventDefault();
          if (question.answer_type.type === "single_select") {
            handleSelect(question, focusIndex);
            advance();
          } else if (question.answer_type.type === "multi_select") {
            if (e.key === "Enter" && answers[question.id]) {
              // Enter with existing selection → advance
              advance();
            } else {
              handleSelect(question, focusIndex);
            }
          } else if (question.answer_type.type === "yes_no") {
            setAnswer(question.id, { type: "yes_no", answer: focusIndex === 0 });
            advance();
          }
          break;
        case "Tab":
          e.preventDefault();
          if (e.shiftKey) {
            // Shift+Tab → go back (stop at first question)
            setActiveTab((t) => Math.max(t - 1, 0));
          } else if (isLastQuestion) {
            // Tab on last question → submit
            advance();
          } else {
            // Tab → next question
            setActiveTab((t) => t + 1);
          }
          break;
      }
    },
    [optionCount, question, focusIndex, handleSelect, setAnswer, advance, answers, isLastQuestion],
  );

  return (
    <fieldset
      ref={cardRef}
      onKeyDown={handleKeyDown}
      className="flex justify-start border-none p-0 m-0"
    >
      <div className="w-full max-w-[85%] rounded-xl bg-accent border border-border overflow-hidden">
        {/* Header */}
        <div className="px-4 pt-3 pb-2 text-[11px] font-light text-muted-foreground">
          Klynt is asking…
        </div>

        {/* Tabs (if >1 question) */}
        {request.questions.length > 1 && (
          <div className="flex gap-1 px-4 pb-2">
            {request.questions.map((q, i) => (
              <button
                type="button"
                key={q.id}
                onClick={() => setActiveTab(i)}
                className={`px-3 py-1 rounded-md text-[11px] font-light transition-colors ${
                  i === activeTab
                    ? "bg-muted text-foreground"
                    : "text-muted-foreground hover:text-foreground hover:bg-muted"
                }`}
              >
                {q.title}
              </button>
            ))}
          </div>
        )}

        {/* Question body */}
        <div className="px-4 pb-3">
          <p className="text-[13px] font-light text-foreground mb-3">{question.text}</p>

          {/* Single / Multi select */}
          {(question.answer_type.type === "single_select" ||
            question.answer_type.type === "multi_select") && (
            <div className="space-y-1.5">
              {question.answer_type.options.map((opt, i) => {
                const currentAnswer = answers[question.id];
                const isSelected =
                  question.answer_type.type === "single_select"
                    ? currentAnswer?.type === "selected" && currentAnswer.value === opt.value
                    : currentAnswer?.type === "multi_selected" &&
                      currentAnswer.values.includes(opt.value);

                return (
                  <button
                    type="button"
                    key={opt.value}
                    onClick={() => handleSelect(question, i)}
                    className={cn(
                      "w-full text-left px-3 py-2",
                      optionButtonClass(isSelected, focusIndex === i),
                    )}
                  >
                    <div className="text-[12px] font-light">{opt.label}</div>
                    {opt.description && (
                      <div className="text-[11px] font-light text-muted-foreground mt-0.5">
                        {opt.description}
                      </div>
                    )}
                  </button>
                );
              })}
            </div>
          )}

          {/* Yes / No */}
          {question.answer_type.type === "yes_no" && (
            <div className="flex gap-2">
              {["Yes", "No"].map((label, i) => {
                const val = i === 0;
                const current = answers[question.id];
                const isSelected = current?.type === "yes_no" && current.answer === val;

                return (
                  <button
                    type="button"
                    key={label}
                    onClick={() => setAnswer(question.id, { type: "yes_no", answer: val })}
                    className={cn("flex-1 py-2", optionButtonClass(isSelected, focusIndex === i))}
                  >
                    {label}
                  </button>
                );
              })}
            </div>
          )}

          {/* Free text */}
          {question.answer_type.type === "free_text" &&
            (() => {
              const v = answers[question.id];
              const textContent = v?.type === "text" ? v.content : "";
              return (
                <input
                  type="text"
                  value={textContent}
                  onChange={(e) =>
                    setAnswer(question.id, { type: "text", content: e.target.value })
                  }
                  onKeyDown={(e) => {
                    if (e.key === "Enter") {
                      e.preventDefault();
                      advance();
                    }
                  }}
                  placeholder={question.answer_type.placeholder ?? ""}
                  className="w-full bg-muted text-foreground text-[12px] font-light px-3 py-2 rounded-lg border border-border"
                />
              );
            })()}
        </div>

        {/* Footer: Submit / Cancel */}
        <div className="flex items-center justify-end gap-2 px-4 py-2 border-t border-border">
          <button
            type="button"
            onClick={handleCancel}
            disabled={submitting}
            className="flex items-center gap-1.5 px-3 py-1.5 rounded-lg text-[11px] font-light text-muted-foreground hover:text-foreground hover:bg-muted transition-colors disabled:opacity-50"
          >
            <X className="w-3 h-3" strokeWidth={1.5} />
            Cancel
          </button>
          <button
            type="button"
            onClick={handleSubmit}
            disabled={submitting}
            className="flex items-center gap-1.5 px-3 py-1.5 rounded-lg bg-brand text-[11px] font-light hover:bg-brand/90 transition-colors disabled:opacity-50"
          >
            <Check className="w-3 h-3" strokeWidth={2} />
            Submit
          </button>
        </div>
      </div>
    </fieldset>
  );
}
