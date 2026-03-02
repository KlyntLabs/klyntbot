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
