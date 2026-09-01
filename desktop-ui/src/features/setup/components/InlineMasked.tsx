import { Eye, EyeOff } from "lucide-react";
import { useEffect, useRef, useState } from "react";

interface InlineMaskedProps {
  defaultValue?: string;
  onSubmit: (value: string) => void;
  disabled?: boolean;
  autoFocus?: boolean;
}

export function InlineMasked({
  defaultValue = "",
  onSubmit,
  disabled,
  autoFocus = true,
}: InlineMaskedProps) {
  const [value, setValue] = useState(defaultValue);
  const [show, setShow] = useState(false);
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
    <span className="inline-flex items-center gap-1">
      <input
        ref={inputRef}
        type={show ? "text" : "password"}
        value={value}
        onChange={(e) => setValue(e.target.value)}
        onKeyDown={handleKeyDown}
        placeholder="paste your key"
        disabled={disabled}
        className="inline-block border-b-2 border-accent bg-transparent text-accent font-semibold outline-none min-w-[200px] placeholder:text-muted-foreground/50 disabled:opacity-50 transition-colors"
      />
      <button
        type="button"
        onClick={() => setShow(!show)}
        className="text-muted-foreground hover:text-foreground transition-colors"
      >
        {show ? <EyeOff className="size-3.5" /> : <Eye className="size-3.5" />}
      </button>
    </span>
  );
}
