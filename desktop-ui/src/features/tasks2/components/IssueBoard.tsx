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
import type { Issue } from "../lib/mappers";
import { groupIssuesByStatus } from "../lib/mappers";
import { status as allStatus } from "../lib/status-icons";
import { useViewStore } from "../store/view-store";
import { GroupIssues } from "./GroupIssues";
import { IssueGrid } from "./IssueGrid";

interface IssueBoardProps {
  issues: Issue[];
  onUpdateStatus?: (issueId: string, statusId: string) => void;
}

export function IssueBoard({ issues, onUpdateStatus }: IssueBoardProps) {
  const viewType = useViewStore((s) => s.viewType);

  const [activeIssue, setActiveIssue] = useState<Issue | null>(null);

  const sensors = useSensors(useSensor(PointerSensor, { activationConstraint: { distance: 8 } }));

  const grouped = useMemo(() => groupIssuesByStatus(issues), [issues]);
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
    const targetStatus = allStatus.find((s) => s.id === (over.id as string));
    if (targetStatus && onUpdateStatus) {
      onUpdateStatus(issueId, targetStatus.id);
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
        {allStatus.map((s) => {
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
