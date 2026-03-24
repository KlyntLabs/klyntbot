import * as DialogPrimitive from "@radix-ui/react-dialog";
import { cn } from "@shared/lib/utils";
import { X } from "lucide-react";
import type { ReactNode } from "react";

export interface DialogProps {
  open: boolean;
  onClose: () => void;
  title: string;
  children: ReactNode;
  size?: "sm" | "md" | "lg" | "xl";
  className?: string;
}

const sizeClasses = {
  sm: "max-w-sm",
  md: "max-w-md",
  lg: "max-w-lg",
  xl: "max-w-xl",
};

export function Dialog({ open, onClose, title, children, size = "md", className }: DialogProps) {
  return (
    <DialogPrimitive.Root
      open={open}
      onOpenChange={(isOpen) => {
        if (!isOpen) onClose();
      }}
    >
      <DialogPrimitive.Portal>
        <DialogPrimitive.Overlay className="fixed inset-0 z-50 bg-overlay backdrop-blur-sm" />
        <DialogPrimitive.Content
          className={cn(
            "fixed left-1/2 top-[15vh] z-50 -translate-x-1/2 glass-panel w-full",
            sizeClasses[size],
            className,
          )}
          style={{ animation: "glass-appear 0.2s ease-out" }}
        >
          <DialogPrimitive.Description className="sr-only">{title}</DialogPrimitive.Description>
          <div className="bg-card rounded-[var(--glass-radius-inner)]">
            <div className="flex items-center justify-between px-5 py-4 border-b border-border">
              <DialogPrimitive.Title className="text-[14px] font-medium text-foreground">
                {title}
              </DialogPrimitive.Title>
              <DialogPrimitive.Close asChild>
                <button
                  type="button"
                  aria-label="Close dialog"
                  className="size-7 rounded-md flex items-center justify-center text-muted-foreground hover:text-foreground hover:bg-accent transition-colors"
                >
                  <X className="size-4" />
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
