import type { InputHTMLAttributes } from "react";
import { cn } from "@/utils/cn";

/* ═══════════════════════════════════════════════════════════════════════════
   Input — Form controls matching legacy ds-panel-search and input styles
   ══════════════════════════════════════════════════════════════════════════ */

export interface InputProps extends Omit<InputHTMLAttributes<HTMLInputElement>, "className"> {
  className?: string;
  inputClassName?: string;
  error?: string;
  icon?: React.ReactNode;
  trailing?: React.ReactNode;
}

export function Input({
  className,
  inputClassName,
  error,
  icon,
  trailing,
  ...props
}: InputProps) {
  return (
    <div className={cn("relative flex items-center gap-2 w-full", className)}>
      {icon ? (
        <span className="shrink-0 inline-flex items-center justify-center text-text-faint">
          {icon}
        </span>
      ) : null}
      <input
        className={cn(
          "flex-1 min-w-0 bg-transparent text-text-primary placeholder:text-foreground-faint outline-none",
          "text-ui-sm leading-none",
          inputClassName,
        )}
        {...props}
      />
      {trailing ? <span className="shrink-0">{trailing}</span> : null}
      {error ? (
        <p className="absolute -bottom-4 left-0 text-ui-2xs text-status-error" role="alert">
          {error}
        </p>
      ) : null}
    </div>
  );
}

/* ── Search field variant (styled container) ────────────────────────────── */

export interface SearchFieldProps extends Omit<InputProps, "className" | "inputClassName"> {
  className?: string;
  inputClassName?: string;
}

export function SearchField({ className, inputClassName, icon, ...props }: SearchFieldProps) {
  return (
    <div
      className={cn(
        "flex items-center gap-2 px-2 py-1.5 rounded-ui-lg",
        "bg-surface-raised border border-border-subtle text-text-faint",
        "focus-within:border-border-strong focus-within:text-text-emphasis",
        "transition-colors duration-ui-fast",
        className,
      )}
    >
      {icon ? (
        <span className="shrink-0 inline-flex items-center justify-center w-3.5 h-3.5">
          {icon}
        </span>
      ) : null}
      <input
        type="search"
        className={cn(
          "flex-1 min-w-0 bg-transparent text-text-primary placeholder:text-foreground-faint outline-none",
          "text-ui-sm leading-none",
          inputClassName,
        )}
        {...props}
      />
    </div>
  );
}
