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
      className={cn("ds-modal", className)}
      role="dialog"
      aria-modal="true"
      aria-label={ariaLabel}
      aria-labelledby={ariaLabelledBy}
      aria-describedby={ariaDescribedBy}
    >
      <button
        type="button"
        className="ds-modal-backdrop"
        aria-label="Close dialog"
        onClick={onBackdropClick}
      />
      <div className={cn("ds-modal-card", cardClassName)}>{children}</div>
    </div>
  );
}
