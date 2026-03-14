import { useMutation } from "@shared/hooks/useMutation";
import type { Task, TaskUpdateParams } from "@shared/types/tasks";
import type React from "react";
import type { Issue } from "../lib/mappers";
import { priorityToNumber, statusToBackend } from "../lib/mappers";
import { priorities } from "../lib/priority-icons";
import { status as allStatus } from "../lib/status-icons";
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
  const updateTask = useMutation<Task, TaskUpdateParams>("task_update", "params");
  const deleteTask = useMutation<boolean, { id: string }>("task_delete");

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
            <span className="ml-auto text-xs text-[hsl(var(--muted-foreground))]">
              {issue.identifier}
            </span>
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
            <span className="mr-2 flex items-center">{renderStatusIcon(issue.status.id)}</span>
            Status
          </ContextMenuSubTrigger>
          <ContextMenuSubContent className="w-48">
            {allStatus.map((s) => (
              <ContextMenuItem
                key={s.id}
                onSelect={() => updateTask.mutate({ id: issue.id, status: statusToBackend(s) })}
              >
                <span className="mr-2 flex items-center">{renderStatusIcon(s.id)}</span>
                {s.name}
                {issue.status.id === s.id && (
                  <span className="ml-auto text-xs text-[hsl(var(--muted-foreground))]">
                    Current
                  </span>
                )}
              </ContextMenuItem>
            ))}
          </ContextMenuSubContent>
        </ContextMenuSub>

        {/* Priority submenu */}
        <ContextMenuSub>
          <ContextMenuSubTrigger>
            <issue.priority.icon className="mr-2 h-4 w-4 text-[hsl(var(--muted-foreground))]" />
            Priority
          </ContextMenuSubTrigger>
          <ContextMenuSubContent className="w-48">
            {priorities.map((p) => {
              const Icon = p.icon;
              return (
                <ContextMenuItem
                  key={p.id}
                  onSelect={() =>
                    updateTask.mutate({ id: issue.id, priority: priorityToNumber(p.id) })
                  }
                >
                  <Icon className="mr-2 h-4 w-4 text-[hsl(var(--muted-foreground))]" />
                  {p.name}
                  {issue.priority.id === p.id && (
                    <span className="ml-auto text-xs text-[hsl(var(--muted-foreground))]">
                      Current
                    </span>
                  )}
                </ContextMenuItem>
              );
            })}
          </ContextMenuSubContent>
        </ContextMenuSub>

        <ContextMenuSeparator />

        <ContextMenuItem
          className="text-[hsl(var(--destructive))]"
          onSelect={() => deleteTask.mutate({ id: issue.id })}
        >
          Delete
        </ContextMenuItem>
      </ContextMenuContent>
    </ContextMenu>
  );
}
