import { createContext, useCallback, useContext, useEffect, useRef, useState } from "react";

export type ToastVariant = "error" | "success" | "info";

export interface ToastAction {
  label: string;
  onClick: () => void;
}

export interface Toast {
  id: number;
  message: string;
  variant: ToastVariant;
  action?: ToastAction;
}

export interface ShowOptions {
  variant?: ToastVariant;
  action?: ToastAction;
}

/**
 * Minimal toast state hook. Returns helpers to show/dismiss toasts.
 * Toasts auto-dismiss after `duration` ms (default 4000).
 */
export function useToast(duration = 4000) {
  const [toasts, setToasts] = useState<Toast[]>([]);
  const nextId = useRef(0);
  const timers = useRef<Set<ReturnType<typeof setTimeout>>>(new Set());

  useEffect(
    () => () => {
      for (const t of timers.current) clearTimeout(t);
    },
    [],
  );

  const show = useCallback(
    (message: string, optsOrVariant?: ShowOptions | ToastVariant) => {
      const opts: ShowOptions =
        typeof optsOrVariant === "string" ? { variant: optsOrVariant } : (optsOrVariant ?? {});
      const id = nextId.current++;
      setToasts((prev) => [
        ...prev,
        { id, message, variant: opts.variant ?? "error", action: opts.action },
      ]);
      const timer = setTimeout(() => {
        setToasts((prev) => prev.filter((t) => t.id !== id));
        timers.current.delete(timer);
      }, duration);
      timers.current.add(timer);
    },
    [duration],
  );

  const dismiss = useCallback((id: number) => {
    setToasts((prev) => prev.filter((t) => t.id !== id));
  }, []);

  return { toasts, show, dismiss };
}

// ── Context for shared toast (e.g. SettingsLayout) ───────────────────

interface ToastActions {
  show: (message: string, optsOrVariant?: ShowOptions | ToastVariant) => void;
}

const ToastContext = createContext<ToastActions | null>(null);

export const ToastContextProvider = ToastContext.Provider;

export function useToastContext(): ToastActions {
  const ctx = useContext(ToastContext);
  if (!ctx) throw new Error("useToastContext must be used within a ToastProvider");
  return ctx;
}
