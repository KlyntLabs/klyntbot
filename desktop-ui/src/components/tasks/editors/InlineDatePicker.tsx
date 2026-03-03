import { useState, useRef, useCallback } from 'react';
import { useClickOutside } from '../../../hooks/useClickOutside';
import { MiniCalendar } from './MiniCalendar';
import { formatDate } from '../../../lib/dates';

interface InlineDatePickerProps {
  value: string | null;
  onSave: (value: string | null) => void;
}

export function InlineDatePicker({ value, onSave }: InlineDatePickerProps) {
  const [open, setOpen] = useState(false);
  const ref = useRef<HTMLDivElement>(null);
  useClickOutside(ref, useCallback(() => setOpen(false), []), open);

  return (
    <div ref={ref} className="relative">
      <button
        onClick={(e) => { e.stopPropagation(); setOpen(!open); }}
        className="text-[12px] text-muted font-light rounded px-1 -mx-1 cursor-pointer transition-colors"
      >
        {value ? formatDate(value) : <span className="text-dim">—</span>}
      </button>
      {open && (
        <div className="absolute z-50 top-full right-0 mt-1 glass-panel">
          <MiniCalendar
            value={value}
            onSelect={(iso) => { onSave(iso); setOpen(false); }}
            onClear={value ? () => { onSave(null); setOpen(false); } : undefined}
          />
        </div>
      )}
    </div>
  );
}
