import type { TextareaHTMLAttributes } from "react";
import { cn } from "@/utils/cn";

/* ═══════════════════════════════════════════════════════════════════════════
   Textarea — Multi-line input primitive
   ══════════════════════════════════════════════════════════════════════════ */

export interface TextareaProps extends Omit<TextareaHTMLAttributes<HTMLTextAreaElement>, "className"> {
  className?: string;
  error?: string;
}

export function Textarea({ className, error, ...props }: TextareaProps) {
  return (
    <div className="relative w-full">
      <textarea
        className={cn(
          "flex w-full rounded-ui-md border border-border-subtle bg-surface-control px-3 py-2",
          "text-ui-sm text-text-strong placeholder:text-text-faint",
          "resize-y min-h-[80px]",
          "focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-border-accent focus-visible:ring-offset-2",
          "disabled:cursor-not-allowed disabled:opacity-50",
          error && "border-status-error focus-visible:ring-status-error",
          className,
        )}
        aria-invalid={!!error}
        {...props}
      />
      {error && (
        <p className="mt-1 text-ui-xs text-status-error" role="alert">
          {error}
        </p>
      )}
    </div>
  );
}
