import { cn } from "@shared/lib/utils";
import { cva, type VariantProps } from "class-variance-authority";
import type { InputHTMLAttributes } from "react";

const inputVariants = cva(
  "px-3 py-1.5 text-sm font-light rounded-lg transition-colors focus:outline-none",
  {
    variants: {
      variant: {
        default:
          "bg-accent border border-border text-foreground placeholder:text-dim hover:border-border focus:border-brand/50",
        glass: "glass-input text-foreground placeholder:text-dim",
      },
    },
    defaultVariants: {
      variant: "default",
    },
  },
);

export interface InputProps
  extends InputHTMLAttributes<HTMLInputElement>,
    VariantProps<typeof inputVariants> {
  ref?: React.Ref<HTMLInputElement>;
}

export function Input({ variant, className, ref, ...props }: InputProps) {
  return <input ref={ref} className={cn(inputVariants({ variant, className }))} {...props} />;
}

export { inputVariants };
