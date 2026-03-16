import { Check } from "lucide-react";
import { useEffect, useRef, useState } from "react";

export interface CheckboxOption {
  label: string;
  value: string;
  detected?: boolean;
  hint?: string;
}

interface InlineCheckboxListProps {
  options: CheckboxOption[];
  defaultValue?: string[];
  onSubmit: (values: string[]) => void;
  autoFocus?: boolean;
}

export function InlineCheckboxList({
  options,
  defaultValue = [],
  onSubmit,
  autoFocus = true,
}: InlineCheckboxListProps) {
  const [selected, setSelected] = useState<Set<string>>(new Set(defaultValue));
  const containerRef = useRef<HTMLFieldSetElement>(null);

  useEffect(() => {
    if (autoFocus) containerRef.current?.focus();
  }, [autoFocus]);

  const toggle = (value: string) => {
    setSelected((prev) => {
      const next = new Set(prev);
      if (next.has(value)) {
        next.delete(value);
      } else {
        next.add(value);
      }
      return next;
    });
  };

  const handleKeyDown = (e: React.KeyboardEvent) => {
    if (e.key === "Enter") {
      e.preventDefault();
      onSubmit([...selected]);
    }
  };

  return (
    <fieldset ref={containerRef} className="border-0 p-0 m-0 mt-4" onKeyDown={handleKeyDown}>
      <div className="grid grid-cols-2 gap-2">
        {options.map((opt) => {
          const isChecked = selected.has(opt.value);
          return (
            <button
              key={opt.value}
              type="button"
              onClick={() => toggle(opt.value)}
              className={`flex items-center gap-3 px-4 py-3 rounded-xl text-left transition-all duration-200 border ${
                isChecked
                  ? "bg-brand/8 border-brand/25 text-primary"
                  : "bg-white/[0.03] border-white/[0.06] text-muted hover:bg-white/[0.06] hover:border-white/[0.1]"
              }`}
            >
              {/* Checkbox */}
              <div
                className={`w-[18px] h-[18px] rounded-md border-2 flex items-center justify-center flex-shrink-0 transition-all duration-200 ${
                  isChecked ? "bg-brand border-brand" : "border-white/20"
                }`}
              >
                {isChecked && <Check className="w-3 h-3 text-white" />}
              </div>

              {/* Label */}
              <span className="flex-1 text-sm font-medium truncate">{opt.label}</span>

              {/* Detection badge */}
              {opt.detected !== undefined && (
                <span
                  className={`text-[10px] font-medium px-1.5 py-0.5 rounded-md flex-shrink-0 ${
                    opt.detected ? "bg-success/10 text-success" : "bg-white/5 text-dim"
                  }`}
                >
                  {opt.detected ? "Found" : "N/A"}
                </span>
              )}
            </button>
          );
        })}
      </div>

      {/* Confirm button */}
      <button
        type="button"
        onClick={() => onSubmit([...selected])}
        className="mt-4 w-full px-5 py-2.5 text-sm font-medium text-white bg-brand hover:bg-brand-hover rounded-xl transition-all duration-200 hover:scale-[1.01] active:scale-[0.99]"
      >
        {selected.size > 0
          ? `Connect ${selected.size} tool${selected.size > 1 ? "s" : ""}`
          : "Skip this step"}
      </button>
    </fieldset>
  );
}
