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
import type { Issue } from "../mock-data/issues";
import { groupIssuesByStatus } from "../mock-data/issues";
import { status as allStatus } from "../mock-data/status";
import { useIssuesStore } from "../store/issues-store";
import { useViewStore } from "../store/view-store";
import { GroupIssues } from "./GroupIssues";
import { IssueGrid } from "./IssueGrid";

interface IssueBoardProps {
  issues: Issue[];
}

export function IssueBoard({ issues }: IssueBoardProps) {
  const updateIssueStatus = useIssuesStore((s) => s.updateIssueStatus);
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
    if (targetStatus) updateIssueStatus(issueId, targetStatus);
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
