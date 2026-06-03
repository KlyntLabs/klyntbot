import type { InputHTMLAttributes } from "react";
import { cn } from "@/utils/cn";

/* ═══════════════════════════════════════════════════════════════════════════
   Switch — Toggle input primitive
   ══════════════════════════════════════════════════════════════════════════ */

export interface SwitchProps extends Omit<InputHTMLAttributes<HTMLInputElement>, "className" | "type"> {
  className?: string;
  label?: string;
}

export function Switch({ className, label, ...props }: SwitchProps) {
  return (
    <label className={cn("inline-flex items-center gap-2 cursor-pointer", className)}>
      <div className="relative inline-flex items-center">
        <input
          type="checkbox"
          className="peer sr-only"
          {...props}
        />
        <div
          className={cn(
            "w-9 h-5 rounded-full bg-surface-control border border-border-subtle",
            "peer-checked:bg-text-accent-cyan peer-checked:border-text-accent-cyan",
            "transition-colors duration-ui-fast",
            "after:content-[''] after:absolute after:top-0.5 after:left-0.5",
            "after:w-4 after:h-4 after:rounded-full after:bg-white",
            "after:transition-transform after:duration-ui-fast",
            "peer-checked:after:translate-x-4",
            "peer-disabled:opacity-50 peer-disabled:cursor-not-allowed",
          )}
        />
      </div>
      {label && (
        <span className="text-ui-sm text-text-strong">{label}</span>
      )}
    </label>
  );
}
