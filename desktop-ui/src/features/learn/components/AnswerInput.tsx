import { Loader2 } from "lucide-react";
import { useEffect, useRef, useState } from "react";

interface AnswerInputProps {
  onSubmit: (answer: string) => void;
  grading: boolean;
  disabled: boolean;
}

export function AnswerInput({ onSubmit, grading, disabled }: AnswerInputProps) {
  const [answer, setAnswer] = useState("");
  const inputRef = useRef<HTMLTextAreaElement>(null);

  useEffect(() => {
    if (!disabled && !grading) {
      inputRef.current?.focus();
    }
  }, [disabled, grading]);

  const handleKeyDown = (e: React.KeyboardEvent<HTMLTextAreaElement>) => {
    if (e.key === "Enter" && !e.shiftKey) {
      e.preventDefault();
      if (answer.trim() && !grading && !disabled) {
        onSubmit(answer.trim());
      }
    }
  };

  return (
    <div className="flex flex-col items-center gap-2 w-full max-w-md mx-auto">
      <textarea
        ref={inputRef}
        value={answer}
        onChange={(e) => setAnswer(e.target.value)}
        onKeyDown={handleKeyDown}
        disabled={grading || disabled}
        placeholder="Type your answer..."
        rows={2}
        className="glass-input w-full px-4 py-3 text-sm text-fg resize-none disabled:opacity-50"
      />
      <button
        type="button"
        onClick={() => {
          if (answer.trim() && !grading && !disabled) {
            onSubmit(answer.trim());
          }
        }}
        disabled={!answer.trim() || grading || disabled}
        className="glass-button px-6 py-2 text-sm text-fg disabled:opacity-50 flex items-center gap-2"
      >
        {grading ? (
          <>
            <Loader2 size={14} className="animate-spin" />
            Grading...
          </>
        ) : (
          <>
            Submit
            <span className="text-ui-xs text-fg-secondary">Enter</span>
          </>
        )}
      </button>
    </div>
  );
}
