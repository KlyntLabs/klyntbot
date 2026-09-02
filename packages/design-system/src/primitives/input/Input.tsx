import { cva } from "class-variance-authority";
import { cn } from "../../lib/cn";
import { focusRing } from "../../lib/focus";
import type { InputProps } from "./Input.types";

export const inputVariants = cva(
  `px-3 py-1.5 text-ui font-light rounded-control transition-colors focus:outline-none ${focusRing}`,
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

export function Input({ variant, className, ref, ...props }: InputProps) {
  return <input ref={ref} className={cn(inputVariants({ variant, className }))} {...props} />;
}
