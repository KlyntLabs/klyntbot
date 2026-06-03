import type { HTMLAttributes, ReactNode } from "react";
import { cn } from "@/utils/cn";

/* ═══════════════════════════════════════════════════════════════════════════
   Box — Generic layout primitive
   ══════════════════════════════════════════════════════════════════════════ */

export interface BoxProps extends Omit<HTMLAttributes<HTMLDivElement>, "className"> {
  className?: string;
  children: ReactNode;
  as?: "div" | "span" | "section" | "article" | "aside" | "header" | "footer" | "main" | "nav";
}

export function Box({ className, children, as: Component = "div", ...props }: BoxProps) {
  return (
    <Component className={cn(className)} {...props}>
      {children}
    </Component>
  );
}
