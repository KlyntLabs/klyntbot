import type { HTMLAttributes } from "react";
import { cn } from "@/utils/cn";

/* ═══════════════════════════════════════════════════════════════════════════
   Skeleton — Loading placeholder
   ══════════════════════════════════════════════════════════════════════════ */

export interface SkeletonProps extends Omit<HTMLAttributes<HTMLDivElement>, "className"> {
  className?: string;
  variant?: "text" | "circular" | "rectangular";
  width?: string;
  height?: string;
}

export function Skeleton({
  className,
  variant = "text",
  width,
  height,
  style,
  ...props
}: SkeletonProps) {
  return (
    <div
      className={cn(
        "animate-pulse bg-surface-card-strong",
        variant === "text" && "h-4 rounded-ui-sm",
        variant === "circular" && "rounded-full",
        variant === "rectangular" && "rounded-ui-md",
        className,
      )}
      style={{
        width,
        height,
        ...style,
      }}
      {...props}
    />
  );
}
