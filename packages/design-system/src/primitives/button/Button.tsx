import { cva, type VariantProps } from "class-variance-authority";
import type { ButtonHTMLAttributes, Ref } from "react";
import { cn } from "../../lib/cn";

export const buttonVariants = cva(
  "inline-flex items-center justify-center font-medium transition-colors focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-brand/40 disabled:opacity-50 disabled:pointer-events-none",
  {
    variants: {
      variant: {
        primary: "bg-brand text-brand-foreground hover:bg-brand-hover active:bg-brand/80",
        secondary:
          "bg-control-hover text-fg-secondary hover:bg-control-active hover:text-fg border border-separator",
        ghost: "text-fg-secondary hover:text-fg hover:bg-control-hover",
        destructive:
          "bg-status-danger/10 text-status-danger hover:bg-status-danger/20 border border-status-danger/20",
        outline:
          "border border-separator text-fg-secondary hover:text-fg hover:border-fg-secondary/40",
        default:
          "border border-separator bg-glass-subtle text-fg hover:bg-control-hover active:bg-control-active",
      },
      size: {
        xs: "h-6 px-2 text-ui-xs rounded-md gap-1",
        sm: "h-7 px-2.5 text-ui-xs rounded-control gap-1.5",
        md: "h-8 px-3 text-ui rounded-control gap-2",
        lg: "h-10 px-4 text-body rounded-panel gap-2",
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
  ref?: Ref<HTMLButtonElement>;
}

export function Button({
  variant,
  size,
  loading,
  className,
  disabled,
  children,
  ref,
  type = "button",
  ...props
}: ButtonProps) {
  return (
    <button
      ref={ref}
      type={type}
      disabled={disabled || loading}
      className={cn(buttonVariants({ variant, size, className }))}
      {...props}
    >
      {loading && <span className="animate-spin mr-1" aria-hidden="true">⟳</span>}
      {children}
    </button>
  );
}
