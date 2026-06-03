import { cva, type VariantProps } from "class-variance-authority";
import type { ButtonHTMLAttributes, ReactNode } from "react";
import { cn } from "@/utils/cn";

/* ═══════════════════════════════════════════════════════════════════════════
   Button — Tailwind/CVA replacement for legacy .primary / .secondary / .ghost
   ══════════════════════════════════════════════════════════════════════════ */

const buttonVariants = cva(
  // Base styles matching legacy buttons.css
  "inline-flex items-center justify-center gap-2 rounded-ui-lg font-semibold cursor-pointer select-none " +
    "text-ui-sm leading-none whitespace-nowrap " +
    "transition-all duration-ui-fast ease-ui-out " +
    "active:scale-[var(--ds-active-scale)] active:shadow-[0_4px_8px_rgba(0,0,0,0.15)] active:duration-50 " +
    "disabled:opacity-50 disabled:cursor-not-allowed disabled:active:scale-100 disabled:active:shadow-none " +
    "[-webkit-app-region:no-drag]",
  {
    variants: {
      variant: {
        primary:
          "bg-gradient-to-br from-[#62b7ff] to-[#4fe3a3] text-[#0b0f1a] " +
          "shadow-[0_12px_22px_var(--shadow-accent)] " +
          "hover:-translate-y-px hover:shadow-[0_12px_18px_rgba(0,0,0,0.2)] " +
          "active:brightness-[0.92]",
        secondary:
          "bg-surface-card-strong text-text-primary " +
          "hover:-translate-y-px hover:shadow-[0_12px_18px_rgba(0,0,0,0.2)]",
        ghost:
          "bg-transparent text-text-muted border border-border-strong " +
          "hover:-translate-y-px hover:shadow-[0_12px_18px_rgba(0,0,0,0.2)] " +
          "active:bg-surface-hover",
        danger:
          "bg-status-error/90 text-white " +
          "hover:-translate-y-px hover:shadow-[0_12px_18px_rgba(0,0,0,0.2)] " +
          "active:brightness-[0.92]",
        link:
          "bg-transparent text-text-accent-cyan underline-offset-4 hover:underline " +
          "shadow-none hover:shadow-none active:scale-100",
      },
      size: {
        default: "px-3.5 py-2",
        sm: "px-2.5 py-1.5 text-ui-xs rounded-ui-md",
        lg: "px-5 py-2.5 text-ui-md rounded-ui-lg",
        icon: "size-8 p-1.5 rounded-ui-md active:scale-[var(--ds-active-scale-sm)]",
      },
    },
    defaultVariants: {
      variant: "secondary",
      size: "default",
    },
  },
);

export interface ButtonProps
  extends Omit<ButtonHTMLAttributes<HTMLButtonElement>, "className">,
    VariantProps<typeof buttonVariants> {
  className?: string;
  asChild?: boolean;
  children: ReactNode;
}

export function Button({
  className,
  variant,
  size,
  asChild = false,
  children,
  ...props
}: ButtonProps) {
  if (asChild) {
    // Simple passthrough for asChild — consumers can wrap with Slot if needed
    return (
      <button className={cn(buttonVariants({ variant, size, className }))} {...props}>
        {children}
      </button>
    );
  }

  return (
    <button className={cn(buttonVariants({ variant, size, className }))} {...props}>
      {children}
    </button>
  );
}
