import { cn } from "@shared/lib/utils";
import type { Toast } from "../hooks/useToast";

interface Props {
  toasts: Toast[];
  onDismiss: (id: number) => void;
}

const VARIANT_CLASSES: Record<Toast["variant"], string> = {
  error: "bg-red-950/90 border-red-800/50 text-red-200",
  success: "bg-emerald-950/90 border-emerald-800/50 text-emerald-200",
  info: "bg-surface-base border-border text-foreground",
};

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
            "flex items-center gap-2 px-4 py-3 rounded-lg shadow-lg border text-sm animate-[slideIn_0.2s_ease-out]",
            VARIANT_CLASSES[toast.variant],
          )}
        >
          <span className="flex-1">{toast.message}</span>
          {toast.action && (
            <button
              type="button"
              onClick={() => {
                toast.action?.onClick();
                onDismiss(toast.id);
              }}
              className="rounded px-2 py-0.5 text-[12px] font-medium text-brand hover:underline"
            >
              {toast.action.label}
            </button>
          )}
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
