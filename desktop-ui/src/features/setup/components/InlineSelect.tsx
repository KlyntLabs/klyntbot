import { useEffect, useRef, useState } from "react";
import { createPortal } from "react-dom";

interface InlineSelectProps {
  options: { label: string; value: string }[];
  defaultValue?: string;
  onSubmit: (value: string) => void;
  disabled?: boolean;
  autoFocus?: boolean;
}

export function InlineSelect({
  options,
  defaultValue,
  onSubmit,
  disabled,
  autoFocus = true,
}: InlineSelectProps) {
  const initial = defaultValue || options[0]?.value || "";
  const [value, setValue] = useState(initial);
  const [open, setOpen] = useState(false);
  const triggerRef = useRef<HTMLButtonElement>(null);
  const [dropdownPos, setDropdownPos] = useState({ top: 0, left: 0 });

  const selectedLabel = options.find((o) => o.value === value)?.label ?? value;

  useEffect(() => {
    if (autoFocus) triggerRef.current?.focus();
  }, [autoFocus]);

  const openDropdown = () => {
    if (disabled) return;
    const rect = triggerRef.current?.getBoundingClientRect();
    if (rect) {
      setDropdownPos({ top: rect.bottom + 4, left: rect.left });
    }
    setOpen(true);
  };

  const select = (val: string) => {
    setValue(val);
    setOpen(false);
    onSubmit(val);
  };

  const handleKeyDown = (e: React.KeyboardEvent) => {
    if (e.key === "Enter" && !open && !disabled) {
      e.preventDefault();
      onSubmit(value);
    }
  };

  return (
    <>
      <button
        ref={triggerRef}
        type="button"
        onClick={openDropdown}
        onKeyDown={handleKeyDown}
        disabled={disabled}
        className="inline-block bg-control-hover border border-separator text-fg font-semibold outline-none cursor-pointer px-2 py-0.5 rounded-control focus:border-fg-secondary/50 focus:ring-2 focus:ring-separator transition-colors disabled:opacity-50"
      >
        {selectedLabel} <span className="text-fg-secondary/50 text-ui-sm">&#9662;</span>
      </button>

      {open &&
        createPortal(
          <>
            {/* Backdrop */}
            {/* biome-ignore lint/a11y/noStaticElementInteractions: dropdown backdrop dismissal */}
            <div
              className="fixed inset-0 z-50"
              onClick={() => setOpen(false)}
              onKeyDown={(e) => e.key === "Escape" && setOpen(false)}
              role="presentation"
            />
            {/* Dropdown */}
            <div
              className="fixed z-50 glass-panel border border-separator rounded-lg py-1 shadow-lg min-w-[160px]"
              style={{ top: dropdownPos.top, left: dropdownPos.left }}
            >
              {options.map((opt) => (
                <button
                  key={opt.value}
                  type="button"
                  onClick={() => select(opt.value)}
                  className={`block w-full text-left px-3 py-1.5 text-ui transition-colors ${
                    opt.value === value
                      ? "text-brand bg-brand/10"
                      : "text-fg hover:bg-control-hover"
                  }`}
                >
                  {opt.label}
                </button>
              ))}
            </div>
          </>,
          document.body,
        )}
    </>
  );
}
