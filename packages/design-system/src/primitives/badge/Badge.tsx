import { cva } from "class-variance-authority";
import { cn } from "../../lib/cn";
import type { BadgeProps } from "./Badge.types";

export const badgeVariants = cva("inline-flex items-center font-light rounded-full border", {
  variants: {
    variant: {
      default: "bg-control-hover text-fg-secondary border-separator",
      success: "bg-status-success/10 text-status-success border-status-success/30",
      warning: "bg-status-warning/10 text-status-warning border-status-warning/30",
      destructive: "bg-status-danger/10 text-status-danger border-status-danger/30",
      info: "bg-status-info/10 text-status-info border-status-info/30",
      brand: "bg-brand/10 text-brand border-brand/30",
    },
    size: {
      sm: "px-1.5 py-0.5 text-ui-xs",
      md: "px-2 py-0.5 text-ui-sm",
    },
  },
  defaultVariants: {
    variant: "default",
    size: "md",
  },
});

export function Badge({ variant, size, className, children, ...props }: BadgeProps) {
  return (
    <span className={cn(badgeVariants({ variant, size, className }))} {...props}>
      {children}
    </span>
  );
}
