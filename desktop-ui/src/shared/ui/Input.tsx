import { cn } from "@klyntbot/design-system";
import { cva, type VariantProps } from "class-variance-authority";
import type { InputHTMLAttributes, Ref } from "react";

const inputVariants = cva(
  "px-3 py-1.5 text-ui font-light rounded-control transition-colors focus:outline-none focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-separator focus-visible:border-fg-secondary/50",
  {
    variants: {
      variant: {
        default:
          "bg-control-hover border border-separator text-fg placeholder:text-fg-dim hover:border-fg-secondary/40",
        glass: "glass-input text-fg placeholder:text-fg-dim",
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
  ref?: Ref<HTMLInputElement>;
}

export function Input({ variant, className, ref, ...props }: InputProps) {
  return <input ref={ref} className={cn(inputVariants({ variant, className }))} {...props} />;
}

export { inputVariants };
