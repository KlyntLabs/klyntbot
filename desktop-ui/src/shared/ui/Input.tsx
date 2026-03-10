import { cn } from "@shared/lib/cn";
import { forwardRef, type InputHTMLAttributes } from "react";

export interface InputProps extends InputHTMLAttributes<HTMLInputElement> {
  variant?: "default" | "glass";
}

export const Input = forwardRef<HTMLInputElement, InputProps>(
  ({ variant = "default", className, ...props }, ref) => (
    <input
      ref={ref}
      className={cn(
        "px-3 py-1.5 text-sm font-light rounded-lg",
        "transition-colors focus:outline-none",
        variant === "glass"
          ? "glass-input text-primary placeholder:text-dim"
          : "bg-surface-base border border-border text-primary placeholder:text-dim hover:border-white/15 focus:border-brand/50",
        className,
      )}
      {...props}
    />
  ),
);
Input.displayName = "Input";
