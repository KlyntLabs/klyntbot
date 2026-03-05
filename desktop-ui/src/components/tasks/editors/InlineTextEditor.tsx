import { useCallback, useState } from "react";

interface InlineTextEditorProps {
  value: string;
  onSave: (value: string) => void;
  className?: string;
  placeholder?: string;
}

export function InlineTextEditor({ value, onSave, className, placeholder }: InlineTextEditorProps) {
  const [editing, setEditing] = useState(false);
  const [draft, setDraft] = useState("");

  const startEdit = useCallback(
    (e: React.SyntheticEvent) => {
      e.stopPropagation();
      setDraft(value);
      setEditing(true);
    },
    [value],
  );

  const save = useCallback(() => {
    if (draft.trim() && draft !== value) onSave(draft.trim());
    setEditing(false);
  }, [draft, value, onSave]);

  if (editing) {
    return (
      <input
        value={draft}
        onChange={(e) => setDraft(e.target.value)}
        onKeyDown={(e) => {
          if (e.key === "Enter") save();
          if (e.key === "Escape") setEditing(false);
        }}
        onBlur={save}
        onClick={(e) => e.stopPropagation()}
        placeholder={placeholder}
        className={`bg-transparent border-b border-brand outline-none w-full ${className ?? "text-[13px] font-light text-primary"}`}
      />
    );
  }

  return (
    <button
      type="button"
      onClick={startEdit}
      className={`cursor-text rounded px-1 -mx-1 transition-colors text-left ${className ?? "text-[13px] font-light text-secondary"}`}
    >
      {value || <span className="text-dim">{placeholder ?? "—"}</span>}
    </button>
  );
}
