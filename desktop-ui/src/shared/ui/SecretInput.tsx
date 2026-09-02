import { Eye, EyeOff } from "lucide-react";
import { useState } from "react";

interface SecretInputProps {
  value: string;
  onChange: (value: string) => void;
  placeholder?: string;
  className?: string;
}

export function SecretInput({ value, onChange, placeholder, className }: SecretInputProps) {
  const [show, setShow] = useState(false);

  return (
    <div className="relative">
      <input
        type={show ? "text" : "password"}
        value={value}
        onChange={(e) => onChange(e.target.value)}
        placeholder={placeholder}
        className={
          className ??
          "w-full px-3 py-1.5 pr-9 text-ui-sm text-fg bg-control-hover border border-separator rounded-control focus:outline-none focus:border-fg-secondary/50 focus:ring-2 focus:ring-separator transition-colors placeholder:text-fg-dim"
        }
      />
      <button
        type="button"
        onClick={() => setShow(!show)}
        className="absolute right-2 top-1/2 -translate-y-1/2 text-fg-secondary hover:text-fg transition-colors"
      >
        {show ? <EyeOff className="size-3" /> : <Eye className="size-3" />}
      </button>
    </div>
  );
}
