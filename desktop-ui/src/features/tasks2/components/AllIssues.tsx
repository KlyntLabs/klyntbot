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
import { useFilterStore } from "../store/filter-store";
import { useIssuesStore } from "../store/issues-store";
import { useSearchStore } from "../store/search-store";
import { useViewStore } from "../store/view-store";
import { GroupIssues } from "./GroupIssues";
import { IssueGrid } from "./IssueGrid";
import { SearchIssues } from "./SearchIssues";

export default function AllIssues() {
  const issues = useIssuesStore((s) => s.issues);
  const updateIssueStatus = useIssuesStore((s) => s.updateIssueStatus);
  const filterIssues = useIssuesStore((s) => s.filterIssues);
  const { isSearchOpen, searchQuery } = useSearchStore();
  const filters = useFilterStore((s) => s.filters);
  const isFiltered = useFilterStore((s) => Object.values(s.filters).some((a) => a.length > 0));
  const viewType = useViewStore((s) => s.viewType);

  const [activeIssue, setActiveIssue] = useState<Issue | null>(null);

  const sensors = useSensors(
    useSensor(PointerSensor, {
      activationConstraint: {
        distance: 8,
      },
    }),
  );

  const handleDragStart = (event: DragStartEvent) => {
    const issue = event.active.data.current?.issue as Issue | undefined;
    if (issue) {
      setActiveIssue(issue);
    }
  };

  const handleDragEnd = (event: DragEndEvent) => {
    const { active, over } = event;
    setActiveIssue(null);

    if (!over) return;

    const issueId = active.id as string;
    const targetStatusId = over.id as string;

    const targetStatus = allStatus.find((s) => s.id === targetStatusId);
    if (targetStatus) {
      updateIssueStatus(issueId, targetStatus);
    }
  };

  // Search mode
  if (isSearchOpen) {
    if (searchQuery.trim()) {
      return <SearchIssues />;
    }
    return (
      <div className="px-6 py-8 text-center text-sm text-[hsl(var(--muted-foreground))]">
        Search results will appear here
      </div>
    );
  }

  // Memoize filtered/grouped issues
  const displayIssues = useMemo(
    () => (isFiltered ? filterIssues(filters) : issues),
    [isFiltered, filters, issues, filterIssues],
  );
  const grouped = useMemo(() => groupIssuesByStatus(displayIssues), [displayIssues]);

  const isGrid = viewType === "grid";

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
