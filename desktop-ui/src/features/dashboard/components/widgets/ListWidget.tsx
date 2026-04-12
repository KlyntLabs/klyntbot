import { useDatabase } from "@features/database/hooks/useDatabase";
import { useEntities } from "@features/database/hooks/useEntities";

interface ListWidgetProps {
  databaseId: string;
  title: string;
  limit?: number;
}

export function ListWidget({ databaseId, title, limit = 5 }: ListWidgetProps) {
  const { data: schema } = useDatabase(databaseId);
  const { data, loading } = useEntities(databaseId, { limit });

  const titleField =
    schema?.fields.find((f) => f.slug === "title" || f.slug === "name") ??
    schema?.fields.find((f) => f.required && f.fieldType === "text");

  return (
    <div className="flex flex-col h-full">
      <h3 className="text-sm font-semibold px-3 py-2 border-b border-border">{title}</h3>
      <div className="flex-1 overflow-y-auto">
        {loading ? (
          <div className="p-3 text-sm text-muted">Loading...</div>
        ) : (
          <div className="divide-y divide-border/50">
            {(data?.entities ?? []).map((entity) => (
              <div key={entity.id} className="px-3 py-1.5 text-sm truncate">
                {titleField
                  ? String(entity.fields[titleField.slug] ?? "\u2014")
                  : entity.id.slice(0, 8)}
              </div>
            ))}
            {(data?.entities ?? []).length === 0 && (
              <div className="p-3 text-sm text-muted text-center">No items</div>
            )}
          </div>
        )}
      </div>
    </div>
  );
}
