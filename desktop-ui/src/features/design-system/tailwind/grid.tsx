import type { HTMLAttributes, ReactNode } from "react";
import { cn } from "@/utils/cn";

/* ═══════════════════════════════════════════════════════════════════════════
   Grid — Responsive grid layout primitive
   ══════════════════════════════════════════════════════════════════════════ */

export interface GridProps extends Omit<HTMLAttributes<HTMLDivElement>, "className"> {
  className?: string;
  children: ReactNode;
  cols?: 1 | 2 | 3 | 4 | 5 | 6 | 12;
  gap?: "0" | "1" | "2" | "3" | "4" | "5" | "6" | "8";
}

const colsMap = {
  1: "grid-cols-1",
  2: "grid-cols-1 sm:grid-cols-2",
  3: "grid-cols-1 sm:grid-cols-2 lg:grid-cols-3",
  4: "grid-cols-1 sm:grid-cols-2 lg:grid-cols-4",
  5: "grid-cols-2 sm:grid-cols-3 lg:grid-cols-5",
  6: "grid-cols-2 sm:grid-cols-3 lg:grid-cols-6",
  12: "grid-cols-2 sm:grid-cols-4 lg:grid-cols-6 xl:grid-cols-12",
};

export function Grid({ className, children, cols = 3, gap = "4", ...props }: GridProps) {
  return (
    <div className={cn("grid", colsMap[cols], `gap-${gap}`, className)} {...props}>
      {children}
    </div>
  );
}

/* ═══════════════════════════════════════════════════════════════════════════
   Container — Centered max-width container
   ══════════════════════════════════════════════════════════════════════════ */

export interface ContainerProps extends Omit<HTMLAttributes<HTMLDivElement>, "className"> {
  className?: string;
  children: ReactNode;
  size?: "sm" | "md" | "lg" | "xl" | "2xl" | "full";
}

const sizeMap = {
  sm: "max-w-screen-sm",
  md: "max-w-screen-md",
  lg: "max-w-screen-lg",
  xl: "max-w-screen-xl",
  "2xl": "max-w-screen-2xl",
  full: "max-w-full",
};

export function Container({ className, children, size = "xl", ...props }: ContainerProps) {
  return (
    <div className={cn("mx-auto w-full px-4 sm:px-6 lg:px-8", sizeMap[size], className)} {...props}>
      {children}
    </div>
  );
}
