import { cva, type VariantProps } from "class-variance-authority";
import type { HTMLAttributes, ReactNode } from "react";
import { cn } from "@/utils/cn";

/* ═══════════════════════════════════════════════════════════════════════════
   Badge — Status pills, chips, and labels
   ══════════════════════════════════════════════════════════════════════════ */

const badgeVariants = cva(
  "inline-flex items-center gap-1 rounded-ui-full font-medium whitespace-nowrap select-none",
  {
    variants: {
      variant: {
        default:
          "bg-surface-elevated-strong text-foreground-muted border border-border-subtle",
        primary: "bg-surface-active text-text-strong",
        success: "bg-cm-green-bg text-cm-green-fg border border-cm-green-border",
        warning: "bg-cm-amber-bg text-cm-amber-fg border border-cm-amber-border",
        error: "bg-cm-neutral-bg text-status-error border border-cm-neutral-border",
        info: "bg-cm-blue-bg text-cm-blue-fg border border-cm-blue-border",
        ghost: "bg-transparent text-text-faint",
      },
      size: {
        default: "px-2 py-0.5 text-ui-2xs",
        sm: "px-1.5 py-px text-ui-3xs",
        lg: "px-2.5 py-1 text-ui-xs",
      },
    },
    defaultVariants: {
      variant: "default",
      size: "default",
    },
  },
);

export interface BadgeProps
  extends Omit<HTMLAttributes<HTMLSpanElement>, "className">,
    VariantProps<typeof badgeVariants> {
  className?: string;
  children: ReactNode;
}

export function Badge({ className, variant, size, children, ...props }: BadgeProps) {
  return (
    <span className={cn(badgeVariants({ variant, size, className }))} {...props}>
      {children}
    </span>
  );
}
