import { useEffect, useRef, useState } from "react";

interface TypedAnswerInputProps {
  onSubmit: (answer: string) => void;
  disabled?: boolean;
  initialValue?: string;
}

export function TypedAnswerInput({ onSubmit, disabled, initialValue }: TypedAnswerInputProps) {
  // Use a ref to track value without causing re-renders on every keystroke
  const valueRef = useRef(initialValue ?? "");
  const [charCount, setCharCount] = useState(initialValue?.length ?? 0);
  const textareaRef = useRef<HTMLTextAreaElement>(null);

  // Auto-focus on mount
  useEffect(() => {
    textareaRef.current?.focus();
  }, []);

  const handleKeyDown = (e: React.KeyboardEvent<HTMLTextAreaElement>) => {
    if (e.key === "Enter" && !e.shiftKey) {
      e.preventDefault();
      const val = valueRef.current.trim();
      if (val && !disabled) {
        onSubmit(val);
      }
    }
  };

  const handleChange = (e: React.ChangeEvent<HTMLTextAreaElement>) => {
    valueRef.current = e.target.value;
    setCharCount(e.target.value.length);
  };

  return (
    <div className="flex flex-col gap-1">
      <textarea
        ref={textareaRef}
        defaultValue={initialValue ?? ""}
        onChange={handleChange}
        onKeyDown={handleKeyDown}
        disabled={disabled}
        placeholder="Type your answer… (Enter to submit, Shift+Enter for newline)"
        rows={3}
        className="bg-white/[0.04] border border-border rounded-lg p-3 text-[12px] text-foreground placeholder:text-dim resize-none focus:outline-none focus:ring-1 focus:ring-accent/40 disabled:opacity-50"
      />
      <span className="text-[9px] text-dim text-right">{charCount} chars</span>
    </div>
  );
}
