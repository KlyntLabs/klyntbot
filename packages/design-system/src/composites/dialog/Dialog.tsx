import * as DialogPrimitive from "@radix-ui/react-dialog";
import { cn } from "../../lib/cn";
import type { DialogProps } from "./Dialog.types";

export type { DialogProps, DialogSize } from "./Dialog.types";

const sizeClasses = {
  sm: "max-w-sm",
  md: "max-w-md",
  lg: "max-w-lg",
  xl: "max-w-xl",
} as const;

function CloseIcon({ className }: { className?: string }) {
  return (
    <svg
      className={className}
      viewBox="0 0 24 24"
      fill="none"
      stroke="currentColor"
      strokeWidth={2}
      strokeLinecap="round"
      strokeLinejoin="round"
      aria-hidden="true"
    >
      <path d="M18 6 6 18" />
      <path d="m6 6 12 12" />
    </svg>
  );
}

/** Modal dialog — liquid-glass chrome with an island body. */
export function Dialog({
  open,
  onClose,
  title,
  children,
  size = "md",
  className,
}: DialogProps) {
  return (
    <DialogPrimitive.Root
      open={open}
      onOpenChange={(isOpen) => {
        if (!isOpen) onClose();
      }}
    >
      <DialogPrimitive.Portal>
        <DialogPrimitive.Overlay className="fixed inset-0 z-modal bg-overlay backdrop-blur-sm" />
        <DialogPrimitive.Content
          className={cn(
            "fixed left-1/2 top-[15vh] z-modal w-full -translate-x-1/2 liquid-glass rounded-card p-1.5",
            "animate-glass-appear",
            sizeClasses[size],
            className,
          )}
        >
          <DialogPrimitive.Description className="sr-only">{title}</DialogPrimitive.Description>
          <div className="island rounded-[calc(var(--ds-radius-card)-6px)]">
            <div className="flex items-center justify-between border-b border-separator px-5 py-4">
              <DialogPrimitive.Title className="text-body font-medium text-fg">
                {title}
              </DialogPrimitive.Title>
              <DialogPrimitive.Close asChild>
                <button
                  type="button"
                  aria-label="Close dialog"
                  className="flex size-7 items-center justify-center rounded-control text-fg-secondary transition-colors hover:bg-control-hover hover:text-fg"
                >
                  <CloseIcon className="size-4" />
                </button>
              </DialogPrimitive.Close>
            </div>
            <div className="px-5 py-4">{children}</div>
          </div>
        </DialogPrimitive.Content>
      </DialogPrimitive.Portal>
    </DialogPrimitive.Root>
  );
}
