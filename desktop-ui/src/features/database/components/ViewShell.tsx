import type { DatabaseSchema, Entity, SortRule, ViewDefinition } from "@shared/types";
import { useState } from "react";
import { ViewToolbar } from "./ViewToolbar";
import { BoardView } from "./views/BoardView";
import { CalendarView } from "./views/CalendarView";
import { GalleryView } from "./views/GalleryView";
import { ListView } from "./views/ListView";
import { TimelineView } from "./views/TimelineView";
import { TableView } from "./views/table/TableView";
import { ViewConfigPanel } from "./views/ViewConfigPanel";
import { ViewSwitcher } from "./views/ViewSwitcher";

interface ViewShellProps {
  schema: DatabaseSchema;
  entities: Entity[];
  totalCount: number;
  sorts: SortRule[];
  onSortChange: (sorts: SortRule[]) => void;
  searchQuery: string;
  onSearchChange: (query: string) => void;
  onEntityClick: (entity: Entity) => void;
  onNewEntity: () => void;
  activeViewId?: string;
  onActiveViewChange?: (id: string) => void;
}

export function ViewShell({
  schema,
  entities,
  totalCount,
  sorts,
  onSortChange,
  searchQuery,
  onSearchChange,
  onEntityClick,
  onNewEntity,
  activeViewId: controlledActiveId,
  onActiveViewChange,
}: ViewShellProps) {
  const views = schema.views ?? [];
  const [internalActiveId, setInternalActiveId] = useState<string | undefined>(
    views.find((v) => v.isDefault)?.id ?? views[0]?.id,
  );
  const [configViewId, setConfigViewId] = useState<string | null>(null);

  const activeViewId = controlledActiveId ?? internalActiveId;
  const setActiveViewId = (id: string) => {
    setInternalActiveId(id);
    onActiveViewChange?.(id);
  };
  const activeView = views.find((v) => v.id === activeViewId) ?? views[0];
  const configView = configViewId ? views.find((v) => v.id === configViewId) : null;

  return (
    <div className="flex min-h-0 flex-1">
      <div className="flex min-h-0 flex-1 flex-col">
        <div className="shrink-0 flex items-center justify-between gap-3 border-b border-border px-4">
          {views.length > 0 && (
            <ViewSwitcher
              schema={schema}
              views={views}
              activeViewId={activeViewId}
              onViewSelect={setActiveViewId}
              onConfigView={(v) => setConfigViewId(v.id)}
            />
          )}
          <div className="ml-auto py-1.5">
            <ViewToolbar
              searchQuery={searchQuery}
              onSearchChange={onSearchChange}
              onNewEntity={onNewEntity}
              entityCount={totalCount}
            />
          </div>
        </div>

        <div className="min-h-0 flex-1 overflow-y-auto">
          {activeView && (
            <ActiveViewRenderer
              view={activeView}
              schema={schema}
              entities={entities}
              sorts={sorts}
              onSortChange={onSortChange}
              onEntityClick={onEntityClick}
            />
          )}
        </div>
      </div>
      {configView && (
        <ViewConfigPanel schema={schema} view={configView} onClose={() => setConfigViewId(null)} />
      )}
    </div>
  );
}

function ActiveViewRenderer({
  view,
  schema,
  entities,
  sorts,
  onSortChange,
  onEntityClick,
}: {
  view: ViewDefinition;
  schema: DatabaseSchema;
  entities: Entity[];
  sorts: SortRule[];
  onSortChange: (sorts: SortRule[]) => void;
  onEntityClick: (entity: Entity) => void;
}) {
  switch (view.viewType) {
    case "table":
      return (
        <TableView
          schema={schema}
          view={view}
          entities={entities}
          sorts={sorts}
          onSortChange={onSortChange}
          onEntityClick={onEntityClick}
        />
      );
    case "board":
      return (
        <BoardView
          schema={schema}
          entities={entities}
          groupByField={view.config.groupBy ?? "status"}
          cardFields={view.config.cardFields}
          onEntityClick={onEntityClick}
        />
      );
    case "calendar":
      return (
        <CalendarView
          schema={schema}
          entities={entities}
          dateField={view.config.calendarField ?? "due_date"}
          onEntityClick={onEntityClick}
        />
      );
    case "list":
      return (
        <ListView
          schema={schema}
          view={view}
          entities={entities}
          cardFields={view.config.cardFields}
          onEntityClick={onEntityClick}
        />
      );
    case "gallery":
      return (
        <GalleryView
          schema={schema}
          view={view}
          entities={entities}
          cardFields={view.config.cardFields}
          onEntityClick={onEntityClick}
        />
      );
    case "timeline":
      return (
        <TimelineView
          schema={schema}
          entities={entities}
          startField={view.config.calendarField ?? "start_date"}
          endField={view.config.layout?.endField as string | undefined}
          onEntityClick={onEntityClick}
        />
      );
  }
}
