import type { Issue } from "../lib/mappers";
import { useTabStore } from "../store/tab-store";
import { AssigneeUser } from "./AssigneeUser";
import { IssueContextMenu } from "./IssueContextMenu";
import { LabelBadge } from "./LabelBadge";
import { PrioritySelector } from "./PrioritySelector";
import { ProjectBadge } from "./ProjectBadge";
import { StatusSelector } from "./StatusSelector";

interface IssueLineProps {
  issue: Issue;
}

export function IssueLine({ issue }: IssueLineProps) {
  const navigateInPlace = useTabStore((s) => s.navigateInPlace);
  const createdDate = new Date(issue.createdAt).toLocaleDateString("en-US", {
    month: "short",
    day: "numeric",
  });

  return (
    <IssueContextMenu issue={issue}>
      {/* biome-ignore lint/a11y/useKeyWithClickEvents: row click navigates, keyboard nav handled by parent list */}
      {/* biome-ignore lint/a11y/noStaticElementInteractions: row click navigates */}
      <div
        className="group flex items-center gap-2 px-4 py-2 border-b border-border hover:bg-accent/50 transition-colors cursor-pointer"
        onClick={() => navigateInPlace("issue", issue.id, issue.identifier)}
      >
        {/* Priority — stop propagation so clicking it doesn't navigate */}
        {/* biome-ignore lint/a11y/useKeyWithClickEvents: stop-propagation wrapper, not interactive */}
        {/* biome-ignore lint/a11y/noStaticElementInteractions: stop-propagation wrapper */}
        <div onClick={(e) => e.stopPropagation()}>
          <PrioritySelector issueId={issue.id} priority={issue.priority} />
        </div>

        {/* Identifier */}
        <span className="text-xs text-muted-foreground w-[72px] shrink-0">{issue.identifier}</span>

        {/* Status — stop propagation */}
        {/* biome-ignore lint/a11y/useKeyWithClickEvents: stop-propagation wrapper, not interactive */}
        {/* biome-ignore lint/a11y/noStaticElementInteractions: stop-propagation wrapper */}
        <div onClick={(e) => e.stopPropagation()}>
          <StatusSelector issueId={issue.id} status={issue.status} />
        </div>

        {/* Title */}
        <span className="text-sm text-foreground truncate flex-1 min-w-0">{issue.title}</span>

        {/* Labels */}
        <div className="hidden lg:flex items-center gap-1 shrink-0">
          {issue.labels.length > 0 && <LabelBadge label={issue.labels} />}
        </div>

        {/* Project */}
        {issue.project && (
          <div className="hidden xl:flex shrink-0">
            <ProjectBadge project={issue.project} />
          </div>
        )}

        {/* Date */}
        <span className="text-xs text-muted-foreground w-[60px] shrink-0 text-right">
          {createdDate}
        </span>

        {/* Assignee */}
        <AssigneeUser user={issue.assignee} />
      </div>
    </IssueContextMenu>
  );
}
