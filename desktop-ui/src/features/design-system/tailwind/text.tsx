import type { HTMLAttributes, ReactNode } from "react";
import { cn } from "@/utils/cn";

/* ═══════════════════════════════════════════════════════════════════════════
   Text — Semantic typography primitive
   ══════════════════════════════════════════════════════════════════════════ */

export interface TextProps extends Omit<HTMLAttributes<HTMLElement>, "className"> {
  className?: string;
  children: ReactNode;
  as?: "p" | "span" | "div" | "label" | "small" | "strong" | "em";
  size?: "3xs" | "2xs" | "xs" | "sm" | "md" | "lg" | "xl";
  weight?: "normal" | "medium" | "semibold" | "bold";
  color?: "primary" | "strong" | "muted" | "subtle" | "faint" | "accent" | "accent-cyan" | "danger" | "success";
  truncate?: boolean;
}

const sizeMap = {
  "3xs": "text-ui-3xs",
  "2xs": "text-ui-2xs",
  xs: "text-ui-xs",
  sm: "text-ui-sm",
  md: "text-ui-md",
  lg: "text-ui-lg",
  xl: "text-ui-xl",
};

const weightMap = {
  normal: "font-normal",
  medium: "font-medium",
  semibold: "font-semibold",
  bold: "font-bold",
};

const colorMap = {
  primary: "text-text-primary",
  strong: "text-text-strong",
  muted: "text-text-muted",
  subtle: "text-text-subtle",
  faint: "text-text-faint",
  accent: "text-text-accent",
  "accent-cyan": "text-text-accent-cyan",
  danger: "text-status-error",
  success: "text-status-success",
};

export function Text({
  className,
  children,
  as: Component = "span",
  size = "sm",
  weight = "normal",
  color = "primary",
  truncate = false,
  ...props
}: TextProps) {
  return (
    <Component
      className={cn(
        "leading-normal",
        sizeMap[size],
        weightMap[weight],
        colorMap[color],
        truncate && "truncate",
        className,
      )}
      {...props}
    >
      {children}
    </Component>
  );
}

/* ═══════════════════════════════════════════════════════════════════════════
   Heading — Section heading primitive
   ══════════════════════════════════════════════════════════════════════════ */

export interface HeadingProps extends Omit<HTMLAttributes<HTMLHeadingElement>, "className"> {
  className?: string;
  children: ReactNode;
  as?: "h1" | "h2" | "h3" | "h4" | "h5" | "h6";
  size?: "xs" | "sm" | "md" | "lg" | "xl" | "2xl";
  weight?: "semibold" | "bold";
  color?: "primary" | "strong" | "muted" | "subtle";
  truncate?: boolean;
}

const headingSizeMap = {
  xs: "text-ui-xs",
  sm: "text-ui-sm",
  md: "text-ui-md",
  lg: "text-ui-lg",
  xl: "text-ui-xl",
  "2xl": "text-ui-display-sm",
};

export function Heading({
  className,
  children,
  as: Component = "h3",
  size = "md",
  weight = "semibold",
  color = "strong",
  truncate = false,
  ...props
}: HeadingProps) {
  return (
    <Component
      className={cn(
        "leading-tight tracking-tight",
        headingSizeMap[size],
        weightMap[weight],
        colorMap[color],
        truncate && "truncate",
        className,
      )}
      {...props}
    >
      {children}
    </Component>
  );
}
