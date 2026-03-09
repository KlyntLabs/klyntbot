import { useCallback, useEffect, useRef, useState } from "react";
import { createPortal } from "react-dom";
import { useClickOutside } from "../../../hooks/useClickOutside";
import { formatDate } from "../../../lib/dates";
import { MiniCalendar } from "./MiniCalendar";

interface InlineDatePickerProps {
  value: string | null;
  onSave: (value: string | null) => void;
}

export function InlineDatePicker({ value, onSave }: InlineDatePickerProps) {
  const [open, setOpen] = useState(false);
  const triggerRef = useRef<HTMLButtonElement>(null);
  const dropdownRef = useRef<HTMLDivElement>(null);
  const [pos, setPos] = useState({ top: 0, right: 0 });

  useClickOutside(dropdownRef, () => setOpen(false), open);

  const updatePosition = useCallback(() => {
    if (!triggerRef.current) return;
    const rect = triggerRef.current.getBoundingClientRect();
    setPos({ top: rect.bottom + 4, right: window.innerWidth - rect.right });
  }, []);

  useEffect(() => {
    if (open) updatePosition();
  }, [open, updatePosition]);

  return (
    <div>
      <button
        ref={triggerRef}
        type="button"
        onClick={(e) => {
          e.stopPropagation();
          setOpen(!open);
        }}
        aria-label="Select date"
        aria-haspopup="dialog"
        aria-expanded={open}
        className="text-[12px] text-muted font-light rounded px-1 -mx-1 cursor-pointer transition-colors"
      >
        {value ? formatDate(value) : <span className="text-dim">—</span>}
      </button>
      {open &&
        createPortal(
          <div
            ref={dropdownRef}
            className="fixed z-[9999] glass-dropdown"
            style={{ top: pos.top, right: pos.right }}
          >
            <MiniCalendar
              value={value}
              onSelect={(iso) => {
                onSave(iso);
                setOpen(false);
              }}
              onClear={
                value
                  ? () => {
                      onSave(null);
                      setOpen(false);
                    }
                  : undefined
              }
            />
          </div>,
          document.body,
        )}
    </div>
  );
}
