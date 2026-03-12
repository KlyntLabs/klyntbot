import type React from "react";
import { renderStatusIcon } from "../lib/status-utils";
import type { Issue } from "../mock-data/issues";
import { labels } from "../mock-data/labels";
import { priorities } from "../mock-data/priorities";
import { projects } from "../mock-data/projects";
import { status as allStatus } from "../mock-data/status";
import { users } from "../mock-data/users";
import { useIssuesStore } from "../store/issues-store";
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
  const {
    updateIssueStatus,
    updateIssuePriority,
    updateIssueAssignee,
    updateIssue,
    deleteIssue,
    addIssueLabel,
    removeIssueLabel,
  } = useIssuesStore();

  return (
    <ContextMenu>
      <ContextMenuTrigger asChild>{children}</ContextMenuTrigger>
      <ContextMenuContent className="w-64">
        <ContextMenuGroup>
          <ContextMenuItem
            onSelect={() => {
              navigator.clipboard.writeText(issue.identifier);
            }}
          >
            Copy ID
            <span className="ml-auto text-xs text-[hsl(var(--muted-foreground))]">
              {issue.identifier}
            </span>
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
              <ContextMenuItem key={s.id} onSelect={() => updateIssueStatus(issue.id, s)}>
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
                <ContextMenuItem key={p.id} onSelect={() => updateIssuePriority(issue.id, p)}>
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

        {/* Assignee submenu */}
        <ContextMenuSub>
          <ContextMenuSubTrigger>Assignee</ContextMenuSubTrigger>
          <ContextMenuSubContent className="w-48">
            <ContextMenuItem onSelect={() => updateIssueAssignee(issue.id, null)}>
              <span className="text-[hsl(var(--muted-foreground))]">Unassigned</span>
              {issue.assignee === null && (
                <span className="ml-auto text-xs text-[hsl(var(--muted-foreground))]">Current</span>
              )}
            </ContextMenuItem>
            {users.map((user) => (
              <ContextMenuItem key={user.id} onSelect={() => updateIssueAssignee(issue.id, user)}>
                {user.name}
                {issue.assignee?.id === user.id && (
                  <span className="ml-auto text-xs text-[hsl(var(--muted-foreground))]">
                    Current
                  </span>
                )}
              </ContextMenuItem>
            ))}
          </ContextMenuSubContent>
        </ContextMenuSub>

        {/* Label submenu */}
        <ContextMenuSub>
          <ContextMenuSubTrigger>Label</ContextMenuSubTrigger>
          <ContextMenuSubContent className="w-48">
            {labels.map((label) => (
              <ContextMenuItem
                key={label.id}
                onSelect={() => {
                  const hasLabel = issue.labels.some((l) => l.id === label.id);
                  if (hasLabel) {
                    removeIssueLabel(issue.id, label.id);
                  } else {
                    addIssueLabel(issue.id, label);
                  }
                }}
              >
                <span
                  className="mr-2 size-2 rounded-full"
                  style={{ backgroundColor: label.color }}
                />
                {label.name}
                {issue.labels.some((l) => l.id === label.id) && (
                  <span className="ml-auto text-xs text-[hsl(var(--muted-foreground))]">
                    Active
                  </span>
                )}
              </ContextMenuItem>
            ))}
          </ContextMenuSubContent>
        </ContextMenuSub>

        {/* Project submenu */}
        <ContextMenuSub>
          <ContextMenuSubTrigger>Project</ContextMenuSubTrigger>
          <ContextMenuSubContent className="w-48">
            {projects.map((project) => {
              const Icon = project.icon;
              return (
                <ContextMenuItem
                  key={project.id}
                  onSelect={() => updateIssue(issue.id, { project })}
                >
                  <Icon className="mr-2 h-4 w-4" />
                  {project.name}
                  {issue.project?.id === project.id && (
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
          onSelect={() => deleteIssue(issue.id)}
        >
          Delete
        </ContextMenuItem>
      </ContextMenuContent>
    </ContextMenu>
  );
}
