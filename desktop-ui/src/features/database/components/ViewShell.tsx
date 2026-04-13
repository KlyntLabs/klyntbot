import type { DatabaseSchema, Entity, SortRule, ViewDefinition } from "@shared/types";
import { useState } from "react";
import { ViewTabBar } from "./ViewTabBar";
import { ViewToolbar } from "./ViewToolbar";
import { BoardView } from "./views/BoardView";
import { CalendarView } from "./views/CalendarView";
import { GalleryView } from "./views/GalleryView";
import { ListView } from "./views/ListView";
import { TableView } from "./views/TableView";
import { TimelineView } from "./views/TimelineView";

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
}: ViewShellProps) {
  const views = schema.views ?? [];
  const [activeViewId, setActiveViewId] = useState<string | undefined>(
    views.find((v) => v.isDefault)?.id ?? views[0]?.id,
  );

  const activeView = views.find((v) => v.id === activeViewId) ?? views[0];

  return (
    <div className="flex min-h-0 flex-1 flex-col">
      {/* View tabs + toolbar with page-level horizontal padding */}
      <div className="shrink-0 px-10">
        {views.length > 0 && (
          <ViewTabBar views={views} activeViewId={activeViewId} onViewSelect={setActiveViewId} />
        )}
        <ViewToolbar
          searchQuery={searchQuery}
          onSearchChange={onSearchChange}
          onNewEntity={onNewEntity}
          entityCount={totalCount}
        />
      </div>

      {/* Scrollable content — full bleed for table, padded for others */}
      <div className="min-h-0 flex-1 overflow-y-auto">
        {activeView ? (
          <ActiveViewRenderer
            view={activeView}
            schema={schema}
            entities={entities}
            sorts={sorts}
            onSortChange={onSortChange}
            onEntityClick={onEntityClick}
          />
        ) : (
          <TableView
            schema={schema}
            entities={entities}
            sorts={sorts}
            onSortChange={onSortChange}
            onEntityClick={onEntityClick}
          />
        )}
      </div>
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
          entities={entities}
          visibleFields={view.config.visibleFields}
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
          entities={entities}
          cardFields={view.config.cardFields}
          onEntityClick={onEntityClick}
        />
      );
    case "gallery":
      return (
        <GalleryView
          schema={schema}
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
