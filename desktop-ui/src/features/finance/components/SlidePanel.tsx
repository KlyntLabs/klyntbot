import { X } from "lucide-react";
import { type ReactNode, useCallback, useEffect } from "react";

interface SlidePanelProps {
  open: boolean;
  onClose: () => void;
  title: string;
  children: ReactNode;
}

export function SlidePanel({ open, onClose, title, children }: SlidePanelProps) {
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

  return (
    <>
      {/* Backdrop */}
      <div
        className={`fixed inset-0 z-40 bg-overlay transition-opacity duration-300 ${
          open ? "opacity-100" : "opacity-0 pointer-events-none"
        }`}
        onClick={onClose}
      />

      {/* Panel */}
      <div
        className={`fixed top-0 right-0 h-full w-[420px] glass-panel z-40 flex flex-col transition-transform duration-300 ${
          open ? "translate-x-0" : "translate-x-full"
        }`}
      >
        <div className="bg-surface-low flex-1 flex flex-col rounded-[var(--glass-radius-inner)]">
          <div className="flex items-center justify-between px-5 py-4 border-b border-border shrink-0">
            <h3 className="text-[14px] font-medium text-primary">{title}</h3>
            <button
              type="button"
              onClick={onClose}
              className="w-7 h-7 rounded-md flex items-center justify-center text-muted hover:text-secondary hover:bg-surface-base transition-colors"
            >
              <X className="w-4 h-4" />
            </button>
          </div>

          <div className="flex-1 overflow-y-auto px-5 py-4">{children}</div>
        </div>
      </div>
    </>
  );
}
