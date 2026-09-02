import { X } from "lucide-react";
import { forwardRef, useCallback, useEffect, useImperativeHandle, useRef, useState } from "react";

interface NoteTagsProps {
  tags: string[];
  onChange: (tags: string[]) => void;
}

export interface NoteTagsHandle {
  startAdding: () => void;
}

export const NoteTags = forwardRef<NoteTagsHandle, NoteTagsProps>(function NoteTags(
  { tags, onChange },
  ref,
) {
  const [adding, setAdding] = useState(false);
  const [input, setInput] = useState("");
  const inputRef = useRef<HTMLInputElement>(null);

  useImperativeHandle(ref, () => ({
    startAdding: () => setAdding(true),
  }));

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

  useEffect(() => {
    if (adding) inputRef.current?.focus();
  }, [adding]);

  return (
    <div className="flex items-center gap-1 flex-wrap">
      {tags.map((tag) => (
        <span key={tag} className="tag-pill group">
          {tag}
          <button
            type="button"
            onClick={() => handleRemove(tag)}
            className="opacity-0 group-hover:opacity-100 transition-opacity ml-0.5"
            aria-label={`Remove tag ${tag}`}
          >
            <X className="size-2.5" />
          </button>
        </span>
      ))}

      {adding && (
        <input
          ref={inputRef}
          type="text"
          value={input}
          onChange={(e) => setInput(e.target.value)}
          onBlur={handleAdd}
          onKeyDown={handleKeyDown}
          placeholder="tag..."
          className="text-ui-xs bg-transparent border-b border-brand/30 text-fg outline-none w-16 py-0.5"
        />
      )}
    </div>
  );
});
