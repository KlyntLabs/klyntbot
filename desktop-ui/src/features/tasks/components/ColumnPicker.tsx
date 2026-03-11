import { Columns3 } from "lucide-react";
import { useEffect, useRef, useState } from "react";
import { ALL_COLUMNS, type ColumnId } from "../hooks/useColumnVisibility";

interface ColumnPickerProps {
  visibleColumns: Set<ColumnId>;
  onToggle: (id: ColumnId) => void;
  onReset: () => void;
}

export function ColumnPicker({ visibleColumns, onToggle, onReset }: ColumnPickerProps) {
  const [open, setOpen] = useState(false);
  const ref = useRef<HTMLDivElement>(null);

  useEffect(() => {
    if (!open) return;
    const handler = (e: MouseEvent) => {
      if (ref.current && !ref.current.contains(e.target as Node)) setOpen(false);
    };
    document.addEventListener("mousedown", handler);
    return () => document.removeEventListener("mousedown", handler);
  }, [open]);

  const coreColumns = ALL_COLUMNS.filter((c) => c.group === "core");
  const agenticColumns = ALL_COLUMNS.filter((c) => c.group === "agentic");

  return (
    <div className="relative" ref={ref}>
      <button
        type="button"
        onClick={() => setOpen(!open)}
        aria-label="Toggle columns"
        className={`p-1.5 rounded-md transition-all ${
          open ? "glass-button-active text-brand" : "text-muted hover:text-secondary"
        }`}
      >
        <Columns3 className="w-[14px] h-[14px]" strokeWidth={1.5} />
      </button>

      {open && (
        <div className="absolute right-0 top-full mt-1.5 w-52 glass-panel rounded-xl p-2 z-50 shadow-lg">
          <p className="text-[10px] text-dim font-light uppercase tracking-wider px-2 py-1">Core</p>
          {coreColumns.map((col) => (
            <label
              key={col.id}
              className="flex items-center gap-2 px-2 py-1.5 rounded-lg hover:bg-white/[0.06] cursor-pointer"
            >
              <input
                type="checkbox"
                checked={visibleColumns.has(col.id)}
                onChange={() => onToggle(col.id)}
                className="accent-[var(--brand)] w-3 h-3"
              />
              <span className="text-[11px] font-light text-secondary">{col.label}</span>
            </label>
          ))}

          <div className="border-t border-white/[0.06] my-1.5" />

          <p className="text-[10px] text-dim font-light uppercase tracking-wider px-2 py-1">
            Agentic
          </p>
          {agenticColumns.map((col) => (
            <label
              key={col.id}
              className="flex items-center gap-2 px-2 py-1.5 rounded-lg hover:bg-white/[0.06] cursor-pointer"
            >
              <input
                type="checkbox"
                checked={visibleColumns.has(col.id)}
                onChange={() => onToggle(col.id)}
                className="accent-[var(--brand)] w-3 h-3"
              />
              <span className="text-[11px] font-light text-secondary">{col.label}</span>
            </label>
          ))}

          <div className="border-t border-white/[0.06] my-1.5" />

          <button
            type="button"
            onClick={onReset}
            className="w-full text-[10px] font-light text-muted hover:text-brand px-2 py-1 text-left"
          >
            Reset to defaults
          </button>
        </div>
      )}
    </div>
  );
}
