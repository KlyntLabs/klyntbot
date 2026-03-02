import { clsx, type ClassValue } from "clsx";
import { twMerge } from "tailwind-merge";

export function cn(...inputs: ClassValue[]) {
  return twMerge(clsx(inputs));
}

export const isTauri = typeof window !== 'undefined' && '__TAURI_INTERNALS__' in window;

export const taskGridCols = (showArea: boolean) =>
  showArea
    ? 'grid-cols-[40px_1fr_180px_100px_80px_100px_120px_140px]'
    : 'grid-cols-[40px_1fr_180px_80px_100px_120px_140px]';

/** Format a millisecond duration as a human-readable string (e.g. "123ms", "1.2s"). */
export function formatDuration(ms: number): string {
  if (ms < 1000) return `${ms}ms`;
  return `${(ms / 1000).toFixed(1)}s`;
}
