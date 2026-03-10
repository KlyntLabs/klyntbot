import type { WorkContextType } from "@shared/types";

export const CONTEXT_TYPE_COLORS: Record<WorkContextType, string> = {
  coding: "#3B82F6",
  research: "#10B981",
  communication: "#F59E0B",
  planning: "#8B5CF6",
  review: "#EC4899",
  meeting: "#F97316",
  learning: "#06B6D4",
  general: "#6B7280",
};

/** Get display color for a context — user-set color wins, else type default. */
export function contextColor(userColor?: string, contextType?: string): string {
  if (userColor) return userColor;
  return (
    CONTEXT_TYPE_COLORS[(contextType as WorkContextType) ?? "general"] ??
    CONTEXT_TYPE_COLORS.general
  );
}
