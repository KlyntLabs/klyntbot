import { type ClassValue, clsx } from "clsx";
import { twMerge } from "tailwind-merge";

/**
 * Merge Tailwind classes with deduplication and conflict resolution.
 * Replaces `joinClassNames` across the codebase.
 */
export function cn(...inputs: ClassValue[]) {
  return twMerge(clsx(inputs));
}
