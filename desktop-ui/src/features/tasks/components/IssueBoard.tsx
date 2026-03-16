import {
  closestCenter,
  DndContext,
  type DragEndEvent,
  DragOverlay,
  type DragStartEvent,
  PointerSensor,
  useSensor,
  useSensors,
} from "@dnd-kit/core";
import { useMemo, useState } from "react";
import { useStatusWorkflow } from "../contexts/StatusWorkflowContext";
import type { Issue } from "../lib/mappers";
import { groupIssuesByStatus, statusToMutationParams } from "../lib/mappers";
import { useViewStore } from "../store/view-store";
import { GroupIssues } from "./GroupIssues";
import { IssueGrid } from "./IssueGrid";

interface IssueBoardProps {
  issues: Issue[];
  onUpdateStatus?: (issueId: string, status: string, statusLabelId: string | null) => void;
}

export function IssueBoard({ issues, onUpdateStatus }: IssueBoardProps) {
  const { statuses } = useStatusWorkflow();
  const viewType = useViewStore((s) => s.viewType);

  const [activeIssue, setActiveIssue] = useState<Issue | null>(null);

  const sensors = useSensors(useSensor(PointerSensor, { activationConstraint: { distance: 8 } }));

  // Group issues by status, with graceful degradation for unmatched statuses
  const grouped = useMemo(() => {
    const groups = groupIssuesByStatus(issues);
    const statusIds = new Set(statuses.map((s) => s.id));

    // Bucket issues whose status doesn't match any column by statusGroup
    const orphans: Issue[] = [];
    for (const [statusId, statusIssues] of Object.entries(groups)) {
      if (!statusIds.has(statusId)) {
        orphans.push(...statusIssues);
        delete groups[statusId];
      }
    }

    if (orphans.length > 0) {
      for (const issue of orphans) {
        const group = issue.status.statusGroup;
        let targetStatus = statuses[0]; // default: first column

        if (group === "active") {
          const mid = Math.floor(statuses.length / 2);
          targetStatus = statuses[mid] ?? statuses[0];
        } else if (group === "done") {
          targetStatus =
            statuses.find((s) => s.statusGroup === "done") ?? statuses[statuses.length - 1];
        } else if (group === "stuck") {
          targetStatus = statuses[statuses.length - 1];
        } else {
          // "not_started" or undefined → first column
          targetStatus = statuses[0];
        }

        if (targetStatus) {
          if (!groups[targetStatus.id]) groups[targetStatus.id] = [];
          groups[targetStatus.id].push(issue);
        }
      }
    }

    return groups;
  }, [issues, statuses]);

  const isGrid = viewType === "grid";

  const handleDragStart = (event: DragStartEvent) => {
    const issue = event.active.data.current?.issue as Issue | undefined;
    if (issue) setActiveIssue(issue);
  };

  const handleDragEnd = (event: DragEndEvent) => {
    const { active, over } = event;
    setActiveIssue(null);
    if (!over) return;
    const issueId = active.id as string;

    // over.id may be a droppable column (status.id) or another draggable issue card.
    // First try matching a column directly.
    let targetStatus = statuses.find((s) => s.id === (over.id as string));

    // If we hit an issue card instead of a column, find which column that issue belongs to.
    if (!targetStatus && over.data.current?.issue) {
      const overIssue = over.data.current.issue as Issue;
      targetStatus = statuses.find((s) => s.id === overIssue.status.id);
    }

    if (targetStatus && onUpdateStatus) {
      const { status, statusLabelId } = statusToMutationParams(targetStatus);
      onUpdateStatus(issueId, status, statusLabelId);
    }
  };

  return (
    <DndContext
      sensors={sensors}
      collisionDetection={closestCenter}
      onDragStart={handleDragStart}
      onDragEnd={handleDragEnd}
    >
      <div className={isGrid ? "flex gap-4 p-4 h-full min-w-max" : ""}>
        {statuses.map((s) => {
          const statusIssues = grouped[s.id];
          if (!isGrid && (!statusIssues || statusIssues.length === 0)) return null;
          return <GroupIssues key={s.id} status={s} issues={statusIssues ?? []} />;
        })}
      </div>
      <DragOverlay>
        {activeIssue ? (
          <div className="opacity-80">
            <IssueGrid issue={activeIssue} />
          </div>
        ) : null}
      </DragOverlay>
    </DndContext>
  );
}
