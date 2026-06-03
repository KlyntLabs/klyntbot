import type { HTMLAttributes, ReactNode } from "react";
import { cn } from "@/utils/cn";

/* ═══════════════════════════════════════════════════════════════════════════
   Card — Content containers matching legacy surface-card patterns
   ══════════════════════════════════════════════════════════════════════════ */

export interface CardProps extends Omit<HTMLAttributes<HTMLDivElement>, "className"> {
  className?: string;
  children: ReactNode;
}

export function Card({ className, children, ...props }: CardProps) {
  return (
    <div
      className={cn(
        "rounded-lg border border-border bg-surface-elevated",
        "transition-colors duration-ui-fast",
        className,
      )}
      {...props}
    >
      {children}
    </div>
  );
}

export function CardHeader({ className, children, ...props }: CardProps) {
  return (
    <div className={cn("flex items-center justify-between gap-2 px-4 py-3", className)} {...props}>
      {children}
    </div>
  );
}

export function CardTitle({ className, children, ...props }: CardProps) {
  return (
    <h3 className={cn("text-ui-md font-semibold text-text-strong leading-tight", className)} {...props}>
      {children}
    </h3>
  );
}

export function CardDescription({ className, children, ...props }: CardProps) {
  return (
    <p className={cn("text-ui-xs text-text-muted leading-snug", className)} {...props}>
      {children}
    </p>
  );
}

export function CardContent({ className, children, ...props }: CardProps) {
  return (
    <div className={cn("px-4 pb-4", className)} {...props}>
      {children}
    </div>
  );
}

export function CardFooter({ className, children, ...props }: CardProps) {
  return (
    <div className={cn("flex items-center gap-2 px-4 pb-4 pt-0", className)} {...props}>
      {children}
    </div>
  );
}
