import { useClickOutside } from "@shared/hooks/useClickOutside";
import type { ColumnValueSetParams, CustomColumn } from "@shared/types";
import { Check } from "lucide-react";
import { useCallback, useRef, useState } from "react";
import { ColumnRenderer } from "./ColumnRenderer";

interface CustomColumnCellProps {
  taskId: string;
  column: CustomColumn;
  value: unknown;
  onSetValue: (params: ColumnValueSetParams) => void;
}

export function CustomColumnCell({ taskId, column, value, onSetValue }: CustomColumnCellProps) {
  const save = (newValue: unknown) => {
    onSetValue({ taskId, columnId: column.id, value: newValue });
  };

  switch (column.columnType) {
    case "text":
      return <TextCell value={value} onSave={save} />;
    case "number":
      return <NumberCell value={value} onSave={save} />;
    case "checkbox":
      return <CheckboxCell value={value} onSave={save} />;
    case "rating":
      return <RatingCell value={value} onSave={save} />;
    case "dropdown":
      return <DropdownCell value={value} options={column.options} onSave={save} />;
    default:
      return (
        <ColumnRenderer columnType={column.columnType} value={value} options={column.options} />
      );
  }
}

function TextCell({ value, onSave }: { value: unknown; onSave: (v: unknown) => void }) {
  const [editing, setEditing] = useState(false);
  const [draft, setDraft] = useState("");
  const inputRef = useCallback((node: HTMLInputElement | null) => {
    node?.focus();
  }, []);

  if (editing) {
    return (
      <input
        ref={inputRef}
        value={draft}
        onChange={(e) => setDraft(e.target.value)}
        onKeyDown={(e) => {
          if (e.key === "Enter") {
            onSave(draft);
            setEditing(false);
          }
          if (e.key === "Escape") setEditing(false);
        }}
        onBlur={() => {
          onSave(draft);
          setEditing(false);
        }}
        className="w-full bg-white/[0.04] rounded px-2 py-1 text-[12px] font-light text-secondary border border-brand/40 outline-none"
      />
    );
  }

  return (
    <button
      type="button"
      onClick={() => {
        setDraft(typeof value === "string" ? value : "");
        setEditing(true);
      }}
      className="w-full text-left rounded px-1 -mx-1 cursor-pointer hover:bg-white/[0.04] transition-colors min-h-[24px]"
    >
      <ColumnRenderer columnType="text" value={value} />
    </button>
  );
}

function NumberCell({ value, onSave }: { value: unknown; onSave: (v: unknown) => void }) {
  const [editing, setEditing] = useState(false);
  const [draft, setDraft] = useState("");
  const inputRef = useCallback((node: HTMLInputElement | null) => {
    node?.focus();
  }, []);

  if (editing) {
    return (
      <input
        ref={inputRef}
        type="number"
        value={draft}
        onChange={(e) => setDraft(e.target.value)}
        onKeyDown={(e) => {
          if (e.key === "Enter") {
            onSave(draft ? Number(draft) : null);
            setEditing(false);
          }
          if (e.key === "Escape") setEditing(false);
        }}
        onBlur={() => {
          onSave(draft ? Number(draft) : null);
          setEditing(false);
        }}
        className="w-full bg-white/[0.04] rounded px-2 py-1 text-[12px] font-light text-secondary border border-brand/40 outline-none text-right"
      />
    );
  }

  return (
    <button
      type="button"
      onClick={() => {
        setDraft(typeof value === "number" ? String(value) : "");
        setEditing(true);
      }}
      className="w-full text-left rounded px-1 -mx-1 cursor-pointer hover:bg-white/[0.04] transition-colors min-h-[24px]"
    >
      <ColumnRenderer columnType="number" value={value} />
    </button>
  );
}

function CheckboxCell({ value, onSave }: { value: unknown; onSave: (v: unknown) => void }) {
  const checked = Boolean(value);
  return (
    <button
      type="button"
      onClick={() => onSave(!checked)}
      className="flex items-center justify-center w-full cursor-pointer py-0.5"
    >
      <div
        className={`w-4 h-4 rounded border transition-colors flex items-center justify-center ${
          checked
            ? "bg-brand/20 border-brand/40 text-brand"
            : "bg-white/[0.04] border-white/[0.12] text-transparent hover:border-white/[0.2]"
        }`}
      >
        {checked && <Check className="w-3 h-3" strokeWidth={2} />}
      </div>
    </button>
  );
}

function RatingCell({ value, onSave }: { value: unknown; onSave: (v: unknown) => void }) {
  const current = typeof value === "number" ? Math.min(5, Math.max(0, Math.round(value))) : 0;

  return (
    <div className="flex gap-0.5">
      {[1, 2, 3, 4, 5].map((star) => (
        <button
          type="button"
          key={star}
          onClick={() => onSave(star === current ? 0 : star)}
          className={`text-[14px] leading-none cursor-pointer transition-colors ${
            star <= current ? "text-amber-400" : "text-white/[0.15] hover:text-amber-400/50"
          }`}
        >
          &#9733;
        </button>
      ))}
    </div>
  );
}

function DropdownCell({
  value,
  options,
  onSave,
}: {
  value: unknown;
  options: string[] | null;
  onSave: (v: unknown) => void;
}) {
  const [open, setOpen] = useState(false);
  const ref = useRef<HTMLDivElement>(null);
  useClickOutside(ref, () => setOpen(false), open);

  const current = typeof value === "string" ? value : null;

  return (
    <div ref={ref} className="relative">
      <button
        type="button"
        onClick={() => setOpen(!open)}
        className="w-full text-left rounded px-1 -mx-1 cursor-pointer hover:bg-white/[0.04] transition-colors min-h-[24px]"
      >
        <ColumnRenderer columnType="dropdown" value={value} />
      </button>
      {open && options && (
        <div
          className="absolute z-50 top-full left-0 mt-1 min-w-[140px] glass-dropdown"
          role="listbox"
        >
          <button
            type="button"
            role="option"
            aria-selected={!current}
            onClick={() => {
              onSave(null);
              setOpen(false);
            }}
            className={`w-full flex items-center gap-2 px-3 py-1.5 text-[12px] font-light transition-colors ${
              !current ? "text-brand bg-white/[0.12]" : "text-dim hover:bg-white/[0.08]"
            }`}
            style={{ borderRadius: "var(--glass-radius-inner)" }}
          >
            <span className="w-3.5 flex-shrink-0">
              {!current && <Check className="w-3.5 h-3.5" strokeWidth={2} />}
            </span>
            None
          </button>
          {options.map((opt) => {
            const isSelected = current === opt;
            return (
              <button
                type="button"
                key={opt}
                role="option"
                aria-selected={isSelected}
                onClick={() => {
                  onSave(opt);
                  setOpen(false);
                }}
                className={`w-full flex items-center gap-2 px-3 py-1.5 text-[12px] font-light transition-colors ${
                  isSelected ? "text-brand bg-white/[0.12]" : "text-secondary hover:bg-white/[0.08]"
                }`}
                style={{ borderRadius: "var(--glass-radius-inner)" }}
              >
                <span className="w-3.5 flex-shrink-0">
                  {isSelected && <Check className="w-3.5 h-3.5" strokeWidth={2} />}
                </span>
                {opt}
              </button>
            );
          })}
        </div>
      )}
    </div>
  );
}
