import { useEffect, useRef, useState } from "react";

interface InlineInputProps {
  defaultValue?: string;
  placeholder?: string;
  onSubmit: (value: string) => void;
  disabled?: boolean;
  autoFocus?: boolean;
}

export function InlineInput({
  defaultValue = "",
  placeholder = "...",
  onSubmit,
  disabled,
  autoFocus = true,
}: InlineInputProps) {
  const [value, setValue] = useState(defaultValue);
  const inputRef = useRef<HTMLInputElement>(null);

  useEffect(() => {
    if (autoFocus) inputRef.current?.focus();
  }, [autoFocus]);

  const handleKeyDown = (e: React.KeyboardEvent) => {
    if (e.key === "Enter" && !disabled) {
      e.preventDefault();
      onSubmit(value);
    }
  };

  return (
    <input
      ref={inputRef}
      type="text"
      value={value}
      onChange={(e) => setValue(e.target.value)}
      onKeyDown={handleKeyDown}
      placeholder={placeholder}
      disabled={disabled}
      className="inline-block bg-control-hover border border-separator text-fg font-semibold outline-none min-w-[120px] px-2 py-0.5 rounded-control placeholder:text-fg-secondary/50 focus:border-fg-secondary/50 focus:ring-2 focus:ring-separator disabled:opacity-50 transition-colors"
      style={{ width: `${Math.max(value.length, placeholder.length) + 2}ch` }}
    />
  );
}
