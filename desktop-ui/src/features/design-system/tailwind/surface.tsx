import type { HTMLAttributes, ReactNode } from "react";
import { cn } from "@/utils/cn";

/* ═══════════════════════════════════════════════════════════════════════════
   Surface — Background container primitive
   ══════════════════════════════════════════════════════════════════════════ */

export interface SurfaceProps extends Omit<HTMLAttributes<HTMLDivElement>, "className"> {
  className?: string;
  children: ReactNode;
  variant?:
    | "sidebar"
    | "topbar"
    | "messages"
    | "card"
    | "card-strong"
    | "control"
    | "hover"
    | "active"
    | "popover"
    | "command"
    | "transparent";
  padding?: "none" | "1" | "2" | "3" | "4";
  radius?: "none" | "sm" | "md" | "lg" | "xl" | "full";
  border?: boolean;
}

const variantMap = {
  sidebar: "bg-surface",
  topbar: "bg-surface-elevated",
  messages: "bg-surface",
  card: "bg-surface-elevated",
  "card-strong": "bg-surface",
  control: "bg-surface-sunken",
  hover: "bg-surface-hover",
  active: "bg-surface-active",
  popover: "bg-overlay",
  command: "bg-overlay",
  transparent: "bg-transparent",
};

const radiusMap = {
  none: "rounded-none",
  sm: "rounded-ui-sm",
  md: "rounded-ui-md",
  lg: "rounded-ui-lg",
  xl: "rounded-ui-xl",
  full: "rounded-full",
};

export function Surface({
  className,
  children,
  variant = "card",
  padding = "none",
  radius = "lg",
  border = false,
  ...props
}: SurfaceProps) {
  return (
    <div
      className={cn(
        variantMap[variant],
        radiusMap[radius],
        border && "border border-border-subtle",
        padding !== "none" && `p-${padding}`,
        className,
      )}
      {...props}
    >
      {children}
    </div>
  );
}

/* ═══════════════════════════════════════════════════════════════════════════
   Divider — Horizontal separator
   ══════════════════════════════════════════════════════════════════════════ */

export interface DividerProps extends Omit<HTMLAttributes<HTMLDivElement>, "className"> {
  className?: string;
  color?: "subtle" | "muted" | "strong";
  spacing?: "none" | "1" | "2" | "3" | "4";
}

const dividerColorMap = {
  subtle: "bg-border-subtle",
  muted: "bg-border-muted",
  strong: "bg-border-strong",
};

export function Divider({ className, color = "subtle", spacing = "2", ...props }: DividerProps) {
  return (
    <div
      className={cn(
        "h-px w-full",
        dividerColorMap[color],
        spacing !== "none" && `my-${spacing}`,
        className,
      )}
      {...props}
    />
  );
}
