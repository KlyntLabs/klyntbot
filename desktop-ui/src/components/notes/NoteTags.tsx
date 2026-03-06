import { Plus, X } from "lucide-react";
import { useCallback, useEffect, useRef, useState } from "react";

interface NoteTagsProps {
  tags: string[];
  onChange: (tags: string[]) => void;
}

export function NoteTags({ tags, onChange }: NoteTagsProps) {
  const [adding, setAdding] = useState(false);
  const [input, setInput] = useState("");
  const inputRef = useRef<HTMLInputElement>(null);

  const handleAdd = useCallback(() => {
    const tag = input.trim().toLowerCase();
    if (tag && !tags.includes(tag)) {
      onChange([...tags, tag]);
    }
    setInput("");
    setAdding(false);
  }, [input, tags, onChange]);

  const handleRemove = useCallback(
    (tag: string) => {
      onChange(tags.filter((t) => t !== tag));
    },
    [tags, onChange],
  );

  const handleKeyDown = useCallback(
    (e: React.KeyboardEvent) => {
      if (e.key === "Enter") {
        e.preventDefault();
        handleAdd();
      } else if (e.key === "Escape") {
        setInput("");
        setAdding(false);
      }
    },
    [handleAdd],
  );

  // Focus input when entering add mode
  useEffect(() => {
    if (adding) inputRef.current?.focus();
  }, [adding]);

  return (
    <div className="flex items-center gap-1 flex-wrap">
      {tags.map((tag) => (
        <span
          key={tag}
          className="inline-flex items-center gap-0.5 text-[11px] px-1.5 py-0.5 rounded-md bg-white/[0.06] text-dim group"
        >
          {tag}
          <button
            type="button"
            onClick={() => handleRemove(tag)}
            className="opacity-0 group-hover:opacity-100 transition-opacity"
            aria-label={`Remove tag ${tag}`}
          >
            <X className="w-2.5 h-2.5" />
          </button>
        </span>
      ))}

      {adding ? (
        <input
          ref={inputRef}
          type="text"
          value={input}
          onChange={(e) => setInput(e.target.value)}
          onBlur={handleAdd}
          onKeyDown={handleKeyDown}
          placeholder="tag..."
          className="text-[11px] bg-transparent border-b border-brand/30 text-primary outline-none w-16 py-0.5"
        />
      ) : (
        <button
          type="button"
          onClick={() => setAdding(true)}
          className="w-4 h-4 rounded flex items-center justify-center text-dim hover:text-secondary hover:bg-white/[0.06] transition-colors"
          aria-label="Add tag"
        >
          <Plus className="w-3 h-3" />
        </button>
      )}
    </div>
  );
}
