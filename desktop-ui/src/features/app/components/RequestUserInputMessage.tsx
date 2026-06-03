import { useEffect, useMemo, useState } from "react";
import type { RequestUserInputRequest, RequestUserInputResponse } from "@/types";

type RequestUserInputMessageProps = {
  requests: RequestUserInputRequest[];
  activeThreadId: string | null;
  activeWorkspaceId?: string | null;
  onSubmit: (request: RequestUserInputRequest, response: RequestUserInputResponse) => void;
};

type SelectionState = Record<string, number | null>;
type NotesState = Record<string, string>;

export function RequestUserInputMessage({
  requests,
  activeThreadId,
  activeWorkspaceId,
  onSubmit,
}: RequestUserInputMessageProps) {
  const activeRequests = useMemo(
    () =>
      requests.filter((request) => {
        if (!activeThreadId) {
          return false;
        }
        if (request.params.thread_id !== activeThreadId) {
          return false;
        }
        if (activeWorkspaceId && request.workspace_id !== activeWorkspaceId) {
          return false;
        }
        return true;
      }),
    [requests, activeThreadId, activeWorkspaceId],
  );
  const activeRequest = activeRequests[0];
  const [selections, setSelections] = useState<SelectionState>({});
  const [notes, setNotes] = useState<NotesState>({});

  useEffect(() => {
    if (!activeRequest) {
      setSelections({});
      setNotes({});
      return;
    }
    const nextSelections: SelectionState = {};
    const nextNotes: NotesState = {};
    activeRequest.params.questions.forEach((question, index) => {
      const key = question.id || `question-${index}`;
      nextSelections[key] = null;
      nextNotes[key] = "";
    });
    setSelections(nextSelections);
    setNotes(nextNotes);
  }, [activeRequest]);

  if (!activeRequest) {
    return null;
  }

  const { questions } = activeRequest.params;
  const totalRequests = activeRequests.length;

  const buildAnswers = () => {
    const answers: RequestUserInputResponse["answers"] = {};
    questions.forEach((question, index) => {
      if (!question.id) {
        return;
      }
      const answerList: string[] = [];
      const key = question.id || `question-${index}`;
      const selectedIndex = selections[key];
      const options = question.options ?? [];
      const hasOptions = options.length > 0;
      if (hasOptions && selectedIndex !== null) {
        const selected = options[selectedIndex];
        const selectedValue = selected?.label?.trim() || selected?.description?.trim() || "";
        if (selectedValue) {
          answerList.push(selectedValue);
        }
      }
      const note = (notes[key] ?? "").trim();
      if (note) {
        if (hasOptions) {
          answerList.push(`user_note: ${note}`);
        } else {
          answerList.push(note);
        }
      }
      answers[question.id] = { answers: answerList };
    });
    return answers;
  };

  const handleSelect = (questionId: string, optionIndex: number) => {
    setSelections((current) => ({ ...current, [questionId]: optionIndex }));
  };

  const handleNotesChange = (questionId: string, value: string) => {
    setNotes((current) => ({ ...current, [questionId]: value }));
  };

  const handleSubmit = () => {
    onSubmit(activeRequest, { answers: buildAnswers() });
  };

  return (
    <div className="message items-start">
      <fieldset className="bubble w-[min(520px,72%)] max-w-full bg-surface-card-strong border border-border-stronger rounded-2xl p-2.5 px-3 flex flex-col gap-2" aria-label="User input requested">
        <div className="flex justify-between items-baseline gap-2">
          <div className="text-ui-sm font-semibold text-text-strong">Input requested</div>
          {totalRequests > 1 ? (
            <div className="text-ui-xs text-text-subtle">{`Request 1 of ${totalRequests}`}</div>
          ) : null}
        </div>
        <div className="grid gap-2">
          {questions.length ? (
            questions.map((question, index) => {
              const questionId = question.id || `question-${index}`;
              const selectedIndex = selections[questionId];
              const options = question.options ?? [];
              const notePlaceholder = question.isOther
                ? "Type your answer (optional)"
                : options.length
                  ? "Add notes (optional)"
                  : "Type your answer (optional)";
              return (
                <section key={questionId} className="grid gap-1">
                  {question.header ? (
                    <div className="text-ui-2xs uppercase tracking-[0.06em] text-text-faint">{question.header}</div>
                  ) : null}
                  <div className="text-ui-sm text-text-primary">{question.question}</div>
                  {options.length ? (
                    <div className="grid gap-1">
                      {options.map((option, optionIndex) => (
                        <button
                          key={`${questionId}-${option.label}-${option.description ?? ""}`}
                          type="button"
                          className={`request-user-input-option${
                            selectedIndex === optionIndex ? " is-selected" : ""
                          }`}
                          onClick={() => handleSelect(questionId, optionIndex)}
                        >
                          <div className="text-ui-sm font-semibold text-text-strong">{option.label}</div>
                          {option.description ? (
                            <div className="text-ui-xs text-text-subtle">
                              {option.description}
                            </div>
                          ) : null}
                        </button>
                      ))}
                    </div>
                  ) : null}
                  <textarea
                    className="rounded-xl border border-border-subtle bg-surface-card-muted text-text-strong p-1.5 px-2 text-ui-sm leading-snug resize-y outline-none focus:outline focus:outline-2 focus:outline-[rgba(77,163,255,0.35)] focus:outline-offset-1"
                    placeholder={notePlaceholder}
                    value={notes[questionId] ?? ""}
                    onChange={(event) => handleNotesChange(questionId, event.target.value)}
                    rows={2}
                  />
                </section>
              );
            })
          ) : (
            <div className="text-ui-sm text-text-muted">No questions provided.</div>
          )}
        </div>
        <div className="flex justify-end gap-2">
          <button type="button" className="primary" onClick={handleSubmit}>
            Submit
          </button>
        </div>
      </fieldset>
    </div>
  );
}
