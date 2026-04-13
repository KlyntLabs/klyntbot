import { useDeleteEntity, useUpdateEntity } from "@features/database/hooks/useEntity";
import { getEntityTitle } from "@features/database/lib/schema-utils";
import type { DatabaseSchema, Entity } from "@shared/types";
import { Button } from "@shared/ui/Button";
import { useState } from "react";
import { PropertyList } from "./PropertyList";

interface EntityDetailProps {
  schema: DatabaseSchema;
  entity: Entity;
  onClose: () => void;
}

export function EntityDetail({ schema, entity, onClose }: EntityDetailProps) {
  const [editing, setEditing] = useState(false);
  const [draft, setDraft] = useState<Record<string, unknown>>({ ...entity.fields });
  const { mutate: updateEntity, loading: saving } = useUpdateEntity(schema.id);
  const { mutate: deleteEntity } = useDeleteEntity(schema.id);

  const handleChange = (slug: string, value: unknown) => {
    setDraft((prev) => ({ ...prev, [slug]: value }));
  };

  const handleSave = async () => {
    await updateEntity(entity.id, { fields: draft });
    setEditing(false);
  };

  const handleDelete = async () => {
    await deleteEntity(entity.id);
    onClose();
  };

  const title = getEntityTitle(schema, entity.fields);

  return (
    <div className="flex h-full flex-col">
      {/* Header */}
      <div className="flex items-center justify-between border-b border-border px-5 py-3">
        <div className="flex items-center gap-2 min-w-0">
          <button
            type="button"
            onClick={onClose}
            className="shrink-0 rounded-md p-1 text-muted-foreground hover:bg-accent hover:text-foreground transition-colors cursor-pointer"
          >
            <svg
              className="h-4 w-4"
              fill="none"
              stroke="currentColor"
              viewBox="0 0 24 24"
              strokeWidth={1.5}
            >
              <path strokeLinecap="round" strokeLinejoin="round" d="M6 18 18 6M6 6l12 12" />
            </svg>
          </button>
        </div>
        <div className="flex items-center gap-1.5">
          {editing ? (
            <>
              <Button
                variant="ghost"
                size="sm"
                onClick={() => {
                  setEditing(false);
                  setDraft({ ...entity.fields });
                }}
              >
                Cancel
              </Button>
              <Button variant="primary" size="sm" onClick={handleSave} loading={saving}>
                Save
              </Button>
            </>
          ) : (
            <>
              <Button variant="ghost" size="sm" onClick={() => setEditing(true)}>
                <svg
                  className="h-3.5 w-3.5"
                  fill="none"
                  stroke="currentColor"
                  viewBox="0 0 24 24"
                  strokeWidth={1.5}
                >
                  <path
                    strokeLinecap="round"
                    strokeLinejoin="round"
                    d="m16.862 4.487 1.687-1.688a1.875 1.875 0 1 1 2.652 2.652L10.582 16.07a4.5 4.5 0 0 1-1.897 1.13L6 18l.8-2.685a4.5 4.5 0 0 1 1.13-1.897l8.932-8.931Zm0 0L19.5 7.125M18 14v4.75A2.25 2.25 0 0 1 15.75 21H5.25A2.25 2.25 0 0 1 3 18.75V8.25A2.25 2.25 0 0 1 5.25 6H10"
                  />
                </svg>
                Edit
              </Button>
              <Button variant="destructive" size="sm" onClick={handleDelete}>
                Delete
              </Button>
            </>
          )}
        </div>
      </div>

      {/* Title area */}
      <div className="px-5 pt-5 pb-2">
        <h2 className="text-xl font-semibold text-foreground leading-tight">{title}</h2>
      </div>

      {/* Properties */}
      <div className="flex-1 overflow-y-auto px-5 pb-5">
        <PropertyList
          schema={schema}
          entity={editing ? { ...entity, fields: draft } : entity}
          editing={editing}
          onChange={handleChange}
        />
      </div>

      {/* Footer */}
      <div className="border-t border-border px-5 py-2.5 text-[11px] text-dim">
        Created {new Date(entity.createdAt).toLocaleString()} · Updated{" "}
        {new Date(entity.updatedAt).toLocaleString()}
      </div>
    </div>
  );
}
