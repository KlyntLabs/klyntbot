import { CreateEntityModal } from "@features/database/components/entity/CreateEntityModal";
import { EntityDetail } from "@features/database/components/entity/EntityDetail";
import { SchemaEditor } from "@features/database/components/schema/SchemaEditor";
import { ViewShell } from "@features/database/components/ViewShell";
import { useDatabase } from "@features/database/hooks/useDatabase";
import { useEntities } from "@features/database/hooks/useEntities";
import type { Entity, SortRule } from "@shared/types";
import { useState } from "react";
import { useParams } from "react-router";

export default function DatabasePage() {
  const { databaseId } = useParams<{ databaseId: string }>();
  const { data: schema, loading: schemaLoading } = useDatabase(databaseId);

  const [sorts, setSorts] = useState<SortRule[]>([]);
  const [searchQuery, setSearchQuery] = useState("");
  const [selectedEntity, setSelectedEntity] = useState<Entity | null>(null);
  const [showCreate, setShowCreate] = useState(false);
  const [showSchema, setShowSchema] = useState(false);

  const { data: queryResult } = useEntities(databaseId, {
    sorts,
    limit: 100,
  });

  if (schemaLoading || !schema) {
    return (
      <div className="flex h-full items-center justify-center text-muted-foreground">
        Loading...
      </div>
    );
  }

  const entities = queryResult?.entities ?? [];
  const filtered = searchQuery
    ? entities.filter((e) =>
        Object.values(e.fields).some(
          (v) => v != null && String(v).toLowerCase().includes(searchQuery.toLowerCase()),
        ),
      )
    : entities;

  return (
    <div className="flex h-full w-full min-w-0 flex-1">
      <div className="flex min-w-0 flex-1 flex-col gap-2 overflow-hidden">
        <div className="glass-card shrink-0 flex items-center justify-between gap-3 px-4 py-2">
          <div className="flex items-center gap-2">
            {schema.icon && (
              <span className="text-[16px] leading-none select-none">{schema.icon}</span>
            )}
            <h1 className="text-[14px] font-semibold text-foreground leading-none tracking-tight">
              {schema.name}
            </h1>
          </div>
          <button
            type="button"
            onClick={() => setShowSchema(!showSchema)}
            className="flex items-center gap-1.5 rounded-md px-2 py-1 text-[12px] text-foreground/70 hover:bg-accent hover:text-foreground transition-colors"
          >
            <svg
              className="h-3.5 w-3.5"
              fill="none"
              stroke="currentColor"
              viewBox="0 0 24 24"
              strokeWidth={1.5}
            >
              <title>Properties</title>
              <path
                strokeLinecap="round"
                strokeLinejoin="round"
                d="M10.5 6h9.75M10.5 6a1.5 1.5 0 1 1-3 0m3 0a1.5 1.5 0 1 0-3 0M3.75 6H7.5m3 12h9.75m-9.75 0a1.5 1.5 0 0 1-3 0m3 0a1.5 1.5 0 0 0-3 0m-3.75 0H7.5m9-6h3.75m-3.75 0a1.5 1.5 0 0 1-3 0m3 0a1.5 1.5 0 0 0-3 0m-9.75 0h9.75"
              />
            </svg>
            Properties
          </button>
        </div>

        <ViewShell
          schema={schema}
          entities={filtered}
          totalCount={queryResult?.total ?? 0}
          sorts={sorts}
          onSortChange={setSorts}
          searchQuery={searchQuery}
          onSearchChange={setSearchQuery}
          onEntityClick={setSelectedEntity}
          onNewEntity={() => setShowCreate(true)}
        />
      </div>

      {showSchema && <SchemaEditor schema={schema} onClose={() => setShowSchema(false)} />}

      {selectedEntity && (
        <div className="fixed inset-y-0 right-0 z-40 w-[420px] border-l border-border bg-background shadow-2xl animate-[slideInRight_200ms_ease-out]">
          <EntityDetail
            schema={schema}
            entity={selectedEntity}
            onClose={() => setSelectedEntity(null)}
          />
        </div>
      )}

      {showCreate && <CreateEntityModal schema={schema} onClose={() => setShowCreate(false)} />}
    </div>
  );
}
