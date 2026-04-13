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
    <div className="flex h-full">
      <div className="flex flex-1 flex-col overflow-hidden">
        <div className="shrink-0 px-12 pt-10 pb-1">
          <div className="flex items-center gap-3">
            {schema.icon && <span className="text-4xl">{schema.icon}</span>}
            <h1 className="text-3xl font-bold text-foreground">{schema.name}</h1>
          </div>
        </div>
        <div className="flex shrink-0 items-center gap-2 px-12 py-1">
          <button
            type="button"
            onClick={() => setShowSchema(!showSchema)}
            className="rounded-lg px-2 py-1 text-sm text-muted-foreground hover:bg-accent hover:text-foreground transition-colors"
          >
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
        <div className="fixed inset-y-0 right-0 z-40 w-96 border-l border-border bg-background shadow-xl">
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
