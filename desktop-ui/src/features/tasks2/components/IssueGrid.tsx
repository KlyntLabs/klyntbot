import { useDraggable } from "@dnd-kit/core";
import { format } from "date-fns";
import { motion } from "motion/react";
import { useRef } from "react";
import type { Issue } from "../mock-data/issues";
import { useTabStore } from "../store/tab-store";
import { AssigneeUser } from "./AssigneeUser";
import { IssueContextMenu } from "./IssueContextMenu";
import { LabelBadge } from "./LabelBadge";
import { ProjectBadge } from "./ProjectBadge";

export function IssueGrid({ issue }: { issue: Issue }) {
  const navigateInPlace = useTabStore((s) => s.navigateInPlace);
  const pointerStart = useRef<{ x: number; y: number } | null>(null);
  const { attributes, listeners, setNodeRef, transform, isDragging } = useDraggable({
    id: issue.id,
    data: { issue },
  });

  const style = transform
    ? { transform: `translate3d(${transform.x}px, ${transform.y}px, 0)` }
    : undefined;

  // Track pointer start so we only navigate on true clicks (no drag movement)
  const handlePointerDown = (e: React.PointerEvent) => {
    pointerStart.current = { x: e.clientX, y: e.clientY };
    listeners?.onPointerDown?.(e as never);
  };

  const handleClick = (e: React.MouseEvent) => {
    if (!pointerStart.current) return;
    const dx = e.clientX - pointerStart.current.x;
    const dy = e.clientY - pointerStart.current.y;
    // Only navigate if pointer barely moved (not a drag)
    if (Math.abs(dx) < 5 && Math.abs(dy) < 5) {
      navigateInPlace("issue", issue.id, issue.identifier);
    }
    pointerStart.current = null;
  };

  return (
    <IssueContextMenu issue={issue}>
      <motion.div
        ref={setNodeRef}
        style={style}
        {...listeners}
        {...attributes}
        onPointerDown={handlePointerDown}
        onClick={handleClick}
        layoutId={`issue-grid-${issue.identifier}`}
        className={`bg-[hsl(var(--background))] rounded-md shadow-xs border border-[hsl(var(--border))]/50 p-3 cursor-pointer space-y-2 ${isDragging ? "opacity-50 cursor-grabbing" : ""}`}
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
