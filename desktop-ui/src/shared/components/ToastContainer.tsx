import { cn } from "@shared/lib/cn";
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
            "flex items-start gap-2 px-4 py-3 rounded-lg shadow-lg border text-sm animate-[slideIn_0.2s_ease-out]",
            toast.variant === "error"
              ? "bg-red-950/90 border-red-800/50 text-red-200"
              : "bg-emerald-950/90 border-emerald-800/50 text-emerald-200",
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
