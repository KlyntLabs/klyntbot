import { useDraggable } from "@dnd-kit/core";
import { format } from "date-fns";
import { motion } from "motion/react";
import type { Issue } from "../mock-data/issues";
import { AssigneeUser } from "./AssigneeUser";
import { IssueContextMenu } from "./IssueContextMenu";
import { LabelBadge } from "./LabelBadge";
import { ProjectBadge } from "./ProjectBadge";

export function IssueGrid({ issue }: { issue: Issue }) {
  const { attributes, listeners, setNodeRef, transform, isDragging } = useDraggable({
    id: issue.id,
    data: { issue },
  });

  const style = transform
    ? { transform: `translate3d(${transform.x}px, ${transform.y}px, 0)` }
    : undefined;

  return (
    <IssueContextMenu issue={issue}>
      <motion.div
        ref={setNodeRef}
        style={style}
        {...listeners}
        {...attributes}
        layoutId={`issue-grid-${issue.identifier}`}
        className={`bg-[hsl(var(--background))] rounded-md shadow-xs border border-[hsl(var(--border))]/50 p-3 cursor-grab space-y-2 ${isDragging ? "opacity-50" : ""}`}
      >
        <div className="flex items-center gap-2">
          <issue.priority.icon className="size-4 text-[hsl(var(--muted-foreground))]" />
          <span className="text-xs text-[hsl(var(--muted-foreground))] font-medium">
            {issue.identifier}
          </span>
        </div>
        <p className="text-sm font-semibold line-clamp-2">{issue.title}</p>
        {issue.labels.length > 0 && (
          <div className="flex flex-wrap gap-1">
            <LabelBadge label={issue.labels} />
          </div>
        )}
        {issue.project && <ProjectBadge project={issue.project} />}
        <div className="flex items-center justify-between">
          <span className="text-xs text-[hsl(var(--muted-foreground))]">
            {format(new Date(issue.createdAt), "MMM dd")}
          </span>
          <AssigneeUser user={issue.assignee} />
        </div>
      </motion.div>
    </IssueContextMenu>
  );
}
