import { useState, useCallback, useEffect, useRef } from 'react';
import { Check, X } from 'lucide-react';
import { ipc } from '../../hooks/useIpc';
import type { InteractionRequest, Question, AnswerValue, Answer } from '../../lib/types';

interface InteractionCardProps {
  sessionKey: string;
  requestId: string;
  request: InteractionRequest;
  onSubmitted: () => void;
}

export function InteractionCard({ sessionKey, requestId, request, onSubmitted }: InteractionCardProps) {
  const [activeTab, setActiveTab] = useState(0);
  const [answers, setAnswers] = useState<Map<string, AnswerValue>>(() => {
    const map = new Map<string, AnswerValue>();
    for (const q of request.questions) {
      if (q.answer_type.type === 'yes_no' && q.answer_type.default != null) {
        map.set(q.id, { type: 'yes_no', answer: q.answer_type.default });
      }
    }
    return map;
  });
  const [focusIndex, setFocusIndex] = useState(0);
  const [submitting, setSubmitting] = useState(false);
  const cardRef = useRef<HTMLDivElement>(null);

  const question = request.questions[activeTab];
  const optionCount = question.answer_type.type === 'single_select' || question.answer_type.type === 'multi_select'
    ? question.answer_type.options.length
    : question.answer_type.type === 'yes_no' ? 2 : 0;

  // Focus the card on mount for keyboard navigation
  useEffect(() => { cardRef.current?.focus(); }, []);

  // Reset focus when switching tabs
  useEffect(() => { setFocusIndex(0); }, [activeTab]);

  const setAnswer = useCallback((questionId: string, value: AnswerValue) => {
    setAnswers((prev) => new Map(prev).set(questionId, value));
  }, []);

  const handleSelect = useCallback((q: Question, index: number) => {
    if (q.answer_type.type === 'single_select') {
      setAnswer(q.id, { type: 'selected', value: q.answer_type.options[index].value });
    } else if (q.answer_type.type === 'multi_select') {
      const current = answers.get(q.id);
      const values = current?.type === 'multi_selected' ? [...current.values] : [];
      const val = q.answer_type.options[index].value;
      const idx = values.indexOf(val);
      if (idx >= 0) values.splice(idx, 1);
      else values.push(val);
      setAnswer(q.id, { type: 'multi_selected', values });
    }
  }, [answers, setAnswer]);

  const handleSubmit = useCallback(async () => {
    setSubmitting(true);
    // Build answer list with snake_case question_id to match Rust serde
    const answerList: Answer[] = request.questions.map((q) => ({
      question_id: q.id,
      value: answers.get(q.id) ?? { type: 'skipped' as const },
    }));
    try {
      await ipc('chat_respond_interaction', {
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
      await ipc('chat_respond_interaction', {
        sessionKey,
        requestId,
        response: 'Cancelled',
      });
      onSubmitted();
    } catch {
      setSubmitting(false);
    }
  }, [sessionKey, requestId, onSubmitted]);

  const handleKeyDown = useCallback((e: React.KeyboardEvent) => {
    switch (e.key) {
      case 'ArrowDown':
      case 'j':
        e.preventDefault();
        setFocusIndex((i) => Math.min(i + 1, optionCount - 1));
        break;
      case 'ArrowUp':
      case 'k':
        e.preventDefault();
        setFocusIndex((i) => Math.max(i - 1, 0));
        break;
      case 'Enter':
      case ' ':
        e.preventDefault();
        if (question.answer_type.type === 'single_select' || question.answer_type.type === 'multi_select') {
          handleSelect(question, focusIndex);
        } else if (question.answer_type.type === 'yes_no') {
          setAnswer(question.id, { type: 'yes_no', answer: focusIndex === 0 });
        }
        break;
      case 'Tab':
        if (request.questions.length > 1) {
          e.preventDefault();
          setActiveTab((t) => e.shiftKey
            ? (t - 1 + request.questions.length) % request.questions.length
            : (t + 1) % request.questions.length
          );
        }
        break;
    }
  }, [optionCount, question, focusIndex, handleSelect, setAnswer, request.questions]);

  return (
    <div
      ref={cardRef}
      tabIndex={0}
      onKeyDown={handleKeyDown}
      className="flex justify-start focus:outline-none"
    >
      <div className="w-full max-w-[85%] rounded-xl bg-surface-base border border-border overflow-hidden">
        {/* Header */}
        <div className="px-4 pt-3 pb-2 text-[11px] font-light text-muted">
          Klynt is asking…
        </div>

        {/* Tabs (if >1 question) */}
        {request.questions.length > 1 && (
          <div className="flex gap-1 px-4 pb-2">
            {request.questions.map((q, i) => (
              <button
                key={q.id}
                onClick={() => setActiveTab(i)}
                className={`px-3 py-1 rounded-md text-[11px] font-light transition-colors ${
                  i === activeTab
                    ? 'bg-surface-highest text-primary'
                    : 'text-muted hover:text-secondary hover:bg-surface-raised'
                }`}
              >
                {q.title}
              </button>
            ))}
          </div>
        )}

        {/* Question body */}
        <div className="px-4 pb-3">
          <p className="text-[13px] font-light text-primary mb-3">{question.text}</p>

          {/* Single / Multi select */}
          {(question.answer_type.type === 'single_select' || question.answer_type.type === 'multi_select') && (
            <div className="space-y-1.5">
              {question.answer_type.options.map((opt, i) => {
                const currentAnswer = answers.get(question.id);
                const isSelected = question.answer_type.type === 'single_select'
                  ? currentAnswer?.type === 'selected' && currentAnswer.value === opt.value
                  : currentAnswer?.type === 'multi_selected' && currentAnswer.values.includes(opt.value);
                const isFocused = focusIndex === i;

                return (
                  <button
                    key={opt.value}
                    onClick={() => handleSelect(question, i)}
                    className={`w-full text-left px-3 py-2 rounded-lg border transition-colors ${
                      isSelected
                        ? 'border-brand bg-brand/10 text-primary'
                        : isFocused
                          ? 'border-border bg-surface-raised text-primary'
                          : 'border-transparent bg-surface-raised/50 text-secondary hover:bg-surface-raised'
                    }`}
                  >
                    <div className="text-[12px] font-light">{opt.label}</div>
                    {opt.description && (
                      <div className="text-[11px] font-light text-muted mt-0.5">{opt.description}</div>
                    )}
                  </button>
                );
              })}
            </div>
          )}

          {/* Yes / No */}
          {question.answer_type.type === 'yes_no' && (
            <div className="flex gap-2">
              {['Yes', 'No'].map((label, i) => {
                const val = i === 0;
                const current = answers.get(question.id);
                const isSelected = current?.type === 'yes_no' && current.answer === val;
                const isFocused = focusIndex === i;

                return (
                  <button
                    key={label}
                    onClick={() => setAnswer(question.id, { type: 'yes_no', answer: val })}
                    className={`flex-1 py-2 rounded-lg border text-[12px] font-light transition-colors ${
                      isSelected
                        ? 'border-brand bg-brand/10 text-primary'
                        : isFocused
                          ? 'border-border bg-surface-raised text-primary'
                          : 'border-transparent bg-surface-raised/50 text-secondary hover:bg-surface-raised'
                    }`}
                  >
                    {label}
                  </button>
                );
              })}
            </div>
          )}

          {/* Free text */}
          {question.answer_type.type === 'free_text' && (
            <input
              type="text"
              value={(answers.get(question.id) as { type: 'text'; content: string } | undefined)?.content ?? ''}
              onChange={(e) => setAnswer(question.id, { type: 'text', content: e.target.value })}
              placeholder={question.answer_type.placeholder ?? ''}
              className="w-full bg-surface-raised text-primary text-[12px] font-light px-3 py-2 rounded-lg border border-border focus:outline-none focus:border-brand"
            />
          )}
        </div>

        {/* Footer: Submit / Cancel */}
        <div className="flex items-center justify-end gap-2 px-4 py-2 border-t border-border">
          <button
            onClick={handleCancel}
            disabled={submitting}
            className="flex items-center gap-1.5 px-3 py-1.5 rounded-lg text-[11px] font-light text-muted hover:text-secondary hover:bg-surface-raised transition-colors disabled:opacity-50"
          >
            <X className="w-3 h-3" strokeWidth={1.5} />
            Cancel
          </button>
          <button
            onClick={handleSubmit}
            disabled={submitting}
            className="flex items-center gap-1.5 px-3 py-1.5 rounded-lg bg-brand text-[11px] font-light hover:bg-brand/90 transition-colors disabled:opacity-50"
          >
            <Check className="w-3 h-3" strokeWidth={2} />
            Submit
          </button>
        </div>
      </div>
    </div>
  );
}
