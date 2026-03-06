import { Check } from "lucide-react";
import { useRef, useState } from "react";
import { useClickOutside } from "../../../hooks/useClickOutside";

interface Option {
  value: string | null;
  label: string;
  className?: string;
}

interface InlineSelectProps {
  value: string | null;
  options: Option[];
  onSelect: (value: string | null) => void;
  renderDisplay: (value: string | null) => React.ReactNode;
  className?: string;
}

export function InlineSelect({
  value,
  options,
  onSelect,
  renderDisplay,
  className,
}: InlineSelectProps) {
  const [open, setOpen] = useState(false);
  const ref = useRef<HTMLDivElement>(null);
  useClickOutside(ref, () => setOpen(false), open);

  return (
    <div ref={ref} className={`relative ${className ?? ""}`}>
      <button
        type="button"
        onClick={(e) => {
          e.stopPropagation();
          setOpen(!open);
        }}
        aria-haspopup="listbox"
        aria-expanded={open}
        className="w-full text-left rounded px-1 -mx-1 cursor-pointer transition-colors"
      >
        {renderDisplay(value)}
      </button>
      {open && (
        <div
          className="absolute z-50 top-full left-0 mt-1 min-w-[140px] glass-panel"
          role="listbox"
        >
          {options.map((opt) => {
            const isSelected = value === opt.value;
            return (
              <button
                type="button"
                key={opt.value ?? "__none"}
                role="option"
                aria-selected={isSelected}
                onClick={(e) => {
                  e.stopPropagation();
                  onSelect(opt.value);
                  setOpen(false);
                }}
                className={`w-full flex items-center gap-2 px-3 py-1.5 text-[12px] font-light transition-colors ${
                  isSelected ? "text-brand bg-white/[0.12]" : "text-secondary hover:bg-white/[0.08]"
                } ${opt.className ?? ""}`}
                style={{ borderRadius: "var(--glass-radius-inner)" }}
              >
                <span className="w-3.5 flex-shrink-0">
                  {isSelected && <Check className="w-3.5 h-3.5" strokeWidth={2} />}
                </span>
                {opt.label}
              </button>
            );
          })}
        </div>
      )}
    </div>
  );
}
