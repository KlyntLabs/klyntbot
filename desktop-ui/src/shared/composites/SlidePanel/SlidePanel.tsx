import { X } from "lucide-react";
import { type ReactNode, useCallback, useEffect, useState } from "react";
import { createPortal } from "react-dom";

export interface SlidePanelProps {
  open: boolean;
  onClose: () => void;
  title: string;
  children: ReactNode;
  width?: number;
  className?: string;
}

export function SlidePanel({
  open,
  onClose,
  title,
  children,
  width = 420,
  className,
}: SlidePanelProps) {
  const [mounted, setMounted] = useState(false);

  useEffect(() => {
    setMounted(true);
  }, []);

  const handleKeyDown = useCallback(
    (e: KeyboardEvent) => {
      if (e.key === "Escape") onClose();
    },
    [onClose],
  );

  useEffect(() => {
    if (!open) return;
    document.addEventListener("keydown", handleKeyDown);
    return () => document.removeEventListener("keydown", handleKeyDown);
  }, [open, handleKeyDown]);

  if (!mounted) return null;

  return createPortal(
    <>
      {/* biome-ignore lint/a11y/noStaticElementInteractions: backdrop overlay — click to dismiss, keyboard handled by Escape listener */}
      {/* biome-ignore lint/a11y/useKeyWithClickEvents: keyboard dismiss handled globally via Escape key */}
      <div
        className={`fixed inset-0 z-40 bg-overlay transition-opacity duration-300 ${
          open ? "opacity-100" : "opacity-0 pointer-events-none"
        }`}
        onClick={onClose}
      />

      <div
        className={`fixed top-0 right-0 h-full glass rounded-card p-1.5 z-40 flex flex-col transition-transform duration-300 ${
          open ? "translate-x-0" : "translate-x-full"
        } ${className ?? ""}`}
        style={{ width }}
      >
        <div className="bg-bg-elevated flex-1 flex flex-col rounded-[calc(var(--ds-radius-card) - var(--ds-space-1-5))]">
          <div className="flex items-center justify-between px-5 py-4 border-b border-separator shrink-0">
            <h3 className="text-body font-medium text-fg">{title}</h3>
            <button
              type="button"
              onClick={onClose}
              aria-label="Close panel"
              className="size-7 rounded-control flex items-center justify-center text-fg-secondary hover:text-fg hover:bg-control-hover transition-colors"
            >
              <X className="size-4" />
            </button>
          </div>

          <div className="flex-1 overflow-y-auto px-5 py-4">{children}</div>
        </div>
      </div>
    </>,
    document.body,
  );
}
