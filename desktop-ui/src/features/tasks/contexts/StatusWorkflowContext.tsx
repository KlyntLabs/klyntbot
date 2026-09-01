import { useEffectiveLabels } from "@shared/hooks/useWorkflows";
import type { StatusLabel } from "@shared/types/common";
import type { Task } from "@shared/types/tasks";
import { createContext, type ReactNode, useContext, useMemo } from "react";
import { resolveStatus } from "../lib/mappers";
import { makeColoredCircle, matchIcon, type Status } from "../lib/status-icons";

interface StatusWorkflowContextValue {
  statuses: Status[];
  labels: StatusLabel[];
  resolveStatusById: (id: string) => Status | undefined;
  resolveStatusByTask: (task: Task) => Status;
}

const StatusWorkflowContext = createContext<StatusWorkflowContextValue | null>(null);

function labelsToStatuses(labels: StatusLabel[]): Status[] {
  return labels.map((label) => {
    const icon = matchIcon(label.name);
    return {
      id: label.id,
      name: label.name,
      color: label.color,
      icon: icon ?? makeColoredCircle(label.color),
      backendStatus: label.statusGroup,
      statusGroup: label.statusGroup,
    };
  });
}

export function StatusWorkflowProvider({
  projectId,
  children,
}: {
  projectId: string | null;
  children: ReactNode;
}) {
  const { data: labels = [] } = useEffectiveLabels(projectId);

  const value = useMemo<StatusWorkflowContextValue>(() => {
    const statuses = labelsToStatuses(labels);
    const statusMap = new Map(statuses.map((s) => [s.id, s]));

    return {
      statuses,
      labels,
      resolveStatusById: (id: string) => statusMap.get(id),
      resolveStatusByTask: (task: Task) => resolveStatus(task, labels),
    };
  }, [labels]);

  return <StatusWorkflowContext.Provider value={value}>{children}</StatusWorkflowContext.Provider>;
}

export function useStatusWorkflow(): StatusWorkflowContextValue {
  const ctx = useContext(StatusWorkflowContext);
  if (!ctx) {
    throw new Error("useStatusWorkflow must be used within StatusWorkflowProvider");
  }
  return ctx;
}
