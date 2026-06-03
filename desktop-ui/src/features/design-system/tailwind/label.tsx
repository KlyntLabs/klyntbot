import type { LabelHTMLAttributes, ReactNode } from "react";
import { cn } from "@/utils/cn";

/* ═══════════════════════════════════════════════════════════════════════════
   Label — Form label primitive
   ══════════════════════════════════════════════════════════════════════════ */

export interface LabelProps extends Omit<LabelHTMLAttributes<HTMLLabelElement>, "className"> {
  className?: string;
  children: ReactNode;
  required?: boolean;
}

export function Label({ className, children, required = false, ...props }: LabelProps) {
  return (
    <label
      className={cn(
        "text-ui-sm font-medium text-text-strong leading-none",
        "peer-disabled:cursor-not-allowed peer-disabled:opacity-70",
        className,
      )}
      {...props}
    >
      {children}
      {required && <span className="ml-0.5 text-status-error">*</span>}
    </label>
  );
}
