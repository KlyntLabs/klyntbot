import { cn } from "@shared/lib/utils";
import type { Toast } from "../hooks/useToast";

interface Props {
  toasts: Toast[];
  onDismiss: (id: number) => void;
}

/**
 * Renders a stack of toast notifications at the bottom-right of the viewport.
 */
export function ToastContainer({ toasts, onDismiss }: Props) {
  if (toasts.length === 0) return null;

  return (
    <div className="fixed bottom-4 right-4 z-50 flex flex-col gap-2 max-w-sm">
      {toasts.map((toast) => (
        <div
          key={toast.id}
          role="alert"
          className={cn(
            "flex items-start gap-2 px-4 py-3 rounded-panel shadow-lg border text-ui animate-[slideIn_0.2s_ease-out]",
            toast.variant === "error"
              ? "bg-status-danger/90 border-status-danger/50 text-brand-foreground"
              : "bg-status-success/90 border-status-success/50 text-brand-foreground",
          )}
        >
          <span className="flex-1">{toast.message}</span>
          <button
            type="button"
            onClick={() => onDismiss(toast.id)}
            className="text-current opacity-50 hover:opacity-100 shrink-0"
          >
            &times;
          </button>
        </div>
      ))}
    </div>
  );
}
