import type { HTMLAttributes, ReactNode } from "react";
import { cn } from "@/utils/cn";

/* ═══════════════════════════════════════════════════════════════════════════
   Chip — Compact label/tag primitive
   ══════════════════════════════════════════════════════════════════════════ */

export interface ChipProps extends Omit<HTMLAttributes<HTMLSpanElement>, "className"> {
  className?: string;
  children: ReactNode;
  variant?:
    | "default"
    | "primary"
    | "success"
    | "warning"
    | "error"
    | "info"
    | "neutral";
  size?: "sm" | "md";
}

const variantMap = {
  default: "bg-surface-elevated text-foreground border-border",
  primary: "bg-cm-blue-bg text-cm-blue-fg border-cm-blue-border",
  success: "bg-cm-green-bg text-cm-green-fg border-cm-green-border",
  warning: "bg-cm-amber-bg text-cm-amber-fg border-cm-amber-border",
  error: "bg-cm-orange-bg text-cm-orange-fg border-cm-orange-border",
  info: "bg-cm-cyan-bg text-cm-cyan-fg border-cm-cyan-border",
  neutral: "bg-cm-neutral-bg text-cm-neutral-fg border-cm-neutral-border",
};

const sizeMap = {
  sm: "text-ui-2xs px-1.5 py-0.5",
  md: "text-ui-xs px-2 py-0.5",
};

export function Chip({ className, children, variant = "default", size = "md", ...props }: ChipProps) {
  return (
    <span
      className={cn(
        "inline-flex items-center gap-1 rounded-full border font-medium",
        variantMap[variant],
        sizeMap[size],
        className,
      )}
      {...props}
    >
      {children}
    </span>
  );
}
