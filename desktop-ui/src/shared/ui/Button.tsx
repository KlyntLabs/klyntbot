import { cn } from "@shared/lib/utils";
import { cva, type VariantProps } from "class-variance-authority";
import type { ButtonHTMLAttributes } from "react";

const buttonVariants = cva(
  "inline-flex items-center justify-center font-medium transition-colors focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-brand/50 disabled:opacity-50 disabled:pointer-events-none",
  {
    variants: {
      variant: {
        primary: "bg-brand text-white hover:bg-brand-hover active:bg-brand/80",
        secondary: "bg-accent text-muted-foreground hover:bg-muted hover:text-foreground",
        ghost: "text-muted-foreground hover:text-foreground hover:bg-accent",
        destructive: "bg-destructive/10 text-destructive hover:bg-destructive/20",
        outline:
          "border border-border text-muted-foreground hover:text-foreground hover:border-border",
      },
      size: {
        xs: "h-6 px-2 text-xs rounded-md gap-1",
        sm: "h-7 px-2.5 text-xs rounded-lg gap-1.5",
        md: "h-8 px-3 text-sm rounded-lg gap-2",
        lg: "h-10 px-4 text-sm rounded-xl gap-2",
      },
    },
    defaultVariants: {
      variant: "secondary",
      size: "md",
    },
  },
);

export interface ButtonProps
  extends ButtonHTMLAttributes<HTMLButtonElement>,
    VariantProps<typeof buttonVariants> {
  loading?: boolean;
  ref?: React.Ref<HTMLButtonElement>;
}

export function Button({
  variant,
  size,
  loading,
  className,
  disabled,
  children,
  ref,
  ...props
}: ButtonProps) {
  return (
    <button
      ref={ref}
      disabled={disabled || loading}
      className={cn(buttonVariants({ variant, size, className }))}
      {...props}
    >
      {loading && <span className="animate-spin mr-1">⟳</span>}
      {children}
    </button>
  );
}

export { buttonVariants };
