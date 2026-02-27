import { useState, useCallback } from 'react';
import { MessageSquare, Check } from 'lucide-react';
import type { PendingInteraction } from '../../../lib/hooks/useAgent';
import type { InteractionQuestion } from '../../../lib/types';

export function InteractionPanel({
  interaction,
  onRespond,
}: {
  interaction: PendingInteraction;
  onRespond: (requestId: string, response: Record<string, unknown>) => void;
}) {
  const [answers, setAnswers] = useState<Record<number, unknown>>({});

  const handleSubmit = useCallback(() => {
    const response: Record<string, unknown> = {};
    interaction.questions.forEach((q, idx) => {
      response[q.label] = answers[idx] ?? (q.type === 'yesNo' ? (q.default ?? false) : '');
    });
    onRespond(interaction.requestId, response);
  }, [interaction, answers, onRespond]);

  return (
    <div
      className="rounded-lg overflow-hidden"
      style={{
        backgroundColor: '#141414',
        border: '1px solid var(--codex-accent)',
      }}
    >
      <div
        className="px-4 py-3 flex items-center gap-2"
        style={{
          backgroundColor: 'var(--codex-bg-secondary)',
          borderBottom: '1px solid var(--codex-border)',
        }}
      >
        <MessageSquare
          className="w-4 h-4"
          strokeWidth={1.5}
          style={{ color: 'var(--codex-accent)' }}
        />
        <span
          className="text-[13px]"
          style={{ color: 'var(--codex-fg)', fontWeight: 500 }}
        >
          {interaction.title}
        </span>
      </div>

      <div className="px-4 py-3 space-y-4">
        {interaction.questions.map((q, idx) => (
          <InteractionField
            key={idx}
            question={q}
            value={answers[idx]}
            onChange={(val) => setAnswers((prev) => ({ ...prev, [idx]: val }))}
          />
        ))}

        <button
          onClick={handleSubmit}
          className="w-full py-2 rounded-md text-[13px] transition-colors"
          style={{
            backgroundColor: 'var(--codex-accent)',
            color: '#000',
            fontWeight: 500,
          }}
          onMouseEnter={(e) => { e.currentTarget.style.opacity = '0.9'; }}
          onMouseLeave={(e) => { e.currentTarget.style.opacity = '1'; }}
        >
          Submit
        </button>
      </div>
    </div>
  );
}

function InteractionField({
  question,
  value,
  onChange,
}: {
  question: InteractionQuestion;
  value: unknown;
  onChange: (val: unknown) => void;
}) {
  switch (question.type) {
    case 'freeText':
      return (
        <div>
          <label className="block text-[12px] mb-1.5" style={{ color: 'var(--codex-fg-subtle)' }}>
            {question.label}
          </label>
          <input
            type="text"
            value={(value as string) ?? ''}
            onChange={(e) => onChange(e.target.value)}
            placeholder={question.placeholder ?? ''}
            className="w-full px-3 py-2 rounded-md text-[13px] outline-none"
            style={{
              backgroundColor: 'var(--codex-bg-tertiary)',
              border: '1px solid var(--codex-border)',
              color: 'var(--codex-fg)',
            }}
          />
        </div>
      );

    case 'yesNo':
      return (
        <div>
          <label className="block text-[12px] mb-1.5" style={{ color: 'var(--codex-fg-subtle)' }}>
            {question.label}
          </label>
          <div className="flex gap-2">
            {(['Yes', 'No'] as const).map((opt) => {
              const selected =
                (opt === 'Yes' && value === true) ||
                (opt === 'No' && value === false);
              return (
                <button
                  key={opt}
                  onClick={() => onChange(opt === 'Yes')}
                  className="flex-1 py-1.5 rounded-md text-[12px] transition-colors"
                  style={{
                    backgroundColor: selected ? 'var(--codex-accent)' : 'var(--codex-bg-tertiary)',
                    color: selected ? '#000' : 'var(--codex-fg)',
                    border: `1px solid ${selected ? 'var(--codex-accent)' : 'var(--codex-border)'}`,
                    fontWeight: selected ? 500 : 400,
                  }}
                >
                  {opt}
                </button>
              );
            })}
          </div>
        </div>
      );

    case 'singleSelect':
      return (
        <div>
          <label className="block text-[12px] mb-1.5" style={{ color: 'var(--codex-fg-subtle)' }}>
            {question.label}
          </label>
          <div className="space-y-1">
            {question.options.map((opt) => (
              <button
                key={opt}
                onClick={() => onChange(opt)}
                className="w-full text-left px-3 py-1.5 rounded-md text-[12px] transition-colors"
                style={{
                  backgroundColor: value === opt ? 'var(--codex-accent-dim)' : 'var(--codex-bg-tertiary)',
                  color: value === opt ? 'var(--codex-accent)' : 'var(--codex-fg)',
                  border: `1px solid ${value === opt ? 'var(--codex-accent)' : 'var(--codex-border)'}`,
                }}
              >
                {opt}
              </button>
            ))}
          </div>
        </div>
      );

    case 'multiSelect':
      return (
        <div>
          <label className="block text-[12px] mb-1.5" style={{ color: 'var(--codex-fg-subtle)' }}>
            {question.label}
          </label>
          <div className="space-y-1">
            {question.options.map((opt) => {
              const selected = Array.isArray(value) && (value as string[]).includes(opt);
              return (
                <button
                  key={opt}
                  onClick={() => {
                    const current = (Array.isArray(value) ? value : []) as string[];
                    onChange(
                      selected
                        ? current.filter((v) => v !== opt)
                        : [...current, opt],
                    );
                  }}
                  className="w-full text-left px-3 py-1.5 rounded-md text-[12px] transition-colors flex items-center gap-2"
                  style={{
                    backgroundColor: selected ? 'var(--codex-accent-dim)' : 'var(--codex-bg-tertiary)',
                    color: selected ? 'var(--codex-accent)' : 'var(--codex-fg)',
                    border: `1px solid ${selected ? 'var(--codex-accent)' : 'var(--codex-border)'}`,
                  }}
                >
                  <div
                    className="w-3 h-3 rounded-sm border flex items-center justify-center"
                    style={{
                      borderColor: selected ? 'var(--codex-accent)' : 'var(--codex-border)',
                      backgroundColor: selected ? 'var(--codex-accent)' : 'transparent',
                    }}
                  >
                    {selected && <Check className="w-2 h-2" strokeWidth={3} style={{ color: '#000' }} />}
                  </div>
                  {opt}
                </button>
              );
            })}
          </div>
        </div>
      );
  }
}
