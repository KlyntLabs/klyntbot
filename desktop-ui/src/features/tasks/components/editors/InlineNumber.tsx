import { useState } from "react";

interface InlineNumberProps {
  value: number | null | undefined;
  onSave: (value: number | null) => void;
  suffix?: string;
  placeholder?: string;
  min?: number;
  max?: number;
}

export function InlineNumber({
  value,
  onSave,
  suffix = "",
  placeholder = "\u2014",
  min,
  max,
}: InlineNumberProps) {
  const [editing, setEditing] = useState(false);
  const [draft, setDraft] = useState("");

  const startEdit = (e: React.SyntheticEvent) => {
    e.stopPropagation();
    setDraft(value != null ? String(value) : "");
    setEditing(true);
  };

  const save = () => {
    const num = draft.trim() === "" ? null : Number(draft);
    if (num !== value) onSave(num);
    setEditing(false);
  };

  if (editing) {
    return (
      <input
        type="number"
        value={draft}
        onChange={(e) => setDraft(e.target.value)}
        onKeyDown={(e) => {
          if (e.key === "Enter") save();
          if (e.key === "Escape") setEditing(false);
        }}
        onBlur={save}
        onClick={(e) => e.stopPropagation()}
        min={min}
        max={max}
        className="bg-transparent border-b border-brand outline-none w-16 text-[11px] font-light text-primary"
      />
    );
  }

  return (
    <button
      type="button"
      onClick={startEdit}
      className="cursor-text text-[11px] font-light text-muted rounded px-1 -mx-1 transition-colors"
    >
      {value != null ? `${value}${suffix}` : <span className="text-dim">{placeholder}</span>}
    </button>
  );
}
