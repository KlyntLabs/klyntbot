import { useSyncExternalStore } from "react";

export type AppMode = "assistant" | "code";

export const APP_MODE_STORAGE_KEY = "klynt.appMode";
const DEFAULT_MODE: AppMode = "assistant";

function isAppMode(value: unknown): value is AppMode {
  return value === "assistant" || value === "code";
}

function readStoredMode(): AppMode {
  if (typeof window === "undefined") {
    return DEFAULT_MODE;
  }
  try {
    const raw = window.localStorage.getItem(APP_MODE_STORAGE_KEY);
    return isAppMode(raw) ? raw : DEFAULT_MODE;
  } catch {
    return DEFAULT_MODE;
  }
}

let currentMode: AppMode = readStoredMode();
const listeners = new Set<() => void>();

function emit(): void {
  for (const listener of listeners) {
    listener();
  }
}

function subscribe(listener: () => void): () => void {
  listeners.add(listener);
  return () => {
    listeners.delete(listener);
  };
}

function getSnapshot(): AppMode {
  return currentMode;
}

function getServerSnapshot(): AppMode {
  return DEFAULT_MODE;
}

export function setAppMode(next: AppMode): void {
  if (!isAppMode(next) || next === currentMode) {
    return;
  }
  currentMode = next;
  try {
    if (typeof window !== "undefined") {
      window.localStorage.setItem(APP_MODE_STORAGE_KEY, next);
    }
  } catch {
    // localStorage may be unavailable (private mode, etc.) — non-fatal.
  }
  emit();
}

export function useAppMode(): { mode: AppMode; setMode: (mode: AppMode) => void } {
  const mode = useSyncExternalStore(subscribe, getSnapshot, getServerSnapshot);
  return { mode, setMode: setAppMode };
}

// Test-only helpers. Do not use in production code.
export const __testing = {
  reset(initial: AppMode = DEFAULT_MODE): void {
    currentMode = initial;
    listeners.clear();
  },
  rehydrateFromStorage(): void {
    currentMode = readStoredMode();
  },
};
