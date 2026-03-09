import { createContext, use } from "react";
import type {
  Area,
  Project,
  StatusLabel,
  Task,
  TaskGroup,
  TaskUpdateParams,
} from "../../lib/types";

export interface TaskTableCtx {
  completedTasks: Set<string>;
  expandedTasks: Set<string>;
  childrenCache: Map<string, Task[]>;
  projects: Project[];
  areas: Area[];
  statusLabels: StatusLabel[];
  groups: TaskGroup[];
  collapsedGroups: Set<string>;
  showArea: boolean;
  onToggleTask: (id: string) => void;
  onToggleExpandTask: (id: string) => void;
  onToggleGroup: (groupId: string) => void;
  onUpdate: (params: TaskUpdateParams) => void;
  onCreateSubtask: (parentId: string, title: string) => void;
  onSelectTask: (id: string) => void;
}

export const TaskTableContext = createContext<TaskTableCtx | null>(null);

export function useTaskTable(): TaskTableCtx {
  const ctx = use(TaskTableContext);
  if (!ctx) throw new Error("useTaskTable must be used inside TaskTable");
  return ctx;
}
