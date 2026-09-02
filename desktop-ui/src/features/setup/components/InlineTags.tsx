import { X } from "lucide-react";
import { useEffect, useRef, useState } from "react";
import { AREA_COLORS } from "../schema";

interface InlineTagsProps {
  defaultValue?: string[];
  onSubmit: (tags: string[]) => void;
  disabled?: boolean;
  autoFocus?: boolean;
}

export function InlineTags({
  defaultValue = [],
  onSubmit,
  disabled,
  autoFocus = true,
}: InlineTagsProps) {
  const [tags, setTags] = useState<string[]>(defaultValue);
  const [input, setInput] = useState("");
  const inputRef = useRef<HTMLInputElement>(null);

  useEffect(() => {
    if (autoFocus) inputRef.current?.focus();
  }, [autoFocus]);

  const addTag = (name: string) => {
    const trimmed = name.trim();
    if (trimmed && !tags.includes(trimmed)) {
      setTags((prev) => [...prev, trimmed]);
    }
    setInput("");
  };

  const removeTag = (index: number) => {
    setTags((prev) => prev.filter((_, i) => i !== index));
    inputRef.current?.focus();
  };

  const handleKeyDown = (e: React.KeyboardEvent) => {
    if (disabled) return;

    if (e.key === "Enter") {
      e.preventDefault();
      if (input.trim()) {
        addTag(input);
      } else if (tags.length > 0) {
        // Empty enter = confirm all tags
        onSubmit(tags);
      }
    } else if (e.key === "," || e.key === "Tab") {
      e.preventDefault();
      if (input.trim()) addTag(input);
    } else if (e.key === "Backspace" && !input && tags.length > 0) {
      removeTag(tags.length - 1);
    }
  };

  return (
    <span className="inline-flex flex-wrap items-center gap-1.5">
      {tags.map((tag, i) => (
        <span
          key={tag}
          className="inline-flex items-center gap-1 px-2 py-0.5 rounded-full text-ui font-medium text-white"
          style={{ backgroundColor: AREA_COLORS[i % AREA_COLORS.length] }}
        >
          {tag}
          {!disabled && (
            <button
              type="button"
              onClick={() => removeTag(i)}
              className="hover:opacity-70 transition-opacity"
            >
              <X className="size-3" />
            </button>
          )}
        </span>
      ))}
      <input
        ref={inputRef}
        type="text"
        value={input}
        onChange={(e) => setInput(e.target.value)}
        onKeyDown={handleKeyDown}
        placeholder={
          tags.length === 0 ? "type and press Enter" : "add more or press Enter to confirm"
        }
        disabled={disabled}
        className="inline-block bg-control-hover border border-separator text-fg font-semibold outline-none min-w-[120px] px-2 py-0.5 rounded-control placeholder:text-fg-secondary/50 text-ui focus:border-fg-secondary/50 focus:ring-2 focus:ring-separator disabled:opacity-50 transition-colors"
      />
    </span>
  );
}
