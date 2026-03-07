import { X } from "lucide-react";
import { type FormEvent, type ReactNode, useCallback, useEffect, useRef } from "react";

interface FormModalProps {
  open: boolean;
  onClose: () => void;
  title: string;
  children: ReactNode;
  onSubmit: () => void;
  submitLabel?: string;
  canSubmit?: boolean;
}

export function FormModal({
  open,
  onClose,
  title,
  children,
  onSubmit,
  submitLabel = "Save",
  canSubmit = true,
}: FormModalProps) {
  const backdropRef = useRef<HTMLDivElement>(null);

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

  if (!open) return null;

  const handleBackdropClick = (e: React.MouseEvent) => {
    if (e.target === backdropRef.current) onClose();
  };

  const handleSubmit = (e: FormEvent) => {
    e.preventDefault();
    if (canSubmit) onSubmit();
  };

  return (
    <div
      ref={backdropRef}
      onClick={handleBackdropClick}
      className="fixed inset-0 z-50 flex items-start justify-center bg-black/40 backdrop-blur-sm pt-[15vh]"
    >
      <form
        onSubmit={handleSubmit}
        className="glass-panel w-full max-w-md"
        style={{ animation: "glass-appear 0.2s ease-out" }}
      >
        <div className="bg-white/[0.04] rounded-[var(--glass-radius-inner)]">
          <div className="flex items-center justify-between px-5 py-4 border-b border-white/[0.08]">
            <h3 className="text-[14px] font-medium text-primary">{title}</h3>
            <button
              type="button"
              onClick={onClose}
              className="w-7 h-7 rounded-md flex items-center justify-center text-muted hover:text-secondary hover:bg-white/[0.06] transition-colors"
            >
              <X className="w-4 h-4" />
            </button>
          </div>

          <div className="px-5 py-4 space-y-3">{children}</div>

          <div className="flex items-center justify-end gap-2 px-5 py-3 border-t border-white/[0.08]">
            <button
              type="button"
              onClick={onClose}
              className="px-3 py-1.5 text-[12px] text-muted hover:text-secondary rounded-md hover:bg-white/[0.06] transition-colors"
            >
              Cancel
            </button>
            <button
              type="submit"
              disabled={!canSubmit}
              className="px-4 py-1.5 text-[12px] rounded-md bg-brand text-white hover:bg-brand-hover transition-colors disabled:opacity-40 disabled:cursor-not-allowed"
            >
              {submitLabel}
            </button>
          </div>
        </div>
      </form>
    </div>
  );
}

export function FormField({ label, children }: { label: string; children: ReactNode }) {
  return (
    <div className="flex flex-col gap-1.5">
      <label className="text-[11px] font-medium text-secondary">{label}</label>
      {children}
    </div>
  );
}

export const fieldClass =
  "glass-input w-full px-3 py-2 text-[12px] font-light text-primary placeholder:text-dim rounded-lg";
