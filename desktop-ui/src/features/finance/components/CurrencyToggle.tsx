import { ArrowLeftRight, Check, ChevronDown } from "lucide-react";
import { useEffect, useRef, useState } from "react";
import type { CurrencyDisplayMode } from "../hooks/useCurrencyDisplayMode";

interface CurrencyToggleProps {
  mode: CurrencyDisplayMode;
  currencies: string[];
  onSelect: (mode: CurrencyDisplayMode) => void;
}

export function CurrencyToggle({ mode, currencies, onSelect }: CurrencyToggleProps) {
  const [open, setOpen] = useState(false);
  const ref = useRef<HTMLDivElement>(null);

  useEffect(() => {
    function handleClickOutside(e: MouseEvent) {
      if (ref.current && !ref.current.contains(e.target as Node)) {
        setOpen(false);
      }
    }
    if (open) document.addEventListener("mousedown", handleClickOutside);
    return () => document.removeEventListener("mousedown", handleClickOutside);
  }, [open]);

  const label = mode === "multi" ? "Multi" : mode;

  return (
    <div ref={ref} className="relative ml-2">
      <button
        type="button"
        onClick={() => setOpen(!open)}
        className="flex items-center gap-1.5 px-2.5 py-1.5 rounded-lg text-[11px] font-light transition-all duration-200 border border-border hover:bg-surface-base"
      >
        {mode === "multi" ? (
          <ArrowLeftRight className="w-3 h-3 text-muted" strokeWidth={1.5} />
        ) : null}
        <span className={mode === "multi" ? "text-muted" : "text-brand font-medium"}>{label}</span>
        <ChevronDown
          className={`w-3 h-3 text-muted transition-transform ${open ? "rotate-180" : ""}`}
          strokeWidth={1.5}
        />
      </button>

      {open && (
        <div className="absolute right-0 top-full mt-1 z-50 min-w-[120px] py-1 rounded-lg glass-panel border border-border shadow-xl">
          <DropItem
            label="Multi"
            icon={<ArrowLeftRight className="w-3 h-3" strokeWidth={1.5} />}
            active={mode === "multi"}
            onClick={() => {
              onSelect("multi");
              setOpen(false);
            }}
          />
          <div className="h-px bg-surface-base mx-2 my-1" />
          {currencies.map((c) => (
            <DropItem
              key={c}
              label={c}
              active={mode === c}
              onClick={() => {
                onSelect(c);
                setOpen(false);
              }}
            />
          ))}
        </div>
      )}
    </div>
  );
}

function DropItem({
  label,
  icon,
  active,
  onClick,
}: {
  label: string;
  icon?: React.ReactNode;
  active: boolean;
  onClick: () => void;
}) {
  return (
    <button
      type="button"
      onClick={onClick}
      className={`w-full flex items-center gap-2 px-3 py-1.5 text-[11px] transition-colors ${
        active ? "text-brand" : "text-secondary hover:text-primary hover:bg-surface-low"
      }`}
    >
      <span className="w-3 flex-shrink-0">
        {active ? <Check className="w-3 h-3" strokeWidth={2} /> : (icon ?? null)}
      </span>
      <span className={active ? "font-medium" : "font-light"}>{label}</span>
    </button>
  );
}
