import { cn } from "@shared/lib/utils";
import { cva, type VariantProps } from "class-variance-authority";
import type { HTMLAttributes } from "react";

const badgeVariants = cva("inline-flex items-center font-light rounded-full border", {
  variants: {
    variant: {
      default: "bg-accent text-muted-foreground border-border",
      success: "bg-success/10 text-success border-success/30",
      warning: "bg-warning/10 text-warning border-warning/30",
      destructive: "bg-destructive/10 text-destructive border-destructive/30",
      info: "bg-info/10 text-info border-info/30",
      brand: "bg-brand/10 text-brand border-brand/30",
    },
    size: {
      sm: "px-1.5 py-0.5 text-[11px]",
      md: "px-2 py-0.5 text-xs",
    },
  },
  defaultVariants: {
    variant: "default",
    size: "md",
  },
});

export interface BadgeProps
  extends HTMLAttributes<HTMLSpanElement>,
    VariantProps<typeof badgeVariants> {}

export function Badge({ variant, size, className, children, ...props }: BadgeProps) {
  return (
    <span className={cn(badgeVariants({ variant, size, className }))} {...props}>
      {children}
    </span>
  );
}

export { badgeVariants };
