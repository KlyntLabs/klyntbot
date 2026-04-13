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
      <div className="flex items-center justify-between border-b border-border px-4 py-3">
        <h2 className="text-lg font-semibold text-foreground truncate">{title}</h2>
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
                Edit
              </Button>
              <Button variant="destructive" size="sm" onClick={handleDelete}>
                Delete
              </Button>
            </>
          )}
          <Button variant="ghost" size="xs" onClick={onClose}>
            ✕
          </Button>
        </div>
      </div>
      <div className="flex-1 overflow-y-auto p-4">
        <PropertyList
          schema={schema}
          entity={editing ? { ...entity, fields: draft } : entity}
          editing={editing}
          onChange={handleChange}
        />
      </div>
      <div className="border-t border-border px-4 py-2 text-xs text-dim">
        Created {new Date(entity.createdAt).toLocaleString()} · Updated{" "}
        {new Date(entity.updatedAt).toLocaleString()}
      </div>
    </div>
  );
}
