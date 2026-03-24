import { cn } from "@shared/lib/utils";
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
          ? "glass-input text-foreground placeholder:text-dim"
          : "bg-accent border border-border text-foreground placeholder:text-dim hover:border-border focus:border-brand/50",
        className,
      )}
      {...props}
    />
  ),
);
Input.displayName = "Input";
