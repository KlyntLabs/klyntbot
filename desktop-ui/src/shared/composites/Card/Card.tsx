import { cn } from "@shared/lib/cn";
import type { ReactNode } from "react";

export interface CardProps {
  variant?: "glass" | "surface" | "outline";
  padding?: "none" | "sm" | "md" | "lg";
  interactive?: boolean;
  className?: string;
  children: ReactNode;
  onClick?: () => void;
}

const cardVariants = {
  glass: "glass-card",
  surface: "bg-accent border border-border rounded-2xl",
  outline: "border border-border rounded-2xl bg-transparent",
};

const cardPadding = {
  none: "",
  sm: "p-3",
  md: "p-4",
  lg: "p-5",
};

export function Card({
  variant = "glass",
  padding = "md",
  interactive,
  className,
  onClick,
  children,
}: CardProps) {
  return (
    <div
      onClick={onClick}
      className={cn(
        cardVariants[variant],
        cardPadding[padding],
        interactive && "cursor-pointer transition-colors hover:border-border",
        onClick && "cursor-pointer",
        className,
      )}
    >
      {children}
    </div>
  );
}

export function CardHeader({ className, children }: { className?: string; children: ReactNode }) {
  return <div className={cn("flex items-center justify-between gap-2", className)}>{children}</div>;
}

export function CardTitle({ className, children }: { className?: string; children: ReactNode }) {
  return (
    <h3 className={cn("text-xs font-medium text-muted-foreground uppercase tracking-wider", className)}>
      {children}
    </h3>
  );
}

export function CardContent({ className, children }: { className?: string; children: ReactNode }) {
  return <div className={cn("mt-3", className)}>{children}</div>;
}

export function CardFooter({ className, children }: { className?: string; children: ReactNode }) {
  return (
    <div className={cn("mt-4 pt-3 border-t border-border flex items-center gap-2", className)}>
      {children}
    </div>
  );
}
