import { useEffect, useRef, useState } from "react";

interface VimCommandLineProps {
  prefix: string;
  onSubmit: (value: string) => void;
  onCancel: () => void;
}

export function VimCommandLine({ prefix, onSubmit, onCancel }: VimCommandLineProps) {
  const [value, setValue] = useState("");
  const inputRef = useRef<HTMLInputElement>(null);

  useEffect(() => {
    inputRef.current?.focus();
  }, []);

  const handleKeyDown = (e: React.KeyboardEvent) => {
    if (e.key === "Enter") {
      e.preventDefault();
      onSubmit(value);
    } else if (e.key === "Escape") {
      e.preventDefault();
      onCancel();
    }
  };

  return (
    <div className="flex items-center gap-1.5 px-4 py-1.5 border-t border-white/[0.06] bg-black/20">
      <span className="font-mono text-xs text-muted select-none">{prefix}</span>
      <input
        ref={inputRef}
        type="text"
        value={value}
        onChange={(e) => setValue(e.target.value)}
        onKeyDown={handleKeyDown}
        onBlur={onCancel}
        className="flex-1 bg-transparent text-xs font-mono text-primary outline-none placeholder:text-dim"
        placeholder={prefix === "/" ? "Search..." : "Command..."}
      />
    </div>
  );
}
