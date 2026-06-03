import type { MouseEventHandler, ReactNode } from "react";
import { cn } from "@/utils/cn";

type ModalShellProps = {
  children: ReactNode;
  className?: string;
  cardClassName?: string;
  onBackdropClick?: MouseEventHandler<HTMLButtonElement>;
  ariaLabel?: string;
  ariaLabelledBy?: string;
  ariaDescribedBy?: string;
};

export function ModalShell({
  children,
  className,
  cardClassName,
  onBackdropClick,
  ariaLabel,
  ariaLabelledBy,
  ariaDescribedBy,
}: ModalShellProps) {
  return (
    <div
      className={cn("fixed inset-0 z-ui-modal", className)}
      role="dialog"
      aria-modal="true"
      aria-label={ariaLabel}
      aria-labelledby={ariaLabelledBy}
      aria-describedby={ariaDescribedBy}
    >
      <button
        type="button"
        className="absolute inset-0 bg-[var(--ds-modal-backdrop)] backdrop-blur-[8px] animate-[ds-modal-backdrop-in_var(--ds-dur-entrance)_var(--ds-ease-out)_both] [app.reduced-transparency_&]:backdrop-blur-none"
        aria-label="Close dialog"
        onClick={onBackdropClick}
      />
      <div
        className={cn(
          "absolute top-1/2 left-1/2 -translate-x-1/2 -translate-y-1/2",
          "bg-ds-surface-card border border-ds-border-strong text-ds-text-strong",
          "shadow-[0_18px_40px_rgba(0,0,0,0.35)]",
          "animate-[ds-modal-card-in_var(--ds-dur-slow)_var(--ds-ease-out)_both]",
          cardClassName,
        )}
      >
        {children}
      </div>
    </div>
  );
}
