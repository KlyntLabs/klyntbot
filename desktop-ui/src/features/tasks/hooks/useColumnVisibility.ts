import { useCallback, useMemo, useSyncExternalStore } from "react";

export type ColumnId =
  | "project"
  | "area"
  | "priority"
  | "status"
  | "dueDate"
  | "tags"
  | "taskType"
  | "energyLevel"
  | "estimatedMinutes"
  | "actualMinutes"
  | "executionState"
  | "complexityScore"
  | "totalTrackedSecs"
  | "focusedAt";

export interface ColumnDef {
  id: ColumnId;
  label: string;
  group: "core" | "agentic";
}

export const ALL_COLUMNS: ColumnDef[] = [
  { id: "project", label: "Project", group: "core" },
  { id: "priority", label: "Priority", group: "core" },
  { id: "status", label: "Status", group: "core" },
  { id: "dueDate", label: "Due Date", group: "core" },
  { id: "tags", label: "Tags", group: "core" },
  { id: "taskType", label: "Task Type", group: "agentic" },
  { id: "energyLevel", label: "Energy Level", group: "agentic" },
  { id: "estimatedMinutes", label: "Est. Minutes", group: "agentic" },
  { id: "actualMinutes", label: "Actual Minutes", group: "agentic" },
  { id: "executionState", label: "Execution State", group: "agentic" },
  { id: "complexityScore", label: "Complexity", group: "agentic" },
  { id: "totalTrackedSecs", label: "Time Tracked", group: "agentic" },
  { id: "focusedAt", label: "Focused At", group: "agentic" },
];

const STORAGE_KEY = "klyntbot:tasks:visibleColumns";
const DEFAULT_VISIBLE: ColumnId[] = ["project", "priority", "status", "dueDate", "tags"];

function getStored(): ColumnId[] {
  try {
    const raw = localStorage.getItem(STORAGE_KEY);
    if (!raw) return DEFAULT_VISIBLE;
    return JSON.parse(raw) as ColumnId[];
  } catch {
    return DEFAULT_VISIBLE;
  }
}

function setStored(cols: ColumnId[]) {
  localStorage.setItem(STORAGE_KEY, JSON.stringify(cols));
  window.dispatchEvent(new StorageEvent("storage", { key: STORAGE_KEY }));
}

function subscribe(cb: () => void) {
  const handler = (e: StorageEvent) => {
    if (e.key === STORAGE_KEY) cb();
  };
  window.addEventListener("storage", handler);
  return () => window.removeEventListener("storage", handler);
}

export function useColumnVisibility() {
  const stored = useSyncExternalStore(subscribe, getStored, () => DEFAULT_VISIBLE);
  const visibleSet = useMemo(() => new Set(stored), [stored]);

  const toggleColumn = useCallback((id: ColumnId) => {
    const current = getStored();
    const next = current.includes(id) ? current.filter((c) => c !== id) : [...current, id];
    setStored(next);
  }, []);

  const resetToDefaults = useCallback(() => setStored(DEFAULT_VISIBLE), []);
  const isVisible = useCallback((id: ColumnId) => visibleSet.has(id), [visibleSet]);

  return { visibleColumns: visibleSet, toggleColumn, resetToDefaults, isVisible };
}
