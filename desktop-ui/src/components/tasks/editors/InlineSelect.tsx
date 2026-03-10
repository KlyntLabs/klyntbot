import { Check } from "lucide-react";
import { useCallback, useEffect, useRef, useState } from "react";
import { createPortal } from "react-dom";
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
  const triggerRef = useRef<HTMLButtonElement>(null);
  const dropdownRef = useRef<HTMLDivElement>(null);
  const [pos, setPos] = useState({ top: 0, left: 0 });

  useClickOutside(dropdownRef, () => setOpen(false), open);

  const updatePosition = useCallback(() => {
    if (!triggerRef.current) return;
    const rect = triggerRef.current.getBoundingClientRect();
    setPos({ top: rect.bottom + 4, left: rect.left });
  }, []);

  useEffect(() => {
    if (open) updatePosition();
  }, [open, updatePosition]);

  return (
    <div className={className ?? ""}>
      <button
        ref={triggerRef}
        type="button"
        onClick={(e) => {
          e.stopPropagation();
          setOpen(!open);
        }}
        aria-haspopup="listbox"
        aria-expanded={open}
        className="inline-flex text-left rounded px-1 -mx-1 cursor-pointer transition-colors"
      >
        {renderDisplay(value)}
      </button>
      {open &&
        createPortal(
          <div
            ref={dropdownRef}
            className="fixed z-[9999] min-w-[140px] glass-dropdown"
            style={{ top: pos.top, left: pos.left }}
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
                    isSelected
                      ? "text-brand bg-white/[0.12]"
                      : "text-secondary hover:bg-white/[0.08]"
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
          </div>,
          document.body,
        )}
    </div>
  );
}
