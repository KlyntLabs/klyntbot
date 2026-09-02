import { useCallback, useEffect, useRef, useState } from "react";

interface IssueDetailTitleProps {
  title: string;
  onUpdate: (title: string) => void;
}

export function IssueDetailTitle({ title, onUpdate }: IssueDetailTitleProps) {
  const [value, setValue] = useState(title);
  const textareaRef = useRef<HTMLTextAreaElement>(null);

  useEffect(() => {
    setValue(title);
  }, [title]);

  // Auto-resize
  useEffect(() => {
    const el = textareaRef.current;
    if (!el) return;
    el.style.height = "auto";
    el.style.height = `${el.scrollHeight}px`;
  }, []);

  const handleBlur = useCallback(() => {
    const trimmed = value.trim();
    if (trimmed && trimmed !== title) {
      onUpdate(trimmed);
    } else {
      setValue(title);
    }
  }, [value, title, onUpdate]);

  const handleKeyDown = useCallback((e: React.KeyboardEvent) => {
    if (e.key === "Enter") {
      e.preventDefault();
      (e.target as HTMLTextAreaElement).blur();
    }
  }, []);

  return (
    <textarea
      ref={textareaRef}
      value={value}
      onChange={(e) => setValue(e.target.value)}
      onBlur={handleBlur}
      onKeyDown={handleKeyDown}
      rows={1}
      className="w-full text-2xl font-semibold text-fg bg-transparent border-none outline-none resize-none mb-4 p-0 leading-tight placeholder:text-fg-secondary"
      placeholder="Task title"
    />
  );
}
