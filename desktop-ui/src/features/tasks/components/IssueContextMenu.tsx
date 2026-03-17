import { useMutation } from "@shared/hooks/useMutation";
import type { Task, TaskUpdateParams } from "@shared/types/tasks";
import type React from "react";
import { useStatusWorkflow } from "../contexts/StatusWorkflowContext";
import { useRefetchTasks } from "../hooks/useTasksContext";
import type { Issue } from "../lib/mappers";
import { priorityToNumber, statusToMutationParams } from "../lib/mappers";
import { priorities } from "../lib/priority-icons";
import { renderStatusIcon } from "../lib/status-utils";
import { useTabStore } from "../store/tab-store";
import {
  ContextMenu,
  ContextMenuContent,
  ContextMenuGroup,
  ContextMenuItem,
  ContextMenuSeparator,
  ContextMenuSub,
  ContextMenuSubContent,
  ContextMenuSubTrigger,
  ContextMenuTrigger,
} from "./ui/context-menu";

interface IssueContextMenuProps {
  issue: Issue;
  children: React.ReactNode;
}

export function IssueContextMenu({ issue, children }: IssueContextMenuProps) {
  const { statuses } = useStatusWorkflow();
  const updateTask = useMutation<Task, TaskUpdateParams>("task_update", "params");
  const deleteTask = useMutation<boolean, { id: string }>("task_delete");
  const refetch = useRefetchTasks();

  return (
    <ContextMenu>
      <ContextMenuTrigger asChild>{children}</ContextMenuTrigger>
      <ContextMenuContent className="w-64">
        <ContextMenuGroup>
          <ContextMenuItem
            onSelect={() => {
              navigator.clipboard.writeText(issue.identifier).catch(() => {
                console.warn("Failed to copy issue ID to clipboard");
              });
            }}
          >
            Copy ID
            <span className="ml-auto text-xs text-muted-foreground">{issue.identifier}</span>
          </ContextMenuItem>
          <ContextMenuItem
            onSelect={() => {
              useTabStore
                .getState()
                .openTab("issue", issue.id, `${issue.identifier} ${issue.title}`);
            }}
          >
            Open in new tab
          </ContextMenuItem>
        </ContextMenuGroup>

        <ContextMenuSeparator />

        {/* Status submenu */}
        <ContextMenuSub>
          <ContextMenuSubTrigger>
            <span className="mr-2 flex items-center">{renderStatusIcon(issue.status)}</span>
            Status
          </ContextMenuSubTrigger>
          <ContextMenuSubContent className="w-48">
            {statuses.map((s) => (
              <ContextMenuItem
                key={s.id}
                onSelect={async () => {
                  const { status: backendStatus, statusLabelId } = statusToMutationParams(s);
                  await updateTask.mutate({ id: issue.id, status: backendStatus, statusLabelId });
                  refetch();
                }}
              >
                <span className="mr-2 flex items-center">{renderStatusIcon(s)}</span>
                {s.name}
                {issue.status.id === s.id && (
                  <span className="ml-auto text-xs text-muted-foreground">Current</span>
                )}
              </ContextMenuItem>
            ))}
          </ContextMenuSubContent>
        </ContextMenuSub>

        {/* Priority submenu */}
        <ContextMenuSub>
          <ContextMenuSubTrigger>
            <issue.priority.icon className="mr-2 h-4 w-4 text-muted-foreground" />
            Priority
          </ContextMenuSubTrigger>
          <ContextMenuSubContent className="w-48">
            {priorities.map((p) => {
              const Icon = p.icon;
              return (
                <ContextMenuItem
                  key={p.id}
                  onSelect={async () => {
                    await updateTask.mutate({ id: issue.id, priority: priorityToNumber(p.id) });
                    refetch();
                  }}
                >
                  <Icon className="mr-2 h-4 w-4 text-muted-foreground" />
                  {p.name}
                  {issue.priority.id === p.id && (
                    <span className="ml-auto text-xs text-muted-foreground">Current</span>
                  )}
                </ContextMenuItem>
              );
            })}
          </ContextMenuSubContent>
        </ContextMenuSub>

        <ContextMenuSeparator />

        <ContextMenuItem
          className="text-destructive"
          onSelect={async () => {
            await deleteTask.mutate({ id: issue.id });
            refetch();
          }}
        >
          Delete
        </ContextMenuItem>
      </ContextMenuContent>
    </ContextMenu>
  );
}
